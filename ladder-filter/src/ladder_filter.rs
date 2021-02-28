//! This zero-delay feedback filter is based on a 4-stage transistor ladder filter.
//! It follows the following equations:
//! x = input - tanh(self.res * self.vout[3])
//! vout[0] = self.params.g.get() * (tanh(x) - tanh(self.vout[0])) + self.s[0]
//! vout[1] = self.params.g.get() * (tanh(self.vout[0]) - tanh(self.vout[1])) + self.s[1]
//! vout[0] = self.params.g.get() * (tanh(self.vout[1]) - tanh(self.vout[2])) + self.s[2]
//! vout[0] = self.params.g.get() * (tanh(self.vout[2]) - tanh(self.vout[3])) + self.s[3]
//! since we can't easily solve a nonlinear equation,
//! Mystran's fixed-pivot method is used to approximate the tanh() parts.
//! Quality can be improved a lot by oversampling a bit.
//! Feedback is clipped independently of the input, so it doesn't disappear at high gains.

use std::f32::consts::PI;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc};

use std::fmt::Debug;

use carnyx::buffer::AudioBuffer;
use vst::util::AtomicFloat;
use carnyx::carnyx::{CarnyxModel, CarnyxParam, BasicParam, CarnyxProcessor, CarnyxHost, SettableListener};

use carnyx_druid::{Dial, DruidEditor, EditorState};
use druid::widget::{Axis, CrossAxisAlignment, Flex, Label, LabelText, RadioGroup, Slider};
use druid::{Data, Insets, Lens, LensExt, Widget, WidgetExt};

pub struct LadderShared {
    // the "cutoff" parameter. Determines how heavy filtering is
    cutoff: AtomicFloat,

    g: AtomicFloat,
    // needed to calculate cutoff.
    sample_rate: AtomicFloat,
    // makes a peak at cutoff
    res: AtomicFloat,
    // used to choose where we want our output to be
    poles: AtomicUsize,
    // pole_value is just to be able to use get_parameter on poles
    pole_value: AtomicFloat,
    // a drive parameter. Just used to increase the volume, which results in heavier distortion
    drive: AtomicFloat,
}

pub struct LadderProcessor {
    host: Arc<dyn CarnyxHost>,
    model: Arc<LadderShared>,
    listener: SettableListener<LadderShared>,

    // the output of the different filter stages
    vout: [f32; 4],
    // s is the "state" parameter. In an IIR it would be the last value from the filter
    // In this we find it by trapezoidal integration to avoid the unit delay
    s: [f32; 4],
}

impl CarnyxProcessor for LadderProcessor {
    type Model = LadderShared;
    type Editor = DruidEditor<Self::Model>;

    fn set_sample_rate(&mut self, rate: f32) {
        self.model.sample_rate.set(rate);
    }

    fn parameters(&self) -> Vec<Box<dyn CarnyxParam<Self::Model>>> {
        vec![
            Box::new( BasicParam::new("cutoff", "Hz",
                                      |lp: &LadderShared|lp.get_cutoff(),
                                      |lp, val|lp.set_cutoff(val),
                                      |lp| format!("{:.0}", lp.cutoff.get()))),
            Box::new( BasicParam::new("resonance", "%",
                                      |lp: &LadderShared|lp.res.get() / 4.,
                                      |lp, val|lp.res.set(val * 4.),
                                      |lp| format!("{:.3}", lp.res.get()))),
            Box::new( BasicParam::new("filter order", "poles",
                                      |lp: &LadderShared|lp.pole_value.get(),
                                      |lp, val|lp.set_poles(val),
                                      |lp| format!("{}", lp.poles.load(Ordering::Relaxed) + 1))),
            Box::new( BasicParam::new("drive", "%",
                                      |lp: &LadderShared|lp.drive.get() / 5.,
                                      |lp, val|lp.drive.set(val * 5.),
                                      |lp| format!("{:.3}", lp.drive.get()))),
        ]
    }

    fn model(&self)->Arc<Self::Model>{
        Arc::clone(&self.model)
    }



    fn editor(&self) -> Self::Editor {
        DruidEditor::new(
            Arc::clone(&self.host),
            self.listener.clone(),
            Arc::clone(&self.model),
            make_editor_widget,
        )
    }

    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        for (input_buffer, output_buffer) in buffer.zip() {
            for (input_sample, output_sample) in input_buffer.iter().zip(output_buffer) {
                self.tick_pivotal(*input_sample);
                // the poles parameter chooses which filter stage we take our output from.
                *output_sample = self.vout[self.model.poles.load(Ordering::Relaxed)];
            }
        }
    }

    fn listener(&self) -> SettableListener<Self::Model> {
        self.listener.clone()
    }
}

impl CarnyxModel for LadderShared {
    type Snap = LadderParametersSnap;

    fn snap(&self) -> LadderParametersSnap {
        LadderParametersSnap {
            cutoff: self.get_cutoff(),
            res: self.res.get(),
            poles: self.poles.load(Ordering::Relaxed),
            drive: self.drive.get(),
        }
    }

    fn set_snap(&self, snap: &LadderParametersSnap) {
        self.set_cutoff(snap.cutoff);
        self.res.set(snap.res);
        self.set_poles_usize(snap.poles);
        self.drive.set(snap.drive);
    }

}

#[derive(Data, Clone, Lens, Debug)]
pub struct LadderParametersSnap {
    cutoff: f32,
    // makes a peak at cutoff
    res: f32,
    // used to choose where we want our output to be
    poles: usize,
    // a drive parameter. Just used to increase the volume, which results in heavier distortion
    drive: f32,
}

impl Default for LadderShared {
    fn default() -> LadderShared {
        LadderShared {
            cutoff: AtomicFloat::new(1000.),
            res: AtomicFloat::new(2.),
            poles: AtomicUsize::new(3),
            pole_value: AtomicFloat::new(1.),
            drive: AtomicFloat::new(0.),
            sample_rate: AtomicFloat::new(44100.),
            g: AtomicFloat::new(0.07135868),
        }
    }
}

// member methods for the struct
impl LadderProcessor {

    pub fn new(host: Arc<dyn CarnyxHost>)->Self{
        LadderProcessor {
            host,
            listener: SettableListener::new(),
            model: Arc::new(LadderShared::default()),
            vout: [0f32; 4],
            s: [0f32; 4],
        }
    }

    // the state needs to be updated after each process. Found by trapezoidal integration
    fn update_state(&mut self) {
        self.s[0] = 2. * self.vout[0] - self.s[0];
        self.s[1] = 2. * self.vout[1] - self.s[1];
        self.s[2] = 2. * self.vout[2] - self.s[2];
        self.s[3] = 2. * self.vout[3] - self.s[3];
    }
    // performs a complete filter process (mystran's method)
    fn tick_pivotal(&mut self, input: f32) {
        let g = self.model.g.get();
        let res = self.model.res.get();
        let drive = self.model.drive.get();

        if drive > 0. {
            self.run_ladder_nonlinear(g, res, input * (drive + 0.7));
        } else {
            //
            self.run_ladder_linear(g, res, input);
        }
        self.update_state();
    }
    // nonlinear ladder filter function with distortion.
    fn run_ladder_nonlinear(&mut self, g: f32, res: f32, input: f32) {
        let mut a = [1f32; 5];
        let base = [input, self.s[0], self.s[1], self.s[2], self.s[3]];
        // a[n] is the fixed-pivot approximation for tanh()
        for n in 0..base.len() {
            a[n] = if base[n] == 0. {
                1.
            } else {
                base[n].tanh() / base[n]
            };
        }
        // denominators of solutions of individual stages. Simplifies the math a bit
        let g0 = 1. / (1. + g * a[1]);
        let g1 = 1. / (1. + g * a[2]);
        let g2 = 1. / (1. + g * a[3]);
        let g3 = 1. / (1. + g * a[4]);
        //  these are just factored out of the feedback solution. Makes the math way easier to read
        let f3 = g * a[3] * g3;
        let f2 = g * a[2] * g2 * f3;
        let f1 = g * a[1] * g1 * f2;
        let f0 = g * g0 * f1;
        // outputs a 24db filter

        self.vout[3] = (f0 * input * a[0]
            + f1 * g0 * self.s[0]
            + f2 * g1 * self.s[1]
            + f3 * g2 * self.s[2]
            + g3 * self.s[3])
            / (f0 * res * a[3] + 1.);
        // since we know the feedback, we can solve the remaining outputs:
        self.vout[0] = g0 * (g * a[1] * (input * a[0] - res * a[3] * self.vout[3]) + self.s[0]);
        self.vout[1] = g1 * (g * a[2] * self.vout[0] + self.s[1]);
        self.vout[2] = g2 * (g * a[3] * self.vout[1] + self.s[2]);
    }
    // linear version without distortion
    pub fn run_ladder_linear(&mut self, g: f32, res: f32, input: f32) {
        // denominators of solutions of individual stages. Simplifies the math a bit
        let g0 = 1. / (1. + g);
        let g1 = g * g0 * g0;
        let g2 = g * g1 * g0;
        let g3 = g * g2 * g0;
        // outputs a 24db filter
        self.vout[3] =
            (g3 * g * input + g0 * self.s[3] + g1 * self.s[2] + g2 * self.s[1] + g3 * self.s[0])
                / (g3 * g * res + 1.);
        // since we know the feedback, we can solve the remaining outputs:
        self.vout[0] = g0 * (g * (input - res * self.vout[3]) + self.s[0]);
        self.vout[1] = g0 * (g * self.vout[0] + self.s[1]);
        self.vout[2] = g0 * (g * self.vout[1] + self.s[2]);
    }
}

impl LadderShared {
    pub fn set_cutoff(&self, value: f32) {
        // cutoff formula gives us a natural feeling cutoff knob that spends more time in the low frequencies
        let cutoff_hz = 20000. * (1.8f32.powf(10. * value - 10.));
        self.cutoff.set(cutoff_hz);
        // bilinear transformation for g gives us a very accurate cutoff
        self.g
            .set((PI * cutoff_hz / (self.sample_rate.get())).tan());
    }
    // returns the value used to set cutoff. for get_parameter function
    pub fn get_cutoff(&self) -> f32 {
        1. + 0.17012975 * (0.00005 * self.cutoff.get()).ln()
    }
    pub fn set_poles(&self, value: f32) {
        self.pole_value.set(value);
        self.poles
            .store(((value * 3.).round()) as usize, Ordering::Relaxed);
    }

    pub fn set_poles_usize(&self, value: usize) {
        let value = value.clamp(0, 3);
        self.pole_value.set((value as f32) / 4.);
        self.poles.store(value, Ordering::Relaxed);
    }
}


/// Gui code

struct F32Lens;

impl Lens<f32, f64> for F32Lens {
    fn with<V, F: FnOnce(&f64) -> V>(&self, data: &f32, f: F) -> V {
        f(&(*data as f64))
    }

    fn with_mut<V, F: FnOnce(&mut f64) -> V>(&self, data: &mut f32, f: F) -> V {
        let mut temp = *data as f64;
        let v = f(&mut temp);
        *data = temp as f32;
        v
    }
}

fn control_labelled<P: Data>(
    axis: Axis,
    name: impl Into<LabelText<P>>,
    w: impl Widget<P> + 'static,
) -> impl Widget<P> {
    Flex::for_axis(axis)
        .with_child(Label::new(name).fix_width(80.))
        .with_flex_child(w, 1.0)
        .padding(Insets::uniform_xy(0., 5.))
}

fn slider_labelled<P: Data>(
    name: impl Into<LabelText<P>>,
    end: f64,
    l: impl Lens<P, f32> + 'static,
) -> impl Widget<P> {
    control_labelled(
        Axis::Vertical,
        name,
        Slider::for_axis(Axis::Vertical)
            .with_range(0., end)
            .lens(l.then(F32Lens))
            .expand_height(),
    )
}

fn dial_labelled<P: Data>(
    name: impl Into<LabelText<P>>,
    end: f64,
    l: impl Lens<P, f32> + 'static,
) -> impl Widget<P> {
    control_labelled(
        Axis::Vertical,
        name,
        Dial::new().with_range(0., end).lens(l.then(F32Lens)),
    )
}

fn make_editor_widget() -> impl Widget<EditorState<LadderShared>> {
    Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_flex_child(
            Flex::row()
                .with_child(slider_labelled("Cutoff", 1.0, LadderParametersSnap::cutoff))
                .with_child(slider_labelled("Resonance", 4.0, LadderParametersSnap::res))
                .with_child(slider_labelled("Drive", 5.0, LadderParametersSnap::drive)),
            1.0,
        )
        .with_flex_child(
            Flex::row()
                .with_child(dial_labelled("Cutoff", 1.0, LadderParametersSnap::cutoff))
                .with_child(dial_labelled("Resonance", 4.0, LadderParametersSnap::res))
                .with_child(dial_labelled("Drive", 5.0, LadderParametersSnap::drive)),
            1.0,
        )
        .with_child(control_labelled(
            Axis::Horizontal,
            "Filter order",
            RadioGroup::for_axis(Axis::Horizontal, (0..=3).map(|i| (i.to_string(), i)))
                .lens(LadderParametersSnap::poles),
        ))
        .lens(EditorState::snap)
}


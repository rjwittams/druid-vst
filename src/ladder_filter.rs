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
use std::sync::{Arc, Mutex};

use druid::widget::{Axis, Controller, Flex, Label, LabelText, RadioGroup, Slider, CrossAxisAlignment};
use druid::{AppLauncher, Data, EmbeddedApp, Env, Event, EventCtx, ExtEventSink, Insets, Lens, LensExt,
            Selector, Target, Widget, WidgetExt, WindowDesc};
use raw_window_handle::macos::MacOSHandle;
use raw_window_handle::RawWindowHandle;
use std::ffi::c_void;
use std::fmt::Debug;
use vst::buffer::AudioBuffer;
use vst::editor::Editor;
use vst::host::{Host};
use vst::plugin::{Category, HostCallback, Info, Plugin, PluginParameters};
use vst::util::AtomicFloat;
use crate::dial::Dial;
use std::ops::Deref;
use druid::lens::Unit;
use crate::HostResizeDragArea;

// this is a 4-pole filter with resonance, which is why there's 4 states and vouts
#[derive(Clone)]
pub struct LadderFilter {
    host: Option<HostCallback>,

    // Store a handle to the plugin's parameter object.
    params: Arc<LadderParameters>,
    // the output of the different filter stages
    vout: [f32; 4],
    // s is the "state" parameter. In an IIR it would be the last value from the filter
    // In this we find it by trapezoidal integration to avoid the unit delay
    s: [f32; 4],
}

struct LadderParameters {
    sink: Mutex<Option<ExtEventSink>>,
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

#[derive(Data, Clone, Lens, Debug)]
struct LadderParametersSnap {
    cutoff: f32,
    // makes a peak at cutoff
    res: f32,
    // used to choose where we want our output to be
    poles: usize,
    // a drive parameter. Just used to increase the volume, which results in heavier distortion
    drive: f32,
}

impl Default for LadderParameters {
    fn default() -> LadderParameters {
        LadderParameters {
            sink: Mutex::new(None),
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
impl LadderFilter {
    // the state needs to be updated after each process. Found by trapezoidal integration
    fn update_state(&mut self) {
        self.s[0] = 2. * self.vout[0] - self.s[0];
        self.s[1] = 2. * self.vout[1] - self.s[1];
        self.s[2] = 2. * self.vout[2] - self.s[2];
        self.s[3] = 2. * self.vout[3] - self.s[3];
    }
    // performs a complete filter process (mystran's method)
    fn tick_pivotal(&mut self, input: f32) {
        let g = self.params.g.get();
        let res = self.params.res.get();
        let drive = self.params.drive.get();

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
        self.vout[0] = g0
            * (g
                * a[1]
                * (input * a[0] - res * a[3] * self.vout[3])
                + self.s[0]);
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
        self.vout[3] = (g3 * g * input
            + g0 * self.s[3]
            + g1 * self.s[2]
            + g2 * self.s[1]
            + g3 * self.s[0])
            / (g3 * g * res + 1.);
        // since we know the feedback, we can solve the remaining outputs:
        self.vout[0] =
            g0 * (g * (input - res * self.vout[3]) + self.s[0]);
        self.vout[1] = g0 * (g * self.vout[0] + self.s[1]);
        self.vout[2] = g0 * (g * self.vout[1] + self.s[2]);
    }
}

impl LadderParameters {
    pub fn set_cutoff(&self, value: f32) {
        // cutoff formula gives us a natural feeling cutoff knob that spends more time in the low frequencies
        let cutoff_hz = 20000. * (1.8f32.powf(10. * value - 10.));
        self.cutoff.set(cutoff_hz);
        // bilinear transformation for g gives us a very accurate cutoff
        self.g.set((PI * cutoff_hz / (self.sample_rate.get())).tan());
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

    pub fn snap(&self) -> LadderParametersSnap {
        LadderParametersSnap {
            cutoff: self.get_cutoff(),
            res: self.res.get(),
            poles: self.poles.load(Ordering::Relaxed),
            drive: self.drive.get(),
        }
    }

    pub fn set_snap(&self, snap: &LadderParametersSnap) {
        self.set_cutoff(snap.cutoff);
        self.res.set(snap.res);
        self.set_poles_usize(snap.poles);
        self.drive.set(snap.drive);
    }
}

pub const PARAMS_CHANGED: Selector = Selector::new("druid-vst.params-changed");

impl PluginParameters for LadderParameters {
    // get_parameter has to return the value used in set_parameter
    fn get_parameter(&self, index: i32) -> f32 {
        match index {
            0 => self.get_cutoff(),
            1 => self.res.get() / 4.,
            2 => self.pole_value.get(),
            3 => self.drive.get() / 5.,
            _ => 0.0,
        }
    }
    fn set_parameter(&self, index: i32, value: f32) {
        match index {
            0 => self.set_cutoff(value),
            1 => self.res.set(value * 4.),
            2 => self.set_poles(value),
            3 => self.drive.set(value * 5.),
            _ => (),
        }
        let sink_b = &self.sink.lock();
        if let Ok(sink) = sink_b {
            if let Some(sink) = sink.as_ref() {
                println!("Sent params changed");
                sink.submit_command(PARAMS_CHANGED, (), Target::Global)
                    .expect("Submit command to sink")
            }
        }
    }

    fn get_parameter_name(&self, index: i32) -> String {
        match index {
            0 => "cutoff".to_string(),
            1 => "resonance".to_string(),
            2 => "filter order".to_string(),
            3 => "drive".to_string(),
            _ => "".to_string(),
        }
    }
    fn get_parameter_label(&self, index: i32) -> String {
        match index {
            0 => "Hz".to_string(),
            1 => "%".to_string(),
            2 => "poles".to_string(),
            3 => "%".to_string(),
            _ => "".to_string(),
        }
    }
    // This is what will display underneath our control.  We can
    // format it into a string that makes the most sense.
    fn get_parameter_text(&self, index: i32) -> String {
        match index {
            0 => format!("{:.0}", self.cutoff.get()),
            1 => format!("{:.3}", self.res.get()),
            2 => format!("{}", self.poles.load(Ordering::Relaxed) + 1),
            3 => format!("{:.3}", self.drive.get()),
            _ => format!(""),
        }
    }
}
impl Default for LadderFilter {
    fn default() -> LadderFilter {
        LadderFilter {
            host: None,
            vout: [0f32; 4],
            s: [0f32; 4],
            params: Arc::new(LadderParameters::default()),
        }
    }
}
impl Plugin for LadderFilter {
    fn get_info(&self) -> Info {
        Info {
            name: "LadderFilter".to_string(),
            unique_id: 9263,
            inputs: 1,
            outputs: 1,
            category: Category::Effect,
            parameters: 4,
            ..Default::default()
        }
    }

    fn new(host: HostCallback) -> Self
    where
        Self: Sized + Default,
    {
        LadderFilter {
            host: Some(host),
            vout: [0f32; 4],
            s: [0f32; 4],
            params: Arc::new(LadderParameters::default()),
        }
    }

    fn set_sample_rate(&mut self, rate: f32) {
        self.params.sample_rate.set(rate);
    }

    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        for (input_buffer, output_buffer) in buffer.zip() {
            for (input_sample, output_sample) in input_buffer.iter().zip(output_buffer) {
                self.tick_pivotal(*input_sample);
                // the poles parameter chooses which filter stage we take our output from.
                *output_sample = self.vout[self.params.poles.load(Ordering::Relaxed)];
            }
        }
    }

    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }

    fn get_editor(&mut self) -> Option<Box<dyn Editor>> {
        self.host
            .clone()
            .map(|h| Box::new(DruidEditor::new(h, Arc::clone(&self.params))) as Box<dyn Editor>)
    }
}

struct DruidEditor {
    host_callback: HostCallback,
    params: Arc<LadderParameters>,
    app: Option<EmbeddedApp>,
}

impl DruidEditor {
    pub fn new(host_callback: HostCallback, params: Arc<LadderParameters>) -> Self {
        DruidEditor {
            host_callback,
            params,
            app: None,
        }
    }
}

#[derive(Clone, Default)]
struct HostCallbackData{
    inner: HostCallback
}

impl Data for HostCallbackData{
    fn same(&self, other: &Self) -> bool {
        self.inner.raw_callback() == other.inner.raw_callback() &&
            self.inner.raw_effect() == other.inner.raw_effect()
    }
}

impl Deref for HostCallbackData{
    type Target = HostCallback;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Data, Lens, Clone)]
pub struct EditorState {
    host_callback: HostCallbackData,
    #[data(ignore)]
    params: Arc<LadderParameters>,
    params_snap: LadderParametersSnap,
}

impl Default for EditorState{
    fn default() -> Self {
        let params = Arc::new(LadderParameters::default());
        let params_snap = params.snap();
        Self{
            host_callback: Default::default(),
            params,
            params_snap
        }
    }
}

struct EditorController;

impl<W: Widget<EditorState>> Controller<EditorState, W> for EditorController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut EditorState,
        env: &Env,
    ) {
        match event {
            Event::Command(cmd) if cmd.is(PARAMS_CHANGED) => {
                data.params_snap = data.params.snap();
            }
            _ => {
                let old_snap = data.params_snap.clone();
                child.event(ctx, event, data, env);
                if !old_snap.same(&data.params_snap) {
                    data.params.set_snap(&data.params_snap);
                    if data.host_callback.raw_callback().is_some() {
                        data.host_callback.update_display();
                    }
                }
            }
        }
    }
}

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
        Dial::new()
            .with_range(0., end)
            .lens(l.then(F32Lens)),
    )
}

#[cfg(target_os="macos")]
fn to_raw_window_handle(parent: *mut c_void)->RawWindowHandle{
    RawWindowHandle::MacOS(MacOSHandle {
        ns_view: parent as *mut _,
        ..MacOSHandle::empty()
    })
}

pub fn make_editor_widget(host: HostCallback)->impl Widget<EditorState>{
    Flex::column().with_flex_child(
        Flex::column()
            .cross_axis_alignment(CrossAxisAlignment::Start)
            // .with_child(
            //     Label::dynamic(|params: &LadderParametersSnap, _env: &Env| format!("{:#?}", params))
            //         .with_line_break_mode(LineBreaking::WordWrap)
            //         .fix_height(150.),
            // )
            .with_flex_child(Flex::row()
                                 .with_child(slider_labelled("Cutoff", 1.0, LadderParametersSnap::cutoff))
                                 .with_child(slider_labelled("Resonance", 4.0, LadderParametersSnap::res))
                                 .with_child(slider_labelled("Drive", 5.0, LadderParametersSnap::drive)), 1.0)
            .with_flex_child(Flex::row()
                                 .with_child(dial_labelled("Cutoff", 1.0, LadderParametersSnap::cutoff))
                                 .with_child(dial_labelled("Resonance", 4.0, LadderParametersSnap::res))
                                 .with_child(dial_labelled("Drive", 5.0, LadderParametersSnap::drive))
                                 , 1.0)
            .with_child(control_labelled(Axis::Horizontal, "Filter order",
                                         RadioGroup::for_axis(Axis::Horizontal, (0..=3).map(|i| (i.to_string(), i))).lens(LadderParametersSnap::poles), )
            )
            .lens(EditorState::params_snap)
            .controller(EditorController), 1.0 )
        .with_child(Flex::row().with_flex_spacer(1.0).with_child(HostResizeDragArea::new(host).lens(Unit)))
}



impl Editor for DruidEditor {
    fn size(&self) -> (i32, i32) {
        (500, 500)
    }

    fn position(&self) -> (i32, i32) {
        (100, 100)
    }

    fn open(&mut self, parent: *mut c_void) -> bool {

        let snap_edit = make_editor_widget(self.host_callback.clone());
        let window_desc = WindowDesc::new(snap_edit);
        let state = EditorState {
            host_callback: HostCallbackData{ inner: self.host_callback.clone() },
            params: Arc::clone(&self.params),
            params_snap: self.params.snap(),
        };

        let raw = to_raw_window_handle(parent);
        self.app = AppLauncher::with_window(window_desc)
            .launch_embedded(state, raw)
            .ok();

        if let Some(app) = &self.app {
            if let Ok(mut ps) = self.params.sink.lock() {
                *ps = Some(app.sink.clone());
            }
        }
        true
    }

    fn is_open(&mut self) -> bool {
        self.app.is_some()
    }
}

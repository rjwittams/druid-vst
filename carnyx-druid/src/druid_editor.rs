use std::sync::Arc;

use druid::{AppLauncher, Data, EmbeddedApp, Env, Event, EventCtx, Lens, Selector, Widget, WidgetExt, WindowDesc, Target, ExtEventSink, Size};
use druid::lens::Unit;
use druid::widget::{Controller, Flex, Label};

use raw_window_handle::RawWindowHandle;
use crate::HostResizeDragArea;
use carnyx::carnyx::{CarnyxModel, CarnyxModelListener, CarnyxHost, CarnyxEditor, SettableListener};
use std::marker::PhantomData;
use carnyx::CarnyxWindowResizer;

pub struct DruidEditor<Model: CarnyxModel> {
    make_editor: Box<dyn Fn() -> Box<dyn Widget<EditorState<Model>>>>,
    host: Arc<dyn CarnyxHost>,
    listener: SettableListener<Model>,
    model: Arc<Model>,
    app: Option<EmbeddedApp>,
}

impl<Model: CarnyxModel> DruidEditor<Model> where Model::Snap : Data{
    pub fn new<W: Widget<EditorState<Model>> + 'static>(
        host: Arc<dyn CarnyxHost>,
        listener: SettableListener<Model>,
        model: Arc<Model>,
        f: impl Fn() -> W + 'static,
    ) -> Self {
        DruidEditor {
            make_editor: Box::new(move || f().boxed()),
            host,
            listener,
            model,
            app: None,
        }
    }
}

fn wrap_editor_widget<Model: CarnyxModel>(
    host: Arc<dyn CarnyxHost>,
    window_resizer: Box<dyn CarnyxWindowResizer>,
    params: Arc<Model>,
    child: impl Widget<EditorState<Model>> + 'static) -> impl Widget<EditorState<Model>> where Model::Snap : Data {

    Flex::column()
        .with_flex_child(
            child,
            1.0
        )
        .with_child(
            Flex::row()
                .with_flex_spacer(1.0)
                .with_child(HostResizeDragArea::new(window_resizer).lens(Unit)),
        ).controller(EditorController::new(host, params))
}

struct ExtEventListener<Model: CarnyxModel>{
    sink: ExtEventSink,
    phantom_m: PhantomData<fn()->Model>
}

impl<Model: CarnyxModel> ExtEventListener<Model> {
    pub fn new(sink: ExtEventSink) -> Self {
        ExtEventListener { sink, phantom_m: PhantomData }
    }
}

impl <Model: CarnyxModel> CarnyxModelListener<Model> for ExtEventListener<Model>{
    fn notify_change(&self, _model: &Model) {
        self.sink.submit_command(MODEL_CHANGED, (), Target::Global)
            .expect("Submit command to sink");
    }
}

impl<Model: CarnyxModel> CarnyxEditor for DruidEditor<Model> where Model::Snap : Data {

    fn initial_size(&self) -> (usize, usize) {
        (500, 500)
    }

    fn initial_position(&self) -> (isize, isize) {
        (100, 100)
    }

    fn open(&mut self, handle: Option<RawWindowHandle>, window_resizer: Box<dyn CarnyxWindowResizer>) -> bool {
        if let Some(raw) = handle {
            let make_editor = &self.make_editor;
            let snap_edit = make_editor();
            let wrapped = wrap_editor_widget(self.host.clone(), window_resizer, Arc::clone(&self.model), snap_edit);
            let (w, h) = self.initial_size();
            let window_desc = WindowDesc::new(wrapped)
                .window_size(Size::new(w as f64, h as f64))
                .show_titlebar(false)
                .resizable(false);
            let state = EditorState {
                snap: self.model.snap(),
            };

            self.app = AppLauncher::with_window(window_desc)
                .launch_embedded(state, raw).ok();

            if let Some(app) = &self.app {
                let sink = app.sink.clone();
                self.listener.set_listener(Box::new(ExtEventListener::new(sink)));
                true
            } else {
                false
            }
        }else{
            false
        }
    }

    fn is_open(&self) -> bool {
        self.app.is_some()
    }
}

#[derive(Lens)]
pub struct EditorState<Model: CarnyxModel> {
    snap: Model::Snap,
}

impl<Model: CarnyxModel> Clone for EditorState<Model> where Model::Snap : Clone {
    fn clone(&self) -> Self {
        EditorState {
            snap: self.snap.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.snap = source.snap.clone();
    }
}

impl<Model: CarnyxModel> Data for EditorState<Model> where Model::Snap : Data {
    fn same(&self, other: &Self) -> bool {
        self.snap.same(&other.snap)
    }
}

pub const MODEL_CHANGED: Selector = Selector::new("carnyx.model-changed");

pub struct EditorController<Model: CarnyxModel>{
    host: Arc<dyn CarnyxHost>,
    params: Arc<Model>
}

impl <Model: CarnyxModel> EditorController<Model> {
    pub fn new(host: Arc<dyn CarnyxHost>, params: Arc<Model>) -> Self {
        EditorController { host, params }
    }
}

impl<Model: CarnyxModel, W: Widget<EditorState<Model>>>
Controller<EditorState<Model>, W> for EditorController<Model> where Model::Snap : Data
{
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut EditorState<Model>,
        env: &Env,
    ) {
        match event {
            Event::Command(cmd) if cmd.is(MODEL_CHANGED) => {
                data.snap = self.params.snap();
            }
            _ => {
                let old_snap = data.snap.clone();
                child.event(ctx, event, data, env);
                if !old_snap.same(&data.snap) {
                    self.params.set_snap(&data.snap);
                    self.host.update_host_display();
                }
            }
        }
    }
}
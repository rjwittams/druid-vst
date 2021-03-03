use ladder_filter::LadderProcessor;
use carnyx::carnyx::{CarnyxHost, CarnyxProcessor, CarnyxEditor};
use druid::widget::prelude::*;
use std::sync::Arc;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use druid::{Selector, ExtEventError, ExtEventSink, WindowSizePolicy, WidgetExt, NativeWindowHandle, Color};
use carnyx::CarnyxWindowResizer;
use druid::widget::{Flex, Button};

struct DruidHost{

}

impl CarnyxHost for DruidHost{
    fn update_host_display(&self) {

    }
}

struct EditorHost<Editor: CarnyxEditor>{
    editor: Editor,
    desired_size: Option<Size>,
    native_child: Option<NativeWindowHandle>
}

impl<Editor: CarnyxEditor> EditorHost<Editor> {
    pub fn new(editor: Editor) -> Self {
        EditorHost { editor, desired_size: None, native_child: None }
    }
}

struct EditorResizer{
    ext_event_sink: ExtEventSink,
    widget_id: WidgetId
}

impl CarnyxWindowResizer for EditorResizer{
    fn resize_editor_window(&self, width: usize, height: usize) -> bool {
        self.ext_event_sink.submit_command(HOST_RESIZE, Size::new(width as f64, height as f64), self.widget_id).is_ok()
    }
}

pub const HOST_RESIZE: Selector<Size> = Selector::new("druid-vst.host_resize");
impl <Editor: CarnyxEditor> Widget<()> for EditorHost<Editor>{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, _data: &mut (), _env: &Env) {
        match event{
            Event::Command(cmd) if cmd.is(HOST_RESIZE) => {
                if let Some(size) = cmd.get(HOST_RESIZE){
                    eprintln!("HOST_RESIZE desired size to {:?}", size);
                    self.desired_size = Some(*size);
                    ctx.request_layout()
                }
            },
            Event::NativeWindowConnected(native)=> {
                self.native_child = Some(native.clone());
                let raw = native.0.raw_window_handle();
                let (w, h) = self.editor.initial_size();
                let size = Size::new(w as f64, h as f64);
                self.desired_size = Some(size);
                self.editor.open(Some(raw), Box::new(EditorResizer{
                    ext_event_sink: ctx.get_external_handle(),
                    widget_id: ctx.widget_id()
                }));
                ctx.request_layout();
            }
            _=>()
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _data: &(), _env: &Env) {
        if let LifeCycle::WidgetAdded = event {
            let (w, h) = self.editor.initial_size();
            let size = Size::new(w as f64, h as f64);
            ctx.request_native_window(size)
        }
    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &(), _data: &(), _env: &Env) {

    }

    fn layout(&mut self, _ctx: &mut LayoutCtx, bc: &BoxConstraints, _data: &(), _env: &Env) -> Size {
        let size = bc.constrain(self.desired_size.unwrap_or(Size::ZERO));
        if let Some(nc) = &self.native_child {
            nc.0.set_native_layout(None, self.desired_size);
        }
        size
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _data: &(), _env: &Env) {

    }

    fn post_render(&mut self) {

    }
}

pub fn main() {
    use druid::{WindowDesc, AppLauncher};

    let opener = Flex::column()
        .with_child(Button::new("Add plugin window").on_click(|ctx, _, _|{
            let processor = LadderProcessor::new(Arc::new(DruidHost{}));
            let editor = processor.editor();
            let edit_window = WindowDesc::new(EditorHost::new(editor).border(Color::WHITE, 1.))
                .title("Plugin Editor")
                .resizable(false)
                .window_size_policy(WindowSizePolicy::Content);
            ctx.new_window(edit_window);
        }));


    //.window_size_policy(WindowSizePolicy::Content);

    // create the initial app state
    // start the application
    AppLauncher::with_window(WindowDesc::new(opener).window_size_policy(WindowSizePolicy::Content))
        .use_env_tracing()
        .launch(())
        .expect("Failed to launch application");
}

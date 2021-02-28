use ladder_filter::LadderProcessor;
use carnyx::carnyx::{CarnyxHost, CarnyxProcessor, CarnyxEditor};
use druid::widget::prelude::*;
use std::sync::Arc;
use raw_window_handle::HasRawWindowHandle;

struct DruidHost{

}

impl CarnyxHost for DruidHost{
    fn update_host_display(&self) {

    }

    fn resize_editor_window(&self, _width: usize, _height: usize) {

    }
}

struct EditorHost<Editor: CarnyxEditor>{
    editor: Editor
}

impl <Editor: CarnyxEditor> Widget<()> for EditorHost<Editor>{
    fn event(&mut self, _ctx: &mut EventCtx, event: &Event, _data: &mut (), _env: &Env) {
        match event{
            Event::NativeWindowConnected(native)=>{
                let raw = native.0.raw_window_handle();
                self.editor.open(Some(raw));
            }
            _=>()
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _data: &(), _env: &Env) {
        if let LifeCycle::WidgetAdded = event {
            let (w, h) = self.editor.initial_size();
            ctx.request_native_window(Size::new(w as f64, h as f64));
        }

    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &(), _data: &(), _env: &Env) {

    }

    fn layout(&mut self, _ctx: &mut LayoutCtx, bc: &BoxConstraints, _data: &(), _env: &Env) -> Size {
        let (w, h) = self.editor.initial_size();

        bc.constrain( (w as f64, h as f64) )
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _data: &(), _env: &Env) {

    }

    fn post_render(&mut self) {

    }
}

pub fn main() {
    use druid::{WindowDesc, AppLauncher};


    let processor = LadderProcessor::new(Arc::new(DruidHost{}));
    let editor = processor.editor();


    let main_window = WindowDesc::new( EditorHost{ editor } )
        .title("Plugin Editor")
        .window_size((500., 500.));
    //.window_size_policy(WindowSizePolicy::Content);

    // create the initial app state
    // start the application
    AppLauncher::with_window(main_window)
        .use_env_tracing()
        .launch(())
        .expect("Failed to launch application");
}

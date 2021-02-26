use vst::plugin::HostCallback;
use druid::widget::prelude::*;
use vst::host::Host;
use std::ffi::{CString, c_void};
use druid::{MouseEvent, theme, Point};
use druid::kurbo::Line;

pub struct HostResizeDragArea{
    host: HostCallback,
    drag_start_window: Option<(Point, Size)>,
}

impl HostResizeDragArea {
    pub fn new(host: HostCallback) -> Self {
        HostResizeDragArea { host, drag_start_window: None }
    }

    fn set_window_size(&self, ctx: &EventCtx, size: Size){
        let hcb = self.host.clone();

        if let Some(idle) = ctx.window().get_idle_handle() {
            let (_, vendor, _) = hcb.get_info();
            let is_ableton = "Ableton".eq(&vendor);

            idle.add_idle(move |_|{
                if let Some(callback) = hcb.raw_callback() {
                    let effect = hcb.raw_effect();
                    let string = CString::new("sizeWindow").unwrap();

                    let res = callback(effect, vst::host::OpCode::CanDo.into(), 0, 0, string.as_bytes().as_ptr() as *mut c_void, 0.);
                    if res == 1 || is_ableton {

                        let res = callback(effect, vst::host::OpCode::SizeWindow.into(), size.width as i32, size.height as isize, std::ptr::null_mut(), 0.);
                        eprintln!("Result of SizeWindow {:?}", res);
                    }
                }}
            );
        }
    }

    fn resize(&self, ctx: &EventCtx, mouse: &MouseEvent)->bool{
        if let Some((start, size)) = self.drag_start_window {
            let change = mouse.window_pos - start;
            let desired_size = size + change.to_size();
            self.set_window_size(ctx, desired_size);
            true
        }else{
            false
        }
    }
}

impl Widget<()> for HostResizeDragArea{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, _data: &mut (), _env: &Env) {

        match event{
            Event::MouseDown(mouse)=>{
                ctx.set_active(true);
                self.drag_start_window = Some((mouse.window_pos, ctx.window().get_size()));
            },
            Event::MouseMove(mouse)=>{
                self.resize(ctx, mouse);
            },
            Event::MouseUp(mouse)=>{
                self.resize(ctx, mouse);
                self.drag_start_window = None;
                ctx.set_active(false);
            }
            _=>()
        }
    }

    fn lifecycle(&mut self, _ctx: &mut LifeCycleCtx, _event: &LifeCycle, _data: &(), _env: &Env) {

    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &(), _data: &(), _env: &Env) {

    }

    fn layout(&mut self, _ctx: &mut LayoutCtx, bc: &BoxConstraints, _data: &(), env: &Env) -> Size {
        let h = env.get(theme::BASIC_WIDGET_HEIGHT);
        bc.constrain(Size::new(h, h))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _data: &(), env: &Env) {
        let rect = ctx.size().to_rect().inset(-env.get(theme::WIDGET_CONTROL_COMPONENT_PADDING) );
        let line = Line::new( (rect.x0, rect.y1), (rect.x1, rect.y0));
        ctx.stroke(line, &env.get(theme::FOREGROUND_DARK), 2.);
    }

    fn post_render(&mut self) {

    }
}
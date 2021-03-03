use druid::kurbo::Line;
use druid::widget::prelude::*;
use druid::{theme, MouseEvent, Point, Selector, Scalable};
use std::sync::Arc;
use carnyx::{CarnyxHost, CarnyxWindowResizer};
use raw_window_handle::HasRawWindowHandle;

pub struct HostResizeDragArea {
    resizer: Box<dyn CarnyxWindowResizer>,
    drag_start_window: Option<(Point, Size)>,
}

impl HostResizeDragArea {
    pub fn new(resizer: Box<dyn CarnyxWindowResizer>) -> Self {
        HostResizeDragArea {
            resizer,
            drag_start_window: None,
        }
    }

    fn resize(&self, ctx: &mut EventCtx, mouse: &MouseEvent) {
        if let Some((start, size)) = self.drag_start_window {
            let change = mouse.window_pos - start;
            let desired_size = size + change.to_size();
            //eprintln!("Submitting idle resize {:?}", (start, mouse.window_pos, change, size, desired_size));
            ctx.submit_command(IDLE_RESIZE.with(desired_size).to(ctx.widget_id()));
        }
    }
}

pub const IDLE_RESIZE: Selector<Size> = Selector::new("carnyx-druid.idle-resize");
impl Widget<()> for HostResizeDragArea {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, _data: &mut (), _env: &Env) {
        match event {
            Event::Command(cmd) if cmd.is(IDLE_RESIZE) => {
                if let Some(size) = cmd.get(IDLE_RESIZE) {
                    //eprintln!("idle resize {:?}", size);
                    if self.resizer.resize_editor_window(size.width as usize, size.height as usize) {
                        //ctx.window().set_native_layout(None, Some(*size));
                        ctx.window().set_size(*size)
                    }
                }
            },
            Event::MouseDown(mouse) => {
                ctx.set_active(true);
                if let Ok(scale) = ctx.window().get_scale() {
                    let size = ctx.window().get_size();
                    let dp_size = size.to_dp(scale);
                    self.drag_start_window = Some((mouse.window_pos, dp_size));
                }
            }
            Event::MouseMove(mouse) => {
                self.resize(ctx, mouse);
            }
            Event::MouseUp(mouse) => {
                self.resize(ctx, mouse);
                self.drag_start_window = None;
                ctx.set_active(false);
            }
            _ => (),
        }
    }

    fn lifecycle(&mut self, _ctx: &mut LifeCycleCtx, _event: &LifeCycle, _data: &(), _env: &Env) {

    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &(), _data: &(), _env: &Env) {}

    fn layout(&mut self, _ctx: &mut LayoutCtx, bc: &BoxConstraints, _data: &(), env: &Env) -> Size {
        let h = env.get(theme::BASIC_WIDGET_HEIGHT);
        bc.constrain(Size::new(h, h))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _data: &(), env: &Env) {
        let rect = ctx
            .size()
            .to_rect()
            .inset(-env.get(theme::WIDGET_CONTROL_COMPONENT_PADDING));
        let line = Line::new((rect.x0, rect.y1), (rect.x1, rect.y0));
        ctx.stroke(line, &env.get(theme::FOREGROUND_DARK), 2.);
    }

    fn post_render(&mut self) {}
}

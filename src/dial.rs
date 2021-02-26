// Copyright 2019 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A dial widget.

use druid::kurbo::{Shape, CircleSegment};
use druid::widget::prelude::*;
use druid::{theme, LinearGradient, Point, UnitPoint};
use std::f64::consts::PI;

const STROKE_WIDTH: f64 = 2.0;

/// A slider, allowing interactive update of a numeric value.
///
/// This slider implements `Widget<f64>`, and works on values clamped
/// in the range `min..max`.
#[derive(Debug, Clone)]
pub struct Dial {
    min: f64,
    max: f64,
    mouse_last: Option<Point>,
    hovered: bool
}

impl Default for Dial {
    fn default() -> Self {
        Dial::new()
    }
}

impl Dial {
    /// Create a new `Dial`
    pub fn new() -> Dial {
        Dial {
            min: 0.,
            max: 1.,
            mouse_last: None,
            hovered: false
        }
    }

    /// Builder-style method to set the range covered by this dial.
    ///
    /// The default range is `0.0..1.0`.
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self
    }
}

impl Dial {
    fn normalize(&self, data: f64) -> f64 {
        (data.clamp(self.min, self.max) - self.min) / (self.max - self.min)
    }

    fn make_segment(&self, data: &f64, env: &Env, size: Size) -> CircleSegment {
        let rect = size.to_rect();
        let clamped = self.normalize(*data);
        let center = rect.center();
        let inset_rect = rect
            .contained_rect_with_aspect_ratio(1.0)
            .inset(-env.get(theme::WIDGET_CONTROL_COMPONENT_PADDING));

        let start_angle = 0.75 * PI;
        //let end_angle = 2.25 * PI;

        let outer = inset_rect.height() / 2.;
        let seg = CircleSegment::new(center, outer, outer * 0.5, start_angle, 2. * PI * 0.75 * clamped);
        seg
    }
}

impl Widget<f64> for Dial {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut f64, env: &Env) {
        match event {
            Event::MouseDown(mouse) => {
                ctx.set_active(true);
                self.mouse_last = Some(mouse.pos);
                ctx.request_paint();
            }
            Event::MouseUp(_) => {
                if ctx.is_active() {
                    ctx.set_active(false);
                    ctx.request_paint();
                }
            }
            Event::MouseMove(mouse) => {
                if ctx.is_active() {
                    if let Some(last) = self.mouse_last {
                        let y_move = last.y - mouse.pos.y;
                        let tmp = *data + (self.max - self.min) * y_move / ctx.size().height;
                        *data = tmp.clamp(self.min, self.max);
                        ctx.request_paint();
                    }
                    self.mouse_last = Some(mouse.pos);
                }
                if ctx.is_hot() {
                    let shape = self.make_segment(data, env, ctx.size());
                    let mouse_pos = mouse.pos;
                    let hover = shape.winding(mouse_pos) > 0;
                    if hover != self.hovered {
                        self.hovered = hover;
                        ctx.request_paint();
                    }
                }
            }
            _ => (),
        }
    }

    fn lifecycle(&mut self, _ctx: &mut LifeCycleCtx, _event: &LifeCycle, _data: &f64, _env: &Env) {}

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &f64, _data: &f64, _env: &Env) {

        ctx.request_paint();
    }

    fn layout(&mut self, _ctx: &mut LayoutCtx, bc: &BoxConstraints, _data: &f64, _env: &Env) -> Size {
        bc.debug_check("Dial");
        bc.constrain_aspect_ratio(1.0, f64::INFINITY)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &f64, env: &Env) {
        let seg = self.make_segment(data, env, ctx.size());

        let is_active = ctx.is_active();
        let is_hovered = self.hovered;
        let (start, end) = (UnitPoint::TOP, UnitPoint::BOTTOM);
        let stops = (env.get(theme::FOREGROUND_LIGHT), env.get(theme::FOREGROUND_DARK));
        let stops = if is_active { (stops.1, stops.0) } else { stops };
        let gradient = LinearGradient::new(start, end, stops);

        let border_color = if is_hovered || is_active {
            env.get(theme::FOREGROUND_LIGHT)
        } else {
            env.get(theme::FOREGROUND_DARK)
        };

        ctx.stroke(&seg, &border_color, STROKE_WIDTH);
        ctx.fill(&seg, &gradient);
    }

    fn post_render(&mut self) {}
}



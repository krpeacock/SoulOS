use crate::form::Form;
use crate::palette::{BLACK, WHITE};
use crate::primitives::hit_test;
use alloc::string::String;
use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
}

pub struct EditOverlay {
    pub selected_id: Option<String>,
    pub dragging: bool,
    pub resize_handle: Option<ResizeHandle>,
    last_pen: Point,
}

const HANDLE_SIZE: u32 = 6;

impl Default for EditOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl EditOverlay {
    pub fn new() -> Self {
        Self {
            selected_id: None,
            dragging: false,
            resize_handle: None,
            last_pen: Point::zero(),
        }
    }

    pub fn draw<D>(&self, target: &mut D, form: &Form) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        if let Some(id) = &self.selected_id {
            if let Some(comp) = form.components.iter().find(|c| &c.id == id) {
                let rect = comp.bounds.to_eg_rect();

                // Draw dotted or thin selection border
                rect.into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
                    .draw(target)?;

                // Draw handles
                self.draw_handle(target, rect.top_left)?; // TL
                self.draw_handle(
                    target,
                    rect.top_left + Point::new(rect.size.width as i32, 0),
                )?; // TR
                self.draw_handle(
                    target,
                    rect.top_left + Point::new(0, rect.size.height as i32),
                )?; // BL
                self.draw_handle(
                    target,
                    rect.top_left + Point::new(rect.size.width as i32, rect.size.height as i32),
                )?; // BR

                // Midpoints
                self.draw_handle(
                    target,
                    rect.top_left + Point::new(rect.size.width as i32 / 2, 0),
                )?; // T
                self.draw_handle(
                    target,
                    rect.top_left + Point::new(rect.size.width as i32 / 2, rect.size.height as i32),
                )?; // B
                self.draw_handle(
                    target,
                    rect.top_left + Point::new(0, rect.size.height as i32 / 2),
                )?; // L
                self.draw_handle(
                    target,
                    rect.top_left + Point::new(rect.size.width as i32, rect.size.height as i32 / 2),
                )?; // R
            }
        }
        Ok(())
    }

    fn draw_handle<D>(&self, target: &mut D, center: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let rect = Rectangle::with_center(center, Size::new(HANDLE_SIZE, HANDLE_SIZE));
        rect.into_styled(PrimitiveStyle::with_fill(WHITE))
            .draw(target)?;
        rect.into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
            .draw(target)?;
        Ok(())
    }

    pub fn pen_down(&mut self, form: &Form, x: i16, y: i16) -> bool {
        let p = Point::new(x as i32, y as i32);
        self.last_pen = p;

        // Check handles if something is selected
        if let Some(id) = &self.selected_id {
            if let Some(comp) = form.components.iter().find(|c| &c.id == id) {
                let rect = comp.bounds.to_eg_rect();
                if let Some(h) = self.hit_test_handles(rect, p) {
                    self.resize_handle = Some(h);
                    return true;
                }

                // If inside component but not on handle, start dragging
                if hit_test(&rect, x, y) {
                    self.dragging = true;
                    return true;
                }
            }
        }

        // Try selecting a new component
        if let Some(comp) = form.hit_test(x, y) {
            self.selected_id = Some(comp.id.clone());
            self.dragging = true;
            true
        } else {
            self.selected_id = None;
            false
        }
    }

    pub fn pen_move(&mut self, form: &mut Form, x: i16, y: i16) -> bool {
        let p = Point::new(x as i32, y as i32);
        let delta = p - self.last_pen;
        self.last_pen = p;

        if let Some(id) = &self.selected_id {
            if let Some(comp) = form.components.iter_mut().find(|c| &c.id == id) {
                if let Some(h) = self.resize_handle {
                    match h {
                        ResizeHandle::TopLeft => {
                            comp.bounds.x += delta.x;
                            comp.bounds.y += delta.y;
                            comp.bounds.w = (comp.bounds.w as i32 - delta.x).max(8) as u32;
                            comp.bounds.h = (comp.bounds.h as i32 - delta.y).max(8) as u32;
                        }
                        ResizeHandle::TopRight => {
                            comp.bounds.y += delta.y;
                            comp.bounds.w = (comp.bounds.w as i32 + delta.x).max(8) as u32;
                            comp.bounds.h = (comp.bounds.h as i32 - delta.y).max(8) as u32;
                        }
                        ResizeHandle::BottomLeft => {
                            comp.bounds.x += delta.x;
                            comp.bounds.w = (comp.bounds.w as i32 - delta.x).max(8) as u32;
                            comp.bounds.h = (comp.bounds.h as i32 + delta.y).max(8) as u32;
                        }
                        ResizeHandle::BottomRight => {
                            comp.bounds.w = (comp.bounds.w as i32 + delta.x).max(8) as u32;
                            comp.bounds.h = (comp.bounds.h as i32 + delta.y).max(8) as u32;
                        }
                        ResizeHandle::Top => {
                            comp.bounds.y += delta.y;
                            comp.bounds.h = (comp.bounds.h as i32 - delta.y).max(8) as u32;
                        }
                        ResizeHandle::Bottom => {
                            comp.bounds.h = (comp.bounds.h as i32 + delta.y).max(8) as u32;
                        }
                        ResizeHandle::Left => {
                            comp.bounds.x += delta.x;
                            comp.bounds.w = (comp.bounds.w as i32 - delta.x).max(8) as u32;
                        }
                        ResizeHandle::Right => {
                            comp.bounds.w = (comp.bounds.w as i32 + delta.x).max(8) as u32;
                        }
                    }
                    return true;
                } else if self.dragging {
                    comp.bounds.x += delta.x;
                    comp.bounds.y += delta.y;
                    return true;
                }
            }
        }
        false
    }

    pub fn pen_up(&mut self) {
        self.dragging = false;
        self.resize_handle = None;
    }

    pub fn delete_selected(&mut self, form: &mut Form) -> bool {
        if let Some(id) = &self.selected_id {
            if let Some(pos) = form.components.iter().position(|c| &c.id == id) {
                form.components.remove(pos);
                self.selected_id = None;
                return true;
            }
        }
        false
    }

    fn hit_test_handles(&self, rect: Rectangle, p: Point) -> Option<ResizeHandle> {
        let half = HANDLE_SIZE as i32 / 2;
        let check = |center: Point| {
            p.x >= center.x - half
                && p.x <= center.x + half
                && p.y >= center.y - half
                && p.y <= center.y + half
        };

        if check(rect.top_left) {
            return Some(ResizeHandle::TopLeft);
        }
        if check(rect.top_left + Point::new(rect.size.width as i32, 0)) {
            return Some(ResizeHandle::TopRight);
        }
        if check(rect.top_left + Point::new(0, rect.size.height as i32)) {
            return Some(ResizeHandle::BottomLeft);
        }
        if check(rect.top_left + Point::new(rect.size.width as i32, rect.size.height as i32)) {
            return Some(ResizeHandle::BottomRight);
        }

        if check(rect.top_left + Point::new(rect.size.width as i32 / 2, 0)) {
            return Some(ResizeHandle::Top);
        }
        if check(rect.top_left + Point::new(rect.size.width as i32 / 2, rect.size.height as i32)) {
            return Some(ResizeHandle::Bottom);
        }
        if check(rect.top_left + Point::new(0, rect.size.height as i32 / 2)) {
            return Some(ResizeHandle::Left);
        }
        if check(rect.top_left + Point::new(rect.size.width as i32, rect.size.height as i32 / 2)) {
            return Some(ResizeHandle::Right);
        }

        None
    }
}

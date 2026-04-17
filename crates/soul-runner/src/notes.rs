//! Notes — a stylus + keyboard text editor built on [`TextArea`]
//! and [`Keyboard`].
//!
//! This app is deliberately thin: it owns a [`Database`], a
//! [`TextArea`] (the editing surface), and a [`Keyboard`] (the soft
//! keyboard). Pointer events are routed to whichever widget the
//! press started on, and text changes are committed back to the
//! database.

use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::Gray8, prelude::*, primitives::Rectangle,
};
use soul_core::{App, Ctx, Event, KeyCode, APP_HEIGHT, SCREEN_WIDTH};
use soul_db::Database;
use soul_ui::{
    title_bar, Keyboard, TextArea, TextAreaOutput, TypedKey, KEYBOARD_HEIGHT, TITLE_BAR_H,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PressOwner {
    None,
    Text,
    Keyboard,
}

pub struct Notes {
    db: Database,
    active: u32,
    text_area: TextArea,
    keyboard: Keyboard,
    press_owner: PressOwner,
}

impl Notes {
    pub fn new() -> Self {
        let mut db = Database::new("notes");
        let id = db.insert(
            0,
            b"welcome to soulos. tap the text to place the cursor, drag to select, long-press to select a word."
                .to_vec(),
        );
        let buffer = String::from_utf8_lossy(&db.get(id).unwrap().data).into_owned();
        Self {
            db,
            active: id,
            text_area: TextArea::with_text(Self::text_rect(), buffer),
            keyboard: Keyboard::new(APP_HEIGHT as i32 - KEYBOARD_HEIGHT as i32),
            press_owner: PressOwner::None,
        }
    }

    fn text_rect() -> Rectangle {
        let top = TITLE_BAR_H as i32;
        let bottom = Self::keyboard_top();
        Rectangle::new(
            Point::new(0, top),
            Size::new(SCREEN_WIDTH as u32, (bottom - top) as u32),
        )
    }

    fn keyboard_top() -> i32 {
        APP_HEIGHT as i32 - KEYBOARD_HEIGHT as i32
    }

    fn commit(&mut self) {
        self.db
            .update(self.active, self.text_area.text().as_bytes().to_vec());
    }

    fn apply_output(&mut self, out: TextAreaOutput, ctx: &mut Ctx<'_>) {
        if let Some(r) = out.dirty {
            ctx.invalidate(r);
        }
        if out.text_changed {
            self.commit();
        }
    }

    fn apply_typed(&mut self, typed: TypedKey, ctx: &mut Ctx<'_>) {
        let out = match typed {
            TypedKey::Char(c) => self.text_area.insert_char(c),
            TypedKey::Backspace => self.text_area.backspace(),
            TypedKey::Enter => self.text_area.enter(),
        };
        self.apply_output(out, ctx);
    }
}

impl App for Notes {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::PenDown { x, y } => {
                self.press_owner = if (y as i32) >= Self::keyboard_top() {
                    PressOwner::Keyboard
                } else {
                    PressOwner::Text
                };
                match self.press_owner {
                    PressOwner::Keyboard => {
                        if let Some(r) = self.keyboard.pen_moved(x, y) {
                            ctx.invalidate(r);
                        }
                    }
                    PressOwner::Text => {
                        if let Some(r) = self.text_area.pen_down(x, y, ctx.now_ms) {
                            ctx.invalidate(r);
                        }
                    }
                    PressOwner::None => {}
                }
            }
            Event::PenMove { x, y } => match self.press_owner {
                PressOwner::Keyboard => {
                    if let Some(r) = self.keyboard.pen_moved(x, y) {
                        ctx.invalidate(r);
                    }
                }
                PressOwner::Text => {
                    if let Some(r) = self.text_area.pen_moved(x, y) {
                        ctx.invalidate(r);
                    }
                }
                PressOwner::None => {}
            },
            Event::PenUp { x, y } => {
                match self.press_owner {
                    PressOwner::Keyboard => {
                        let out = self.keyboard.pen_released(x, y);
                        if let Some(r) = out.dirty {
                            ctx.invalidate(r);
                        }
                        if let Some(typed) = out.typed {
                            self.apply_typed(typed, ctx);
                        }
                    }
                    PressOwner::Text => self.text_area.pen_released(x, y),
                    PressOwner::None => {}
                }
                self.press_owner = PressOwner::None;
            }
            Event::Tick(now) => {
                if let Some(r) = self.text_area.tick(now) {
                    ctx.invalidate(r);
                }
            }
            Event::Key(KeyCode::Char(c)) => {
                let out = self.text_area.insert_char(c);
                self.apply_output(out, ctx);
            }
            Event::Key(KeyCode::Backspace) => {
                let out = self.text_area.backspace();
                self.apply_output(out, ctx);
            }
            Event::Key(KeyCode::Enter) => {
                let out = self.text_area.enter();
                self.apply_output(out, ctx);
            }
            Event::Key(KeyCode::ArrowLeft) => {
                if let Some(r) = self.text_area.cursor_left() {
                    ctx.invalidate(r);
                }
            }
            Event::Key(KeyCode::ArrowRight) => {
                if let Some(r) = self.text_area.cursor_right() {
                    ctx.invalidate(r);
                }
            }
            Event::Key(KeyCode::ArrowUp) => {
                if let Some(r) = self.text_area.cursor_up() {
                    ctx.invalidate(r);
                }
            }
            Event::Key(KeyCode::ArrowDown) => {
                if let Some(r) = self.text_area.cursor_down() {
                    ctx.invalidate(r);
                }
            }
            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "Notes");
        let _ = self.text_area.draw(canvas);
        let _ = self.keyboard.draw(canvas);
    }
}

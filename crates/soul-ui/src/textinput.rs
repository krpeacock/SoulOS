//! Single-line text input widget.
//!
//! [`TextInput`] is the sibling of [`crate::TextArea`]: single-line,
//! no wrapping, no drag-to-select. Designed for filter boxes
//! (emoji picker, contact search), form fields, and anything else
//! that should fit on one row.
//!
//! Key semantics:
//! - **Enter** submits (signalled via [`TextInputOutput::submitted`])
//!   instead of inserting a newline.
//! - Overflow past the right edge scrolls the text horizontally so
//!   the caret stays visible.
//! - Empty buffer shows a gray placeholder string.
//!
//! # Example
//!
//! ```ignore
//! use embedded_graphics::{prelude::*, primitives::Rectangle};
//! use soul_ui::TextInput;
//!
//! let r = Rectangle::new(Point::new(10, 30), Size::new(200, 18));
//! let mut filter = TextInput::with_placeholder(r, "search");
//!
//! // In App::handle:
//! // Event::Key(KeyCode::Char(c)) => {
//! //     let out = filter.insert_char(c);
//! //     if let Some(r) = out.dirty { ctx.invalidate(r); }
//! //     if out.text_changed { refilter(filter.text()); }
//! // }
//! // Event::Key(KeyCode::Enter) => {
//! //     let out = filter.enter();
//! //     if out.submitted { pick_focused(); }
//! // }
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{Line as EgLine, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};

use crate::palette::{BLACK, GRAY, WHITE};
use soul_core::a11y::{Accessible, AccessibleNode};

const CHAR_W: i32 = 6;
const FONT_H: i32 = 10;
const INPUT_PAD: i32 = 3;

/// The result of a user action against a [`TextInput`].
#[derive(Default, Debug, Clone, Copy)]
#[must_use = "dirty regions must be passed to Ctx::invalidate"]
pub struct TextInputOutput {
    /// Bounding rectangle of pixels that need repainting, if any.
    pub dirty: Option<Rectangle>,
    /// `true` if the buffer contents changed. Cursor moves leave
    /// this `false`.
    pub text_changed: bool,
    /// `true` when the user pressed Enter. Submission does not
    /// clear the buffer automatically — call [`TextInput::clear`]
    /// from the handler if that's what you want.
    pub submitted: bool,
}

/// A single-line text input with an optional placeholder.
pub struct TextInput {
    area: Rectangle,
    buffer: String,
    cursor: usize,
    placeholder: &'static str,
}

impl TextInput {
    /// Create an empty input widget inside `area`.
    pub fn new(area: Rectangle) -> Self {
        Self {
            area,
            buffer: String::new(),
            cursor: 0,
            placeholder: "",
        }
    }

    /// Create an input with the given placeholder, shown in [`GRAY`]
    /// when the buffer is empty.
    pub fn with_placeholder(area: Rectangle, placeholder: &'static str) -> Self {
        Self {
            area,
            buffer: String::new(),
            cursor: 0,
            placeholder,
        }
    }

    /// Borrow the current buffer.
    pub fn text(&self) -> &str {
        &self.buffer
    }

    /// Replace the buffer contents, placing the cursor at the end.
    pub fn set_text(&mut self, text: String) -> Option<Rectangle> {
        self.buffer = text;
        self.cursor = self.buffer.len();
        Some(self.area)
    }

    /// Clear the buffer and reset the cursor. Returns a dirty
    /// rectangle only if the widget had content.
    pub fn clear(&mut self) -> Option<Rectangle> {
        if self.buffer.is_empty() {
            None
        } else {
            self.buffer.clear();
            self.cursor = 0;
            Some(self.area)
        }
    }

    /// The widget's bounding rectangle (including border).
    pub fn area(&self) -> Rectangle {
        self.area
    }

    /// Return `true` if `(x, y)` falls inside the widget.
    pub fn contains(&self, x: i16, y: i16) -> bool {
        let x = x as i32;
        let y = y as i32;
        x >= self.area.top_left.x
            && x < self.area.top_left.x + self.area.size.width as i32
            && y >= self.area.top_left.y
            && y < self.area.top_left.y + self.area.size.height as i32
    }

    /// Place the cursor at the tap point. Apps can call this on
    /// pen-up (tap to focus) or pen-down (immediate feedback).
    pub fn pen_released(&mut self, x: i16, _y: i16) -> Option<Rectangle> {
        let inner_x = (x as i32 - self.area.top_left.x - INPUT_PAD).max(0);
        let char_idx = (inner_x / CHAR_W) as usize;
        let char_count = self.buffer.chars().count();
        let clamped = char_idx.min(char_count);
        let byte = self
            .buffer
            .char_indices()
            .nth(clamped)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len());
        if byte != self.cursor {
            self.cursor = byte;
            Some(self.area)
        } else {
            None
        }
    }

    /// Insert a character at the cursor.
    pub fn insert_char(&mut self, c: char) -> TextInputOutput {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        let pos = self.cursor;
        self.buffer.insert_str(pos, s);
        self.cursor = pos + s.len();
        TextInputOutput {
            dirty: Some(self.area),
            text_changed: true,
            submitted: false,
        }
    }

    /// Delete the character left of the cursor.
    pub fn backspace(&mut self) -> TextInputOutput {
        if self.cursor > 0 {
            let prev = self.buffer[..self.cursor]
                .char_indices()
                .next_back()
                .unwrap()
                .0;
            self.buffer.replace_range(prev..self.cursor, "");
            self.cursor = prev;
            return TextInputOutput {
                dirty: Some(self.area),
                text_changed: true,
                submitted: false,
            };
        }
        TextInputOutput::default()
    }

    /// Signal submission. Enter does *not* insert a character; the
    /// app typically consumes the submission (e.g., "pick focused
    /// result") and decides whether to clear the buffer.
    pub fn enter(&mut self) -> TextInputOutput {
        TextInputOutput {
            dirty: None,
            text_changed: false,
            submitted: true,
        }
    }

    /// Move cursor one character left.
    pub fn cursor_left(&mut self) -> Option<Rectangle> {
        if self.cursor > 0 {
            let prev = self.buffer[..self.cursor]
                .char_indices()
                .next_back()
                .unwrap()
                .0;
            self.cursor = prev;
            return Some(self.area);
        }
        None
    }

    /// Move cursor one character right.
    pub fn cursor_right(&mut self) -> Option<Rectangle> {
        if self.cursor < self.buffer.len() {
            let c = self.buffer[self.cursor..].chars().next().unwrap();
            self.cursor += c.len_utf8();
            return Some(self.area);
        }
        None
    }

    /// Render the input box, placeholder or text, and the caret.
    pub fn draw<D>(&self, canvas: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        // Frame: white fill, black 1-px stroke.
        let frame = PrimitiveStyleBuilder::new()
            .fill_color(WHITE)
            .stroke_color(BLACK)
            .stroke_width(1)
            .build();
        self.area.into_styled(frame).draw(canvas)?;

        let inner_x = self.area.top_left.x + INPUT_PAD;
        let inner_y = self.area.top_left.y + (self.area.size.height as i32 - FONT_H) / 2;
        let max_chars = ((self.area.size.width as i32 - INPUT_PAD * 2) / CHAR_W).max(1) as usize;

        if self.buffer.is_empty() && !self.placeholder.is_empty() {
            let style = MonoTextStyle::new(&FONT_6X10, GRAY);
            let visible: String = self.placeholder.chars().take(max_chars).collect();
            Text::with_baseline(&visible, Point::new(inner_x, inner_y), style, Baseline::Top)
                .draw(canvas)?;
        } else {
            // Horizontal scroll so the caret is visible.
            let cursor_char = self.buffer[..self.cursor].chars().count();
            let total_chars = self.buffer.chars().count();
            let scroll = cursor_char.saturating_sub(max_chars - 1);
            let end = (scroll + max_chars).min(total_chars);
            let start_byte = self
                .buffer
                .char_indices()
                .nth(scroll)
                .map(|(i, _)| i)
                .unwrap_or(self.buffer.len());
            let end_byte = self
                .buffer
                .char_indices()
                .nth(end)
                .map(|(i, _)| i)
                .unwrap_or(self.buffer.len());
            let visible = &self.buffer[start_byte..end_byte];
            let style = MonoTextStyle::new(&FONT_6X10, BLACK);
            Text::with_baseline(visible, Point::new(inner_x, inner_y), style, Baseline::Top)
                .draw(canvas)?;
            // Caret.
            let caret_col = (cursor_char - scroll) as i32;
            let caret_x = inner_x + caret_col * CHAR_W;
            EgLine::new(
                Point::new(caret_x, inner_y),
                Point::new(caret_x, inner_y + FONT_H),
            )
            .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
            .draw(canvas)?;
        }
        Ok(())
    }
}

impl Accessible for TextInput {
    fn a11y_nodes(&self, nodes: &mut Vec<AccessibleNode>) {
        let text = if self.buffer.is_empty() {
            self.placeholder.into()
        } else {
            self.buffer.clone()
        };
        nodes.push(AccessibleNode { text });
    }
}

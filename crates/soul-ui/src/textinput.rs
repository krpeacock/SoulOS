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

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{Line as EgLine, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
};

use crate::emoji;
use crate::palette::{BLACK, GRAY, WHITE};
use soul_core::a11y::{A11yNode, A11yRole};

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
#[derive(Clone)]
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
        let target_cell = inner_x / CHAR_W;
        let byte = byte_at_cell(&self.buffer, target_cell);
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
        let max_cells = ((self.area.size.width as i32 - INPUT_PAD * 2) / CHAR_W).max(1);

        if self.buffer.is_empty() && !self.placeholder.is_empty() {
            let style = MonoTextStyle::new(&FONT_6X10, GRAY);
            let mut visible = String::new();
            let mut cells = 0;
            for c in self.placeholder.chars() {
                let w = emoji::cell_width(c);
                if cells + w > max_cells {
                    break;
                }
                visible.push(c);
                cells += w;
            }
            emoji::draw_text(canvas, &visible, Point::new(inner_x, inner_y), style)?;
        } else {
            // Horizontal scroll so the caret is visible. Track in cells.
            let cursor_cell = emoji::cells_in(&self.buffer[..self.cursor]);
            let scroll_cells = (cursor_cell - (max_cells - 1)).max(0);
            let start_byte = byte_at_cell(&self.buffer, scroll_cells);
            // Take chars from start_byte until we run out of cell budget.
            let mut visible_end = start_byte;
            let mut cells = 0;
            for (b, c) in self.buffer[start_byte..].char_indices() {
                let w = emoji::cell_width(c);
                if cells + w > max_cells {
                    break;
                }
                visible_end = start_byte + b + c.len_utf8();
                cells += w;
            }
            let visible = &self.buffer[start_byte..visible_end];
            let style = MonoTextStyle::new(&FONT_6X10, BLACK);
            emoji::draw_text(canvas, visible, Point::new(inner_x, inner_y), style)?;
            // Caret in cells from the visible region's left edge.
            let caret_col = cursor_cell - scroll_cells;
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

/// Walk `s` accumulating cell widths and return the byte offset of
/// the first char whose start lands at or past `target_cells`.
fn byte_at_cell(s: &str, target_cells: i32) -> usize {
    let mut cells = 0;
    for (b, c) in s.char_indices() {
        if cells >= target_cells {
            return b;
        }
        cells += emoji::cell_width(c);
    }
    s.len()
}

impl TextInput {
    /// Build the accessibility node for this input.
    ///
    /// The host supplies `label` because the widget's `placeholder` is
    /// hint text, not the field's name. The `value` carries the live
    /// buffer when non-empty so a screen reader announces what the user
    /// has typed (or the placeholder when the buffer is empty).
    pub fn a11y_node(&self, label: &str) -> A11yNode {
        let mut node = A11yNode::new(self.area, label, A11yRole::TextField);
        if self.buffer.is_empty() {
            if !self.placeholder.is_empty() {
                node = node.with_value(self.placeholder);
            }
        } else {
            node = node.with_value(self.buffer.clone());
        }
        node
    }
}

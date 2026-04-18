//! Multi-line text editor widget.
//!
//! [`TextArea`] is a stateful widget that owns a mutable text
//! buffer, a cursor, an optional selection, and the word-wrapped
//! visual layout. Apps forward pointer and keyboard events to it
//! and read the current buffer with [`TextArea::text`].
//!
//! # Features
//!
//! - Tap to position the caret; drag to select a range.
//! - Long-press (~500 ms without movement) to select the word under
//!   the pointer.
//! - Word-boundary line wrapping with mid-word fallback when a word
//!   is wider than the widget area.
//! - Arrow-key cursor navigation (horizontal by character, vertical
//!   by visual line).
//! - Selection-aware editing: typing while a selection is active
//!   replaces it.
//!
//! # Ownership model
//!
//! The widget owns the [`String`] buffer. Apps read the current text
//! with [`TextArea::text`] and should persist it whenever a call
//! returns [`TextAreaOutput::text_changed`] `= true`. Pure cursor
//! moves never set that flag.
//!
//! # Example
//!
//! ```ignore
//! use embedded_graphics::{prelude::*, primitives::Rectangle};
//! use soul_ui::{TextArea, TextAreaOutput};
//!
//! let area = Rectangle::new(Point::new(0, 15), Size::new(240, 200));
//! let mut editor = TextArea::with_text(area, "hello world".into());
//!
//! // In your App::handle:
//! // Event::PenDown { x, y } =>
//! //     if let Some(r) = editor.pen_down(x, y, ctx.now_ms) {
//! //         ctx.invalidate(r);
//! //     }
//! //
//! // Event::Key(KeyCode::Char(c)) => {
//! //     let out = editor.insert_char(c);
//! //     if let Some(r) = out.dirty { ctx.invalidate(r); }
//! //     if out.text_changed { persist(editor.text()); }
//! // }
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{Line as EgLine, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};

use crate::palette::{BLACK, WHITE};

const CHAR_W: i32 = 6;
const LINE_H: i32 = 12;
const TEXT_PAD: i32 = 4;

/// Time (ms) a motionless press must persist before it becomes a
/// long-press and selects the word under the pointer.
pub const LONG_PRESS_MS: u64 = 500;

/// Maximum Manhattan distance (px) a press may travel before it's
/// considered a drag rather than a tap.
pub const DRAG_THRESHOLD: i32 = 3;

/// One visual (wrapped) line: `[start, end)` byte indices into the
/// widget's buffer. Newline characters are *not* included in the
/// range — they're implied between adjacent lines.
#[derive(Clone, Copy, Debug)]
struct VisualLine {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug)]
struct Press {
    start_ms: u64,
    start_x: i16,
    start_y: i16,
    start_cursor: usize,
    moved: bool,
    long_press_fired: bool,
}

/// The result of a user action against a [`TextArea`].
///
/// Callers feed `dirty` to `Ctx::invalidate` so the runtime repaints
/// the changed region, and persist the backing buffer whenever
/// `text_changed` is `true`.
#[derive(Default, Debug, Clone, Copy)]
#[must_use = "dirty regions must be passed to Ctx::invalidate; text_changed should trigger a save"]
pub struct TextAreaOutput {
    /// Bounding rectangle of pixels that need repainting, if any.
    pub dirty: Option<Rectangle>,
    /// `true` if the buffer contents changed (insert, delete,
    /// replace). Pure cursor or selection moves leave this `false`.
    pub text_changed: bool,
}

/// A multi-line, word-wrapping text editor widget.
///
/// See the [module docs](self) for lifecycle and typical wiring.
#[derive(Clone)]
pub struct TextArea {
    area: Rectangle,
    buffer: String,
    cursor: usize,
    anchor: Option<usize>,
    layout: Vec<VisualLine>,
    press: Option<Press>,
}

impl TextArea {
    /// Create an empty text area bound to `area` in virtual-screen
    /// coordinates.
    pub fn new(area: Rectangle) -> Self {
        let mut this = Self {
            area,
            buffer: String::new(),
            cursor: 0,
            anchor: None,
            layout: Vec::new(),
            press: None,
        };
        this.recompute_layout();
        this
    }

    /// Create a text area preloaded with `text`. The cursor is placed
    /// at the end of the buffer.
    pub fn with_text(area: Rectangle, text: String) -> Self {
        let mut this = Self::new(area);
        this.cursor = text.len();
        this.buffer = text;
        this.recompute_layout();
        this
    }

    /// Borrow the current buffer.
    pub fn text(&self) -> &str {
        &self.buffer
    }

    /// Replace the buffer contents. Clears any selection and clamps
    /// the cursor. Returns the full widget area as the dirty rect.
    pub fn set_text(&mut self, text: String) -> Option<Rectangle> {
        self.buffer = text;
        if self.cursor > self.buffer.len() {
            self.cursor = self.buffer.len();
        }
        self.anchor = None;
        self.recompute_layout();
        Some(self.area)
    }

    /// The widget's bounding rectangle.
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

    // --- pointer events ------------------------------------------------

    /// Begin a press at `(x, y)`. Moves the cursor to the hit
    /// position and clears any selection.
    pub fn pen_down(&mut self, x: i16, y: i16, now_ms: u64) -> Option<Rectangle> {
        let cursor = self.char_at_point(x, y);
        self.cursor = cursor;
        self.anchor = None;
        self.press = Some(Press {
            start_ms: now_ms,
            start_x: x,
            start_y: y,
            start_cursor: cursor,
            moved: false,
            long_press_fired: false,
        });
        Some(self.area)
    }

    /// Extend the press to `(x, y)`. If the pointer has travelled
    /// more than [`DRAG_THRESHOLD`], the press becomes a drag and
    /// the selection is extended from the original down-point to
    /// the new cursor.
    pub fn pen_moved(&mut self, x: i16, y: i16) -> Option<Rectangle> {
        let drag = if let Some(press) = self.press.as_mut() {
            let dx = (x - press.start_x).abs() as i32;
            let dy = (y - press.start_y).abs() as i32;
            if !press.moved && (dx + dy) > DRAG_THRESHOLD {
                press.moved = true;
            }
            if press.moved {
                Some(press.start_cursor)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(start_cursor) = drag {
            let new_cursor = self.char_at_point(x, y);
            if self.anchor.is_none() {
                self.anchor = Some(start_cursor);
            }
            self.cursor = new_cursor;
            Some(self.area)
        } else {
            None
        }
    }

    /// End the current press. Any selection remains; if this was
    /// neither a drag nor a long-press, the cursor simply rests at
    /// the tap point from [`TextArea::pen_down`].
    pub fn pen_released(&mut self, _x: i16, _y: i16) {
        self.press = None;
    }

    /// Abandon an in-flight press without committing it.
    pub fn pen_cancelled(&mut self) {
        self.press = None;
    }

    /// Check whether a motionless press has aged into a long-press;
    /// if so, expand the selection to the word under the pointer.
    /// Call this on every [`Tick`] event so long-press fires even
    /// without further input.
    ///
    /// [`Tick`]: soul-core's `Event::Tick` in the app's main loop.
    pub fn tick(&mut self, now_ms: u64) -> Option<Rectangle> {
        if let Some(press) = self.press.as_mut() {
            if !press.moved
                && !press.long_press_fired
                && now_ms.saturating_sub(press.start_ms) >= LONG_PRESS_MS
            {
                press.long_press_fired = true;
                let (s, e) = word_range(&self.buffer, press.start_cursor);
                if s != e {
                    self.anchor = Some(s);
                    self.cursor = e;
                    return Some(self.area);
                }
            }
        }
        None
    }

    // --- editing -------------------------------------------------------

    /// Insert a single character at the cursor. Replaces any active
    /// selection.
    pub fn insert_char(&mut self, c: char) -> TextAreaOutput {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.insert_str(s)
    }

    /// Insert a string at the cursor. Replaces any active selection.
    pub fn insert_str(&mut self, s: &str) -> TextAreaOutput {
        self.delete_selection();
        let pos = self.cursor;
        self.buffer.insert_str(pos, s);
        self.cursor = pos + s.len();
        self.recompute_layout();
        TextAreaOutput {
            dirty: Some(self.area),
            text_changed: true,
        }
    }

    /// Delete the selection if any, otherwise the character left of
    /// the cursor.
    pub fn backspace(&mut self) -> TextAreaOutput {
        if self.delete_selection() {
            self.recompute_layout();
            return TextAreaOutput {
                dirty: Some(self.area),
                text_changed: true,
            };
        }
        if self.cursor > 0 {
            let prev = self.buffer[..self.cursor]
                .char_indices()
                .next_back()
                .unwrap()
                .0;
            self.buffer.replace_range(prev..self.cursor, "");
            self.cursor = prev;
            self.recompute_layout();
            return TextAreaOutput {
                dirty: Some(self.area),
                text_changed: true,
            };
        }
        TextAreaOutput::default()
    }

    /// Insert a newline at the cursor.
    pub fn enter(&mut self) -> TextAreaOutput {
        self.insert_str("\n")
    }

    // --- cursor navigation --------------------------------------------

    /// Move the cursor one character to the left. Collapses any
    /// selection.
    pub fn cursor_left(&mut self) -> Option<Rectangle> {
        self.anchor = None;
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

    /// Move the cursor one character to the right. Collapses any
    /// selection.
    pub fn cursor_right(&mut self) -> Option<Rectangle> {
        self.anchor = None;
        if self.cursor < self.buffer.len() {
            let c = self.buffer[self.cursor..].chars().next().unwrap();
            self.cursor += c.len_utf8();
            return Some(self.area);
        }
        None
    }

    /// Move the cursor to the same column on the previous visual
    /// line. Collapses any selection.
    pub fn cursor_up(&mut self) -> Option<Rectangle> {
        self.anchor = None;
        if let Some(p) = self.caret_position(self.cursor) {
            if p.y > self.area.top_left.y + TEXT_PAD {
                let new_y = p.y - LINE_H;
                self.cursor = self.char_at_point(p.x as i16, new_y as i16);
                return Some(self.area);
            }
        }
        None
    }

    /// Move the cursor to the same column on the next visual line.
    /// Collapses any selection.
    pub fn cursor_down(&mut self) -> Option<Rectangle> {
        self.anchor = None;
        if let Some(p) = self.caret_position(self.cursor) {
            let new_y = p.y + LINE_H;
            self.cursor = self.char_at_point(p.x as i16, new_y as i16);
            return Some(self.area);
        }
        None
    }

    // --- render --------------------------------------------------------

    /// Render the current text, caret, and selection into `canvas`.
    pub fn draw<D>(&self, canvas: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let black_style = MonoTextStyle::new(&FONT_6X10, BLACK);
        let white_style = MonoTextStyle::new(&FONT_6X10, WHITE);
        let max_y = self.area.top_left.y + self.area.size.height as i32;

        // Pass 1: paint all text in black.
        for (idx, line) in self.layout.iter().enumerate() {
            let y = self.area.top_left.y + TEXT_PAD + idx as i32 * LINE_H;
            if y + LINE_H > max_y {
                break;
            }
            let text = &self.buffer[line.start..line.end];
            Text::with_baseline(
                text,
                Point::new(self.area.top_left.x + TEXT_PAD, y),
                black_style,
                Baseline::Top,
            )
            .draw(canvas)?;
        }

        // Pass 2: invert selected spans.
        let selection = self.anchor.and_then(|a| {
            let (lo, hi) = (a.min(self.cursor), a.max(self.cursor));
            if lo == hi {
                None
            } else {
                Some((lo, hi))
            }
        });

        if let Some((sel_start, sel_end)) = selection {
            for (idx, line) in self.layout.iter().enumerate() {
                let ls = sel_start.max(line.start);
                let le = sel_end.min(line.end);
                if ls >= le {
                    continue;
                }
                let y = self.area.top_left.y + TEXT_PAD + idx as i32 * LINE_H;
                if y + LINE_H > max_y {
                    break;
                }
                let s_chars = self.buffer[line.start..ls].chars().count() as i32;
                let e_chars = self.buffer[line.start..le].chars().count() as i32;
                let rect = Rectangle::new(
                    Point::new(self.area.top_left.x + TEXT_PAD + s_chars * CHAR_W, y),
                    Size::new(((e_chars - s_chars) * CHAR_W) as u32, LINE_H as u32),
                );
                rect.into_styled(PrimitiveStyle::with_fill(BLACK))
                    .draw(canvas)?;
                let selected_text = &self.buffer[ls..le];
                Text::with_baseline(
                    selected_text,
                    Point::new(self.area.top_left.x + TEXT_PAD + s_chars * CHAR_W, y),
                    white_style,
                    Baseline::Top,
                )
                .draw(canvas)?;
            }
        } else if let Some(p) = self.caret_position(self.cursor) {
            if p.y + LINE_H <= max_y {
                EgLine::new(Point::new(p.x, p.y), Point::new(p.x, p.y + LINE_H))
                    .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
                    .draw(canvas)?;
            }
        }
        Ok(())
    }

    // --- internals -----------------------------------------------------

    fn chars_per_line(&self) -> usize {
        ((self.area.size.width as i32 - TEXT_PAD * 2) / CHAR_W).max(1) as usize
    }

    fn recompute_layout(&mut self) {
        self.layout = compute_layout(&self.buffer, self.chars_per_line());
    }

    fn delete_selection(&mut self) -> bool {
        if let Some(anchor) = self.anchor.take() {
            let (lo, hi) = (anchor.min(self.cursor), anchor.max(self.cursor));
            if lo != hi {
                self.buffer.replace_range(lo..hi, "");
                self.cursor = lo;
                return true;
            }
        }
        false
    }

    fn char_at_point(&self, x: i16, y: i16) -> usize {
        if self.layout.is_empty() {
            return 0;
        }
        let y_rel = (y as i32 - self.area.top_left.y - TEXT_PAD).max(0);
        let line_idx = ((y_rel / LINE_H) as usize).min(self.layout.len() - 1);
        let line = self.layout[line_idx];
        let x_rel = (x as i32 - self.area.top_left.x - TEXT_PAD).max(0);
        let char_in_line = (x_rel / CHAR_W) as usize;
        let line_text = &self.buffer[line.start..line.end];
        let line_char_count = line_text.chars().count();
        let char_in_line = char_in_line.min(line_char_count);
        let byte_offset = line_text
            .char_indices()
            .nth(char_in_line)
            .map(|(i, _)| i)
            .unwrap_or(line_text.len());
        line.start + byte_offset
    }

    fn caret_position(&self, cursor: usize) -> Option<Point> {
        for (idx, line) in self.layout.iter().enumerate() {
            if cursor >= line.start && cursor <= line.end {
                let char_offset = self.buffer[line.start..cursor].chars().count() as i32;
                let x = self.area.top_left.x + TEXT_PAD + char_offset * CHAR_W;
                let y = self.area.top_left.y + TEXT_PAD + idx as i32 * LINE_H;
                return Some(Point::new(x, y));
            }
        }
        None
    }
}

/// Word-wrap `buffer` into visual lines of at most `chars_per_line`
/// characters each. Break at spaces where possible; break mid-word
/// only when a single word is wider than one line.
fn compute_layout(buffer: &str, chars_per_line: usize) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut cur_count = 0usize;
    let mut last_break: Option<usize> = None;

    let mut i = 0;
    while i < buffer.len() {
        let c = buffer[i..].chars().next().unwrap();
        let c_len = c.len_utf8();

        if c == '\n' {
            lines.push(VisualLine {
                start: line_start,
                end: i,
            });
            i += c_len;
            line_start = i;
            cur_count = 0;
            last_break = None;
            continue;
        }

        if cur_count >= chars_per_line {
            let break_at = last_break.unwrap_or(i);
            lines.push(VisualLine {
                start: line_start,
                end: break_at,
            });
            line_start = break_at;
            cur_count = buffer[line_start..i].chars().count();
            last_break = None;
            continue;
        }

        cur_count += 1;
        if c == ' ' {
            last_break = Some(i + c_len);
        }
        i += c_len;
    }
    lines.push(VisualLine {
        start: line_start,
        end: buffer.len(),
    });
    lines
}

/// Expand `pos` to the surrounding word's `[start, end)` byte range.
/// A word is a run of alphanumerics or underscores. Returns
/// `(pos, pos)` when `pos` is on a non-word character.
fn word_range(buffer: &str, pos: usize) -> (usize, usize) {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let pos = pos.min(buffer.len());
    let mut start = pos;
    let mut end = pos;
    for (i, c) in buffer[..pos].char_indices().rev() {
        if is_word(c) {
            start = i;
        } else {
            break;
        }
    }
    for (i, c) in buffer[pos..].char_indices() {
        if is_word(c) {
            end = pos + i + c.len_utf8();
        } else {
            break;
        }
    }
    (start, end)
}

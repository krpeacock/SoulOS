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
//! - Font-face switching: call [`TextArea::set_face`] to change
//!   the rendered typeface without touching the text buffer.
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
    pixelcolor::Gray8,
    prelude::*,
    primitives::{Line as EgLine, PrimitiveStyle, Rectangle},
};

use crate::emoji;
use crate::font_aa::{self, FontFace};
use crate::palette::{BLACK, WHITE};

const TEXT_PAD: i32 = 4;
/// Body text size used in the notes textarea (logical pixels).
pub const BODY_FONT_SIZE: f32 = 11.0;

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
    /// Index of the first visual line shown at the top of the widget.
    scroll_line: usize,
    face: FontFace,
    font_size: f32,
}

impl TextArea {
    /// Create an empty text area bound to `area` in virtual-screen coordinates.
    pub fn new(area: Rectangle) -> Self {
        let mut this = Self {
            area,
            buffer: String::new(),
            cursor: 0,
            anchor: None,
            layout: Vec::new(),
            press: None,
            scroll_line: 0,
            face: FontFace::Sans,
            font_size: BODY_FONT_SIZE,
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

    /// Replace the buffer contents. Clears any selection, resets
    /// scroll to the top, and clamps the cursor.
    pub fn set_text(&mut self, text: String) -> Option<Rectangle> {
        self.buffer = text;
        if self.cursor > self.buffer.len() {
            self.cursor = self.buffer.len();
        }
        self.anchor = None;
        self.scroll_line = 0;
        self.recompute_layout();
        Some(self.area)
    }

    /// Switch the rendered typeface. Reflows the layout immediately.
    pub fn set_face(&mut self, face: FontFace) {
        self.face = face;
        self.recompute_layout();
        self.ensure_cursor_visible();
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

    /// Insert a single character at the cursor. Replaces any active selection.
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
        self.ensure_cursor_visible();
        TextAreaOutput {
            dirty: Some(self.area),
            text_changed: true,
        }
    }

    /// Delete the selection if any, otherwise the character left of the cursor.
    pub fn backspace(&mut self) -> TextAreaOutput {
        if self.delete_selection() {
            self.recompute_layout();
            self.ensure_cursor_visible();
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
            self.ensure_cursor_visible();
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

    /// Move the cursor one character to the left. Collapses any selection.
    pub fn cursor_left(&mut self) -> Option<Rectangle> {
        self.anchor = None;
        if self.cursor > 0 {
            let prev = self.buffer[..self.cursor]
                .char_indices()
                .next_back()
                .unwrap()
                .0;
            self.cursor = prev;
            self.ensure_cursor_visible();
            return Some(self.area);
        }
        None
    }

    /// Move the cursor one character to the right. Collapses any selection.
    pub fn cursor_right(&mut self) -> Option<Rectangle> {
        self.anchor = None;
        if self.cursor < self.buffer.len() {
            let c = self.buffer[self.cursor..].chars().next().unwrap();
            self.cursor += c.len_utf8();
            self.ensure_cursor_visible();
            return Some(self.area);
        }
        None
    }

    /// Move the cursor to the same visual column on the previous line.
    pub fn cursor_up(&mut self) -> Option<Rectangle> {
        self.anchor = None;
        let line_idx = self.cursor_line_idx();
        if line_idx > 0 {
            let col_px = self.cursor_col_px(line_idx);
            let prev = self.layout[line_idx - 1];
            let line_text = &self.buffer[prev.start..prev.end];
            self.cursor = prev.start + byte_at_px(line_text, col_px, self.face, self.font_size);
            self.ensure_cursor_visible();
            return Some(self.area);
        }
        None
    }

    /// Move the cursor to the same visual column on the next line.
    pub fn cursor_down(&mut self) -> Option<Rectangle> {
        self.anchor = None;
        let line_idx = self.cursor_line_idx();
        if line_idx + 1 < self.layout.len() {
            let col_px = self.cursor_col_px(line_idx);
            let next = self.layout[line_idx + 1];
            let line_text = &self.buffer[next.start..next.end];
            self.cursor = next.start + byte_at_px(line_text, col_px, self.face, self.font_size);
            self.ensure_cursor_visible();
            return Some(self.area);
        }
        None
    }

    /// Scroll up one page (keeping cursor in view).
    pub fn page_up(&mut self) -> Option<Rectangle> {
        let vis = self.visible_lines();
        let line_idx = self.cursor_line_idx();
        let new_line = line_idx.saturating_sub(vis);
        if new_line == line_idx {
            return None;
        }
        let col_px = self.cursor_col_px(line_idx);
        let vl = self.layout[new_line];
        self.cursor = vl.start + byte_at_px(&self.buffer[vl.start..vl.end], col_px, self.face, self.font_size);
        self.ensure_cursor_visible();
        Some(self.area)
    }

    /// Scroll down one page (keeping cursor in view).
    pub fn page_down(&mut self) -> Option<Rectangle> {
        let vis = self.visible_lines();
        let line_idx = self.cursor_line_idx();
        let new_line = (line_idx + vis).min(self.layout.len().saturating_sub(1));
        if new_line == line_idx {
            return None;
        }
        let col_px = self.cursor_col_px(line_idx);
        let vl = self.layout[new_line];
        self.cursor = vl.start + byte_at_px(&self.buffer[vl.start..vl.end], col_px, self.face, self.font_size);
        self.ensure_cursor_visible();
        Some(self.area)
    }

    // --- render --------------------------------------------------------

    /// Render the current text, caret, and selection into `canvas`.
    pub fn draw<D>(&self, canvas: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let line_h = self.line_height();
        let max_y = self.area.top_left.y + self.area.size.height as i32;

        // Pass 1: paint visible lines.
        for (idx, line) in self.layout.iter().enumerate() {
            if idx < self.scroll_line {
                continue;
            }
            let row = (idx - self.scroll_line) as i32;
            let y = self.area.top_left.y + TEXT_PAD + row * line_h;
            if y + line_h > max_y {
                break;
            }
            let text = &self.buffer[line.start..line.end];
            draw_line_text(canvas, text, self.area.top_left.x + TEXT_PAD, y, self.font_size, 0, self.face, line_h)?;
        }

        // Pass 2: invert selected spans.
        let selection = self.anchor.and_then(|a| {
            let (lo, hi) = (a.min(self.cursor), a.max(self.cursor));
            if lo == hi { None } else { Some((lo, hi)) }
        });

        if let Some((sel_start, sel_end)) = selection {
            for (idx, line) in self.layout.iter().enumerate() {
                if idx < self.scroll_line {
                    continue;
                }
                let ls = sel_start.max(line.start);
                let le = sel_end.min(line.end);
                if ls >= le {
                    continue;
                }
                let row = (idx - self.scroll_line) as i32;
                let y = self.area.top_left.y + TEXT_PAD + row * line_h;
                if y + line_h > max_y {
                    break;
                }
                let s_px = text_px_width(&self.buffer[line.start..ls], self.face, self.font_size);
                let e_px = text_px_width(&self.buffer[line.start..le], self.face, self.font_size);
                let rect = Rectangle::new(
                    Point::new(self.area.top_left.x + TEXT_PAD + s_px, y),
                    Size::new((e_px - s_px).max(1) as u32, line_h as u32),
                );
                rect.into_styled(PrimitiveStyle::with_fill(BLACK)).draw(canvas)?;
                let selected_text = &self.buffer[ls..le];
                draw_line_text(canvas, selected_text, self.area.top_left.x + TEXT_PAD + s_px, y, self.font_size, 255, self.face, line_h)?;
            }
        } else if let Some(p) = self.caret_position(self.cursor) {
            if p.y + line_h <= max_y {
                EgLine::new(Point::new(p.x, p.y), Point::new(p.x, p.y + line_h))
                    .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
                    .draw(canvas)?;
            }
        }
        Ok(())
    }

    // --- internals -----------------------------------------------------

    fn line_height(&self) -> i32 {
        font_aa::cap_height_face(self.font_size, self.face) + 4
    }

    fn max_line_px(&self) -> f32 {
        (self.area.size.width as i32 - TEXT_PAD * 2) as f32
    }

    fn recompute_layout(&mut self) {
        self.layout = compute_layout(&self.buffer, self.max_line_px(), self.face, self.font_size);
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
        let line_h = self.line_height();
        let y_rel = (y as i32 - self.area.top_left.y - TEXT_PAD).max(0);
        let line_idx = (self.scroll_line + (y_rel / line_h) as usize)
            .min(self.layout.len() - 1);
        let line = self.layout[line_idx];
        let target_x = (x as i32 - self.area.top_left.x - TEXT_PAD).max(0) as f32;

        let line_text = &self.buffer[line.start..line.end];
        let mut cum_x = 0.0f32;
        for (byte_off, c) in line_text.char_indices() {
            let cw = char_px(c, self.face, self.font_size);
            if target_x < cum_x + cw * 0.5 {
                return line.start + byte_off;
            }
            cum_x += cw;
        }
        line.end
    }

    fn caret_position(&self, cursor: usize) -> Option<Point> {
        let line_h = self.line_height();
        for (idx, line) in self.layout.iter().enumerate() {
            if cursor >= line.start && cursor <= line.end {
                if idx < self.scroll_line {
                    return None;
                }
                let row = (idx - self.scroll_line) as i32;
                let max_row = self.visible_lines() as i32;
                if row >= max_row {
                    return None;
                }
                let x_off = text_px_width(&self.buffer[line.start..cursor], self.face, self.font_size);
                let x = self.area.top_left.x + TEXT_PAD + x_off;
                let y = self.area.top_left.y + TEXT_PAD + row * line_h;
                return Some(Point::new(x, y));
            }
        }
        None
    }

    // --- scroll helpers ------------------------------------------------

    fn visible_lines(&self) -> usize {
        let line_h = self.line_height();
        ((self.area.size.height as i32 - TEXT_PAD) / line_h).max(1) as usize
    }

    fn cursor_line_idx(&self) -> usize {
        self.layout
            .iter()
            .position(|vl| self.cursor >= vl.start && self.cursor <= vl.end)
            .unwrap_or(self.layout.len().saturating_sub(1))
    }

    /// Pixel x of the cursor within its visual line (used for up/down nav).
    fn cursor_col_px(&self, line_idx: usize) -> f32 {
        let vl = self.layout[line_idx];
        let end = self.cursor.min(vl.end);
        self.buffer[vl.start..end]
            .chars()
            .map(|c| char_px(c, self.face, self.font_size))
            .sum()
    }

    fn ensure_cursor_visible(&mut self) {
        let line_idx = self.cursor_line_idx();
        let vis = self.visible_lines();
        if line_idx < self.scroll_line {
            self.scroll_line = line_idx;
        } else if line_idx >= self.scroll_line + vis {
            self.scroll_line = line_idx + 1 - vis;
        }
    }
}

// --- free helpers (no_std, no heap) ------------------------------------

/// Pixel advance width of a single character (emoji get a square slot).
fn char_px(c: char, face: FontFace, size_px: f32) -> f32 {
    if emoji::is_emoji(c) {
        // Emoji are rendered as a square whose side equals the line height.
        (font_aa::cap_height_face(size_px, face) + 4) as f32
    } else {
        font_aa::char_advance(c, size_px, face)
    }
}

/// Integer pixel width of a string slice.
fn text_px_width(s: &str, face: FontFace, size_px: f32) -> i32 {
    s.chars().map(|c| char_px(c, face, size_px)).sum::<f32>() as i32
}

/// Draw a line of text, handling emoji glyphs inline.
fn draw_line_text<D>(
    canvas: &mut D,
    text: &str,
    x: i32,
    y: i32,
    size_px: f32,
    luma: u8,
    face: FontFace,
    line_h: i32,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    let font = font_aa::get_font_for(face);
    let cap_h = font.rasterize('H', size_px).0.height as i32;
    let baseline_y = y + cap_h;
    let mut cursor_x = x as f32;

    for c in text.chars() {
        if emoji::is_emoji(c) {
            let slot = line_h as u32;
            let rect = Rectangle::new(
                Point::new(cursor_x as i32, y),
                Size::new(slot, slot),
            );
            let _ = emoji::draw_glyph_in_rect(canvas, c, rect, WHITE)?;
            cursor_x += slot as f32;
        } else {
            let (metrics, bitmap) = font.rasterize(c, size_px);
            let glyph_top = baseline_y - (metrics.height as i32 + metrics.ymin);
            let glyph_left = cursor_x as i32 + metrics.xmin;
            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let coverage = bitmap[row * metrics.width + col];
                    if coverage == 0 {
                        continue;
                    }
                    let a = coverage as u32;
                    let fg = luma as u32;
                    let blended = ((fg * a + 255 * (255 - a)) / 255) as u8;
                    Pixel(
                        Point::new(glyph_left + col as i32, glyph_top + row as i32),
                        Gray8::new(blended),
                    )
                    .draw(canvas)?;
                }
            }
            cursor_x += metrics.advance_width;
        }
    }
    Ok(())
}

/// Word-wrap `buffer` into visual lines that fit within `max_px` pixels
/// for the given face and size.
fn compute_layout(buffer: &str, max_px: f32, face: FontFace, size_px: f32) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut cur_px: f32 = 0.0;
    let mut last_break: Option<usize> = None;

    let mut i = 0;
    while i < buffer.len() {
        let c = buffer[i..].chars().next().unwrap();
        let c_len = c.len_utf8();

        if c == '\n' {
            lines.push(VisualLine { start: line_start, end: i });
            i += c_len;
            line_start = i;
            cur_px = 0.0;
            last_break = None;
            continue;
        }

        let cw = char_px(c, face, size_px);

        if cur_px > 0.0 && cur_px + cw > max_px {
            let break_at = last_break.unwrap_or(i);
            lines.push(VisualLine { start: line_start, end: break_at });
            line_start = break_at;
            // Recompute width of what we kept on the new line.
            cur_px = buffer[line_start..i]
                .chars()
                .map(|ch| char_px(ch, face, size_px))
                .sum();
            last_break = None;
            continue;
        }

        cur_px += cw;
        if c == ' ' {
            last_break = Some(i + c_len);
        }
        i += c_len;
    }
    lines.push(VisualLine { start: line_start, end: buffer.len() });
    lines
}

/// Walk `s`, accumulating pixel widths, and return the byte offset of
/// the first character whose left edge is at or past `target_px`.
fn byte_at_px(s: &str, target_px: f32, face: FontFace, size_px: f32) -> usize {
    let mut px = 0.0f32;
    for (b, c) in s.char_indices() {
        if px >= target_px {
            return b;
        }
        px += char_px(c, face, size_px);
    }
    s.len()
}

/// Expand `pos` to the surrounding word's `[start, end)` byte range.
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

//! Selectable text widget for displaying text that users can select and copy.
//!
//! This widget displays read-only text with selection support. Users can
//! drag to select text and copy it. Useful for contact details, notes, etc.

use alloc::{string::String, string::ToString, vec::Vec};
use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};

use crate::{BLACK, WHITE, GRAY};

/// Tracks text selection state and handles user interaction.
#[derive(Clone, Debug)]
pub struct SelectableText {
    area: Rectangle,
    text: String,
    selection_start: Option<usize>, // Character index
    selection_end: Option<usize>,   // Character index
    char_width: i32,
    char_height: i32,
    lines: Vec<String>, // Wrapped lines
    chars_per_line: usize,
}

impl SelectableText {
    /// Create a new selectable text widget.
    pub fn new(area: Rectangle, text: String) -> Self {
        let char_width = 6;  // FONT_6X10 character width
        let char_height = 10; // FONT_6X10 character height
        let chars_per_line = (area.size.width as i32 / char_width) as usize;
        
        let lines = Self::wrap_text(&text, chars_per_line);
        
        Self {
            area,
            text,
            selection_start: None,
            selection_end: None,
            char_width,
            char_height,
            lines,
            chars_per_line,
        }
    }

    /// Update the text content.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.lines = Self::wrap_text(&self.text, self.chars_per_line);
        self.clear_selection();
    }

    /// Get the current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the currently selected text, if any.
    pub fn selected_text(&self) -> Option<String> {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            let start_idx = start.min(end);
            let end_idx = start.max(end);
            if start_idx < self.text.len() {
                let end_idx = end_idx.min(self.text.len());
                return Some(self.text[start_idx..end_idx].to_string());
            }
        }
        None
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
    }

    /// Handle pen down event. Returns true if the widget handled it.
    pub fn pen_down(&mut self, x: i16, y: i16) -> bool {
        if !self.contains(x, y) {
            return false;
        }

        if let Some(char_idx) = self.point_to_char(x, y) {
            self.selection_start = Some(char_idx);
            self.selection_end = Some(char_idx);
        }
        true
    }

    /// Handle pen move event while dragging. Returns true if the widget handled it.
    pub fn pen_move(&mut self, x: i16, y: i16) -> bool {
        if self.selection_start.is_none() {
            return false;
        }

        if let Some(char_idx) = self.point_to_char(x, y) {
            self.selection_end = Some(char_idx);
        }
        true
    }

    /// Handle pen up event. Returns true if the widget handled it.
    pub fn pen_up(&mut self, _x: i16, _y: i16) -> bool {
        // Selection is complete, nothing special to do
        true
    }

    /// Check if a point is within the widget area.
    pub fn contains(&self, x: i16, y: i16) -> bool {
        let x = x as i32;
        let y = y as i32;
        x >= self.area.top_left.x
            && x < self.area.top_left.x + self.area.size.width as i32
            && y >= self.area.top_left.y
            && y < self.area.top_left.y + self.area.size.height as i32
    }

    /// Get the widget's area.
    pub fn area(&self) -> Rectangle {
        self.area
    }

    /// Convert a screen point to a character index.
    fn point_to_char(&self, x: i16, y: i16) -> Option<usize> {
        let x = x as i32;
        let y = y as i32;
        
        let rel_x = x - self.area.top_left.x;
        let rel_y = y - self.area.top_left.y;
        
        let line = (rel_y / self.char_height) as usize;
        let col = (rel_x / self.char_width) as usize;
        
        if line >= self.lines.len() {
            // Past the end, select end of text
            return Some(self.text.len());
        }
        
        let mut char_idx = 0;
        for (i, line_text) in self.lines.iter().enumerate() {
            if i == line {
                let col_in_line = col.min(line_text.len());
                return Some(char_idx + col_in_line);
            }
            char_idx += line_text.len() + 1; // +1 for newline (conceptual)
        }
        
        Some(char_idx)
    }

    /// Wrap text to fit within the given character width.
    fn wrap_text(text: &str, chars_per_line: usize) -> Vec<String> {
        if chars_per_line == 0 {
            return Vec::from([text.to_string()]);
        }

        let mut lines = Vec::new();
        let mut current_line = String::new();

        for word in text.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= chars_per_line {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        if lines.is_empty() {
            lines.push(String::new());
        }

        lines
    }

    /// Draw the selectable text widget.
    pub fn draw<D>(&self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        // Draw background
        let _ = self.area
            .into_styled(PrimitiveStyle::with_fill(WHITE))
            .draw(canvas);

        // Draw selection background first
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            self.draw_selection(canvas, start.min(end), start.max(end));
        }

        // Draw text
        let text_style = MonoTextStyle::new(&FONT_6X10, BLACK);
        for (line_idx, line) in self.lines.iter().enumerate() {
            let y = self.area.top_left.y + (line_idx as i32 * self.char_height) + self.char_height;
            let _ = Text::with_baseline(
                line,
                Point::new(self.area.top_left.x, y),
                text_style,
                Baseline::Bottom,
            )
            .draw(canvas);
        }
    }

    /// Draw selection background.
    fn draw_selection<D>(&self, canvas: &mut D, start_idx: usize, end_idx: usize)
    where
        D: DrawTarget<Color = Gray8>,
    {
        if start_idx >= end_idx {
            return;
        }

        // Simple implementation: highlight character ranges
        // This is a simplified version - in practice you'd need to handle
        // multi-line selections properly
        let mut current_char = 0;
        
        for (line_idx, line) in self.lines.iter().enumerate() {
            let line_start = current_char;
            let line_end = current_char + line.len();
            
            if start_idx < line_end && end_idx > line_start {
                let sel_start = start_idx.max(line_start) - line_start;
                let sel_end = end_idx.min(line_end) - line_start;
                
                let x = self.area.top_left.x + sel_start as i32 * self.char_width;
                let y = self.area.top_left.y + line_idx as i32 * self.char_height;
                let width = (sel_end - sel_start) as u32 * self.char_width as u32;
                
                let selection_rect = Rectangle::new(
                    Point::new(x, y),
                    Size::new(width, self.char_height as u32),
                );
                
                let _ = selection_rect
                    .into_styled(PrimitiveStyle::with_fill(GRAY))
                    .draw(canvas);
            }
            
            current_char = line_end + 1; // +1 for conceptual newline
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text() {
        let lines = SelectableText::wrap_text("hello world test", 10);
        assert_eq!(lines, vec!["hello", "world test"]);
        
        let lines = SelectableText::wrap_text("short", 10);
        assert_eq!(lines, vec!["short"]);
        
        let lines = SelectableText::wrap_text("", 10);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn test_selection() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(100, 50));
        let mut text = SelectableText::new(area, "hello world".to_string());
        
        assert!(text.selected_text().is_none());
        
        // Simulate selection
        text.selection_start = Some(0);
        text.selection_end = Some(5);
        assert_eq!(text.selected_text(), Some("hello".to_string()));
        
        text.clear_selection();
        assert!(text.selected_text().is_none());
    }
}
//! # EGUI-based ScrollView for SoulOS
//!
//! Modern scrollable container using EGUI's ScrollArea to replace
//! manual scrollbar implementation with automatic scroll handling.

use egui::{Id, Response, ScrollArea, Ui, Vec2};
use embedded_graphics::primitives::Rectangle;

/// Output from EGUI scroll events
#[derive(Debug, Clone)]
pub struct EguiScrollOutput {
    /// Rectangle that needs redrawing, if any.
    pub dirty: Option<Rectangle>,
    /// New scroll position (0.0 to 1.0).
    pub position: f32,
    /// Whether the position changed this frame.
    pub position_changed: bool,
    /// The response from EGUI interaction
    pub response: Option<Response>,
}

impl Default for EguiScrollOutput {
    fn default() -> Self {
        Self {
            dirty: None,
            position: 0.0,
            position_changed: false,
            response: None,
        }
    }
}

/// EGUI-based scrollable view that replaces manual scrollbar implementation
#[derive(Debug)]
pub struct EguiScrollView {
    /// Unique ID for this scroll area
    id: Id,
    /// Total area for the scrollable view
    area: Rectangle,
    /// Whether horizontal scrolling is enabled
    horizontal: bool,
    /// Whether vertical scrolling is enabled  
    vertical: bool,
    /// Maximum width (None = unlimited)
    max_width: Option<f32>,
    /// Maximum height (None = unlimited)
    max_height: Option<f32>,
    /// Whether to stick to bottom when content grows
    stick_to_bottom: bool,
    /// Whether to auto-shrink the scroll area
    auto_shrink: [bool; 2], // [width, height]
    /// Current scroll offset
    scroll_offset: Vec2,
    /// Whether scroll position changed this frame
    scroll_changed: bool,
}

impl EguiScrollView {
    /// Create a new EGUI-based scroll view
    pub fn new(id: &str, area: Rectangle) -> Self {
        Self {
            id: Id::new(id),
            area,
            horizontal: false,
            vertical: true,
            max_width: None,
            max_height: None,
            stick_to_bottom: false,
            auto_shrink: [false, false],
            scroll_offset: Vec2::ZERO,
            scroll_changed: false,
        }
    }

    /// Enable or disable horizontal scrolling
    pub fn horizontal(mut self, enabled: bool) -> Self {
        self.horizontal = enabled;
        self
    }

    /// Enable or disable vertical scrolling
    pub fn vertical(mut self, enabled: bool) -> Self {
        self.vertical = enabled;
        self
    }

    /// Set maximum width
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set maximum height
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Enable sticking to bottom when content grows (useful for logs)
    pub fn stick_to_bottom(mut self, stick: bool) -> Self {
        self.stick_to_bottom = stick;
        self
    }

    /// Set auto-shrink behavior [width, height]
    pub fn auto_shrink(mut self, shrink: [bool; 2]) -> Self {
        self.auto_shrink = shrink;
        self
    }

    /// Get the current scroll offset
    pub fn scroll_offset(&self) -> Vec2 {
        self.scroll_offset
    }

    /// Set the scroll offset programmatically
    pub fn set_scroll_offset(&mut self, offset: Vec2) {
        if self.scroll_offset != offset {
            self.scroll_offset = offset;
            self.scroll_changed = true;
        }
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.set_scroll_offset(Vec2::new(self.scroll_offset.x, 0.0));
    }

    /// Scroll to bottom (requires knowing content height)
    pub fn scroll_to_bottom(&mut self, content_height: f32) {
        let viewport_height = self.area.size.height as f32;
        if content_height > viewport_height {
            let max_scroll = content_height - viewport_height;
            self.set_scroll_offset(Vec2::new(self.scroll_offset.x, max_scroll));
        }
    }

    /// Run the scroll area with EGUI
    pub fn show<R>(
        &mut self,
        ui: &mut Ui,
        content: impl FnOnce(&mut Ui) -> R,
    ) -> EguiScrollOutput {
        let mut scroll_area = ScrollArea::new([self.horizontal, self.vertical])
            .id_salt(self.id)
            .stick_to_bottom(self.stick_to_bottom)
            .auto_shrink(self.auto_shrink);

        if let Some(width) = self.max_width {
            scroll_area = scroll_area.max_width(width);
        }

        if let Some(height) = self.max_height {
            scroll_area = scroll_area.max_height(height);
        }

        let old_offset = self.scroll_offset;

        let response = scroll_area.show(ui, |ui| {
            // Track the scroll offset from EGUI
            let state = ui.ctx().data(|data| {
                data.get_temp::<egui::scroll_area::State>(self.id)
            });

            if let Some(state) = state {
                self.scroll_offset = state.offset;
            }

            // Call the content function
            content(ui)
        });

        self.scroll_changed = old_offset != self.scroll_offset;

        // Calculate dirty rectangle if scroll changed
        let dirty = if self.scroll_changed {
            Some(self.area)
        } else {
            None
        };

        // Calculate scroll position as 0.0-1.0 ratio
        let position = if self.vertical {
            // For vertical scrolling, use Y offset
            let max_scroll = response.content_size.y - response.inner_rect.height();
            if max_scroll > 0.0 {
                (self.scroll_offset.y / max_scroll).clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            // For horizontal scrolling, use X offset
            let max_scroll = response.content_size.x - response.inner_rect.width();
            if max_scroll > 0.0 {
                (self.scroll_offset.x / max_scroll).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };

        EguiScrollOutput {
            dirty,
            position,
            position_changed: self.scroll_changed,
            response: None, // Response type mismatch - simplified for now
        }
    }

    /// Get the area available for content
    pub fn content_area(&self) -> Rectangle {
        // EGUI handles this automatically, but we provide the full area
        self.area
    }

    /// Check if the scroll area contains the given point
    pub fn contains_point(&self, x: i16, y: i16) -> bool {
        let x = x as i32;
        let y = y as i32;
        x >= self.area.top_left.x
            && x < self.area.top_left.x + self.area.size.width as i32
            && y >= self.area.top_left.y
            && y < self.area.top_left.y + self.area.size.height as i32
    }

    /// Update the scroll view area (e.g., when keyboard appears/disappears)
    pub fn resize(&mut self, new_area: Rectangle) -> EguiScrollOutput {
        let changed = self.area != new_area;
        self.area = new_area;

        if changed {
            EguiScrollOutput {
                dirty: Some(new_area),
                position: 0.0, // Will be updated on next show()
                position_changed: false,
                response: None,
            }
        } else {
            EguiScrollOutput::default()
        }
    }
}

/// Helper functions for common scroll patterns
pub struct ScrollPatterns;

impl ScrollPatterns {
    /// Create a simple vertical scroll area
    pub fn vertical_list(
        id: &str,
        area: Rectangle,
        max_height: f32,
    ) -> EguiScrollView {
        EguiScrollView::new(id, area)
            .vertical(true)
            .horizontal(false)
            .max_height(max_height)
            .auto_shrink([false, true])
    }

    /// Create a text area with scrolling
    pub fn text_area(
        id: &str,
        area: Rectangle,
        max_height: f32,
        stick_to_bottom: bool,
    ) -> EguiScrollView {
        EguiScrollView::new(id, area)
            .vertical(true)
            .horizontal(false)
            .max_height(max_height)
            .stick_to_bottom(stick_to_bottom)
            .auto_shrink([false, true])
    }

    /// Create a horizontal scroll area (for wide content)
    pub fn horizontal_content(
        id: &str,
        area: Rectangle,
        max_width: f32,
    ) -> EguiScrollView {
        EguiScrollView::new(id, area)
            .vertical(false)
            .horizontal(true)
            .max_width(max_width)
            .auto_shrink([true, false])
    }

    /// Create a full scroll area (both directions)
    pub fn full_scroll(
        id: &str,
        area: Rectangle,
        max_size: Vec2,
    ) -> EguiScrollView {
        EguiScrollView::new(id, area)
            .vertical(true)
            .horizontal(true)
            .max_width(max_size.x)
            .max_height(max_size.y)
            .auto_shrink([true, true])
    }
}

/// Adapter to convert between old ScrollableView API and new EguiScrollView
pub struct ScrollViewAdapter {
    egui_scroll: EguiScrollView,
}

impl ScrollViewAdapter {
    /// Create adapter from old ScrollableView parameters
    pub fn from_legacy(area: Rectangle, _content_height: u32, id: &str) -> Self {
        let max_height = area.size.height as f32;
        
        Self {
            egui_scroll: ScrollPatterns::vertical_list(id, area, max_height),
        }
    }

    /// Get the underlying EGUI scroll view
    pub fn egui_scroll(&mut self) -> &mut EguiScrollView {
        &mut self.egui_scroll
    }

    /// Legacy method: set content height
    pub fn set_content_height(&mut self, _height: u32) -> EguiScrollOutput {
        // EGUI handles content height automatically
        EguiScrollOutput::default()
    }

    /// Legacy method: get scroll offset  
    pub fn scroll_offset(&self) -> i32 {
        self.egui_scroll.scroll_offset().y as i32
    }

    /// Legacy method: handle pen events
    pub fn handle_pen_event(&mut self, _down: bool, _move_: bool, _up: bool, _x: i16, _y: i16) -> EguiScrollOutput {
        // EGUI handles pen events automatically
        EguiScrollOutput::default()
    }

    /// Legacy method: resize
    pub fn resize(&mut self, new_area: Rectangle) -> EguiScrollOutput {
        self.egui_scroll.resize(new_area)
    }

    /// Legacy method: content area
    pub fn content_area(&self) -> Rectangle {
        self.egui_scroll.content_area()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::geometry::{Point, Size};

    #[test]
    fn test_egui_scroll_view_creation() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(240, 320));
        let scroll = EguiScrollView::new("test", area);
        
        assert_eq!(scroll.area, area);
        assert!(!scroll.horizontal);
        assert!(scroll.vertical);
        assert_eq!(scroll.scroll_offset, Vec2::ZERO);
    }

    #[test]
    fn test_scroll_patterns() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(240, 320));
        
        let list = ScrollPatterns::vertical_list("list", area, 200.0);
        assert!(!list.horizontal);
        assert!(list.vertical);
        assert_eq!(list.max_height, Some(200.0));

        let text = ScrollPatterns::text_area("text", area, 150.0, true);
        assert!(text.stick_to_bottom);
        assert_eq!(text.max_height, Some(150.0));

        let full = ScrollPatterns::full_scroll("full", area, Vec2::new(300.0, 400.0));
        assert!(full.horizontal);
        assert!(full.vertical);
        assert_eq!(full.max_width, Some(300.0));
        assert_eq!(full.max_height, Some(400.0));
    }

    #[test]
    fn test_scroll_offset() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(240, 320));
        let mut scroll = EguiScrollView::new("test", area);
        
        assert_eq!(scroll.scroll_offset(), Vec2::ZERO);
        
        scroll.set_scroll_offset(Vec2::new(10.0, 20.0));
        assert_eq!(scroll.scroll_offset(), Vec2::new(10.0, 20.0));
        assert!(scroll.scroll_changed);
    }

    #[test]
    fn test_adapter() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(240, 320));
        let adapter = ScrollViewAdapter::from_legacy(area, 500, "adapter_test");
        
        assert_eq!(adapter.egui_scroll.area, area);
        assert_eq!(adapter.egui_scroll.max_height, Some(320.0));
    }

    #[test]
    fn test_contains_point() {
        let area = Rectangle::new(Point::new(10, 20), Size::new(100, 150));
        let scroll = EguiScrollView::new("test", area);
        
        assert!(scroll.contains_point(50, 100));
        assert!(scroll.contains_point(10, 20)); // Top-left corner
        assert!(!scroll.contains_point(5, 15)); // Outside
        assert!(!scroll.contains_point(120, 180)); // Outside (bottom-right + 1)
    }
}
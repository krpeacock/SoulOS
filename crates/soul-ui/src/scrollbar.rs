//! Scrollbar widget with up/down arrows and draggable thumb.
//!
//! Provides industry-standard accessibility features including keyboard
//! navigation and screen reader support. The scrollbar follows the traditional
//! design with arrow buttons at each end and a proportional thumb in the middle.

use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, Triangle},
};

use alloc::{format, string::String};
use crate::palette::{BLACK, WHITE, GRAY};

/// Output from scrollbar interaction events.
#[derive(Debug, Clone)]
pub struct ScrollbarOutput {
    /// Rectangle that needs redrawing, if any.
    pub dirty: Option<Rectangle>,
    /// New scroll position (0.0 to 1.0).
    pub position: f32,
    /// Whether the position changed this frame.
    pub position_changed: bool,
}

impl Default for ScrollbarOutput {
    fn default() -> Self {
        Self {
            dirty: None,
            position: 0.0,
            position_changed: false,
        }
    }
}

/// State of a scrollbar interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollbarState {
    Normal,
    UpPressed,
    DownPressed,
    ThumbDragging,
}

/// A vertical scrollbar with arrow buttons and draggable thumb.
///
/// Provides proportional scrolling with visual feedback and accessibility support.
/// The thumb size automatically adjusts based on the content-to-viewport ratio.
#[derive(Debug, Clone)]
pub struct Scrollbar {
    /// Screen rectangle for the entire scrollbar.
    area: Rectangle,
    /// Current scroll position (0.0 = top, 1.0 = bottom).
    position: f32,
    /// Ratio of visible content to total content (0.0-1.0).
    /// Controls thumb size.
    viewport_ratio: f32,
    /// Current interaction state.
    state: ScrollbarState,
    /// Y offset where drag started (for smooth dragging).
    drag_start_y: Option<i16>,
    /// Position when drag started.
    drag_start_pos: f32,
}

impl Scrollbar {
    /// Minimum width for the scrollbar.
    pub const MIN_WIDTH: u32 = 16;
    
    /// Height of arrow buttons.
    pub const BUTTON_HEIGHT: u32 = 16;
    
    /// Minimum thumb height.
    pub const MIN_THUMB_HEIGHT: u32 = 12;

    /// Create a new scrollbar in the given rectangle.
    ///
    /// `area` should be at least `MIN_WIDTH` pixels wide.
    /// `viewport_ratio` is the ratio of visible content to total content (0.0-1.0).
    pub fn new(area: Rectangle, viewport_ratio: f32) -> Self {
        Self {
            area,
            position: 0.0,
            viewport_ratio: viewport_ratio.clamp(0.0, 1.0),
            state: ScrollbarState::Normal,
            drag_start_y: None,
            drag_start_pos: 0.0,
        }
    }

    /// Update the viewport ratio (visible content / total content).
    pub fn set_viewport_ratio(&mut self, ratio: f32) -> ScrollbarOutput {
        let new_ratio = ratio.clamp(0.0, 1.0);
        if new_ratio != self.viewport_ratio {
            self.viewport_ratio = new_ratio;
            ScrollbarOutput {
                dirty: Some(self.area),
                position: self.position,
                position_changed: false,
            }
        } else {
            ScrollbarOutput::default()
        }
    }

    /// Get the current scroll position (0.0-1.0).
    pub fn position(&self) -> f32 {
        self.position
    }

    /// Set the scroll position (0.0-1.0).
    pub fn set_position(&mut self, pos: f32) -> ScrollbarOutput {
        let new_pos = pos.clamp(0.0, 1.0);
        let changed = new_pos != self.position;
        self.position = new_pos;
        
        ScrollbarOutput {
            dirty: if changed { Some(self.area) } else { None },
            position: self.position,
            position_changed: changed,
        }
    }

    /// Handle pen/touch down event.
    pub fn pen_down(&mut self, x: i16, y: i16) -> ScrollbarOutput {
        if !self.contains_point(x, y) {
            return ScrollbarOutput::default();
        }

        let up_button = self.up_button_rect();
        let down_button = self.down_button_rect();
        let thumb_rect = self.thumb_rect();

        if self.point_in_rect(x, y, &up_button) {
            self.state = ScrollbarState::UpPressed;
            let new_pos = (self.position - 0.1).max(0.0);
            let changed = new_pos != self.position;
            self.position = new_pos;
            
            ScrollbarOutput {
                dirty: Some(self.area),
                position: self.position,
                position_changed: changed,
            }
        } else if self.point_in_rect(x, y, &down_button) {
            self.state = ScrollbarState::DownPressed;
            let new_pos = (self.position + 0.1).min(1.0);
            let changed = new_pos != self.position;
            self.position = new_pos;
            
            ScrollbarOutput {
                dirty: Some(self.area),
                position: self.position,
                position_changed: changed,
            }
        } else if self.point_in_rect(x, y, &thumb_rect) {
            self.state = ScrollbarState::ThumbDragging;
            self.drag_start_y = Some(y);
            self.drag_start_pos = self.position;
            
            ScrollbarOutput {
                dirty: Some(self.area),
                position: self.position,
                position_changed: false,
            }
        } else {
            // Click in track - jump to position
            let track_rect = self.track_rect();
            if self.point_in_rect(x, y, &track_rect) {
                let track_y = y as i32 - track_rect.top_left.y;
                let track_height = track_rect.size.height as i32;
                let new_pos = (track_y as f32 / track_height as f32).clamp(0.0, 1.0);
                let changed = new_pos != self.position;
                self.position = new_pos;
                
                ScrollbarOutput {
                    dirty: Some(self.area),
                    position: self.position,
                    position_changed: changed,
                }
            } else {
                ScrollbarOutput::default()
            }
        }
    }

    /// Handle pen/touch move event.
    pub fn pen_move(&mut self, _x: i16, y: i16) -> ScrollbarOutput {
        if self.state != ScrollbarState::ThumbDragging {
            return ScrollbarOutput::default();
        }

        if let Some(start_y) = self.drag_start_y {
            let track_rect = self.track_rect();
            let dy = y - start_y;
            let track_height = track_rect.size.height as f32;
            let thumb_height = self.calculate_thumb_height() as f32;
            let available_height = track_height - thumb_height;
            
            if available_height > 0.0 {
                let delta = dy as f32 / available_height;
                let new_pos = (self.drag_start_pos + delta).clamp(0.0, 1.0);
                let changed = new_pos != self.position;
                self.position = new_pos;
                
                ScrollbarOutput {
                    dirty: if changed { Some(self.area) } else { None },
                    position: self.position,
                    position_changed: changed,
                }
            } else {
                ScrollbarOutput::default()
            }
        } else {
            ScrollbarOutput::default()
        }
    }

    /// Handle pen/touch up event.
    pub fn pen_up(&mut self) -> ScrollbarOutput {
        let was_pressed = self.state != ScrollbarState::Normal;
        self.state = ScrollbarState::Normal;
        self.drag_start_y = None;
        
        ScrollbarOutput {
            dirty: if was_pressed { Some(self.area) } else { None },
            position: self.position,
            position_changed: false,
        }
    }

    /// Handle keyboard events for accessibility.
    pub fn handle_key(&mut self, key: &str) -> ScrollbarOutput {
        match key {
            "ArrowUp" => {
                let new_pos = (self.position - 0.1).max(0.0);
                let changed = new_pos != self.position;
                self.position = new_pos;
                
                ScrollbarOutput {
                    dirty: if changed { Some(self.area) } else { None },
                    position: self.position,
                    position_changed: changed,
                }
            }
            "ArrowDown" => {
                let new_pos = (self.position + 0.1).min(1.0);
                let changed = new_pos != self.position;
                self.position = new_pos;
                
                ScrollbarOutput {
                    dirty: if changed { Some(self.area) } else { None },
                    position: self.position,
                    position_changed: changed,
                }
            }
            "PageUp" => {
                let new_pos = (self.position - 0.25).max(0.0);
                let changed = new_pos != self.position;
                self.position = new_pos;
                
                ScrollbarOutput {
                    dirty: if changed { Some(self.area) } else { None },
                    position: self.position,
                    position_changed: changed,
                }
            }
            "PageDown" => {
                let new_pos = (self.position + 0.25).min(1.0);
                let changed = new_pos != self.position;
                self.position = new_pos;
                
                ScrollbarOutput {
                    dirty: if changed { Some(self.area) } else { None },
                    position: self.position,
                    position_changed: changed,
                }
            }
            "Home" => {
                let changed = self.position != 0.0;
                self.position = 0.0;
                
                ScrollbarOutput {
                    dirty: if changed { Some(self.area) } else { None },
                    position: self.position,
                    position_changed: changed,
                }
            }
            "End" => {
                let changed = self.position != 1.0;
                self.position = 1.0;
                
                ScrollbarOutput {
                    dirty: if changed { Some(self.area) } else { None },
                    position: self.position,
                    position_changed: changed,
                }
            }
            _ => ScrollbarOutput::default(),
        }
    }

    /// Draw the scrollbar.
    pub fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        // Draw background track
        let track_style = PrimitiveStyleBuilder::new()
            .fill_color(WHITE)
            .stroke_color(BLACK)
            .stroke_width(1)
            .build();
        
        self.area.into_styled(track_style).draw(target)?;

        // Draw up arrow button
        self.draw_up_button(target)?;
        
        // Draw down arrow button
        self.draw_down_button(target)?;
        
        // Draw thumb
        self.draw_thumb(target)?;

        Ok(())
    }

    /// Check if the scrollbar contains the given point.
    pub fn contains_point(&self, x: i16, y: i16) -> bool {
        self.point_in_rect(x, y, &self.area)
    }

    /// Get accessibility information for screen readers.
    pub fn accessibility_info(&self) -> String {
        format!(
            "Scrollbar, position {:.0}%, thumb size {:.0}%",
            self.position * 100.0,
            self.viewport_ratio * 100.0
        )
    }

    fn up_button_rect(&self) -> Rectangle {
        Rectangle::new(
            self.area.top_left,
            Size::new(self.area.size.width, Self::BUTTON_HEIGHT),
        )
    }

    fn down_button_rect(&self) -> Rectangle {
        Rectangle::new(
            Point::new(
                self.area.top_left.x,
                self.area.top_left.y + self.area.size.height as i32 - Self::BUTTON_HEIGHT as i32,
            ),
            Size::new(self.area.size.width, Self::BUTTON_HEIGHT),
        )
    }

    fn track_rect(&self) -> Rectangle {
        Rectangle::new(
            Point::new(
                self.area.top_left.x,
                self.area.top_left.y + Self::BUTTON_HEIGHT as i32,
            ),
            Size::new(
                self.area.size.width,
                self.area.size.height - 2 * Self::BUTTON_HEIGHT,
            ),
        )
    }

    fn calculate_thumb_height(&self) -> u32 {
        if self.viewport_ratio >= 1.0 {
            // No scrolling needed
            return 0;
        }
        
        let track_height = self.track_rect().size.height;
        let thumb_height = (track_height as f32 * self.viewport_ratio) as u32;
        thumb_height.max(Self::MIN_THUMB_HEIGHT).min(track_height)
    }

    fn thumb_rect(&self) -> Rectangle {
        let track_rect = self.track_rect();
        let thumb_height = self.calculate_thumb_height();
        
        if thumb_height == 0 {
            return Rectangle::new(Point::zero(), Size::zero());
        }
        
        let available_height = track_rect.size.height - thumb_height;
        let thumb_y = track_rect.top_left.y + (available_height as f32 * self.position) as i32;
        
        Rectangle::new(
            Point::new(track_rect.top_left.x, thumb_y),
            Size::new(track_rect.size.width, thumb_height),
        )
    }

    fn point_in_rect(&self, x: i16, y: i16, rect: &Rectangle) -> bool {
        let x = x as i32;
        let y = y as i32;
        x >= rect.top_left.x
            && x < rect.top_left.x + rect.size.width as i32
            && y >= rect.top_left.y
            && y < rect.top_left.y + rect.size.height as i32
    }

    fn draw_up_button<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let rect = self.up_button_rect();
        let pressed = self.state == ScrollbarState::UpPressed;
        
        let fill_color = if pressed { BLACK } else { WHITE };
        let stroke_color = BLACK;
        
        let style = PrimitiveStyleBuilder::new()
            .fill_color(fill_color)
            .stroke_color(stroke_color)
            .stroke_width(1)
            .build();
        
        rect.into_styled(style).draw(target)?;
        
        // Draw up arrow
        let arrow_color = if pressed { WHITE } else { BLACK };
        self.draw_arrow(target, rect, true, arrow_color)?;
        
        Ok(())
    }

    fn draw_down_button<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let rect = self.down_button_rect();
        let pressed = self.state == ScrollbarState::DownPressed;
        
        let fill_color = if pressed { BLACK } else { WHITE };
        let stroke_color = BLACK;
        
        let style = PrimitiveStyleBuilder::new()
            .fill_color(fill_color)
            .stroke_color(stroke_color)
            .stroke_width(1)
            .build();
        
        rect.into_styled(style).draw(target)?;
        
        // Draw down arrow
        let arrow_color = if pressed { WHITE } else { BLACK };
        self.draw_arrow(target, rect, false, arrow_color)?;
        
        Ok(())
    }

    fn draw_thumb<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let rect = self.thumb_rect();
        if rect.size.width == 0 || rect.size.height == 0 {
            return Ok(());
        }
        
        let dragging = self.state == ScrollbarState::ThumbDragging;
        let fill_color = if dragging { BLACK } else { GRAY };
        
        let style = PrimitiveStyleBuilder::new()
            .fill_color(fill_color)
            .stroke_color(BLACK)
            .stroke_width(1)
            .build();
        
        rect.into_styled(style).draw(target)?;
        
        Ok(())
    }

    fn draw_arrow<D>(&self, target: &mut D, button_rect: Rectangle, up: bool, color: Gray8) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let center_x = button_rect.top_left.x + button_rect.size.width as i32 / 2;
        let center_y = button_rect.top_left.y + button_rect.size.height as i32 / 2;
        
        let arrow_size = 4;
        
        let (p1, p2, p3) = if up {
            // Up arrow: point at top
            (
                Point::new(center_x, center_y - arrow_size),
                Point::new(center_x - arrow_size, center_y + arrow_size),
                Point::new(center_x + arrow_size, center_y + arrow_size),
            )
        } else {
            // Down arrow: point at bottom
            (
                Point::new(center_x, center_y + arrow_size),
                Point::new(center_x - arrow_size, center_y - arrow_size),
                Point::new(center_x + arrow_size, center_y - arrow_size),
            )
        };
        
        let style = PrimitiveStyle::with_fill(color);
        Triangle::new(p1, p2, p3).into_styled(style).draw(target)?;
        
        Ok(())
    }
}

/// A scrollable view that combines content with a scrollbar.
///
/// Handles the coordination between scroll position and content display,
/// automatically showing/hiding the scrollbar based on content size.
///
/// Two interaction styles are supported in the same view:
/// - **Touch / drag-on-content**: pen-down on the content area starts a
///   tracked gesture; pen-move past [`ScrollableView::DRAG_THRESHOLD`]
///   pixels begins scrolling so the content follows the finger.
/// - **Mouse / trackpad**: [`ScrollableView::handle_wheel`] consumes a
///   pixel-delta from a wheel/two-finger swipe.
///
/// The included scrollbar widget remains visible and clickable for both.
#[derive(Debug, Clone)]
pub struct ScrollableView {
    /// Total area for the scrollable view including scrollbar.
    area: Rectangle,
    /// Height of the scrollable content.
    content_height: u32,
    /// The scrollbar component.
    scrollbar: Scrollbar,
    /// Whether the scrollbar is currently visible.
    scrollbar_visible: bool,
    /// Pen-down tracking for drag-on-content scrolling.
    /// Holds `(start_y, scroll_offset_at_start_pixels)`.
    content_drag_start: Option<(i16, i32)>,
    /// True once the in-progress pen gesture has crossed the drag threshold
    /// and is being treated as a scroll. Used so the parent app can suppress
    /// click handling for drag gestures.
    content_drag_active: bool,
}

impl ScrollableView {
    /// Create a new scrollable view.
    ///
    /// `content_height` is the total height of the scrollable content.
    pub fn new(area: Rectangle, content_height: u32) -> Self {
        let viewport_height = area.size.height;
        let viewport_ratio = if content_height > 0 {
            (viewport_height as f32 / content_height as f32).min(1.0)
        } else {
            1.0
        };
        
        let scrollbar_visible = viewport_ratio < 1.0;
        
        let scrollbar_area = if scrollbar_visible {
            Rectangle::new(
                Point::new(
                    area.top_left.x + area.size.width as i32 - Scrollbar::MIN_WIDTH as i32,
                    area.top_left.y,
                ),
                Size::new(Scrollbar::MIN_WIDTH, area.size.height),
            )
        } else {
            Rectangle::new(Point::zero(), Size::zero())
        };
        
        Self {
            area,
            content_height,
            scrollbar: Scrollbar::new(scrollbar_area, viewport_ratio),
            scrollbar_visible,
            content_drag_start: None,
            content_drag_active: false,
        }
    }

    /// Pixel distance a pen must travel before drag-on-content scrolling
    /// activates. Below this, the gesture is treated as a click on the
    /// underlying content.
    pub const DRAG_THRESHOLD: i32 = 4;

    /// Resize the scroll view area (e.g., when keyboard appears/disappears).
    pub fn resize(&mut self, new_area: Rectangle) -> ScrollbarOutput {
        let old_area = self.area;
        self.area = new_area;
        
        let viewport_height = new_area.size.height;
        let viewport_ratio = if self.content_height > 0 {
            (viewport_height as f32 / self.content_height as f32).min(1.0)
        } else {
            1.0
        };
        
        let was_visible = self.scrollbar_visible;
        self.scrollbar_visible = viewport_ratio < 1.0;
        
        if self.scrollbar_visible {
            let scrollbar_area = Rectangle::new(
                Point::new(
                    new_area.top_left.x + new_area.size.width as i32 - Scrollbar::MIN_WIDTH as i32,
                    new_area.top_left.y,
                ),
                Size::new(Scrollbar::MIN_WIDTH, new_area.size.height),
            );
            self.scrollbar = Scrollbar::new(scrollbar_area, viewport_ratio);
        }
        
        let dirty = if old_area != new_area || was_visible != self.scrollbar_visible {
            // Return the union of old and new areas to ensure complete redraw
            let union_x = old_area.top_left.x.min(new_area.top_left.x);
            let union_y = old_area.top_left.y.min(new_area.top_left.y);
            let union_right = (old_area.top_left.x + old_area.size.width as i32)
                .max(new_area.top_left.x + new_area.size.width as i32);
            let union_bottom = (old_area.top_left.y + old_area.size.height as i32)
                .max(new_area.top_left.y + new_area.size.height as i32);
            
            Some(Rectangle::new(
                Point::new(union_x, union_y),
                Size::new(
                    (union_right - union_x) as u32,
                    (union_bottom - union_y) as u32,
                ),
            ))
        } else {
            None
        };
        
        ScrollbarOutput {
            dirty,
            position: self.scrollbar.position(),
            position_changed: false,
        }
    }

    /// Update the content height and recalculate scrollbar.
    pub fn set_content_height(&mut self, height: u32) -> ScrollbarOutput {
        self.content_height = height;
        let viewport_height = self.area.size.height;
        let viewport_ratio = if height > 0 {
            (viewport_height as f32 / height as f32).min(1.0)
        } else {
            1.0
        };
        
        let was_visible = self.scrollbar_visible;
        self.scrollbar_visible = viewport_ratio < 1.0;
        
        if self.scrollbar_visible {
            let scrollbar_area = Rectangle::new(
                Point::new(
                    self.area.top_left.x + self.area.size.width as i32 - Scrollbar::MIN_WIDTH as i32,
                    self.area.top_left.y,
                ),
                Size::new(Scrollbar::MIN_WIDTH, self.area.size.height),
            );
            self.scrollbar = Scrollbar::new(scrollbar_area, viewport_ratio);
        }
        
        let dirty = if was_visible != self.scrollbar_visible {
            Some(self.area)
        } else if self.scrollbar_visible {
            self.scrollbar.set_viewport_ratio(viewport_ratio).dirty
        } else {
            None
        };
        
        ScrollbarOutput {
            dirty,
            position: self.scrollbar.position(),
            position_changed: false,
        }
    }

    /// Get the area available for content (excluding scrollbar).
    pub fn content_area(&self) -> Rectangle {
        if self.scrollbar_visible {
            Rectangle::new(
                self.area.top_left,
                Size::new(
                    self.area.size.width - Scrollbar::MIN_WIDTH,
                    self.area.size.height,
                ),
            )
        } else {
            self.area
        }
    }

    /// Get the current scroll offset in pixels.
    pub fn scroll_offset(&self) -> i32 {
        if self.scrollbar_visible && self.content_height > self.area.size.height {
            let max_scroll = self.content_height - self.area.size.height;
            (max_scroll as f32 * self.scrollbar.position()) as i32
        } else {
            0
        }
    }

    /// Handle pen/touch events.
    ///
    /// On pen-down inside the scrollbar, the existing scrollbar widget
    /// handles the press. On pen-down inside the content, we record the
    /// start position so a subsequent drag past [`Self::DRAG_THRESHOLD`]
    /// scrolls the content (touch-style). The scrollbar's own thumb-drag
    /// is unaffected.
    pub fn handle_pen_event(&mut self, down: bool, move_: bool, up: bool, x: i16, y: i16) -> ScrollbarOutput {
        if !self.scrollbar_visible {
            return ScrollbarOutput::default();
        }

        let on_scrollbar = self.scrollbar.contains_point(x, y);

        if down {
            if on_scrollbar {
                self.scrollbar.pen_down(x, y)
            } else {
                self.content_drag_start = Some((y, self.scroll_offset()));
                self.content_drag_active = false;
                ScrollbarOutput::default()
            }
        } else if move_ {
            if let Some((start_y, start_offset)) = self.content_drag_start {
                let dy = (start_y as i32) - (y as i32);
                if !self.content_drag_active && dy.abs() >= Self::DRAG_THRESHOLD {
                    self.content_drag_active = true;
                }
                if self.content_drag_active {
                    self.set_scroll_offset_pixels(start_offset + dy)
                } else {
                    ScrollbarOutput::default()
                }
            } else {
                self.scrollbar.pen_move(x, y)
            }
        } else if up {
            let was_drag = self.content_drag_active;
            self.content_drag_start = None;
            self.content_drag_active = false;
            if was_drag {
                ScrollbarOutput {
                    dirty: Some(self.area),
                    position: self.scrollbar.position(),
                    position_changed: false,
                }
            } else {
                self.scrollbar.pen_up()
            }
        } else {
            ScrollbarOutput::default()
        }
    }

    /// Handle a scroll-wheel / two-finger-swipe delta in pixels.
    /// Positive `dy` scrolls the content down (reveals content below).
    pub fn handle_wheel(&mut self, dy: i16) -> ScrollbarOutput {
        if !self.scrollbar_visible || dy == 0 {
            return ScrollbarOutput::default();
        }
        let cur = self.scroll_offset();
        self.set_scroll_offset_pixels(cur + dy as i32)
    }

    /// Whether the in-progress pen gesture is being treated as a scroll
    /// drag rather than a click. Apps may consult this to suppress click
    /// handling on the same pen-down/up pair.
    pub fn is_drag_scrolling(&self) -> bool {
        self.content_drag_active
    }

    fn set_scroll_offset_pixels(&mut self, offset: i32) -> ScrollbarOutput {
        if self.content_height <= self.area.size.height {
            return ScrollbarOutput::default();
        }
        let max_scroll = (self.content_height - self.area.size.height) as f32;
        if max_scroll <= 0.0 {
            return ScrollbarOutput::default();
        }
        let clamped = (offset as f32).clamp(0.0, max_scroll);
        let new_pos = clamped / max_scroll;
        self.scrollbar.set_position(new_pos)
    }

    /// Handle keyboard events for accessibility.
    pub fn handle_key(&mut self, key: &str) -> ScrollbarOutput {
        if self.scrollbar_visible {
            self.scrollbar.handle_key(key)
        } else {
            ScrollbarOutput::default()
        }
    }

    /// Draw the scrollbar (content should be drawn separately).
    pub fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        if self.scrollbar_visible {
            self.scrollbar.draw(target)?;
        }
        Ok(())
    }

    /// Check if the scrollbar area contains the given point.
    pub fn scrollbar_contains_point(&self, x: i16, y: i16) -> bool {
        self.scrollbar_visible && self.scrollbar.contains_point(x, y)
    }

    /// Get accessibility information.
    pub fn accessibility_info(&self) -> Option<String> {
        if self.scrollbar_visible {
            Some(self.scrollbar.accessibility_info())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view() -> ScrollableView {
        let area = Rectangle::new(Point::new(0, 0), Size::new(240, 200));
        ScrollableView::new(area, 800)
    }

    #[test]
    fn wheel_scrolls_content_down() {
        let mut v = make_view();
        assert_eq!(v.scroll_offset(), 0);
        let out = v.handle_wheel(50);
        assert!(out.position_changed);
        assert_eq!(v.scroll_offset(), 50);
    }

    #[test]
    fn wheel_clamps_to_extents() {
        let mut v = make_view();
        v.handle_wheel(10_000);
        assert_eq!(v.scroll_offset(), (800 - 200) as i32);
        v.handle_wheel(-10_000);
        assert_eq!(v.scroll_offset(), 0);
    }

    #[test]
    fn wheel_no_op_when_unscrollable() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(240, 200));
        let mut v = ScrollableView::new(area, 100);
        let out = v.handle_wheel(50);
        assert!(!out.position_changed);
        assert_eq!(v.scroll_offset(), 0);
    }

    #[test]
    fn drag_on_content_scrolls_after_threshold() {
        let mut v = make_view();
        v.handle_pen_event(true, false, false, 100, 100);
        // Move 2px — below threshold, no scroll yet.
        v.handle_pen_event(false, true, false, 100, 98);
        assert_eq!(v.scroll_offset(), 0);
        assert!(!v.is_drag_scrolling());
        // Move another 8px — past threshold, scroll engages.
        v.handle_pen_event(false, true, false, 100, 90);
        assert!(v.is_drag_scrolling());
        assert_eq!(v.scroll_offset(), 10);
    }

    #[test]
    fn drag_releases_clean_state() {
        let mut v = make_view();
        v.handle_pen_event(true, false, false, 100, 100);
        v.handle_pen_event(false, true, false, 100, 50);
        assert!(v.is_drag_scrolling());
        v.handle_pen_event(false, false, true, 100, 50);
        assert!(!v.is_drag_scrolling());
    }

    #[test]
    fn pen_down_on_scrollbar_uses_scrollbar_path() {
        let mut v = make_view();
        // Scrollbar lives on the right edge. Hit it.
        let sb_x = 240 - 8;
        v.handle_pen_event(true, false, false, sb_x, 100);
        // No content drag was started.
        assert!(v.content_drag_start.is_none());
    }
}
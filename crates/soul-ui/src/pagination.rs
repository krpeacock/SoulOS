//! Pagination widget for navigating through pages of content.
//!
//! Simple pagination control with page numbers and navigation arrows.
//! Designed for lists and other paginated content.

use alloc::format;
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::Gray8, prelude::*, primitives::Rectangle,
};

use crate::{button, hit_test, label};

/// Pagination widget state and rendering.
#[derive(Clone, Debug)]
pub struct Pagination {
    area: Rectangle,
    current_page: usize,
    total_pages: usize,
    items_per_page: usize,
    total_items: usize,
}

impl Pagination {
    /// Create a new pagination widget.
    pub fn new(area: Rectangle, items_per_page: usize) -> Self {
        Self {
            area,
            current_page: 0,
            total_pages: 1,
            items_per_page,
            total_items: 0,
        }
    }

    /// Update the total number of items and recalculate pages.
    pub fn set_total_items(&mut self, total_items: usize) {
        self.total_items = total_items;
        self.total_pages = if total_items == 0 {
            1
        } else {
            (total_items + self.items_per_page - 1) / self.items_per_page
        };

        // Ensure current page is valid
        if self.current_page >= self.total_pages {
            self.current_page = self.total_pages.saturating_sub(1);
        }
    }

    /// Get the current page (0-indexed).
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Get the total number of pages.
    pub fn total_pages(&self) -> usize {
        self.total_pages
    }

    /// Get the starting item index for the current page.
    pub fn page_start_index(&self) -> usize {
        self.current_page * self.items_per_page
    }

    /// Get the ending item index for the current page (exclusive).
    pub fn page_end_index(&self) -> usize {
        ((self.current_page + 1) * self.items_per_page).min(self.total_items)
    }

    /// Move to the next page if possible. Returns true if page changed.
    pub fn next_page(&mut self) -> bool {
        if self.current_page + 1 < self.total_pages {
            self.current_page += 1;
            true
        } else {
            false
        }
    }

    /// Move to the previous page if possible. Returns true if page changed.
    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            true
        } else {
            false
        }
    }

    /// Jump to a specific page. Returns true if page changed.
    pub fn go_to_page(&mut self, page: usize) -> bool {
        if page < self.total_pages && page != self.current_page {
            self.current_page = page;
            true
        } else {
            false
        }
    }

    /// Handle pen/touch input. Returns Some(action) if an action occurred.
    pub fn handle_pen(&self, x: i16, y: i16) -> Option<PaginationAction> {
        if !self.contains(x, y) {
            return None;
        }

        // Check if clicking on prev button
        if hit_test(&self.prev_button_rect(), x, y) && self.current_page > 0 {
            return Some(PaginationAction::PrevPage);
        }

        // Check if clicking on next button
        if hit_test(&self.next_button_rect(), x, y) && self.current_page + 1 < self.total_pages {
            return Some(PaginationAction::NextPage);
        }

        None
    }

    /// Check if a point is within the pagination area.
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

    /// Get the rectangle for the previous page button.
    fn prev_button_rect(&self) -> Rectangle {
        Rectangle::new(
            Point::new(self.area.top_left.x, self.area.top_left.y),
            Size::new(30, 20),
        )
    }

    /// Get the rectangle for the next page button.
    fn next_button_rect(&self) -> Rectangle {
        let x = self.area.top_left.x + self.area.size.width as i32 - 30;
        Rectangle::new(Point::new(x, self.area.top_left.y), Size::new(30, 20))
    }

    /// Draw the pagination widget.
    pub fn draw<D>(&self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        if self.total_pages <= 1 {
            return; // Don't show pagination for single page
        }

        // Draw previous button
        let _prev_enabled = self.current_page > 0;
        let _ = button(canvas, self.prev_button_rect(), "‹", false);

        // Draw page info in center
        let page_info = format!("{}/{}", self.current_page + 1, self.total_pages);
        let center_x = self.area.top_left.x + self.area.size.width as i32 / 2 - 20;
        let _ = label(
            canvas,
            Point::new(center_x, self.area.top_left.y + 6),
            &page_info,
        );

        // Draw next button
        let _next_enabled = self.current_page + 1 < self.total_pages;
        let _ = button(canvas, self.next_button_rect(), "›", false);

        // Show additional info if space allows
        if self.area.size.width >= 200 {
            let range_start = self.page_start_index() + 1; // 1-indexed for display
            let range_end = self.page_end_index();
            let range_info = format!("{}-{} of {}", range_start, range_end, self.total_items);
            let _ = label(
                canvas,
                Point::new(self.area.top_left.x, self.area.top_left.y + 22),
                &range_info,
            );
        }
    }
}

/// Actions that can be triggered by pagination widget interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaginationAction {
    PrevPage,
    NextPage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_basic() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(200, 30));
        let mut pagination = Pagination::new(area, 10);

        assert_eq!(pagination.current_page(), 0);
        assert_eq!(pagination.total_pages(), 1);

        pagination.set_total_items(25);
        assert_eq!(pagination.total_pages(), 3);
        assert_eq!(pagination.page_start_index(), 0);
        assert_eq!(pagination.page_end_index(), 10);
    }

    #[test]
    fn test_pagination_navigation() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(200, 30));
        let mut pagination = Pagination::new(area, 10);
        pagination.set_total_items(25);

        assert!(pagination.next_page());
        assert_eq!(pagination.current_page(), 1);
        assert_eq!(pagination.page_start_index(), 10);
        assert_eq!(pagination.page_end_index(), 20);

        assert!(pagination.next_page());
        assert_eq!(pagination.current_page(), 2);
        assert_eq!(pagination.page_start_index(), 20);
        assert_eq!(pagination.page_end_index(), 25);

        assert!(!pagination.next_page()); // Can't go past last page

        assert!(pagination.prev_page());
        assert_eq!(pagination.current_page(), 1);
    }

    #[test]
    fn test_go_to_page() {
        let area = Rectangle::new(Point::new(0, 0), Size::new(200, 30));
        let mut pagination = Pagination::new(area, 10);
        pagination.set_total_items(50);

        assert!(pagination.go_to_page(3));
        assert_eq!(pagination.current_page(), 3);

        assert!(!pagination.go_to_page(10)); // Invalid page
        assert_eq!(pagination.current_page(), 3); // Unchanged
    }
}

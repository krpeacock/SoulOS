//! # EGUI Layout Containers for SoulOS
//!
//! Modern layout system using EGUI's layout containers to replace 
//! manual positioning with automatic, responsive layout management.

use alloc::string::String;
use alloc::vec::Vec;
use egui::{Context, Grid, Layout, ScrollArea, Ui, Vec2, Align};

/// EGUI-based layout manager for SoulOS apps
pub struct EguiLayoutManager {
    context: Context,
}

impl Default for EguiLayoutManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EguiLayoutManager {
    /// Create a new EGUI layout manager
    pub fn new() -> Self {
        let context = Context::default();
        Self { context }
    }

    /// Get the EGUI context for direct use
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Run EGUI layout with the given UI function
    pub fn run_ui<R>(&self, ui_fn: impl FnOnce(&Context) -> R) -> R {
        ui_fn(&self.context)
    }
}

/// Layout container types that replace manual positioning
#[derive(Debug, Clone)]
pub enum LayoutContainer {
    /// Vertical layout (default)
    Vertical,
    /// Horizontal layout
    Horizontal,
    /// Grid layout with specified columns
    Grid { columns: usize, spacing: Vec2 },
    /// Scrollable area
    ScrollArea { 
        horizontal: bool, 
        vertical: bool, 
        max_width: Option<f32>, 
        max_height: Option<f32> 
    },
    /// Group container with optional title
    Group { title: Option<String> },
    /// Collapsing header
    CollapsingHeader { title: String, default_open: bool },
}

/// Widget alignment options
#[derive(Debug, Clone, Copy)]
pub enum WidgetAlign {
    Left,
    Center, 
    Right,
    Justify,
}

/// Layout builder for creating responsive layouts
pub struct LayoutBuilder {
    containers: Vec<LayoutContainer>,
    spacing: f32,
    margin: f32,
}

impl Default for LayoutBuilder {
    fn default() -> Self {
        Self {
            containers: Vec::new(),
            spacing: 4.0,
            margin: 8.0,
        }
    }
}

impl LayoutBuilder {
    /// Create a new layout builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set spacing between widgets
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Set margin around the layout
    pub fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    /// Add a vertical layout container
    pub fn vertical(mut self) -> Self {
        self.containers.push(LayoutContainer::Vertical);
        self
    }

    /// Add a horizontal layout container  
    pub fn horizontal(mut self) -> Self {
        self.containers.push(LayoutContainer::Horizontal);
        self
    }

    /// Add a grid layout container
    pub fn grid(mut self, columns: usize) -> Self {
        self.containers.push(LayoutContainer::Grid {
            columns,
            spacing: Vec2::new(self.spacing, self.spacing),
        });
        self
    }

    /// Add a scrollable container
    pub fn scroll_area(mut self) -> Self {
        self.containers.push(LayoutContainer::ScrollArea {
            horizontal: false,
            vertical: true,
            max_width: None,
            max_height: None,
        });
        self
    }

    /// Add a scrollable container with custom settings
    pub fn scroll_area_custom(mut self, horizontal: bool, vertical: bool, max_width: Option<f32>, max_height: Option<f32>) -> Self {
        self.containers.push(LayoutContainer::ScrollArea {
            horizontal,
            vertical,
            max_width,
            max_height,
        });
        self
    }

    /// Add a group container
    pub fn group(mut self, title: Option<String>) -> Self {
        self.containers.push(LayoutContainer::Group { title });
        self
    }

    /// Add a collapsing header
    pub fn collapsing_header(mut self, title: String, default_open: bool) -> Self {
        self.containers.push(LayoutContainer::CollapsingHeader {
            title,
            default_open,
        });
        self
    }
}

/// Responsive layout system that adapts to SoulOS screen constraints
pub struct ResponsiveLayout {
    screen_width: f32,
    screen_height: f32,
    min_touch_size: Vec2,
}

impl Default for ResponsiveLayout {
    fn default() -> Self {
        Self {
            screen_width: 240.0,  // SoulOS canonical width
            screen_height: 320.0, // SoulOS canonical height
            min_touch_size: Vec2::new(24.0, 24.0), // Minimum touch target
        }
    }
}

impl ResponsiveLayout {
    /// Create a new responsive layout system
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            screen_width,
            screen_height,
            min_touch_size: Vec2::new(24.0, 24.0),
        }
    }

    /// Calculate responsive columns based on available width
    pub fn calculate_columns(&self, item_width: f32, spacing: f32) -> usize {
        let available_width = self.screen_width - (self.min_touch_size.x * 2.0); // Account for margins
        let columns = ((available_width + spacing) / (item_width + spacing)).floor() as usize;
        columns.max(1)
    }

    /// Get maximum available width
    pub fn max_width(&self) -> f32 {
        self.screen_width
    }

    /// Get maximum available height
    pub fn max_height(&self) -> f32 {
        self.screen_height
    }

    /// Get minimum touch size for buttons/interactive elements
    pub fn min_touch_size(&self) -> Vec2 {
        self.min_touch_size
    }

    /// Check if layout should stack vertically (narrow screen)
    pub fn should_stack_vertically(&self, min_item_width: f32) -> bool {
        self.screen_width < (min_item_width * 2.0 + 16.0) // 2 items + spacing
    }
}

/// Layout utilities for common patterns
pub struct LayoutUtils;

impl LayoutUtils {
    /// Create a toolbar layout (horizontal with fixed height)
    pub fn toolbar_layout(ui: &mut Ui, height: f32, content: impl FnOnce(&mut Ui)) {
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), height),
            Layout::left_to_right(Align::Center),
            content,
        );
    }

    /// Create a sidebar layout (vertical with fixed width)
    pub fn sidebar_layout(ui: &mut Ui, width: f32, content: impl FnOnce(&mut Ui)) {
        ui.allocate_ui_with_layout(
            Vec2::new(width, ui.available_height()),
            Layout::top_down(Align::LEFT),
            content,
        );
    }

    /// Create a centered content layout
    pub fn centered_layout(ui: &mut Ui, max_width: f32, content: impl FnOnce(&mut Ui)) {
        let available_width = ui.available_width().min(max_width);
        ui.allocate_ui_with_layout(
            Vec2::new(available_width, ui.available_height()),
            Layout::top_down(Align::Center),
            content,
        );
    }

    /// Create a two-column layout
    pub fn two_column_layout(
        ui: &mut Ui,
        left_content: impl FnOnce(&mut Ui),
        right_content: impl FnOnce(&mut Ui),
    ) {
        ui.columns(2, |columns| {
            left_content(&mut columns[0]);
            right_content(&mut columns[1]);
        });
    }

    /// Create a responsive grid that adapts to available space
    pub fn responsive_grid<T>(
        ui: &mut Ui,
        items: &[T],
        min_item_width: f32,
        item_renderer: impl Fn(&mut Ui, &T),
    ) {
        let available_width = ui.available_width();
        let columns = (((available_width + 4.0) / (min_item_width + 4.0)).floor() as usize).max(1);
        
        Grid::new("responsive_grid")
            .num_columns(columns)
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                for (i, item) in items.iter().enumerate() {
                    item_renderer(ui, item);
                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    /// Create a scrollable list with pagination hints
    pub fn scrollable_list<T>(
        ui: &mut Ui,
        items: &[T],
        max_height: f32,
        item_renderer: impl Fn(&mut Ui, &T),
    ) {
        ScrollArea::vertical()
            .max_height(max_height)
            .show(ui, |ui| {
                for item in items {
                    item_renderer(ui, item);
                    ui.separator();
                }
            });
    }

    /// Create a form layout with labels and inputs
    pub fn form_layout(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
        Grid::new("form_grid")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_responsive_layout_column_calculation() {
        let layout = ResponsiveLayout::new(240.0, 320.0);
        
        // For 60px wide items with 4px spacing
        let columns = layout.calculate_columns(60.0, 4.0);
        assert!(columns >= 1);
        assert!(columns <= 4); // Should fit reasonably on narrow screen
    }

    #[test]
    fn test_layout_builder() {
        let builder = LayoutBuilder::new()
            .spacing(8.0)
            .margin(12.0)
            .vertical()
            .grid(3)
            .scroll_area();
        
        assert_eq!(builder.containers.len(), 3);
        assert_eq!(builder.spacing, 8.0);
        assert_eq!(builder.margin, 12.0);
    }

    #[test]
    fn test_vertical_stacking_decision() {
        let layout = ResponsiveLayout::new(240.0, 320.0);
        
        // Should stack for wide items
        assert!(layout.should_stack_vertically(150.0));
        
        // Should not stack for narrow items
        assert!(!layout.should_stack_vertically(50.0));
    }
}
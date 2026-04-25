//! # EGUI-based Widget Primitives for SoulOS
//!
//! Modern widget system using EGUI's widget architecture to replace
//! manual drawing primitives with interactive, stateful widgets.

use alloc::string::String;
use egui::{
    Context, Frame, Grid, Label, Layout, Response, RichText, Sense, Ui, Vec2, 
    Align, Color32, FontId, Margin, CornerRadius, Stroke, Widget, WidgetText, Align2, Button
};

/// SoulOS-specific styling for EGUI widgets
pub struct SoulOSStyle {
    pub background: Color32,
    pub text: Color32,
    pub accent: Color32,
    pub border: Color32,
    pub button_rounding: f32,
    pub min_button_size: Vec2,
}

impl Default for SoulOSStyle {
    fn default() -> Self {
        Self {
            background: Color32::WHITE,
            text: Color32::BLACK,
            accent: Color32::GRAY,
            border: Color32::BLACK,
            button_rounding: 4.0,
            min_button_size: Vec2::new(24.0, 24.0), // Touch-friendly minimum
        }
    }
}

/// Apply SoulOS styling to EGUI context
pub fn apply_soulos_style(ctx: &Context) {
    let style = SoulOSStyle::default();
    let mut visuals = ctx.style().visuals.clone();
    
    // Set basic colors
    visuals.override_text_color = Some(style.text);
    visuals.panel_fill = style.background;
    visuals.extreme_bg_color = style.background;
    
    // Button styling
    visuals.widgets.inactive.bg_fill = style.background;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, style.border);
    // Note: rounding is a method in newer EGUI versions
    // We'll apply rounding directly in widget rendering instead
    
    visuals.widgets.hovered.bg_fill = style.accent;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, style.border);
    
    visuals.widgets.active.bg_fill = style.text;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, style.background);
    
    ctx.set_visuals(visuals);
}

/// Enhanced button widget with SoulOS Palm-style behavior
pub struct SoulOSButton {
    text: WidgetText,
    min_size: Vec2,
    pressed: bool,
    sense: Sense,
}

impl SoulOSButton {
    pub fn new(text: impl Into<WidgetText>) -> Self {
        Self {
            text: text.into(),
            min_size: Vec2::new(24.0, 24.0),
            pressed: false,
            sense: Sense::click(),
        }
    }

    /// Set minimum size for touch-friendly interaction
    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = size;
        self
    }

    /// Set pressed state (for showing selected/active state)
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    /// Set interaction sense (click, drag, hover, etc.)
    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = sense;
        self
    }
}

impl Widget for SoulOSButton {
    fn ui(self, ui: &mut Ui) -> Response {
        // Simplified button implementation using EGUI's built-in button
        let button = Button::new(self.text);
        if self.pressed {
            // For now, just use regular button - pressed state would need custom styling
        }
        ui.add_sized(self.min_size, button)
    }
}

/// Title bar widget matching SoulOS design
pub struct SoulOSTitleBar {
    title: String,
    width: f32,
    height: f32,
}

impl SoulOSTitleBar {
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self {
            title: title.into(),
            width,
            height: 15.0, // Standard SoulOS title bar height
        }
    }

    /// Set custom height
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl Widget for SoulOSTitleBar {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = Vec2::new(self.width, self.height);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());
        
        if ui.is_rect_visible(rect) {
            // Draw black background
            ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::BLACK);
            
            // Draw white title text
            let text_rect = rect.shrink2(Vec2::new(4.0, 2.0));
            ui.painter().text(
                text_rect.left_top(),
                Align2::LEFT_TOP,
                &self.title,
                FontId::default(),
                Color32::WHITE,
            );
        }
        
        response
    }
}

/// Enhanced label with SoulOS styling
pub struct SoulOSLabel {
    text: WidgetText,
    selectable: bool,
}

impl SoulOSLabel {
    pub fn new(text: impl Into<WidgetText>) -> Self {
        Self {
            text: text.into(),
            selectable: false,
        }
    }

    /// Make the label selectable
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }
}

impl Widget for SoulOSLabel {
    fn ui(self, ui: &mut Ui) -> Response {
        if self.selectable {
            ui.selectable_label(false, self.text)
        } else {
            ui.add(Label::new(self.text))
        }
    }
}

/// Text input widget with SoulOS behavior
pub struct SoulOSTextInput {
    text: String,
    hint: Option<String>,
    password: bool,
    multiline: bool,
    desired_width: Option<f32>,
}

impl SoulOSTextInput {
    pub fn new(text: String) -> Self {
        Self {
            text,
            hint: None,
            password: false,
            multiline: false,
            desired_width: None,
        }
    }

    /// Set hint text shown when empty
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Make this a password input (masked)
    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    /// Make this a multiline text area
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    /// Set desired width
    pub fn desired_width(mut self, width: f32) -> Self {
        self.desired_width = Some(width);
        self
    }
}

impl Widget for SoulOSTextInput {
    fn ui(self, ui: &mut Ui) -> Response {
        // Note: This is a simplified implementation for demonstration
        // In practice, you'd need to handle the mutable text reference properly
        ui.add(Label::new(&self.text))
        
        // Simplified implementation - in practice you'd need proper state management
        // This would be better handled with a callback pattern
    }
}

/// Container widgets for layout
pub struct Containers;

impl Containers {
    /// Create a scrollable area with SoulOS styling
    pub fn scroll_area() -> egui::ScrollArea {
        egui::ScrollArea::vertical()
            .stick_to_bottom(false)
            .auto_shrink([false, false])
    }

    /// Create a titled group container
    pub fn group(ui: &mut Ui, title: Option<&str>, content: impl FnOnce(&mut Ui)) {
        if let Some(title) = title {
            ui.group(|ui| {
                ui.label(RichText::new(title).strong());
                ui.separator();
                content(ui);
            });
        } else {
            ui.group(content);
        }
    }

    /// Create a form-style layout with labels and inputs
    pub fn form(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
        Grid::new("form")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, content);
    }

    /// Create a toolbar layout
    pub fn toolbar(ui: &mut Ui, height: f32, content: impl FnOnce(&mut Ui)) {
Frame::default()
            .inner_margin(Margin::same(4.0 as i8))
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), height),
                    Layout::left_to_right(Align::Center),
                    content,
                );
            });
    }

    /// Create a two-panel layout (sidebar + main)
    pub fn two_panel(
        ui: &mut Ui, 
        sidebar_width: f32,
        sidebar_content: impl FnOnce(&mut Ui),
        main_content: impl FnOnce(&mut Ui)
    ) {
        ui.horizontal(|ui| {
            // Sidebar
            ui.allocate_ui_with_layout(
                Vec2::new(sidebar_width, ui.available_height()),
                Layout::top_down(Align::LEFT),
                sidebar_content,
            );
            
            ui.separator();
            
            // Main content
            main_content(ui);
        });
    }

    /// Create responsive grid that adapts to available space
    pub fn responsive_grid<T>(
        ui: &mut Ui,
        items: &[T],
        min_item_width: f32,
        item_renderer: impl Fn(&mut Ui, usize, &T),
    ) {
        let available_width = ui.available_width();
        let spacing = ui.spacing().item_spacing.x;
        let columns = (((available_width + spacing) / (min_item_width + spacing))
            .floor() as usize)
            .max(1);
        
        Grid::new("responsive_grid")
            .num_columns(columns)
            .spacing([spacing, ui.spacing().item_spacing.y])
            .show(ui, |ui| {
                for (i, item) in items.iter().enumerate() {
                    item_renderer(ui, i, item);
                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    }
}

/// Widget factory for creating common SoulOS widgets
pub struct WidgetFactory;

impl WidgetFactory {
    /// Create a standard SoulOS button
    pub fn button(text: &str) -> SoulOSButton {
        SoulOSButton::new(text)
    }

    /// Create a small button for toolbars
    pub fn small_button(text: &str) -> SoulOSButton {
        SoulOSButton::new(text).min_size(Vec2::new(20.0, 15.0))
    }

    /// Create a title bar
    pub fn title_bar(title: &str, width: f32) -> SoulOSTitleBar {
        SoulOSTitleBar::new(title, width)
    }

    /// Create a simple label
    pub fn label(text: &str) -> SoulOSLabel {
        SoulOSLabel::new(text)
    }

    /// Create a selectable label (like a list item)
    pub fn selectable_label(text: &str) -> SoulOSLabel {
        SoulOSLabel::new(text).selectable(true)
    }

    /// Create a text input
    pub fn text_input(text: String) -> SoulOSTextInput {
        SoulOSTextInput::new(text)
    }

    /// Create a text input with hint
    pub fn text_input_with_hint(text: String, hint: &str) -> SoulOSTextInput {
        SoulOSTextInput::new(text).hint_text(hint)
    }

    /// Create a password input
    pub fn password_input(text: String) -> SoulOSTextInput {
        SoulOSTextInput::new(text).password(true)
    }

    /// Create a multiline text area
    pub fn text_area(text: String) -> SoulOSTextInput {
        SoulOSTextInput::new(text).multiline(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soulos_style_creation() {
        let style = SoulOSStyle::default();
        assert_eq!(style.background, Color32::WHITE);
        assert_eq!(style.text, Color32::BLACK);
        assert_eq!(style.button_rounding, 4.0);
    }

    #[test]
    fn test_widget_factory() {
        let button = WidgetFactory::button("Test");
        // Test that button was created with correct properties
        assert_eq!(button.min_size, Vec2::new(24.0, 24.0));
        
        let small_button = WidgetFactory::small_button("X");
        assert_eq!(small_button.min_size, Vec2::new(20.0, 15.0));
    }

    #[test] 
    fn test_soulos_button_configuration() {
        let button = SoulOSButton::new("Test")
            .min_size(Vec2::new(50.0, 30.0))
            .pressed(true);
        
        assert_eq!(button.min_size, Vec2::new(50.0, 30.0));
        assert!(button.pressed);
    }
}
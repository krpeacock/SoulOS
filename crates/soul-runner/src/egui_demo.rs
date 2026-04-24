//! Native egui demo app with proper layout and scrolling functionality.

use std::{format, string::String, vec::Vec};
use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use soul_core::{App, Ctx, Event, KeyCode, APP_HEIGHT, SCREEN_WIDTH, a11y::A11yNode};
use soul_ui::{
    prelude::*,
    ScrollableView, TITLE_BAR_H,
};

/// State for various demo components
#[derive(Debug, Clone)]
struct DemoState {
    text_input: String,
    slider_value: i32,
    checkbox_state: bool,
    radio_selection: u8, // 0, 1, 2 for Option1, Option2, Option3
    progress_value: u8,
    selected_color: u8, // 0=Red, 1=Green, 2=Blue
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            text_input: "Hello egui!".into(),
            slider_value: 50,
            checkbox_state: true,
            radio_selection: 1, // Option2
            progress_value: 70,
            selected_color: 0, // Red
        }
    }
}

/// Native egui demo app showcasing layout and scrolling
pub struct EguiDemo {
    state: DemoState,
    scroll_view: ScrollableView,
    scroll_offset: i32,
}

impl EguiDemo {
    pub const APP_ID: &'static str = "com.soulos.egui_demo_native";
    pub const NAME: &'static str = "egui Demo (Native)";
    
    const CONTENT_HEIGHT: u32 = 1200; // Total scrollable content height (increased to ensure scrolling)
    const LINE_HEIGHT: u32 = 20;
    const SECTION_SPACING: u32 = 16;
    const GROUP_SPACING: u32 = 12;
    
    pub fn new() -> Self {
        let scroll_area = Rectangle::new(
            Point::new(0, TITLE_BAR_H as i32),
            Size::new(SCREEN_WIDTH as u32, (APP_HEIGHT as u32) - TITLE_BAR_H),
        );
        
        
        Self {
            state: DemoState::default(),
            scroll_view: ScrollableView::new(scroll_area, Self::CONTENT_HEIGHT),
            scroll_offset: 0,
        }
    }
    
    
    /// Draw a section header
    fn draw_section_header<D>(&self, canvas: &mut D, y: &mut i32, title: &str) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        let point = Point::new(8, *y);
        let _ = label(canvas, point, title);
        *y += (Self::LINE_HEIGHT + Self::GROUP_SPACING) as i32;
        
        // Draw separator line
        let sep_rect = Rectangle::new(Point::new(8, *y - 6), Size::new(SCREEN_WIDTH as u32 - 16, 1));
        let _ = sep_rect.into_styled(PrimitiveStyle::with_fill(GRAY)).draw(canvas);
        
        Ok(())
    }
    
    /// Draw text and input components section
    fn draw_text_input_section<D>(&self, canvas: &mut D, y: &mut i32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        self.draw_section_header(canvas, y, "Text & Input Components")?;
        
        // Basic label
        let _ = label(canvas, Point::new(12, *y), "Basic text label");
        *y += Self::LINE_HEIGHT as i32;
        
        // Rich text (simulated with different styling)
        let _ = label(canvas, Point::new(12, *y), "Rich text in color");
        *y += Self::LINE_HEIGHT as i32;
        
        // Hyperlink
        let _ = label(canvas, Point::new(12, *y), "Click me!");
        *y += Self::LINE_HEIGHT as i32;
        
        // Monospace text
        let _ = label(canvas, Point::new(12, *y), "Code text");
        *y += Self::LINE_HEIGHT as i32;
        
        // Text input
        let rect = Rectangle::new(Point::new(12, *y), Size::new(200, Self::LINE_HEIGHT));
        let display_text = if self.state.text_input.is_empty() {
            "Type here..."
        } else {
            &self.state.text_input
        };
        let _ = button(canvas, rect, display_text, false);
        *y += Self::LINE_HEIGHT as i32;
        
        // Password input (simulated)
        let rect = Rectangle::new(Point::new(12, *y), Size::new(200, Self::LINE_HEIGHT));
        let _ = button(canvas, rect, "••••••", false);
        *y += Self::LINE_HEIGHT as i32;
        
        // Multiline text area
        let rect = Rectangle::new(Point::new(12, *y), Size::new(200, Self::LINE_HEIGHT * 2));
        let _ = button(canvas, rect, "Multiline\nText Area", false);
        *y += (Self::LINE_HEIGHT * 2) as i32;
        
        *y += Self::SECTION_SPACING as i32;
        Ok(())
    }
    
    /// Draw selection and interaction components section
    fn draw_selection_section<D>(&self, canvas: &mut D, y: &mut i32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        self.draw_section_header(canvas, y, "Selection & Interaction")?;
        
        // Checkbox
        let rect = Rectangle::new(Point::new(12, *y), Size::new(150, Self::LINE_HEIGHT));
        let checkbox_text = if self.state.checkbox_state { "☑ Enable feature" } else { "☐ Enable feature" };
        let _ = button(canvas, rect, checkbox_text, false);
        *y += Self::LINE_HEIGHT as i32;
        
        // Radio buttons
        let radio_options = ["○ Option 1", "● Option 2", "○ Option 3"];
        let selected = self.state.radio_selection as usize;
        for (i, &option) in radio_options.iter().enumerate() {
            let rect = Rectangle::new(Point::new(12, *y), Size::new(120, Self::LINE_HEIGHT));
            let text = if i == selected { option.replace('○', "●") } else { option.replace('●', "○") };
            let _ = button(canvas, rect, &text, i == selected);
            *y += Self::LINE_HEIGHT as i32;
        }
        
        // Slider
        let rect = Rectangle::new(Point::new(12, *y), Size::new(200, Self::LINE_HEIGHT));
        let slider_text = format!("Slider: {}", self.state.slider_value);
        let _ = button(canvas, rect, &slider_text, false);
        *y += Self::LINE_HEIGHT as i32;
        
        // Progress bar
        let progress_text = format!("Progress: {}%", self.state.progress_value);
        let _ = label(canvas, Point::new(12, *y), &progress_text);
        *y += Self::LINE_HEIGHT as i32;
        
        // Color selector
        let colors = ["Red", "Green", "Blue"];
        let rect = Rectangle::new(Point::new(12, *y), Size::new(100, Self::LINE_HEIGHT));
        let color_text = format!("Color: {}", colors[self.state.selected_color as usize]);
        let _ = button(canvas, rect, &color_text, false);
        *y += Self::LINE_HEIGHT as i32;
        
        *y += Self::SECTION_SPACING as i32;
        Ok(())
    }
    
    /// Draw layout examples section
    fn draw_layout_section<D>(&self, canvas: &mut D, y: &mut i32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        self.draw_section_header(canvas, y, "Layout Examples")?;
        
        // Three column layout
        let col_width = 70;
        let col_spacing = 4;
        for i in 0..3 {
            let x = 12 + (col_width + col_spacing) * i;
            let rect = Rectangle::new(Point::new(x, *y), Size::new(col_width as u32, Self::LINE_HEIGHT));
            let text = format!("Col {}", i + 1);
            let _ = button(canvas, rect, &text, false);
        }
        *y += Self::LINE_HEIGHT as i32;
        
        // Navigation buttons
        let nav_y = *y;
        let left_rect = Rectangle::new(Point::new(12, nav_y), Size::new(24, Self::LINE_HEIGHT));
        let _ = button(canvas, left_rect, "◀", false);
        
        let _ = label(canvas, Point::new(40, nav_y), "Navigation");
        
        let right_rect = Rectangle::new(Point::new(144, nav_y), Size::new(24, Self::LINE_HEIGHT));
        let _ = button(canvas, right_rect, "▶", false);
        
        *y += Self::LINE_HEIGHT as i32;
        *y += Self::SECTION_SPACING as i32;
        Ok(())
    }
    
    /// Draw visual components section
    fn draw_visual_section<D>(&self, canvas: &mut D, y: &mut i32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        self.draw_section_header(canvas, y, "Visual Components")?;
        
        // Image placeholder
        let img_rect = Rectangle::new(Point::new(12, *y), Size::new(80, 60));
        let _ = img_rect.into_styled(PrimitiveStyle::with_stroke(BLACK, 1)).draw(canvas);
        let _ = label(canvas, Point::new(14, *y + 20), "Image");
        *y += 64;
        
        // Table header
        let header_rect = Rectangle::new(Point::new(12, *y), Size::new(200, Self::LINE_HEIGHT));
        let _ = header_rect.into_styled(PrimitiveStyle::with_fill(GRAY)).draw(canvas);
        
        let headers = ["Name", "Value", "Status"];
        let col_widths = [60, 60, 60];
        let mut x = 14;
        for (header, width) in headers.iter().zip(col_widths.iter()) {
            let _ = label(canvas, Point::new(x, *y), header);
            x += width + 4;
        }
        *y += Self::LINE_HEIGHT as i32;
        
        // Table rows
        let rows = [("Item 1", "123", "✓"), ("Item 2", "456", "○")];
        for (name, value, status) in rows.iter() {
            let mut x = 14;
            for (text, width) in [name, value, status].iter().zip(col_widths.iter()) {
                let _ = label(canvas, Point::new(x, *y), text);
                x += width + 4;
            }
            *y += Self::LINE_HEIGHT as i32;
        }
        
        *y += Self::SECTION_SPACING as i32;
        Ok(())
    }
    
    /// Handle text input
    fn handle_text_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => {
                if self.state.text_input.len() < 50 {
                    self.state.text_input.push(c);
                }
            }
            KeyCode::Backspace => {
                self.state.text_input.pop();
            }
            _ => {}
        }
    }
    
    /// Handle pen interaction with components
    fn handle_component_tap(&mut self, x: i16, y: i16) {
        // Adjust y for scroll offset - subtract to get virtual content coordinate
        let adjusted_y = y - self.scroll_offset as i16;
        
        // Simple hit testing for interactive components
        // This is a simplified version - in a real app you'd have proper hit testing
        
        // Check checkbox area (approximate position)
        if (120..=140).contains(&adjusted_y) && (12..=162).contains(&x) {
            self.state.checkbox_state = !self.state.checkbox_state;
        }
        
        // Check radio buttons (approximate positions)
        for i in 0..3 {
            let radio_y = 140 + (i * 20);
            if (radio_y..=radio_y + 20).contains(&adjusted_y) && (12..=132).contains(&x) {
                self.state.radio_selection = i as u8;
                break;
            }
        }
        
        // Check slider (approximate position)
        if (200..=220).contains(&adjusted_y) && (12..=212).contains(&x) {
            let slider_progress = ((x - 12) as f32 / 200.0).clamp(0.0, 1.0);
            self.state.slider_value = (slider_progress * 100.0) as i32;
        }
        
        // Check color selector (approximate position)
        if (240..=260).contains(&adjusted_y) && (12..=112).contains(&x) {
            self.state.selected_color = (self.state.selected_color + 1) % 3;
        }
    }

    pub fn persist(&mut self) {
        // No persistence needed for demo
    }
}

impl App for EguiDemo {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::AppStart => {
                ctx.invalidate_all();
            }
            
            Event::Key(key) => {
                self.handle_text_input(key);
                ctx.invalidate_all();
            }
            
            Event::PenDown { x, y } => {
                // Check if scrollbar was hit
                let scroll_output = self.scroll_view.handle_pen_event(true, false, false, x, y);
                if let Some(dirty) = scroll_output.dirty {
                    ctx.invalidate(dirty);
                }
                if scroll_output.position_changed {
                    self.scroll_offset = self.scroll_view.scroll_offset();
                    // Invalidate the entire content area when scrolling
                    ctx.invalidate(self.scroll_view.content_area());
                }
                
                // If not scrollbar, handle component interaction
                if !self.scroll_view.scrollbar_contains_point(x, y) {
                    self.handle_component_tap(x, y);
                    ctx.invalidate_all();
                }
            }
            
            Event::PenMove { x, y } => {
                let scroll_output = self.scroll_view.handle_pen_event(false, true, false, x, y);
                if let Some(dirty) = scroll_output.dirty {
                    ctx.invalidate(dirty);
                }
                if scroll_output.position_changed {
                    self.scroll_offset = self.scroll_view.scroll_offset();
                    // Invalidate the entire content area when scrolling
                    ctx.invalidate(self.scroll_view.content_area());
                }
            }
            
            Event::PenUp { x, y } => {
                let scroll_output = self.scroll_view.handle_pen_event(false, false, true, x, y);
                if let Some(dirty) = scroll_output.dirty {
                    ctx.invalidate(dirty);
                }
                if scroll_output.position_changed {
                    self.scroll_offset = self.scroll_view.scroll_offset();
                    // Invalidate the entire content area when scrolling
                    ctx.invalidate(self.scroll_view.content_area());
                }
            }
            
            Event::ButtonDown(soul_core::HardButton::PageUp) => {
                let scroll_output = self.scroll_view.handle_key("PageUp");
                if let Some(dirty) = scroll_output.dirty {
                    ctx.invalidate(dirty);
                }
                if scroll_output.position_changed {
                    self.scroll_offset = self.scroll_view.scroll_offset();
                    // Invalidate the entire content area when scrolling
                    ctx.invalidate(self.scroll_view.content_area());
                }
            }
            
            Event::ButtonDown(soul_core::HardButton::PageDown) => {
                let scroll_output = self.scroll_view.handle_key("PageDown");
                if let Some(dirty) = scroll_output.dirty {
                    ctx.invalidate(dirty);
                }
                if scroll_output.position_changed {
                    self.scroll_offset = self.scroll_view.scroll_offset();
                    // Invalidate the entire content area when scrolling
                    ctx.invalidate(self.scroll_view.content_area());
                }
            }
            
            _ => {}
        }
    }
    
    fn draw<D>(&mut self, canvas: &mut D, _dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>,
    {
        // Clear background
        let _ = clear(canvas, SCREEN_WIDTH as u32, APP_HEIGHT as u32);
        
        // Draw title bar
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "egui Demo (Native)");
        
        // Set up content area with scroll offset
        let content_area = self.scroll_view.content_area();
        let mut clipped_canvas = canvas.clipped(&content_area);
        
        // Apply scroll offset by adjusting y coordinates
        // Negative scroll offset moves content up when scrolling down
        let mut translated_canvas = clipped_canvas.translated(Point::new(0, -self.scroll_offset));
        let mut y = content_area.top_left.y;
        
        // Demo title
        let _ = label(&mut translated_canvas, Point::new(8, y), "egui Component Showcase");
        y += (Self::LINE_HEIGHT + Self::GROUP_SPACING) as i32;
        
        // Draw all sections
        let _ = self.draw_text_input_section(&mut translated_canvas, &mut y);
        let _ = self.draw_selection_section(&mut translated_canvas, &mut y);
        let _ = self.draw_layout_section(&mut translated_canvas, &mut y);
        let _ = self.draw_visual_section(&mut translated_canvas, &mut y);
        
        // Final message
        let _ = label(&mut translated_canvas, Point::new(8, y), "All components rendered!");
        
        // Draw scrollbar
        let _ = self.scroll_view.draw(canvas);
    }
    
    fn a11y_nodes(&self) -> Vec<A11yNode> {
        let mut nodes = Vec::new();
        
        // Add main content area
        nodes.push(A11yNode {
            bounds: self.scroll_view.content_area(),
            label: "egui demo content".into(),
            role: "main".into(),
        });
        
        // Add scrollbar if visible
        if let Some(info) = self.scroll_view.accessibility_info() {
            nodes.push(A11yNode {
                bounds: Rectangle::new(
                    Point::new(SCREEN_WIDTH as i32 - 16, TITLE_BAR_H as i32),
                    Size::new(16, (APP_HEIGHT as u32) - TITLE_BAR_H),
                ),
                label: info,
                role: "scrollbar".into(),
            });
        }
        
        nodes
    }
}
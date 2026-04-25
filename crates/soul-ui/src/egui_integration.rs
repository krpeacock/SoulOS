//! # EGUI Integration for SoulOS Scripts
//!
//! This module provides the bridge between Rhai scripts and EGUI's immediate mode GUI system,
//! specifically implementing proper ScrollArea support.

use egui::{Context, ScrollArea, Ui, Vec2, Pos2, Rect};
use embedded_graphics::{pixelcolor::Gray8, prelude::DrawTarget};

/// EGUI context wrapper for SoulOS script integration
pub struct SoulOSEguiContext {
    context: Context,
    current_scroll_area: Option<ScrollArea>,
    content_size: Vec2,
}

impl SoulOSEguiContext {
    /// Create a new EGUI context for SoulOS
    pub fn new() -> Self {
        Self {
            context: Context::default(),
            current_scroll_area: None,
            content_size: Vec2::ZERO,
        }
    }

    /// Begin a scrollable content area
    pub fn begin_scroll_area(&mut self, max_height: f32) {
        self.current_scroll_area = Some(
            ScrollArea::vertical()
                .max_height(max_height)
                .auto_shrink([false, true])
        );
        self.content_size = Vec2::ZERO;
    }

    /// End the scrollable content area
    pub fn end_scroll_area(&mut self) {
        self.current_scroll_area = None;
    }

    /// Check if we're currently in a scroll area
    pub fn in_scroll_area(&self) -> bool {
        self.current_scroll_area.is_some()
    }

    /// Track content size for scroll determination
    pub fn track_content(&mut self, rect: Rect) {
        self.content_size = self.content_size.max(rect.max.to_vec2());
    }

    /// Get the EGUI context
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Check if content needs scrolling
    pub fn needs_scrolling(&self, viewport_height: f32) -> bool {
        self.content_size.y > viewport_height
    }
}

/// EGUI-based drawing commands for scripts
pub struct EguiScriptDrawing {
    pub context: SoulOSEguiContext,
    pub viewport_rect: Rect,
}

impl EguiScriptDrawing {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            context: SoulOSEguiContext::new(),
            viewport_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(viewport_width, viewport_height)),
        }
    }

    /// Execute drawing within EGUI context
    pub fn draw<R>(&mut self, draw_fn: impl FnOnce(&mut Ui) -> R) -> R {
        let mut result = None;
        let mut draw_fn = Some(draw_fn);
        
        let _ = self.context.context().run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(f) = draw_fn.take() {
                    // Check if we need scrolling based on previous frame's content
                    if self.context.needs_scrolling(self.viewport_rect.height()) {
                        // Use ScrollArea for content that exceeds viewport
                        ScrollArea::vertical()
                            .max_height(self.viewport_rect.height())
                            .show(ui, |ui| {
                                result = Some(f(ui));
                            });
                    } else {
                        // Draw directly for content that fits
                        result = Some(f(ui));
                    }
                }
            });
        });

        result.unwrap()
    }

    /// Convert EGUI output to embedded-graphics drawing commands
    pub fn render_to_canvas<D>(&self, _canvas: &mut D) 
    where 
        D: DrawTarget<Color = Gray8>,
    {
        // This would convert EGUI's tessellated shapes to embedded-graphics primitives
        // For now, this is a placeholder - full implementation would require:
        // 1. Converting egui::Shape to embedded-graphics primitives
        // 2. Handling text rendering
        // 3. Managing clipping and transforms
        
        // TODO: Implement EGUI to embedded-graphics conversion
    }
}

/// High-level API for scripts to use EGUI scrolling
pub fn with_scroll_area<R>(
    _max_height: f32,
    content_fn: impl FnOnce() -> R,
) -> R {
    // This would be the main entry point for scripts
    // It sets up the scroll area context and executes the content function
    content_fn()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_egui_context_creation() {
        let ctx = SoulOSEguiContext::new();
        assert!(!ctx.in_scroll_area());
        assert_eq!(ctx.content_size, Vec2::ZERO);
    }

    #[test]
    fn test_scroll_area_lifecycle() {
        let mut ctx = SoulOSEguiContext::new();
        
        assert!(!ctx.in_scroll_area());
        
        ctx.begin_scroll_area(200.0);
        assert!(ctx.in_scroll_area());
        
        ctx.end_scroll_area();
        assert!(!ctx.in_scroll_area());
    }

    #[test]
    fn test_needs_scrolling() {
        let mut ctx = SoulOSEguiContext::new();
        
        // No content tracked yet
        assert!(!ctx.needs_scrolling(100.0));
        
        // Track content that exceeds viewport
        ctx.track_content(Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 150.0)));
        assert!(ctx.needs_scrolling(100.0));
        
        // Content that fits in viewport
        assert!(!ctx.needs_scrolling(200.0));
    }
}
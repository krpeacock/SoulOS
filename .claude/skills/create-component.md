# Create Component Skill

A comprehensive guide for creating new UI components that follow SoulOS architecture and design principles.

## Architecture Overview

SoulOS follows the **Zen of Palm** - ruthless simplification and paper-replacement metaphor. Components must be immediate, stateful, and cooperative.

### Core Principles

1. **Paper-replacement metaphor** - Components feel like physical objects, not computer abstractions
2. **Instant-on behavior** - No loading states, splash screens, or delays
3. **Cooperative single-focus** - One foreground component at a time, yields control by returning
4. **Database-centric storage** - State persists automatically, no explicit save/load
5. **Dirty-rect redraw** - Only repaint what changed for e-ink efficiency

### Crate Structure

```
soul-ui/          # Widget SDK (no_std + alloc)
├── primitives/   # Stateless draw helpers
├── widgets/      # Stateful interactive components  
└── palette/      # Canonical color set
```

**Critical Boundary:** `soul-ui` must remain `no_std + alloc` - never import `std`.

## Component Architecture Patterns

### 1. Stateless Primitives (`primitives.rs`)

For simple drawing functions with no state between calls.

```rust
/// Draw a button with optional pressed state.
pub fn button<D>(
    target: &mut D,
    rect: Rectangle,
    label: &str,
    pressed: bool,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    // Implementation details
}
```

**When to use:** Simple drawing operations, icons, labels, static UI elements.

### 2. Stateful Widgets

For interactive components that maintain state and handle events.

#### Widget Structure Template

```rust
use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{Rectangle, PrimitiveStyleBuilder},
};
use alloc::{string::String, vec::Vec}; // For no_std compatibility
use crate::palette::{BLACK, WHITE, GRAY};

/// Output from widget interaction events.
#[derive(Debug, Clone)]
pub struct WidgetOutput {
    /// Rectangle that needs redrawing, if any.
    pub dirty: Option<Rectangle>,
    /// Whether the widget's data changed.
    pub data_changed: bool,
    /// Widget-specific output data.
    pub result: Option<WidgetResult>,
}

impl Default for WidgetOutput {
    fn default() -> Self {
        Self {
            dirty: None,
            data_changed: false,
            result: None,
        }
    }
}

/// Internal state for interaction tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WidgetState {
    Normal,
    Pressed,
    Focused,
    Disabled,
}

/// Main widget structure.
#[derive(Debug, Clone)]
pub struct Widget {
    /// Screen rectangle for the widget.
    area: Rectangle,
    /// Widget's internal data.
    data: String,
    /// Current interaction state.
    state: WidgetState,
    /// Widget-specific configuration.
    config: WidgetConfig,
}

impl Widget {
    /// Create a new widget.
    pub fn new(area: Rectangle) -> Self {
        Self {
            area,
            data: String::new(),
            state: WidgetState::Normal,
            config: WidgetConfig::default(),
        }
    }

    /// Handle pen/touch down event.
    pub fn pen_down(&mut self, x: i16, y: i16) -> WidgetOutput {
        if !self.contains_point(x, y) {
            return WidgetOutput::default();
        }
        
        self.state = WidgetState::Pressed;
        // Handle interaction logic
        
        WidgetOutput {
            dirty: Some(self.area),
            data_changed: false,
            result: None,
        }
    }

    /// Handle pen/touch up event.
    pub fn pen_up(&mut self) -> WidgetOutput {
        let was_pressed = self.state == WidgetState::Pressed;
        self.state = WidgetState::Normal;
        
        WidgetOutput {
            dirty: if was_pressed { Some(self.area) } else { None },
            data_changed: false,
            result: None,
        }
    }

    /// Handle keyboard events for accessibility.
    pub fn handle_key(&mut self, key: &str) -> WidgetOutput {
        match key {
            "Enter" | "Space" => {
                // Trigger main action
                WidgetOutput::default()
            }
            _ => WidgetOutput::default(),
        }
    }

    /// Draw the widget.
    pub fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        // Draw background
        let style = PrimitiveStyleBuilder::new()
            .fill_color(WHITE)
            .stroke_color(BLACK)
            .stroke_width(1)
            .build();
        
        self.area.into_styled(style).draw(target)?;
        
        // Draw content based on state
        match self.state {
            WidgetState::Pressed => {
                // Draw pressed appearance
            }
            _ => {
                // Draw normal appearance
            }
        }

        Ok(())
    }

    /// Check if widget contains the given point.
    pub fn contains_point(&self, x: i16, y: i16) -> bool {
        let x = x as i32;
        let y = y as i32;
        x >= self.area.top_left.x
            && x < self.area.top_left.x + self.area.size.width as i32
            && y >= self.area.top_left.y
            && y < self.area.top_left.y + self.area.size.height as i32
    }

    /// Get accessibility information for screen readers.
    pub fn accessibility_info(&self) -> String {
        format!("Widget containing: {}", self.data)
    }
}
```

## Design Guidelines

### Visual Design

1. **Use Gray8 color model** - 8-bit grayscale only (`BLACK`, `WHITE`, `GRAY`)
2. **1-pixel borders** - Crisp lines for Palm aesthetic
3. **Rounded rectangles** - 4px corner radius for buttons
4. **6×10 font** - Use `FONT_6X10` for all text
5. **Visual feedback** - Invert colors for pressed states

### Interaction Design

1. **Immediate feedback** - Visual response on `pen_down`
2. **Forgiving touch** - Accept slight movement during interaction
3. **Hit test boundaries** - Use exact rectangle bounds
4. **State preservation** - Maintain widget state between interactions

### Accessibility Requirements

1. **Keyboard navigation** - Support Arrow keys, Tab, Enter, Space
2. **Screen reader support** - Provide descriptive `accessibility_info()`
3. **Focus indicators** - Visual indication of focused element
4. **Semantic roles** - Clear role identification ("button", "textbox", etc.)

## Event Handling Patterns

### Standard Event Flow

```rust
// In App::handle()
match event {
    Event::PenDown { x, y } => {
        let output = self.widget.pen_down(x, y);
        if let Some(dirty) = output.dirty {
            ctx.invalidate(dirty);
        }
        if output.data_changed {
            // Persist data if needed
        }
    }
    Event::PenUp { x, y } => {
        let output = self.widget.pen_up();
        if let Some(dirty) = output.dirty {
            ctx.invalidate(dirty);
        }
    }
    Event::Key(KeyCode::Char(c)) => {
        let output = self.widget.handle_key(&c.to_string());
        // Handle output...
    }
}
```

### Multi-Widget Coordination

```rust
struct MultiWidgetApp {
    widgets: Vec<Widget>,
    focused_index: Option<usize>,
}

impl MultiWidgetApp {
    fn handle_pen_down(&mut self, x: i16, y: i16, ctx: &mut Ctx<'_>) {
        // Find which widget was touched
        for (i, widget) in self.widgets.iter_mut().enumerate() {
            if widget.contains_point(x, y) {
                self.focused_index = Some(i);
                let output = widget.pen_down(x, y);
                if let Some(dirty) = output.dirty {
                    ctx.invalidate(dirty);
                }
                break;
            }
        }
    }
}
```

## Performance Optimization

### Dirty Rectangle Management

1. **Minimize invalidation** - Only invalidate changed areas
2. **Combine rectangles** - Merge adjacent dirty regions when possible
3. **Early returns** - Return early if no changes occurred
4. **State tracking** - Compare new state with previous state

### Memory Efficiency

1. **Stack allocation** - Prefer stack variables over heap allocation
2. **Fixed buffers** - Use fixed-size arrays when bounds are known
3. **Reuse structures** - Clone existing widgets rather than creating new ones
4. **Minimal state** - Store only essential data in widget structs

## Testing Strategies

### Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;

    #[test]
    fn test_widget_interaction() {
        let mut widget = Widget::new(Rectangle::new(Point::new(0, 0), Size::new(100, 50)));
        
        // Test pen down
        let output = widget.pen_down(50, 25);
        assert!(output.dirty.is_some());
        
        // Test pen up
        let output = widget.pen_up();
        assert!(output.dirty.is_some());
    }

    #[test]
    fn test_widget_drawing() {
        let widget = Widget::new(Rectangle::new(Point::new(0, 0), Size::new(100, 50)));
        let mut display = MockDisplay::new();
        
        assert!(widget.draw(&mut display).is_ok());
    }
}
```

### Integration Testing

Create test apps in `assets/scripts/` to verify widget behavior in the runtime:

```rhai
// assets/scripts/widget_test.rhai
let widget = new_widget(#{x: 10, y: 20, w: 100, h: 50});

fn handle_event(event) {
    return widget.handle_event(event);
}

fn draw(canvas) {
    widget.draw(canvas);
}
```

## Integration Checklist

Before adding a new component to `soul-ui`:

### Code Quality
- [ ] Component follows naming conventions (`PascalCase` for types, `snake_case` for functions)
- [ ] All public APIs are documented with `///` comments
- [ ] Error handling uses `Result<(), D::Error>` pattern
- [ ] No `unwrap()` or `panic!()` calls in production code

### Architecture Compliance
- [ ] Component is `no_std + alloc` compatible
- [ ] Uses only `soul-ui` approved dependencies
- [ ] Follows dirty-rectangle invalidation pattern
- [ ] Implements cooperative event handling

### Design Compliance
- [ ] Uses Gray8 color model exclusively
- [ ] Follows Palm aesthetic (rounded buttons, 1px borders)
- [ ] Provides immediate visual feedback
- [ ] Maintains state between interactions

### Accessibility
- [ ] Supports keyboard navigation
- [ ] Provides meaningful `accessibility_info()`
- [ ] Has clear semantic role
- [ ] Visual focus indicators

### Testing
- [ ] Unit tests for core functionality
- [ ] Integration test script in `assets/scripts/`
- [ ] Manual testing on desktop simulator
- [ ] Performance testing with large datasets

### Documentation
- [ ] Module-level documentation explaining purpose
- [ ] Usage examples in documentation
- [ ] Integration guide for app developers
- [ ] Added to `lib.rs` exports and prelude

## File Structure Template

```
crates/soul-ui/src/
├── your_component.rs    # Main implementation
├── lib.rs               # Add exports here
└── tests.rs             # Unit tests (if needed)

assets/scripts/
└── your_component_test.rhai  # Integration test
```

## Export Pattern

Add to `crates/soul-ui/src/lib.rs`:

```rust
pub mod your_component;

// In prelude module:
pub use crate::your_component::{YourComponent, YourComponentOutput};

// In root exports:
pub use your_component::{YourComponent, YourComponentOutput};
```

This guide ensures new components integrate seamlessly with SoulOS architecture while maintaining the project's design philosophy and technical constraints.

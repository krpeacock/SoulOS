use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::Rectangle,
};
use crate::primitives::{button, label, hit_test};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn to_eg_rect(&self) -> Rectangle {
        Rectangle::new(Point::new(self.x, self.y), Size::new(self.w, self.h))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A11yHints {
    pub label: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Navigate(String),
    SaveRecord(u8),
    DeleteRecord,
    CloseApp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Trigger {
    OnTap,
    OnLongPress,
    OnChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    pub trigger: Trigger,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComponentType {
    Button,
    Label,
    TextInput,
    TextArea,
    Canvas,
    Checkbox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    #[serde(default)]
    pub class: String,
    pub type_: ComponentType,
    pub bounds: Rect,
    pub properties: BTreeMap<String, Value>,
    pub a11y: A11yHints,
    #[serde(default)]
    pub interactions: Vec<Interaction>,
    #[serde(default)]
    pub binding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form {
    pub name: String,
    pub components: Vec<Component>,
}

impl Form {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            components: Vec::new(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    pub fn query_selector(&self, selector: &str) -> Option<&Component> {
        if selector.starts_with('#') {
            let id = &selector[1..];
            return self.components.iter().find(|c| c.id == id);
        } else if selector.starts_with('.') {
            let class = &selector[1..];
            return self.components.iter().find(|c| c.class == class);
        } else if selector.starts_with('[') && selector.ends_with(']') {
            let content = &selector[1..selector.len() - 1];
            return self.components.iter().find(|c| {
                c.properties.get("label").and_then(|v| v.as_str()) == Some(content) ||
                c.properties.get("text").and_then(|v| v.as_str()) == Some(content)
            });
        }
        None
    }

    pub fn query_selector_mut(&mut self, selector: &str) -> Option<&mut Component> {
        if selector.starts_with('#') {
            let id = &selector[1..];
            return self.components.iter_mut().find(|c| c.id == id);
        } else if selector.starts_with('.') {
            let class = &selector[1..];
            return self.components.iter_mut().find(|c| c.class == class);
        } else if selector.starts_with('[') && selector.ends_with(']') {
            let content = &selector[1..selector.len() - 1];
            return self.components.iter_mut().find(|c| {
                c.properties.get("label").and_then(|v| v.as_str()) == Some(content) ||
                c.properties.get("text").and_then(|v| v.as_str()) == Some(content)
            });
        }
        None
    }

    pub fn draw<D>(&self, target: &mut D, pressed_id: Option<&str>) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray8>,
    {
        for comp in &self.components {
            let rect = comp.bounds.to_eg_rect();
            let pressed = pressed_id.map(|id| id == comp.id).unwrap_or(false);
            match comp.type_ {
                ComponentType::Button => {
                    if let Some(color_val) = comp.properties.get("color").and_then(|v| v.as_i64()) {
                        let color = Gray8::new(color_val as u8);
                        let style = if pressed {
                            embedded_graphics::primitives::PrimitiveStyle::with_fill(crate::palette::BLACK)
                        } else {
                            embedded_graphics::primitives::PrimitiveStyle::with_fill(color)
                        };
                        rect.into_styled(style).draw(target)?;
                        rect.into_styled(embedded_graphics::primitives::PrimitiveStyle::with_stroke(crate::palette::BLACK, 1))
                            .draw(target)?;
                    } else {
                        let text = comp.properties.get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&comp.id);
                        button(target, rect, text, pressed)?;
                    }
                }
                ComponentType::Label => {
                    let text = comp.properties.get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&comp.id);
                    label(target, rect.top_left, text)?;
                }
                ComponentType::TextInput => {
                    // Draw a placeholder box for TextInput
                    target.fill_contiguous(
                        &rect,
                        core::iter::repeat(crate::palette::WHITE).take(rect.size.width as usize * rect.size.height as usize)
                    )?;
                    rect.into_styled(
                        embedded_graphics::primitives::PrimitiveStyle::with_stroke(crate::palette::BLACK, 1)
                    ).draw(target)?;
                    let text = comp.properties.get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    label(target, rect.top_left + Point::new(2, 2), text)?;
                }
                ComponentType::TextArea => {
                    // Draw a placeholder box for TextArea
                    rect.into_styled(
                        embedded_graphics::primitives::PrimitiveStyle::with_stroke(crate::palette::GRAY, 1)
                    ).draw(target)?;
                }
                ComponentType::Canvas => {
                    rect.into_styled(
                        embedded_graphics::primitives::PrimitiveStyle::with_stroke(crate::palette::BLACK, 1)
                    ).draw(target)?;
                }
                ComponentType::Checkbox => {
                    let checked = comp.properties.get("checked")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let box_rect = Rectangle::new(rect.top_left, Size::new(12, 12));
                    box_rect.into_styled(
                        embedded_graphics::primitives::PrimitiveStyle::with_stroke(crate::palette::BLACK, 1)
                    ).draw(target)?;
                    if checked {
                        // Draw a cross
                        embedded_graphics::primitives::Line::new(
                            box_rect.top_left,
                            box_rect.top_left + Point::new(11, 11)
                        ).into_styled(embedded_graphics::primitives::PrimitiveStyle::with_stroke(crate::palette::BLACK, 1))
                        .draw(target)?;
                        embedded_graphics::primitives::Line::new(
                            box_rect.top_left + Point::new(11, 0),
                            box_rect.top_left + Point::new(0, 11)
                        ).into_styled(embedded_graphics::primitives::PrimitiveStyle::with_stroke(crate::palette::BLACK, 1))
                        .draw(target)?;
                    }
                    let text = comp.properties.get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&comp.id);
                    label(target, rect.top_left + Point::new(16, 1), text)?;
                }
            }
        }
        Ok(())
    }

    pub fn hit_test(&self, x: i16, y: i16) -> Option<&Component> {
        for comp in self.components.iter().rev() {
            if hit_test(&comp.bounds.to_eg_rect(), x, y) {
                return Some(comp);
            }
        }
        None
    }

    pub fn hit_test_mut(&mut self, x: i16, y: i16) -> Option<&mut Component> {
        for comp in self.components.iter_mut().rev() {
            if hit_test(&comp.bounds.to_eg_rect(), x, y) {
                return Some(comp);
            }
        }
        None
    }

    /// Find the first [`Action`] wired to `component_id` for the given `trigger`.
    ///
    /// Returns `None` if the component doesn't exist or has no matching
    /// interaction. The host app executes the returned action; this method
    /// only performs the lookup.
    pub fn dispatch(&self, trigger: Trigger, component_id: &str) -> Option<&Action> {
        self.components
            .iter()
            .find(|c| c.id == component_id)?
            .interactions
            .iter()
            .find(|i| i.trigger == trigger)
            .map(|i| &i.action)
    }

    /// Hit-test at `(x, y)` and look up the [`Trigger::OnTap`] action on
    /// whichever component was under the stylus.
    ///
    /// Returns `None` when no component was hit. When a component is hit the
    /// tuple is `(component, action)` where `action` is `None` if the component
    /// has no `OnTap` interaction — the host app may still want the component
    /// reference for visual feedback or `binding`-driven script dispatch.
    pub fn tap_dispatch(&self, x: i16, y: i16) -> Option<(&Component, Option<&Action>)> {
        let comp = self.hit_test(x, y)?;
        let action = comp
            .interactions
            .iter()
            .find(|i| i.trigger == Trigger::OnTap)
            .map(|i| &i.action);
        Some((comp, action))
    }

    pub fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        self.components.iter().map(|c| soul_core::a11y::A11yNode {
            bounds: c.bounds.to_eg_rect(),
            label: c.a11y.label.clone(),
            role: c.a11y.role.clone(),
        }).collect()
    }
}

//! Accessibility tree types.
//!
//! Every focusable element on screen reports an [`A11yNode`] with four
//! semantic attributes: `label` (the name a screen reader speaks),
//! `role` (button, slider, …), `state` (checked, selected, disabled),
//! and an optional `value` (slider percent, text content, scroll
//! position). [`A11yNode::utterance`] composes these into the canonical
//! string the screen reader vocalizes.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::{self, Write as _};
use embedded_graphics::primitives::Rectangle;

/// The semantic kind of an accessible element.
///
/// Finite roles allow downstream code (rotor, item chooser, harness
/// queries) to filter without string compare. Forms whose JSON role
/// string doesn't map to a known variant land in [`A11yRole::Custom`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A11yRole {
    Button,
    Label,
    Heading,
    Link,
    TextField,
    TextArea,
    Checkbox,
    Slider,
    ScrollBar,
    ListItem,
    MenuItem,
    Image,
    Keyboard,
    KeyboardKey,
    SystemButton,
    Main,
    Custom(String),
}

impl A11yRole {
    /// Canonical lowercase string for this role. Used for utterance
    /// composition and JSON round-tripping.
    pub fn as_str(&self) -> &str {
        match self {
            A11yRole::Button => "button",
            A11yRole::Label => "label",
            A11yRole::Heading => "heading",
            A11yRole::Link => "link",
            A11yRole::TextField => "textbox",
            A11yRole::TextArea => "textarea",
            A11yRole::Checkbox => "checkbox",
            A11yRole::Slider => "slider",
            A11yRole::ScrollBar => "scrollbar",
            A11yRole::ListItem => "listitem",
            A11yRole::MenuItem => "menuitem",
            A11yRole::Image => "image",
            A11yRole::Keyboard => "keyboard",
            A11yRole::KeyboardKey => "key",
            A11yRole::SystemButton => "system_button",
            A11yRole::Main => "main",
            A11yRole::Custom(s) => s.as_str(),
        }
    }

    /// Parse a role string, falling back to [`A11yRole::Custom`] for
    /// unknown values. Accepts both the canonical names returned by
    /// [`A11yRole::as_str`] and a few legacy spellings.
    pub fn from_str(s: &str) -> Self {
        match s {
            "button" => A11yRole::Button,
            "label" => A11yRole::Label,
            "heading" => A11yRole::Heading,
            "link" => A11yRole::Link,
            "textbox" | "textinput" | "textfield" => A11yRole::TextField,
            "textarea" => A11yRole::TextArea,
            "checkbox" => A11yRole::Checkbox,
            "slider" => A11yRole::Slider,
            "scrollbar" => A11yRole::ScrollBar,
            "listitem" => A11yRole::ListItem,
            "menuitem" => A11yRole::MenuItem,
            "image" | "canvas" => A11yRole::Image,
            "keyboard" => A11yRole::Keyboard,
            "key" => A11yRole::KeyboardKey,
            "system_button" => A11yRole::SystemButton,
            "main" => A11yRole::Main,
            other => A11yRole::Custom(other.to_string()),
        }
    }
}

/// Dynamic state flags on an accessible element.
///
/// All fields default to "no state to report"; only widgets whose state
/// matters to a screen reader populate them. `checked` and `expanded`
/// are tri-valued so a button without those concepts (e.g. a plain
/// label) can stay silent on them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct A11yState {
    pub checked: Option<bool>,
    pub selected: bool,
    pub disabled: bool,
    pub expanded: Option<bool>,
}

impl A11yState {
    pub const fn checked(b: bool) -> Self {
        Self {
            checked: Some(b),
            selected: false,
            disabled: false,
            expanded: None,
        }
    }
}

/// One node in an app's accessibility tree.
///
/// Apps return a `Vec<A11yNode>` from `App::a11y_nodes`. The runtime
/// uses these to drive focus traversal, the screen reader, and harness
/// queries.
#[derive(Debug, Clone)]
pub struct A11yNode {
    pub bounds: Rectangle,
    pub label: String,
    pub role: A11yRole,
    pub state: A11yState,
    pub value: Option<String>,
}

impl A11yNode {
    /// Construct a node with default state and no value. The 80%
    /// constructor — most callers only need bounds, label, role.
    pub fn new(bounds: Rectangle, label: impl Into<String>, role: A11yRole) -> Self {
        Self {
            bounds,
            label: label.into(),
            role,
            state: A11yState::default(),
            value: None,
        }
    }

    /// Builder: attach a value (slider %, text content, scroll %).
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Builder: attach state.
    pub fn with_state(mut self, state: A11yState) -> Self {
        self.state = state;
        self
    }

    /// Compose the canonical screen-reader utterance: `label, role[,
    /// state][: value]`. State terms are included only when
    /// non-default; the value is appended after a colon when present.
    pub fn utterance(&self) -> String {
        let mut out = self.label.clone();
        let role_str = self.role.as_str();
        if !role_str.is_empty() {
            let _ = write!(out, ", {role_str}");
        }
        if let Some(true) = self.state.checked {
            let _ = write!(out, ", checked");
        } else if let Some(false) = self.state.checked {
            let _ = write!(out, ", unchecked");
        }
        if self.state.selected {
            let _ = write!(out, ", selected");
        }
        if self.state.disabled {
            let _ = write!(out, ", disabled");
        }
        if let Some(true) = self.state.expanded {
            let _ = write!(out, ", expanded");
        } else if let Some(false) = self.state.expanded {
            let _ = write!(out, ", collapsed");
        }
        if let Some(v) = &self.value {
            if !v.is_empty() {
                let _ = write!(out, ": {v}");
            }
        }
        out
    }
}

impl fmt::Display for A11yRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Manages the accessibility state, including the screen reader's
/// pending speech queue.
#[derive(Default)]
pub struct A11yManager {
    pub enabled: bool,
    pub focus_index: Option<usize>,
    pub pending_speech: Vec<String>,
}

impl A11yManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue `text` for the runtime to hand off to `Platform::speak`
    /// after the current frame's draw.
    pub fn speak(&mut self, text: &str) {
        self.pending_speech.push(text.to_string());
    }

    /// Queue the canonical utterance for `node`.
    pub fn speak_node(&mut self, node: &A11yNode) {
        self.pending_speech.push(node.utterance());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::*;

    fn rect() -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(10, 10))
    }

    #[test]
    fn utterance_label_and_role() {
        let n = A11yNode::new(rect(), "Save", A11yRole::Button);
        assert_eq!(n.utterance(), "Save, button");
    }

    #[test]
    fn utterance_includes_checked_state() {
        let n = A11yNode::new(rect(), "Notify", A11yRole::Checkbox)
            .with_state(A11yState::checked(true));
        assert_eq!(n.utterance(), "Notify, checkbox, checked");
    }

    #[test]
    fn utterance_includes_unchecked_state() {
        let n = A11yNode::new(rect(), "Notify", A11yRole::Checkbox)
            .with_state(A11yState::checked(false));
        assert_eq!(n.utterance(), "Notify, checkbox, unchecked");
    }

    #[test]
    fn utterance_includes_value() {
        let n = A11yNode::new(rect(), "Volume", A11yRole::Slider).with_value("70%");
        assert_eq!(n.utterance(), "Volume, slider: 70%");
    }

    #[test]
    fn utterance_state_and_value_together() {
        let n = A11yNode::new(rect(), "Track 4", A11yRole::ListItem)
            .with_state(A11yState {
                selected: true,
                ..Default::default()
            })
            .with_value("3:42");
        assert_eq!(n.utterance(), "Track 4, listitem, selected: 3:42");
    }

    #[test]
    fn role_from_str_canonical() {
        assert_eq!(A11yRole::from_str("button"), A11yRole::Button);
        assert_eq!(A11yRole::from_str("textbox"), A11yRole::TextField);
        assert_eq!(A11yRole::from_str("textinput"), A11yRole::TextField);
    }

    #[test]
    fn role_from_str_unknown_falls_back_to_custom() {
        match A11yRole::from_str("toolbar") {
            A11yRole::Custom(s) => assert_eq!(s, "toolbar"),
            other => panic!("expected Custom, got {:?}", other),
        }
    }

    #[test]
    fn role_round_trip_through_str() {
        let r = A11yRole::Slider;
        assert_eq!(A11yRole::from_str(r.as_str()), r);
    }
}

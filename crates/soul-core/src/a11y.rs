use alloc::string::{String, ToString};
use alloc::vec::Vec;
use embedded_graphics::primitives::Rectangle;

/// A node in the accessibility tree.
pub struct AccessibleNode {
    pub text: String,
}

/// A node in the accessibility tree (new format).
#[derive(Debug, Clone)]
pub struct A11yNode {
    pub bounds: Rectangle,
    pub label: String,
    pub role: String,
}

/// A trait for UI elements that can be read by a screen reader.
pub trait Accessible {
    fn a11y_nodes(&self, nodes: &mut Vec<AccessibleNode>);
}

/// Manages the accessibility state, including the screen reader.
#[derive(Default)]
pub struct A11yManager {
    pub enabled: bool,
    pub focus_index: Option<usize>,
    pub pending_speech: Vec<String>,
}

impl A11yManager {
    pub fn new() -> Self {
        Self {
            enabled: false,
            focus_index: None,
            pending_speech: Vec::new(),
        }
    }

    pub fn speak(&mut self, text: &str) {
        self.pending_speech.push(text.to_string());
    }

    pub fn log_focused_element(&mut self, element: &impl Accessible) {
        let mut nodes = Vec::new();
        element.a11y_nodes(&mut nodes);
        for node in nodes {
            self.speak(&node.text);
        }
    }
}

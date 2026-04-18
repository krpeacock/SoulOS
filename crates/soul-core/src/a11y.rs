
use alloc::string::String;
use alloc::vec::Vec;

/// A node in the accessibility tree.
pub struct AccessibleNode {
    pub text: String,
}

/// A trait for UI elements that can be read by a screen reader.
pub trait Accessible {
    fn a11y_nodes(&self, nodes: &mut Vec<AccessibleNode>);
}

/// Manages the accessibility state, including the screen reader.
#[derive(Default)]
pub struct A11yManager {
    // For now, this is just a placeholder.
}

impl A11yManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn log_focused_element(&self, element: &impl Accessible) {
        let mut nodes = Vec::new();
        element.a11y_nodes(&mut nodes);
        for _node in nodes {
            // TODO: speak the text
        }
    }
}

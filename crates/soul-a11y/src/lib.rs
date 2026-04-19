#![no_std]

extern crate alloc;

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

# Accessibility in SoulOS

SoulOS is designed to be accessible to everyone. This document outlines the accessibility features available in the operating system and provides guidance for developers on how to make their applications accessible.

## Screen Reader

SoulOS includes a built-in screen reader that can read the content of the screen aloud. The screen reader is activated by a triple-press of the `Home` button.

### Navigating with the Screen Reader

- **Next/Previous Element:** Use the `PageUp` and `PageDown` buttons to move between UI elements.
- **Activate Element:** Press the `Enter` button to activate the currently focused element.

## Developer Guidance

All UI elements in SoulOS applications must be made accessible. This is achieved by implementing the `Accessible` trait for your UI widgets.

### The `Accessible` Trait

The `Accessible` trait is defined in the `soul-a11y` crate and has one method:

```rust
pub trait Accessible {
    fn a11y_nodes(&self, nodes: &mut Vec<AccessibleNode>);
}
```

The `a11y_nodes` method should return a list of `AccessibleNode` objects that represent the accessible content of your widget. Each `AccessibleNode` has a `text` field that contains the text to be read by the screen reader.

### Example

Here is an example of how to implement the `Accessible` trait for a simple button widget:

```rust
use soul_a11y::{Accessible, AccessibleNode};

pub struct Button {
    text: String,
}

impl Accessible for Button {
    fn a11y_nodes(&self, nodes: &mut Vec<AccessibleNode>) {
        nodes.push(AccessibleNode { text: self.text.clone() });
    }
}
```

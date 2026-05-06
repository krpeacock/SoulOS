//! Accessibility tree types.
//!
//! Every focusable element on screen reports an [`A11yNode`] with four
//! semantic attributes: `label` (the name a screen reader speaks),
//! `role` (button, slider, …), `state` (checked, selected, disabled),
//! and an optional `value` (slider percent, text content, scroll
//! position). [`A11yNode::utterance`] composes these into the canonical
//! string the screen reader vocalizes.
//!
//! Focus traversal is owned by [`FocusRing`], a ring buffer over the
//! active app's a11y tree filtered by an optional [`FocusScope`]. The
//! runtime rebuilds the ring once per frame; widgets and the screen
//! reader interact with it through [`A11yManager`].

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

/// Restricts which nodes the [`FocusRing`] traverses.
///
/// The default `Whole` lets focus walk every node the active app
/// exposes. `Modal { rect }` restricts traversal to nodes whose bounds
/// intersect `rect` — apps drawing a modal return that rect from
/// [`crate::App::a11y_focus_scope`] so focus cannot leak behind the
/// modal and silently activate background controls.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FocusScope {
    #[default]
    Whole,
    Modal {
        rect: Rectangle,
    },
}

fn rects_intersect(a: &Rectangle, b: &Rectangle) -> bool {
    let ax1 = a.top_left.x + a.size.width as i32;
    let ay1 = a.top_left.y + a.size.height as i32;
    let bx1 = b.top_left.x + b.size.width as i32;
    let by1 = b.top_left.y + b.size.height as i32;
    a.top_left.x < bx1 && b.top_left.x < ax1 && a.top_left.y < by1 && b.top_left.y < ay1
}

/// A ring buffer of focusable nodes with a current index.
///
/// The runtime owns one [`FocusRing`] inside [`A11yManager`] and
/// rebuilds it once per frame from `App::a11y_nodes` filtered by the
/// app's `a11y_focus_scope`. Identity is preserved across rebuilds:
/// when the previously focused node still exists (matched on `(label,
/// role)`), focus stays on it; otherwise it falls back to index 0.
///
/// `next` and `prev` wrap around. The ring is empty when the active
/// app exposes no focusable nodes.
#[derive(Debug, Default)]
pub struct FocusRing {
    nodes: Vec<A11yNode>,
    index: Option<usize>,
    scope: FocusScope,
    /// Cheap signature of the last build — node count plus the first
    /// and last `(label, role)` pair plus the scope. When unchanged,
    /// `rebuild` skips work.
    signature: u64,
}

impl FocusRing {
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            index: None,
            scope: FocusScope::Whole,
            signature: 0,
        }
    }

    /// Replace the ring's contents from `all_nodes`, filtered by
    /// `scope`. Returns `true` if the ring actually changed; `false`
    /// when the cached signature said the new tree is equivalent.
    ///
    /// Identity preservation: if the currently focused `(label, role)`
    /// pair is present in the new filtered list, focus moves to that
    /// new index. Otherwise focus falls back to `0` (or `None` when
    /// the new list is empty).
    pub fn rebuild(&mut self, all_nodes: Vec<A11yNode>, scope: FocusScope) -> bool {
        let filtered: Vec<A11yNode> = match &scope {
            FocusScope::Whole => all_nodes,
            FocusScope::Modal { rect } => all_nodes
                .into_iter()
                .filter(|n| rects_intersect(&n.bounds, rect))
                .collect(),
        };

        let new_sig = compute_signature(&filtered, &scope);
        if new_sig == self.signature && !filtered.is_empty() {
            // Even when the signature matches, the bounds may have
            // shifted (e.g., a list reflowed). Replace contents but
            // preserve the index — no identity-search work needed.
            self.nodes = filtered;
            self.scope = scope;
            return false;
        }

        let new_index = self
            .current()
            .and_then(|cur| {
                let cur_label = cur.label.clone();
                let cur_role = cur.role.clone();
                filtered
                    .iter()
                    .position(|n| n.label == cur_label && n.role == cur_role)
            })
            .or(if filtered.is_empty() { None } else { Some(0) });

        self.nodes = filtered;
        self.index = new_index;
        self.scope = scope;
        self.signature = new_sig;
        true
    }

    /// The currently focused node, if any.
    pub fn current(&self) -> Option<&A11yNode> {
        self.index.and_then(|i| self.nodes.get(i))
    }

    /// Advance focus by one with wraparound. Returns the new current
    /// node, or `None` when the ring is empty.
    pub fn next(&mut self) -> Option<&A11yNode> {
        if self.nodes.is_empty() {
            return None;
        }
        let i = match self.index {
            Some(i) => (i + 1) % self.nodes.len(),
            None => 0,
        };
        self.index = Some(i);
        self.nodes.get(i)
    }

    /// Move focus back by one with wraparound. Returns the new current
    /// node, or `None` when the ring is empty.
    pub fn prev(&mut self) -> Option<&A11yNode> {
        if self.nodes.is_empty() {
            return None;
        }
        let i = match self.index {
            Some(0) | None => self.nodes.len() - 1,
            Some(i) => i - 1,
        };
        self.index = Some(i);
        self.nodes.get(i)
    }

    /// Borrow the filtered node list.
    pub fn nodes(&self) -> &[A11yNode] {
        &self.nodes
    }

    /// Current index, if focused.
    pub fn index(&self) -> Option<usize> {
        self.index
    }

    /// Active scope.
    pub fn scope(&self) -> &FocusScope {
        &self.scope
    }

    /// Number of focusable nodes in the ring.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Set focus to `i`, clamped to the ring. Returns the new current
    /// node, or `None` when the ring is empty.
    pub fn set_index(&mut self, i: usize) -> Option<&A11yNode> {
        if self.nodes.is_empty() {
            self.index = None;
            return None;
        }
        let clamped = i.min(self.nodes.len() - 1);
        self.index = Some(clamped);
        self.nodes.get(clamped)
    }

    /// Clear focus without dropping the node list.
    pub fn unfocus(&mut self) {
        self.index = None;
    }
}

fn compute_signature(nodes: &[A11yNode], scope: &FocusScope) -> u64 {
    // FNV-1a over: count, scope, then first and last (label, role).
    let mut hash: u64 = 0xcbf29ce484222325;
    let mix = |hash: &mut u64, byte: u8| {
        *hash ^= byte as u64;
        *hash = hash.wrapping_mul(0x100000001b3);
    };
    let mix_bytes = |hash: &mut u64, bytes: &[u8]| {
        for b in bytes {
            mix(hash, *b);
        }
    };
    let count = nodes.len() as u32;
    mix_bytes(&mut hash, &count.to_le_bytes());
    match scope {
        FocusScope::Whole => mix(&mut hash, 0),
        FocusScope::Modal { rect } => {
            mix(&mut hash, 1);
            mix_bytes(&mut hash, &rect.top_left.x.to_le_bytes());
            mix_bytes(&mut hash, &rect.top_left.y.to_le_bytes());
            mix_bytes(&mut hash, &rect.size.width.to_le_bytes());
            mix_bytes(&mut hash, &rect.size.height.to_le_bytes());
        }
    }
    if let Some(first) = nodes.first() {
        mix_bytes(&mut hash, first.label.as_bytes());
        mix_bytes(&mut hash, first.role.as_str().as_bytes());
    }
    if nodes.len() > 1 {
        if let Some(last) = nodes.last() {
            mix_bytes(&mut hash, last.label.as_bytes());
            mix_bytes(&mut hash, last.role.as_str().as_bytes());
        }
    }
    hash
}

/// Manages the accessibility state, including the focus ring, the
/// screen reader's pending speech queue, and global TTS preferences
/// (rate, punctuation). Phase 4 will hydrate `rate_wpm` and
/// `punctuation` from per-app settings; Phase 3a uses them with their
/// defaults.
pub struct A11yManager {
    pub enabled: bool,
    pub focus: FocusRing,
    pub pending_speech: Vec<String>,
    /// Words-per-minute for the screen reader. Default
    /// [`soul_hal::SpeechRequest::DEFAULT_RATE_WPM`].
    pub rate_wpm: u16,
    /// Punctuation verbosity for the screen reader.
    pub punctuation: soul_hal::Punctuation,
}

impl Default for A11yManager {
    fn default() -> Self {
        Self {
            enabled: false,
            focus: FocusRing::new(),
            pending_speech: Vec::new(),
            rate_wpm: soul_hal::SpeechRequest::DEFAULT_RATE_WPM,
            punctuation: soul_hal::Punctuation::Some,
        }
    }
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

    fn node(label: &str, role: A11yRole, x: i32, y: i32, w: u32, h: u32) -> A11yNode {
        A11yNode::new(
            Rectangle::new(Point::new(x, y), Size::new(w, h)),
            label,
            role,
        )
    }

    #[test]
    fn ring_starts_empty_and_unfocused() {
        let r = FocusRing::new();
        assert!(r.is_empty());
        assert_eq!(r.index(), None);
        assert!(r.current().is_none());
    }

    #[test]
    fn ring_rebuild_focuses_first_node_when_starting_empty() {
        let mut r = FocusRing::new();
        r.rebuild(
            alloc::vec![
                node("Save", A11yRole::Button, 0, 0, 10, 10),
                node("Cancel", A11yRole::Button, 0, 10, 10, 10),
            ],
            FocusScope::Whole,
        );
        assert_eq!(r.index(), Some(0));
        assert_eq!(r.current().unwrap().label, "Save");
    }

    #[test]
    fn ring_next_and_prev_wrap_around() {
        let mut r = FocusRing::new();
        r.rebuild(
            alloc::vec![
                node("A", A11yRole::Button, 0, 0, 10, 10),
                node("B", A11yRole::Button, 0, 10, 10, 10),
                node("C", A11yRole::Button, 0, 20, 10, 10),
            ],
            FocusScope::Whole,
        );
        assert_eq!(r.next().unwrap().label, "B");
        assert_eq!(r.next().unwrap().label, "C");
        assert_eq!(r.next().unwrap().label, "A"); // wraparound
        assert_eq!(r.prev().unwrap().label, "C"); // wraparound back
        assert_eq!(r.prev().unwrap().label, "B");
    }

    #[test]
    fn ring_next_on_empty_returns_none() {
        let mut r = FocusRing::new();
        assert!(r.next().is_none());
        assert!(r.prev().is_none());
    }

    #[test]
    fn ring_rebuild_preserves_focus_by_label_and_role() {
        let mut r = FocusRing::new();
        r.rebuild(
            alloc::vec![
                node("Save", A11yRole::Button, 0, 0, 10, 10),
                node("Cancel", A11yRole::Button, 0, 10, 10, 10),
            ],
            FocusScope::Whole,
        );
        r.next(); // focus moves to Cancel
        assert_eq!(r.current().unwrap().label, "Cancel");

        // Rebuild with the same nodes — focus should stay on Cancel
        r.rebuild(
            alloc::vec![
                node("Save", A11yRole::Button, 0, 0, 10, 10),
                node("Cancel", A11yRole::Button, 0, 10, 10, 10),
            ],
            FocusScope::Whole,
        );
        assert_eq!(r.current().unwrap().label, "Cancel");

        // Insert a new first node — focus should still be on Cancel
        r.rebuild(
            alloc::vec![
                node("Reset", A11yRole::Button, 0, 0, 10, 10),
                node("Save", A11yRole::Button, 0, 10, 10, 10),
                node("Cancel", A11yRole::Button, 0, 20, 10, 10),
            ],
            FocusScope::Whole,
        );
        assert_eq!(r.current().unwrap().label, "Cancel");
    }

    #[test]
    fn ring_rebuild_falls_back_to_zero_when_focused_node_disappears() {
        let mut r = FocusRing::new();
        r.rebuild(
            alloc::vec![
                node("Save", A11yRole::Button, 0, 0, 10, 10),
                node("Cancel", A11yRole::Button, 0, 10, 10, 10),
            ],
            FocusScope::Whole,
        );
        r.next(); // focus = Cancel
        r.rebuild(
            alloc::vec![node("Save", A11yRole::Button, 0, 0, 10, 10)],
            FocusScope::Whole,
        );
        assert_eq!(r.current().unwrap().label, "Save");
        assert_eq!(r.index(), Some(0));
    }

    #[test]
    fn ring_rebuild_clears_index_when_new_list_is_empty() {
        let mut r = FocusRing::new();
        r.rebuild(
            alloc::vec![node("Save", A11yRole::Button, 0, 0, 10, 10)],
            FocusScope::Whole,
        );
        r.rebuild(alloc::vec![], FocusScope::Whole);
        assert!(r.current().is_none());
        assert_eq!(r.index(), None);
    }

    #[test]
    fn ring_modal_scope_filters_to_intersecting_nodes() {
        let mut r = FocusRing::new();
        let modal = Rectangle::new(Point::new(50, 50), Size::new(100, 100));
        r.rebuild(
            alloc::vec![
                node("Behind", A11yRole::Button, 0, 0, 10, 10), // outside modal
                node("Inside1", A11yRole::Button, 60, 60, 20, 20), // inside
                node("Inside2", A11yRole::Button, 100, 100, 20, 20), // inside
                node("OutsideToo", A11yRole::Button, 200, 200, 10, 10), // outside
            ],
            FocusScope::Modal { rect: modal },
        );
        assert_eq!(r.len(), 2);
        let labels: alloc::vec::Vec<_> = r.nodes().iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, alloc::vec!["Inside1", "Inside2"]);

        // next/prev never escapes the modal scope
        r.next();
        assert_eq!(r.current().unwrap().label, "Inside2");
        r.next();
        assert_eq!(r.current().unwrap().label, "Inside1"); // wraps inside scope only
    }

    #[test]
    fn ring_set_index_clamps_and_focuses() {
        let mut r = FocusRing::new();
        r.rebuild(
            alloc::vec![
                node("A", A11yRole::Button, 0, 0, 10, 10),
                node("B", A11yRole::Button, 0, 10, 10, 10),
            ],
            FocusScope::Whole,
        );
        let n = r.set_index(99).unwrap();
        assert_eq!(n.label, "B"); // clamped to last
        assert_eq!(r.index(), Some(1));
    }

    #[test]
    fn ring_signature_skips_redundant_rebuilds() {
        let mut r = FocusRing::new();
        r.rebuild(
            alloc::vec![node("Save", A11yRole::Button, 0, 0, 10, 10)],
            FocusScope::Whole,
        );
        // Rebuild with identical inputs — returns false (no structural change).
        let changed = r.rebuild(
            alloc::vec![node("Save", A11yRole::Button, 0, 0, 10, 10)],
            FocusScope::Whole,
        );
        assert!(!changed);
        assert_eq!(r.current().unwrap().label, "Save");
    }
}

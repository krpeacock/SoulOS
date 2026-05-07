//! The Item Chooser overlay — a single-text-input modal that lists
//! every focusable node on the current screen and jumps focus on
//! selection.
//!
//! Triggered by the Menu hard button while a11y mode is on. The
//! chooser is the Palm-graffiti analogue of VoiceOver's Item Chooser
//! (and TalkBack's Screen Search): instead of swiping past 39
//! widgets to reach the 40th, the user types a few letters and
//! lands on the match.
//!
//! Architecturally the chooser is a Host-level modal overlay, not a
//! full app pushed onto the navigation stack. That keeps state
//! contained — the chooser owns the snapshot of underlying nodes
//! and the query buffer; nothing else changes — and fits the focus-
//! ring rebuild model from Phase 2: while the chooser is open,
//! `Host::a11y_nodes` returns the chooser's own list and
//! `Host::a11y_focus_scope` returns its rect, so the existing modal
//! scope keeps focus inside it.

use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use soul_core::{
    a11y::{A11yNode, A11yRole},
    KeyCode, SCREEN_WIDTH, APP_HEIGHT,
};
use soul_ui::{BLACK, WHITE};

const MARGIN: i32 = 8;
const HEADER_H: i32 = 20;
const QUERY_H: i32 = 16;
const ROW_H: i32 = 12;

/// Visible rows in the result list. Tuned to fit between the query
/// bar and the bottom of the overlay at our 240×304 app area.
const VISIBLE_ROWS: usize = 16;

/// Result of dispatching one event into the chooser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChooserAction {
    /// Nothing user-visible happened; redraw not required.
    NoOp,
    /// The chooser's content changed; the host should invalidate its
    /// rect.
    Repaint,
    /// User picked a node. The Host pops the chooser and sets focus
    /// to the first node matching this `(label, role)` pair.
    Select { label: String, role: A11yRole },
    /// User dismissed the chooser without selecting. Focus returns
    /// to its prior position.
    Dismiss,
}

/// Modal overlay listing focusable nodes filtered by a live query.
pub struct ItemChooser {
    /// Snapshot of underlying-screen a11y nodes at open time.
    snapshot: Vec<A11yNode>,
    /// User-typed search filter. Matched as case-insensitive
    /// substring against `label`.
    query: String,
    /// Index into the *filtered* list, clamped on every query change.
    selected: usize,
    /// Where the overlay sits on screen — identical for every open;
    /// stored so `Host::a11y_focus_scope` can return it without
    /// recomputing.
    bounds: Rectangle,
}

impl ItemChooser {
    /// Open with a fresh snapshot of `nodes`. The Host typically
    /// passes the result of `build_a11y_tree()` (which already
    /// includes the system Home/Menu strip nodes — useful since the
    /// user can type "Home" to jump to the home button).
    pub fn open(nodes: Vec<A11yNode>) -> Self {
        // The overlay covers most of the app area, leaving a thin
        // border of underlying content visible so the user has a
        // sense of context.
        let bounds = Rectangle::new(
            Point::new(MARGIN, MARGIN),
            Size::new(
                (SCREEN_WIDTH as i32 - 2 * MARGIN) as u32,
                (APP_HEIGHT as i32 - 2 * MARGIN) as u32,
            ),
        );
        Self {
            snapshot: nodes,
            query: String::new(),
            selected: 0,
            bounds,
        }
    }

    /// Bounds in virtual-screen coordinates. Used by
    /// `Host::a11y_focus_scope` to confine the focus ring to the
    /// chooser while it's open.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// The full underlying snapshot, unfiltered. Tests and the Host
    /// use this to learn the size of the chooser's universe.
    pub fn snapshot(&self) -> &[A11yNode] {
        &self.snapshot
    }

    /// Current filter text (live as the user types).
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Indices into `snapshot` for nodes matching the current query.
    /// Match is case-insensitive substring against `label`.
    fn filtered_indices(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..self.snapshot.len()).collect();
        }
        let needle = self.query.to_lowercase();
        self.snapshot
            .iter()
            .enumerate()
            .filter(|(_, n)| n.label.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect()
    }

    /// The currently-highlighted node, if any matches.
    pub fn selected_node(&self) -> Option<&A11yNode> {
        let idx = *self.filtered_indices().get(self.selected)?;
        self.snapshot.get(idx)
    }

    /// Replace the query and reset the selection cursor. Returns
    /// `true` if the query actually changed.
    pub fn set_query(&mut self, q: impl Into<String>) -> bool {
        let new_q = q.into();
        if new_q == self.query {
            return false;
        }
        self.query = new_q;
        self.selected = 0;
        true
    }

    /// Dispatch a single input event. Apps don't talk to the
    /// chooser directly — only [`crate::Host`] does, when it has
    /// taken ownership of the event stream.
    pub fn handle_key(&mut self, key: KeyCode) -> ChooserAction {
        match key {
            KeyCode::Char(c) if !c.is_control() => {
                self.query.push(c);
                self.selected = 0;
                ChooserAction::Repaint
            }
            KeyCode::Backspace => {
                if self.query.pop().is_some() {
                    self.selected = 0;
                    ChooserAction::Repaint
                } else {
                    ChooserAction::NoOp
                }
            }
            KeyCode::ArrowUp => {
                if self.selected > 0 {
                    self.selected -= 1;
                    ChooserAction::Repaint
                } else {
                    ChooserAction::NoOp
                }
            }
            KeyCode::ArrowDown => {
                let n = self.filtered_indices().len();
                if self.selected + 1 < n {
                    self.selected += 1;
                    ChooserAction::Repaint
                } else {
                    ChooserAction::NoOp
                }
            }
            KeyCode::Enter => self
                .selected_node()
                .map(|n| ChooserAction::Select {
                    label: n.label.clone(),
                    role: n.role.clone(),
                })
                .unwrap_or(ChooserAction::NoOp),
            _ => ChooserAction::NoOp,
        }
    }

    /// PageUp / PageDown navigation for hard-button users.
    pub fn page_up(&mut self) -> ChooserAction {
        let new = self.selected.saturating_sub(VISIBLE_ROWS);
        if new == self.selected {
            ChooserAction::NoOp
        } else {
            self.selected = new;
            ChooserAction::Repaint
        }
    }
    pub fn page_down(&mut self) -> ChooserAction {
        let n = self.filtered_indices().len();
        if n == 0 {
            return ChooserAction::NoOp;
        }
        let new = (self.selected + VISIBLE_ROWS).min(n - 1);
        if new == self.selected {
            ChooserAction::NoOp
        } else {
            self.selected = new;
            ChooserAction::Repaint
        }
    }

    /// Render the overlay into `canvas`. Drawn after the host's app
    /// + system strip so it sits visually on top.
    pub fn draw<D>(&self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        // White background with a 2px black border.
        let _ = self
            .bounds
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(WHITE)
                    .stroke_color(BLACK)
                    .stroke_width(2)
                    .build(),
            )
            .draw(canvas);

        let inner_x = self.bounds.top_left.x + 4;
        let inner_y = self.bounds.top_left.y + 2;

        // Header
        let header_style = MonoTextStyle::new(&FONT_6X10, BLACK);
        let _ = Text::with_baseline(
            "Choose item",
            Point::new(inner_x, inner_y + 2),
            header_style,
            Baseline::Top,
        )
        .draw(canvas);

        // Query bar — black border, query text inside.
        let query_rect = Rectangle::new(
            Point::new(inner_x, inner_y + HEADER_H),
            Size::new(
                (self.bounds.size.width as i32 - 8) as u32,
                QUERY_H as u32,
            ),
        );
        let _ = query_rect
            .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
            .draw(canvas);
        let query_label: String = if self.query.is_empty() {
            "type to filter".into()
        } else {
            self.query.clone()
        };
        let _ = Text::with_baseline(
            &query_label,
            Point::new(query_rect.top_left.x + 3, query_rect.top_left.y + 3),
            header_style,
            Baseline::Top,
        )
        .draw(canvas);

        // Filtered results — windowed around `selected` so it stays
        // visible without scrolling the cursor offscreen.
        let filtered = self.filtered_indices();
        if filtered.is_empty() {
            let _ = Text::with_baseline(
                "(no matches)",
                Point::new(inner_x, query_rect.top_left.y + QUERY_H + 6),
                header_style,
                Baseline::Top,
            )
            .draw(canvas);
            return;
        }

        let scroll_top = scroll_offset(self.selected, filtered.len());
        let list_top = query_rect.top_left.y + QUERY_H + 4;
        for row in 0..VISIBLE_ROWS.min(filtered.len() - scroll_top) {
            let snap_idx = filtered[scroll_top + row];
            let node = &self.snapshot[snap_idx];
            let highlighted = scroll_top + row == self.selected;
            let row_rect = Rectangle::new(
                Point::new(inner_x, list_top + row as i32 * ROW_H),
                Size::new(
                    (self.bounds.size.width as i32 - 8) as u32,
                    ROW_H as u32,
                ),
            );
            let (fill, fg) = if highlighted {
                (BLACK, WHITE)
            } else {
                (WHITE, BLACK)
            };
            let _ = row_rect
                .into_styled(PrimitiveStyle::with_fill(fill))
                .draw(canvas);
            let style = MonoTextStyle::new(&FONT_6X10, fg);
            let display: String = format!("{} ({})", node.label, node.role.as_str());
            let _ = Text::with_baseline(
                &display,
                Point::new(row_rect.top_left.x + 3, row_rect.top_left.y + 1),
                style,
                Baseline::Top,
            )
            .draw(canvas);
        }
    }

    /// Accessibility tree for the chooser itself. The query bar
    /// counts as a `TextField` whose value is the live query; each
    /// filtered row becomes a `ListItem`. The Phase 2 focus ring
    /// rebuilds against this when the chooser is the active a11y
    /// surface, and `selected_node` keeps the highlighted row in
    /// sync with focus when the screen reader speaks each row.
    pub fn a11y_nodes(&self) -> Vec<A11yNode> {
        let mut out = Vec::new();
        let inner_x = self.bounds.top_left.x + 4;
        let inner_y = self.bounds.top_left.y + 2;
        let query_rect = Rectangle::new(
            Point::new(inner_x, inner_y + HEADER_H),
            Size::new(
                (self.bounds.size.width as i32 - 8) as u32,
                QUERY_H as u32,
            ),
        );
        let mut query_node = A11yNode::new(query_rect, "Choose item", A11yRole::TextField);
        if !self.query.is_empty() {
            query_node = query_node.with_value(self.query.clone());
        }
        out.push(query_node);

        let filtered = self.filtered_indices();
        let scroll_top = scroll_offset(self.selected, filtered.len());
        let list_top = query_rect.top_left.y + QUERY_H + 4;
        for row in 0..VISIBLE_ROWS.min(filtered.len().saturating_sub(scroll_top)) {
            let snap_idx = filtered[scroll_top + row];
            let node = &self.snapshot[snap_idx];
            let row_rect = Rectangle::new(
                Point::new(inner_x, list_top + row as i32 * ROW_H),
                Size::new(
                    (self.bounds.size.width as i32 - 8) as u32,
                    ROW_H as u32,
                ),
            );
            let mut item = A11yNode::new(row_rect, node.label.clone(), A11yRole::ListItem);
            item = item.with_value(format!("of role {}", node.role.as_str()));
            if scroll_top + row == self.selected {
                item.state.selected = true;
            }
            out.push(item);
        }
        out
    }
}

fn scroll_offset(selected: usize, total: usize) -> usize {
    if total <= VISIBLE_ROWS {
        0
    } else if selected < VISIBLE_ROWS / 2 {
        0
    } else if selected + VISIBLE_ROWS / 2 >= total {
        total - VISIBLE_ROWS
    } else {
        selected - VISIBLE_ROWS / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::primitives::Rectangle;

    fn rect() -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(10, 10))
    }
    fn node(label: &str, role: A11yRole) -> A11yNode {
        A11yNode::new(rect(), label, role)
    }

    fn fixture() -> ItemChooser {
        ItemChooser::open(vec![
            node("Save", A11yRole::Button),
            node("Cancel", A11yRole::Button),
            node("Notify", A11yRole::Checkbox),
            node("Notes", A11yRole::Heading),
        ])
    }

    #[test]
    fn empty_query_lists_all_nodes() {
        let c = fixture();
        assert_eq!(c.filtered_indices().len(), 4);
        assert_eq!(c.selected_node().unwrap().label, "Save");
    }

    #[test]
    fn substring_query_is_case_insensitive() {
        let mut c = fixture();
        c.set_query("not");
        let labels: Vec<&str> = c
            .filtered_indices()
            .iter()
            .map(|&i| c.snapshot[i].label.as_str())
            .collect();
        assert_eq!(labels, vec!["Notify", "Notes"]);
    }

    #[test]
    fn typing_filters_live() {
        let mut c = fixture();
        assert_eq!(c.handle_key(KeyCode::Char('S')), ChooserAction::Repaint);
        assert_eq!(c.selected_node().unwrap().label, "Save");
        c.handle_key(KeyCode::Char('a'));
        assert_eq!(c.selected_node().unwrap().label, "Save");
        // Backspace returns to "S"
        c.handle_key(KeyCode::Backspace);
        assert_eq!(c.query(), "S");
    }

    #[test]
    fn enter_emits_select_with_label_and_role() {
        let mut c = fixture();
        c.set_query("Notify");
        let action = c.handle_key(KeyCode::Enter);
        assert_eq!(
            action,
            ChooserAction::Select {
                label: "Notify".into(),
                role: A11yRole::Checkbox,
            }
        );
    }

    #[test]
    fn arrow_keys_move_selection_within_bounds() {
        let mut c = fixture();
        assert_eq!(c.selected_node().unwrap().label, "Save");
        c.handle_key(KeyCode::ArrowDown);
        assert_eq!(c.selected_node().unwrap().label, "Cancel");
        c.handle_key(KeyCode::ArrowUp);
        assert_eq!(c.selected_node().unwrap().label, "Save");
        // ArrowUp at top is a NoOp.
        assert_eq!(c.handle_key(KeyCode::ArrowUp), ChooserAction::NoOp);
    }

    #[test]
    fn enter_with_no_match_is_noop() {
        let mut c = fixture();
        c.set_query("xyz_does_not_exist");
        assert_eq!(c.filtered_indices().len(), 0);
        assert_eq!(c.handle_key(KeyCode::Enter), ChooserAction::NoOp);
    }

    #[test]
    fn changing_query_resets_selection_to_zero() {
        let mut c = fixture();
        c.handle_key(KeyCode::ArrowDown); // selected = 1
        c.set_query("not");
        // selection reset, points at first match ("Notify")
        assert_eq!(c.selected_node().unwrap().label, "Notify");
    }

    #[test]
    fn a11y_tree_includes_query_field_and_filtered_rows() {
        let mut c = fixture();
        c.set_query("not");
        let nodes = c.a11y_nodes();
        // Query field + 2 filtered rows.
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].role, A11yRole::TextField);
        assert_eq!(nodes[0].value.as_deref(), Some("not"));
        assert!(nodes[1..].iter().all(|n| n.role == A11yRole::ListItem));
        // First row is selected.
        assert!(nodes[1].state.selected);
        assert!(!nodes[2].state.selected);
    }
}

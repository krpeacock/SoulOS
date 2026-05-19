//! Palm-style command menu sheet.
//!
//! A data-driven overlay that appears below the title bar when `Event::Menu`
//! fires. The item list is borrowed per call — nothing is stored inside
//! `MenuSheet` itself, so the app's command table stays a `'static` slice
//! and the menu allocates nothing.
//!
//! # Usage
//!
//! ```ignore
//! // App state:
//! menu: MenuSheet,
//!
//! // In App::handle:
//! let out = self.menu.handle(&event, ITEMS);
//! if let Some(r) = out.dirty { ctx.invalidate(r); }
//! if let Some(idx) = out.committed { return self.run_cmd(idx, ctx); }
//! if self.menu.is_open() { return; }   // absorb all other events
//! // ... normal app event handling
//!
//! // In App::draw (after normal content, before returning):
//! self.menu.draw(canvas, ITEMS);
//! ```
//!
//! # Layout
//!
//! The sheet is flush with the left and right screen edges and is anchored
//! immediately below the title bar. Items are 22 px tall with 1 px gaps.
//! Up to 12 items fit before scrolling is needed; navigation wraps via
//! `PageUp`/`PageDown` or `ArrowUp`/`ArrowDown`.

use alloc::{format, vec::Vec};
use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use soul_core::{
    a11y::{A11yNode, A11yRole, A11yState},
    Event, HardButton, KeyCode, SCREEN_WIDTH,
};

use crate::{
    font_aa,
    palette::{BLACK, GRAY, WHITE},
    primitives::{button, hit_test, TITLE_BAR_H},
};

// --- Layout constants -------------------------------------------------------
//
// Derivation of MAX_VISIBLE:
//   available = APP_HEIGHT(304) - TITLE_BAR_H(15) - 2*BORDER(2) - 2*PAD(4) = 283 px
//   per slot  = ITEM_H(22) + ITEM_GAP(1) = 23 px
//   n items take n*23 - 1 px (no trailing gap) ≤ 283 → n ≤ 12.3 → 12.

const ITEM_H: i32 = 22;
const ITEM_GAP: i32 = 1;
const ITEM_SLOT: i32 = ITEM_H + ITEM_GAP;
const SHEET_BORDER: i32 = 1;
const SHEET_PAD: i32 = 2;
const ITEM_INSET: i32 = 3;
const SHORTCUT_SIZE: f32 = 8.0;
const LABEL_SIZE: f32 = 9.0;
const MAX_VISIBLE: usize = 12;

// --- MenuItem ---------------------------------------------------------------

/// A single command offered by a [`MenuSheet`].
///
/// The slice of items is borrowed per `handle`/`draw` call.  Declare
/// item tables as `'static` slices:
///
/// ```ignore
/// const ITEMS: &[MenuItem<'static>] = &[
///     MenuItem::new("Cut"),
///     MenuItem::with_shortcut("Copy", 'C'),
///     MenuItem::with_shortcut("Paste", 'V'),
///     MenuItem::disabled("Undo"),
/// ];
/// ```
pub struct MenuItem<'a> {
    pub label: &'a str,
    /// Palm-style single-character shortcut, ASCII only.  Displayed as "/X"
    /// inside the item and can be triggered by typing `X` while the menu is
    /// open.
    pub shortcut: Option<char>,
    pub enabled: bool,
}

impl<'a> MenuItem<'a> {
    pub const fn new(label: &'a str) -> Self {
        Self { label, shortcut: None, enabled: true }
    }

    pub const fn with_shortcut(label: &'a str, shortcut: char) -> Self {
        Self { label, shortcut: Some(shortcut), enabled: true }
    }

    pub const fn disabled(label: &'a str) -> Self {
        Self { label, shortcut: None, enabled: false }
    }
}

// --- MenuOutput -------------------------------------------------------------

/// Result returned by [`MenuSheet::handle`] on every event.
pub struct MenuOutput {
    /// Index into the item slice the caller passed, set when the user commits
    /// a selection this event.  The menu is already closed when this fires.
    pub committed: Option<usize>,
    /// Screen rectangle that changed and needs repainting.  Pass to
    /// `ctx.invalidate` — never `invalidate_all` — to keep e-ink redraws
    /// minimal.
    pub dirty: Option<Rectangle>,
}

// --- MenuSheet --------------------------------------------------------------

/// Stateful command-menu overlay.  Borrows the item slice on every call;
/// stores only open/close state and navigation position.
pub struct MenuSheet {
    open: bool,
    scroll_top: usize,
    selected: usize,
    touch_item: Option<usize>,
}

impl MenuSheet {
    pub const fn new() -> Self {
        Self { open: false, scroll_top: 0, selected: 0, touch_item: None }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Handle a single event.  Call this before any app-specific event
    /// handling; when `is_open()` returns `true` after this call the app
    /// should suppress its own pen/key handling.
    pub fn handle(&mut self, event: &Event, items: &[MenuItem<'_>]) -> MenuOutput {
        if !self.open {
            if matches!(event, Event::Menu) {
                let dirty = self.do_open(items);
                return MenuOutput { committed: None, dirty: Some(dirty) };
            }
            return MenuOutput { committed: None, dirty: None };
        }

        match event {
            // Re-press Menu or app stop: close without committing.
            Event::Menu | Event::AppStop => {
                let r = self.sheet_rect(items.len());
                self.do_close();
                MenuOutput { committed: None, dirty: Some(r) }
            }

            Event::PenDown { x, y } => {
                let hit = self.hit_item(*x, *y, items.len());
                let old = self.touch_item;
                self.touch_item = hit;
                MenuOutput { committed: None, dirty: self.items_dirty(old, hit, items.len()) }
            }

            Event::PenMove { x, y } => {
                let hit = self.hit_item(*x, *y, items.len());
                if hit == self.touch_item {
                    return MenuOutput { committed: None, dirty: None };
                }
                let old = self.touch_item;
                self.touch_item = hit;
                MenuOutput { committed: None, dirty: self.items_dirty(old, hit, items.len()) }
            }

            Event::PenUp { x, y } => {
                let down = self.touch_item.take();
                let up = self.hit_item(*x, *y, items.len());

                // Tap outside the sheet: close without committing.
                if up.is_none() {
                    let r = self.sheet_rect(items.len());
                    self.do_close();
                    return MenuOutput { committed: None, dirty: Some(r) };
                }

                // Lift on the same enabled item as press: commit.
                if let (Some(d), Some(u)) = (down, up) {
                    if d == u && items.get(d).map_or(false, |it| it.enabled) {
                        let r = self.sheet_rect(items.len());
                        self.do_close();
                        return MenuOutput { committed: Some(d), dirty: Some(r) };
                    }
                }

                // Drag-off: clear touch highlight.
                MenuOutput { committed: None, dirty: self.items_dirty(down, None, items.len()) }
            }

            Event::ButtonDown(HardButton::PageDown) | Event::Key(KeyCode::ArrowDown) => {
                let old = self.selected;
                self.selected = next_enabled(items, self.selected);
                self.ensure_visible(items.len());
                MenuOutput { committed: None, dirty: self.items_dirty(Some(old), Some(self.selected), items.len()) }
            }

            Event::ButtonDown(HardButton::PageUp) | Event::Key(KeyCode::ArrowUp) => {
                let old = self.selected;
                self.selected = prev_enabled(items, self.selected);
                self.ensure_visible(items.len());
                MenuOutput { committed: None, dirty: self.items_dirty(Some(old), Some(self.selected), items.len()) }
            }

            Event::ButtonDown(HardButton::AppA) | Event::Key(KeyCode::Enter) => {
                if items.get(self.selected).map_or(false, |it| it.enabled) {
                    let idx = self.selected;
                    let r = self.sheet_rect(items.len());
                    self.do_close();
                    MenuOutput { committed: Some(idx), dirty: Some(r) }
                } else {
                    MenuOutput { committed: None, dirty: None }
                }
            }

            Event::Key(KeyCode::Char(c)) => {
                let upper = c.to_ascii_uppercase();
                let found = items.iter().enumerate().find(|(_, it)| {
                    it.enabled && it.shortcut.map_or(false, |s| s.to_ascii_uppercase() == upper)
                });
                if let Some((idx, _)) = found {
                    let r = self.sheet_rect(items.len());
                    self.do_close();
                    MenuOutput { committed: Some(idx), dirty: Some(r) }
                } else {
                    MenuOutput { committed: None, dirty: None }
                }
            }

            _ => MenuOutput { committed: None, dirty: None },
        }
    }

    /// Draw the sheet overlay.  No-op when closed.  Call after all other app
    /// content so the sheet renders on top.
    pub fn draw<D: DrawTarget<Color = Gray8>>(&self, canvas: &mut D, items: &[MenuItem<'_>]) {
        if !self.open {
            return;
        }
        let n = items.len();
        let sheet = self.sheet_rect(n);
        let visible = MAX_VISIBLE.min(n);

        // Background + border.
        let _ = sheet.into_styled(PrimitiveStyle::with_fill(WHITE)).draw(canvas);
        let _ = sheet.into_styled(PrimitiveStyle::with_stroke(BLACK, 1)).draw(canvas);

        for di in 0..visible {
            let abs = self.scroll_top + di;
            if abs >= n {
                break;
            }
            let item = &items[abs];
            let rect = self.item_rect(di, &sheet);
            let pressed = self.touch_item == Some(abs)
                || (self.touch_item.is_none() && self.selected == abs);

            if item.enabled {
                let _ = button(canvas, rect, item.label, pressed);
                if let Some(sc) = item.shortcut {
                    self.draw_shortcut(canvas, sc, rect, pressed);
                }
            } else {
                self.draw_disabled_item(canvas, rect, item.label);
            }
        }
    }

    /// Return accessible nodes for all visible items when open.
    pub fn a11y_nodes(&self, items: &[MenuItem<'_>]) -> Vec<A11yNode> {
        if !self.open {
            return Vec::new();
        }
        let n = items.len();
        let sheet = self.sheet_rect(n);
        let visible = MAX_VISIBLE.min(n);
        (0..visible)
            .filter_map(|di| {
                let abs = self.scroll_top + di;
                items.get(abs).map(|item| {
                    let state = A11yState { disabled: !item.enabled, ..A11yState::default() };
                    A11yNode::new(self.item_rect(di, &sheet), item.label, A11yRole::MenuItem)
                        .with_state(state)
                })
            })
            .collect()
    }

    // --- Private helpers ----------------------------------------------------

    fn do_open(&mut self, items: &[MenuItem<'_>]) -> Rectangle {
        self.open = true;
        self.scroll_top = 0;
        self.selected = first_enabled(items, 0);
        self.touch_item = None;
        self.sheet_rect(items.len())
    }

    fn do_close(&mut self) {
        self.open = false;
        self.touch_item = None;
    }

    fn sheet_rect(&self, n_items: usize) -> Rectangle {
        let visible = MAX_VISIBLE.min(n_items);
        // n slots = n * ITEM_H + (n-1) * ITEM_GAP = n * ITEM_SLOT - ITEM_GAP
        let content_h = if visible == 0 {
            0
        } else {
            visible as i32 * ITEM_SLOT - ITEM_GAP
        };
        let h = (2 * SHEET_BORDER + 2 * SHEET_PAD + content_h).max(0) as u32;
        Rectangle::new(Point::new(0, TITLE_BAR_H as i32), Size::new(SCREEN_WIDTH as u32, h))
    }

    fn item_rect(&self, display_pos: usize, sheet: &Rectangle) -> Rectangle {
        let x = sheet.top_left.x + SHEET_BORDER + ITEM_INSET;
        let y = sheet.top_left.y
            + SHEET_BORDER
            + SHEET_PAD
            + display_pos as i32 * ITEM_SLOT;
        let w = (sheet.size.width as i32 - 2 * (SHEET_BORDER + ITEM_INSET)).max(0) as u32;
        Rectangle::new(Point::new(x, y), Size::new(w, ITEM_H as u32))
    }

    fn hit_item(&self, x: i16, y: i16, n_items: usize) -> Option<usize> {
        let sheet = self.sheet_rect(n_items);
        if !hit_test(&sheet, x, y) {
            return None;
        }
        let visible = MAX_VISIBLE.min(n_items);
        for di in 0..visible {
            let abs = self.scroll_top + di;
            if abs >= n_items {
                break;
            }
            if hit_test(&self.item_rect(di, &sheet), x, y) {
                return Some(abs);
            }
        }
        None
    }

    /// Compute dirty rect covering one or two item positions.  Returns `None`
    /// only when both positions are `None` or refer to items outside the
    /// visible window.
    fn items_dirty(
        &self,
        a: Option<usize>,
        b: Option<usize>,
        n_items: usize,
    ) -> Option<Rectangle> {
        let sheet = self.sheet_rect(n_items);
        let rect_for = |abs: usize| -> Option<Rectangle> {
            if abs < self.scroll_top || abs >= self.scroll_top + MAX_VISIBLE {
                return None;
            }
            let di = abs - self.scroll_top;
            Some(self.item_rect(di, &sheet))
        };
        let ra = a.and_then(rect_for);
        let rb = b.and_then(rect_for);
        match (ra, rb) {
            (Some(a), Some(b)) => Some(union_rect(a, b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    fn ensure_visible(&mut self, n_items: usize) {
        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        } else if self.selected >= self.scroll_top + MAX_VISIBLE {
            self.scroll_top = self.selected + 1 - MAX_VISIBLE;
        }
        if n_items > MAX_VISIBLE && self.scroll_top + MAX_VISIBLE > n_items {
            self.scroll_top = n_items - MAX_VISIBLE;
        }
    }

    fn draw_shortcut<D: DrawTarget<Color = Gray8>>(
        &self,
        canvas: &mut D,
        sc: char,
        item_rect: Rectangle,
        pressed: bool,
    ) {
        let s = format!("/{sc}");
        let sw = font_aa::text_width(&s, SHORTCUT_SIZE);
        let sh = font_aa::cap_height(SHORTCUT_SIZE);
        let x = item_rect.top_left.x + item_rect.size.width as i32 - sw - 4;
        let y = item_rect.top_left.y + (ITEM_H - sh) / 2;
        let luma = if pressed { 200u8 } else { 128u8 };
        let _ = font_aa::draw_text(canvas, &s, x, y, SHORTCUT_SIZE, luma);
    }

    fn draw_disabled_item<D: DrawTarget<Color = Gray8>>(
        &self,
        canvas: &mut D,
        rect: Rectangle,
        label: &str,
    ) {
        let _ = rect.into_styled(PrimitiveStyle::with_stroke(GRAY, 1)).draw(canvas);
        let lw = font_aa::text_width(label, LABEL_SIZE);
        let lh = font_aa::cap_height(LABEL_SIZE);
        let x = rect.top_left.x + (rect.size.width as i32 - lw) / 2;
        let y = rect.top_left.y + (ITEM_H - lh) / 2;
        // luma 160: visually gray on the white background
        let _ = font_aa::draw_text(canvas, label, x, y, LABEL_SIZE, 160u8);
    }
}

// --- Free helpers -----------------------------------------------------------

fn first_enabled(items: &[MenuItem<'_>], from: usize) -> usize {
    items.iter()
        .enumerate()
        .skip(from)
        .find(|(_, it)| it.enabled)
        .map(|(i, _)| i)
        .unwrap_or(from)
}

fn next_enabled(items: &[MenuItem<'_>], current: usize) -> usize {
    items.iter()
        .enumerate()
        .skip(current + 1)
        .find(|(_, it)| it.enabled)
        .map(|(i, _)| i)
        .unwrap_or(current)
}

fn prev_enabled(items: &[MenuItem<'_>], current: usize) -> usize {
    if current == 0 {
        return current;
    }
    for i in (0..current).rev() {
        if items.get(i).map_or(false, |it| it.enabled) {
            return i;
        }
    }
    current
}

fn union_rect(a: Rectangle, b: Rectangle) -> Rectangle {
    let ax1 = a.top_left.x + a.size.width as i32;
    let ay1 = a.top_left.y + a.size.height as i32;
    let bx1 = b.top_left.x + b.size.width as i32;
    let by1 = b.top_left.y + b.size.height as i32;
    let x0 = a.top_left.x.min(b.top_left.x);
    let y0 = a.top_left.y.min(b.top_left.y);
    Rectangle::new(
        Point::new(x0, y0),
        Size::new((ax1.max(bx1) - x0) as u32, (ay1.max(by1) - y0) as u32),
    )
}

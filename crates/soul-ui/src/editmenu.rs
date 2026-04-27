//! System edit menu: a shell-owned popup with the standard
//! Cut / Copy / Paste / Select-All actions.
//!
//! The [`EditTarget`] trait + [`EditOutput`] type now live in
//! [`soul_core`]; this module re-exports them for back-compat and
//! adds the [`EditMenu`] widget the runner draws when an app
//! exposes a focused edit target.
//!
//! The widget is intentionally tiny: a vertical list of four
//! buttons rendered as a centered popup with a 1-px black border.
//! Items disable themselves based on the focused target's state
//! (no selection ⇒ Cut/Copy greyed; clipboard rejected ⇒ Paste
//! greyed). Tapping outside the popup or on the Menu strip again
//! dismisses it; tapping a live item dispatches the corresponding
//! [`soul_core::EditIntent`] to the caller and dismisses.

use crate::primitives::{button, hit_test};
use crate::palette::{BLACK, WHITE};
use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use soul_core::{EditIntent, ExchangePayload};

pub use soul_core::{EditOutput, EditTarget};

/// Menu geometry — width is fixed; height grows with the number of
/// active items. Positioned by the host (`origin` + `popup_rect`).
const MENU_W: i32 = 120;
const ITEM_H: i32 = 24;
const ITEM_COUNT: i32 = 4;
const PAD: i32 = 4;
const MENU_H: i32 = ITEM_COUNT * ITEM_H + PAD * 2;

/// Standard edit-menu popup.
///
/// Stateless apart from its anchor point: state about what's
/// enabled is recomputed on every draw from the focused
/// [`EditTarget`] and the current clipboard payload.
pub struct EditMenu {
    origin: Point,
}

impl EditMenu {
    /// Create a popup anchored at `origin` (top-left of the menu).
    pub fn new(origin: Point) -> Self {
        Self { origin }
    }

    /// Anchor the popup so its top-right corner sits at `anchor`,
    /// then nudge it back onto the screen if it overflows.
    pub fn anchored_top_right(anchor: Point, screen_w: i32, screen_h: i32) -> Self {
        let mut x = anchor.x - MENU_W;
        let mut y = anchor.y;
        if x < 0 {
            x = 0;
        }
        if x + MENU_W > screen_w {
            x = screen_w - MENU_W;
        }
        if y + MENU_H > screen_h {
            y = (screen_h - MENU_H).max(0);
        }
        Self {
            origin: Point::new(x, y),
        }
    }

    /// Bounding rectangle of the popup, including its 1-px border.
    pub fn rect(&self) -> Rectangle {
        Rectangle::new(self.origin, Size::new(MENU_W as u32, MENU_H as u32))
    }

    fn item_rect(&self, idx: i32) -> Rectangle {
        Rectangle::new(
            Point::new(self.origin.x + PAD, self.origin.y + PAD + idx * ITEM_H),
            Size::new((MENU_W - PAD * 2) as u32, (ITEM_H - 2) as u32),
        )
    }

    /// Compute which items are active given a focused target and the
    /// current clipboard. Order matches [`Self::item_label`].
    fn enabled<T: EditTarget + ?Sized>(
        target: &T,
        clipboard: Option<&ExchangePayload>,
    ) -> [bool; 4] {
        let has_sel = target.has_selection();
        let can_paste = clipboard.map_or(false, |p| target.accepts_paste(p));
        [has_sel, has_sel, can_paste, true]
    }

    fn item_label(idx: i32) -> &'static str {
        match idx {
            0 => "Cut",
            1 => "Copy",
            2 => "Paste",
            3 => "Select All",
            _ => "",
        }
    }

    fn item_intent(idx: i32) -> EditIntent {
        match idx {
            0 => EditIntent::Cut,
            1 => EditIntent::Copy,
            2 => EditIntent::Paste,
            _ => EditIntent::SelectAll,
        }
    }

    /// Draw the popup. `target` and `clipboard` drive the per-item
    /// enabled state; disabled items render greyed out.
    pub fn draw<D: DrawTarget<Color = Gray8>, T: EditTarget + ?Sized>(
        &self,
        canvas: &mut D,
        target: &T,
        clipboard: Option<&ExchangePayload>,
    ) {
        let r = self.rect();
        let _ = r.into_styled(PrimitiveStyle::with_fill(WHITE)).draw(canvas);
        let _ = r
            .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
            .draw(canvas);
        let enabled = Self::enabled(target, clipboard);
        for i in 0..ITEM_COUNT {
            let label = Self::item_label(i);
            // `button` doesn't have a disabled state; render disabled
            // items as grey text on a grey background as a poor man's
            // affordance.
            if enabled[i as usize] {
                let _ = button(canvas, self.item_rect(i), label, false);
            } else {
                let rect = self.item_rect(i);
                let _ = rect
                    .into_styled(PrimitiveStyle::with_fill(Gray8::new(232)))
                    .draw(canvas);
                let _ = rect
                    .into_styled(PrimitiveStyle::with_stroke(Gray8::new(180), 1))
                    .draw(canvas);
                let style = embedded_graphics::mono_font::MonoTextStyle::new(
                    &embedded_graphics::mono_font::ascii::FONT_6X10,
                    Gray8::new(160),
                );
                let tx = rect.top_left.x + 8;
                let ty = rect.top_left.y + (rect.size.height as i32 - 10) / 2;
                let _ = embedded_graphics::text::Text::with_baseline(
                    label,
                    Point::new(tx, ty),
                    style,
                    embedded_graphics::text::Baseline::Top,
                )
                .draw(canvas);
            }
        }
    }

    /// Test whether `(x, y)` lands on an enabled menu item, returning
    /// the corresponding [`EditIntent`] when it does. Tap-outside or
    /// disabled-item taps return `None` (the host should then dismiss
    /// the menu).
    pub fn hit<T: EditTarget + ?Sized>(
        &self,
        x: i16,
        y: i16,
        target: &T,
        clipboard: Option<&ExchangePayload>,
    ) -> Option<EditIntent> {
        let enabled = Self::enabled(target, clipboard);
        for i in 0..ITEM_COUNT {
            if hit_test(&self.item_rect(i), x, y) && enabled[i as usize] {
                return Some(Self::item_intent(i));
            }
        }
        None
    }

    /// Returns `true` when `(x, y)` is inside the popup rectangle.
    /// The host uses this to detect "tap outside ⇒ dismiss".
    pub fn contains(&self, x: i16, y: i16) -> bool {
        hit_test(&self.rect(), x, y)
    }
}

//! Stateless drawing primitives.
//!
//! These are plain functions, not widgets: each call paints directly
//! into the passed [`DrawTarget`] and holds no state between calls.
//! Because they're stateless, they're cheap to invoke unconditionally
//! from every `draw()` pass — the runtime's dirty-rect clipper will
//! discard pixels that fall outside the redraw region, so drawing
//! "too much" is bounded by the invalidated area.
//!
//! All primitives target [`Gray8`] and use the default mono font
//! `FONT_6X10` from [`embedded-graphics`]. A character is 6 px wide
//! and 10 px tall.
//!
//! [`DrawTarget`]: embedded_graphics::draw_target::DrawTarget
//! [`Gray8`]: embedded_graphics::pixelcolor::Gray8
//! [`embedded-graphics`]: https://crates.io/crates/embedded-graphics

use embedded_graphics::{
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
};

use crate::{font_aa, palette::{BLACK, WHITE}};

/// Height of the standard SoulOS title bar in pixels, including its
/// bottom edge. App content should begin at this Y coordinate.
pub const TITLE_BAR_H: u32 = 15;

// Font size in logical pixels, tuned to fit the 15-px title bar and
// button heights used throughout the UI.
const LABEL_SIZE: f32 = 9.0;
// Fixed-cell width used only for legacy hit-test/centering callers that
// haven't yet switched to font_aa::text_width.  Keep at 6 so any
// existing layout arithmetic stays correct.
const FONT_W: i32 = 6;

/// Fill the rectangle `(0, 0) – (width, height)` with [`WHITE`].
///
/// Rarely needed in app code: the runtime already clears each dirty
/// region to white before invoking `draw`. Reach for this only if
/// you deliberately paint outside the invalidated area (discouraged —
/// it defeats dirty-rect tracking on e-ink).
pub fn clear<D>(target: &mut D, width: u32, height: u32) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    let style = PrimitiveStyleBuilder::new().fill_color(WHITE).build();
    Rectangle::new(Point::zero(), Size::new(width, height))
        .into_styled(style)
        .draw(target)
}

/// Draw a classic SoulOS title bar: a black strip at the top of the
/// screen, with `title` rendered in white.
///
/// Intended to be called unconditionally from `draw()`. The bar is
/// [`TITLE_BAR_H`] pixels tall and `width` pixels wide.
pub fn title_bar<D>(target: &mut D, width: u32, title: &str) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    let bar = PrimitiveStyleBuilder::new().fill_color(BLACK).build();
    Rectangle::new(Point::zero(), Size::new(width, TITLE_BAR_H))
        .into_styled(bar)
        .draw(target)?;
    font_aa::draw_text(target, title, 4, 2, LABEL_SIZE, 255)?;
    Ok(())
}

/// Draw a PalmOS-style rounded-rectangle button.
///
/// `pressed = true` inverts the button (black fill, white text) —
/// use this to render an active touch or a persistent-selected state.
/// The label is centered inside `rect`.
pub fn button<D>(
    target: &mut D,
    rect: Rectangle,
    label: &str,
    pressed: bool,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    let fill = if pressed { BLACK } else { WHITE };
    let style = PrimitiveStyleBuilder::new()
        .fill_color(fill)
        .stroke_color(BLACK)
        .stroke_width(1)
        .build();
    RoundedRectangle::with_equal_corners(rect, Size::new(4, 4))
        .into_styled(style)
        .draw(target)?;
    let luma = if pressed { 255 } else { 0 };
    let label_w = font_aa::text_width(label, LABEL_SIZE);
    let label_h = font_aa::cap_height(LABEL_SIZE);
    let pos = rect.top_left
        + Point::new(
            (rect.size.width as i32 - label_w) / 2,
            (rect.size.height as i32 - label_h) / 2,
        );
    font_aa::draw_text(target, label, pos.x, pos.y, LABEL_SIZE, luma)?;
    Ok(())
}

/// Draw plain [`BLACK`] text anchored at `at` (top-left baseline).
///
/// For headings or styled text, compose with `embedded-graphics`'
/// [`Text`] directly; this is the shortest path for status text,
/// list rows, and small labels.
pub fn label<D>(target: &mut D, at: Point, text: &str) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray8>,
{
    font_aa::draw_text(target, text, at.x, at.y, LABEL_SIZE, 0)?;
    Ok(())
}

/// Return `true` if `(x, y)` is inside `rect`.
///
/// `x` / `y` are the coordinates you receive from pen events
/// (`Event::PenDown`, `Event::PenMove`, `Event::PenUp`) in the
/// SoulOS virtual-screen coordinate space (origin top-left, Y grows
/// downward).
pub fn hit_test(rect: &Rectangle, x: i16, y: i16) -> bool {
    let x = x as i32;
    let y = y as i32;
    x >= rect.top_left.x
        && x < rect.top_left.x + rect.size.width as i32
        && y >= rect.top_left.y
        && y < rect.top_left.y + rect.size.height as i32
}

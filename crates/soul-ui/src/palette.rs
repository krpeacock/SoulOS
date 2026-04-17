//! Canonical SoulOS palette.
//!
//! All SoulOS UI is defined in terms of 8-bit grayscale. On
//! monochrome (1-bit e-ink) panels the HAL thresholds or dithers;
//! on color panels it upsamples into RGB. Apps should express their
//! UI in these named constants rather than raw [`Gray8`] values so
//! rendering stays consistent across targets.
//!
//! [`Gray8`]: embedded_graphics::pixelcolor::Gray8

use embedded_graphics::pixelcolor::Gray8;

/// Ink — solid black. Use for text, strokes, and pressed button faces.
pub const BLACK: Gray8 = Gray8::new(0);

/// Paper — solid white. Use for the background of a form. The runtime
/// clears each dirty region to this color before calling `draw`, so
/// apps rarely need to fill it themselves.
pub const WHITE: Gray8 = Gray8::new(255);

/// Chrome — a neutral gray used for inert surfaces (e.g., the
/// keyboard surround) where black ink would be too heavy.
pub const GRAY: Gray8 = Gray8::new(192);

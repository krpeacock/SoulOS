//! # soul-ui — widget SDK for SoulOS applications
//!
//! Rendering primitives and interactive widgets layered on top of
//! [`embedded-graphics`]. Designed to compile `no_std` so the exact
//! same widget code runs inside the desktop simulator and on a
//! bare-metal e-reader.
//!
//! This crate is the canonical UI SDK for SoulOS app developers.
//! Everything here is meant to be called from an app's `draw` and
//! `handle` implementations; none of it performs I/O on its own.
//!
//! ## Crate layout
//!
//! - [`palette`] — the canonical SoulOS color set.
//! - [`primitives`] — stateless draw helpers: [`title_bar`],
//!   [`button`], [`label`], [`hit_test`].
//! - [`keyboard`] — on-screen keyboard with lowercase, uppercase,
//!   and symbol layers.
//! - [`textarea`] — multi-line text editor with cursor, selection,
//!   long-press word select, and word wrap.
//! - [`textinput`] — single-line text input with placeholder and
//!   submit-on-enter.
//! - [`prelude`] — one glob import for common items.
//!
//! ## Using stateless primitives
//!
//! Primitives are plain functions; call them freely from `draw()`.
//! The runtime clips to the dirty region, so overdrawing is cheap.
//!
//! ```ignore
//! use embedded_graphics::{prelude::*, pixelcolor::Gray8};
//! use soul_ui::{title_bar, button};
//! use embedded_graphics::primitives::Rectangle;
//!
//! fn draw<D: DrawTarget<Color = Gray8>>(d: &mut D) {
//!     let _ = title_bar(d, 240, "My App");
//!     let rect = Rectangle::new(Point::new(10, 40), Size::new(80, 24));
//!     let _ = button(d, rect, "OK", false);
//! }
//! ```
//!
//! ## Using stateful widgets
//!
//! Stateful widgets ([`Keyboard`], [`TextArea`], [`TextInput`]) own
//! their internal state and expose event hooks that return dirty
//! rectangles for the runtime's damage tracker:
//!
//! ```ignore
//! use embedded_graphics::{prelude::*, primitives::Rectangle};
//! use soul_ui::{TextArea, TextAreaOutput};
//!
//! let area = Rectangle::new(Point::new(0, 15), Size::new(240, 200));
//! let mut editor = TextArea::with_text(area, "hello".into());
//!
//! // In App::handle:
//! // Event::Key(KeyCode::Char(c)) => {
//! //     let out = editor.insert_char(c);
//! //     if let Some(r) = out.dirty { ctx.invalidate(r); }
//! //     if out.text_changed { persist(editor.text()); }
//! // }
//! ```
//!
//! ## Color model
//!
//! All SoulOS widgets draw in 8-bit grayscale ([`Gray8`]). On
//! monochrome e-ink the HAL dithers or thresholds; on color LCDs
//! it upsamples. Apps should never assume RGB.
//!
//! [`embedded-graphics`]: https://crates.io/crates/embedded-graphics
//! [`Gray8`]: embedded_graphics::pixelcolor::Gray8

#![no_std]
extern crate alloc;

pub mod builder;
pub mod egui_bridge;
pub mod egui_integration;
pub mod egui_layout;
pub mod egui_scroll;
pub mod egui_widgets;
pub mod emoji;
pub mod form;
pub mod keyboard;
pub mod pagination;
pub mod palette;
pub mod primitives;
pub mod scrollbar;
pub mod selecttext;
pub mod textarea;
pub mod textinput;

#[cfg(test)]
mod tests;

/// Convenience re-exports for apps that prefer one glob import.
pub mod prelude {
    pub use crate::builder::*;
    pub use crate::egui_bridge::*;
    pub use crate::egui_integration::*;
    pub use crate::egui_layout::*;
    pub use crate::egui_scroll::*;
    pub use crate::egui_widgets::*;
    pub use crate::form::*;
    pub use crate::keyboard::{Keyboard, KeyboardOutput, Layer, TypedKey};
    pub use crate::pagination::{Pagination, PaginationAction};
    pub use crate::palette::{BLACK, GRAY, WHITE};
    pub use crate::primitives::{button, clear, hit_test, label, title_bar, TITLE_BAR_H};
    pub use crate::scrollbar::{Scrollbar, ScrollbarOutput, ScrollableView};
    pub use crate::selecttext::SelectableText;
    pub use crate::textarea::{TextArea, TextAreaOutput};
    pub use crate::textinput::{TextInput, TextInputOutput};
}

pub use builder::*;
pub use egui_bridge::*;
pub use egui_integration::*;
pub use egui_layout::*;
pub use egui_scroll::*;
pub use egui_widgets::*;
pub use form::*;
pub use keyboard::{Keyboard, KeyboardOutput, Layer, TypedKey, KEYBOARD_HEIGHT, KEYBOARD_WIDTH};
pub use pagination::{Pagination, PaginationAction};
pub use palette::{BLACK, GRAY, WHITE};
pub use primitives::{button, clear, hit_test, label, title_bar, TITLE_BAR_H};
pub use scrollbar::{Scrollbar, ScrollbarOutput, ScrollableView};
pub use selecttext::SelectableText;
pub use textarea::{TextArea, TextAreaOutput};
pub use textinput::{TextInput, TextInputOutput};

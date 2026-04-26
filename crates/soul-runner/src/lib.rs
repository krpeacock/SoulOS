//! Library crate exposing the SoulOS host and built-in apps.
//!
//! Both the desktop binary (`main.rs`) and the Android cdylib
//! (`soul-runner-android`) construct the same `Host` here and feed it
//! into `soul_core::run` with their respective `Platform` impls.

pub mod builder;
pub mod draw;
pub mod egui_demo;
pub mod launcher;
pub mod paint;

mod host;

pub use host::{Host, ICON_CELL};

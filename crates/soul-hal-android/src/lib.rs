//! Android HAL: drives the SoulOS event loop on top of `android-activity`
//! (NativeActivity glue) with `softbuffer` for direct framebuffer
//! presentation. The 240×320 virtual display is integer-scaled and
//! centred on whatever physical surface the device hands us.
//!
//! # Asset / storage strategy
//!
//! SoulOS apps read scripts and icons from `assets/scripts/*.rhai`,
//! `assets/sprites/*.pgm`, etc., and persist databases to `.soulos/*.sdb`.
//! On Android we extract the read-only assets out of the APK
//! (`AAssetManager`) into the app's internal data path on first launch,
//! then `chdir` into that directory so all the existing relative paths
//! Just Work. Database writes land in the same private directory.
//!
//! This is deliberately the simplest viable port — no `AssetSource`
//! trait, no path abstraction sprinkled across every app.

#![cfg(target_os = "android")]

mod platform;
pub use platform::{bootstrap, AndroidDisplay, AndroidPlatform};

pub use android_activity::AndroidApp;

//! Android entry point for SoulOS.
//!
//! Builds an `AndroidPlatform` (winit-free, softbuffer-presented) and
//! hands it to the same `soul_core::run` + `soul_runner::Host` the desktop
//! binary uses. All app code, all event routing, all storage logic is
//! shared — only the `Platform` impl differs.

#![cfg(target_os = "android")]

use android_activity::AndroidApp;

#[no_mangle]
fn android_main(app: AndroidApp) {
    let mut platform = soul_hal_android::bootstrap(app);
    soul_core::run(&mut platform, soul_runner::Host::new());
}

//! Android entry point for SoulOS.
//!
//! Builds an `AndroidPlatform` (winit-free, softbuffer-presented) and
//! hands it to the same `soul_core::run` + `soul_runner::Host` the desktop
//! binary uses. All app code, all event routing, all storage logic is
//! shared — only the `Platform` impl differs.

#![cfg(target_os = "android")]

use android_activity::AndroidApp;

// Subdirectory list emitted by build.rs — AAssetDir_getNextFileName never
// yields directory names, so bootstrap cannot discover them via open_dir.
const ASSET_DIRS: &str = env!("SOUL_ASSET_DIRS");

#[no_mangle]
fn android_main(app: AndroidApp) {
    let dirs: Vec<&str> = ASSET_DIRS.split(',').filter(|s| !s.is_empty()).collect();
    let mut platform = soul_hal_android::bootstrap(app, &dirs);
    soul_core::run(&mut platform, soul_runner::Host::new());
}

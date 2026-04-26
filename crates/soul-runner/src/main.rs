//! Desktop entry point for SoulOS.
//!
//! Constructs the hosted (minifb) `Platform`, builds a `Host`, and hands
//! both to `soul_core::run`. The Android cdylib in `soul-runner-android`
//! does the same with its own `Platform` impl.

#[cfg(not(target_os = "android"))]
fn main() {
    use soul_core::{run, SCREEN_HEIGHT, SCREEN_WIDTH};
    use soul_hal_hosted::HostedPlatform;
    use soul_runner::Host;

    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();
    log::info!("🚀 SoulOS starting up...");

    let mut platform = HostedPlatform::new("SoulOS", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
    run(&mut platform, Host::new());
}

#[cfg(target_os = "android")]
fn main() {
    // The Android entry point is `android_main` in `soul-runner-android`.
    // Cargo still wants a `main` for the bin target, so this is a no-op.
}

//! Desktop runner: hosts all apps and the system strip.

use soul_core::run;
use soul_hal_hosted::HostedPlatform;
use soul_runner::Host;

fn main() {
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();
    log::info!("🚀 SoulOS starting up...");

    let mut platform = HostedPlatform::new("SoulOS", soul_core::SCREEN_WIDTH as u32, soul_core::SCREEN_HEIGHT as u32);
    run(&mut platform, Host::new());
}
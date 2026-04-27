//! Desktop entry point for SoulOS.
//!
//! Constructs the hosted (minifb) `Platform`, builds a `Host`, and hands
//! both to `soul_core::run`. The Android cdylib in `soul-runner-android`
//! does the same with its own `Platform` impl.

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
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

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn main() {
    // Android entry is `android_main` in `soul-runner-android`; the
    // wasm entry is `start` in `soul-runner-web`. Cargo still wants
    // a `main` for the bin target, so this is a no-op.
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod screenshot_tests {
    use soul_hal_hosted::Harness;
    use soul_runner::{
        builder::MobileBuilder, draw::Draw, egui_demo::EguiDemo, launcher::Launcher, paint::Paint,
    };
    use std::path::PathBuf;

    fn output_dir() -> PathBuf {
        let path = PathBuf::from(
            std::env::var("SCREENSHOT_OUT").unwrap_or_else(|_| "target/screenshots".into()),
        );
        std::fs::create_dir_all(&path).expect("create screenshots dir");
        path
    }

    fn should_run(app: &str) -> bool {
        match std::env::var("SCREENSHOT_APPS") {
            Ok(list) if !list.is_empty() => list.split(',').any(|a| a.trim() == app),
            _ => true,
        }
    }

    #[test]
    fn screenshot_launcher() {
        if !should_run("launcher") {
            return;
        }
        let mut h = Harness::new(Launcher::new());
        h.settle().ok();
        h.save_png(output_dir().join("launcher.png")).expect("save png");
    }

    #[test]
    fn screenshot_draw() {
        if !should_run("draw") {
            return;
        }
        let mut h = Harness::new(Draw::new(PathBuf::new()));
        h.settle().ok();
        h.save_png(output_dir().join("draw.png")).expect("save png");
    }

    #[test]
    fn screenshot_paint() {
        if !should_run("paint") {
            return;
        }
        let mut h = Harness::new(Paint::new(PathBuf::new()));
        h.settle().ok();
        h.save_png(output_dir().join("paint.png")).expect("save png");
    }

    #[test]
    fn screenshot_builder() {
        if !should_run("builder") {
            return;
        }
        let mut h = Harness::new(MobileBuilder::new());
        h.settle().ok();
        h.save_png(output_dir().join("builder.png")).expect("save png");
    }

    #[test]
    fn screenshot_egui_demo() {
        if !should_run("egui_demo") {
            return;
        }
        let mut h = Harness::new(EguiDemo::new());
        h.settle().ok();
        h.save_png(output_dir().join("egui_demo.png")).expect("save png");
    }
}

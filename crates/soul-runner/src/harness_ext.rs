//! `Harness<Host>` extensions — launch, home, with_db, new_runner.
//!
//! Lives here rather than in `soul-hal-hosted` to avoid a circular crate
//! dependency (`soul-runner` → `soul-hal-hosted`, but not the reverse).
//!
//! Rust's orphan rule prevents adding inherent methods to `Harness<Host>`
//! from outside `soul-hal-hosted`, so we use a local trait instead.
//! See `docs/Harness.md §5` for the API spec.

use soul_hal::HardButton;
use soul_hal_hosted::Harness;

use crate::Host;

/// Extension methods on `Harness<Host>` for whole-runner test scenarios.
///
/// Import this trait to call `launch`, `home`, `new_runner`, and `with_db`
/// on a `Harness<Host>`.
pub trait HostHarnessExt: Sized {
    /// Create a runner harness with the full app registry. Every scripted
    /// app starts with an empty in-memory database — no `.soulos/*.sdb`
    /// files are read, guaranteeing a clean, deterministic starting state.
    fn new_runner() -> Self;

    /// Like `new_runner()` but injects `db` as the starting database for
    /// the app identified by `app_id`. Use this to seed fixture records
    /// before calling `launch()`.
    fn with_db(app_id: &str, db: soul_db::Database) -> Self;

    /// Navigate into the app with the given stable `app_id`, pushing it
    /// onto the navigation stack and sending `AppStart`. Equivalent to
    /// tapping the app's icon in the Launcher.
    fn launch(&mut self, app_id: &str);

    /// Return to the home screen (equivalent to pressing the Home hard button).
    fn home(&mut self);
}

impl HostHarnessExt for Harness<Host> {
    fn new_runner() -> Self {
        Self::new(Host::new_headless())
    }

    fn with_db(app_id: &str, db: soul_db::Database) -> Self {
        let mut host = Host::new_headless();
        host.inject_db(app_id, db);
        Self::new(host)
    }

    fn launch(&mut self, app_id: &str) {
        self.with_ctx(|host, ctx| host.launch_by_id(app_id, ctx));
        self.tick();
    }

    fn home(&mut self) {
        self.press(HardButton::Home);
    }
}

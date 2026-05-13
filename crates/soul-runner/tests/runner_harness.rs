use soul_hal_hosted::Harness;
use soul_runner::harness_ext::HostHarnessExt;

// ── Harness<Host>::new_runner() ───────────────────────────────────────────────

#[test]
fn new_runner_starts_at_launcher() {
    let mut h = Harness::new_runner();
    h.settle().expect("launcher should settle");

    // The launcher is the home screen; it should have at least one a11y node.
    let nodes = h.nodes();
    assert!(!nodes.is_empty(), "Launcher must expose a11y nodes");
}

// ── launch() ─────────────────────────────────────────────────────────────────

#[test]
fn launch_notes_reaches_notes_app() {
    let mut h = Harness::new_runner();
    h.launch("notes");
    h.settle().expect("notes should settle after launch");

    // Notes declares its title via a11y. We expect at least one node.
    let nodes = h.nodes();
    assert!(!nodes.is_empty(), "notes must expose a11y nodes after launch");
}

#[test]
fn launch_unknown_id_is_a_noop() {
    let mut h = Harness::new_runner();
    let before = h.nodes().len();
    h.launch("definitely_does_not_exist");
    h.settle().ok();
    // Harness should still be at the launcher; node count unchanged.
    assert_eq!(h.nodes().len(), before);
}

// ── home() ───────────────────────────────────────────────────────────────────

#[test]
fn home_returns_from_launched_app() {
    let mut h = Harness::new_runner();
    h.launch("notes");
    h.settle().ok();

    h.home();
    h.settle().expect("launcher should settle after home");

    // After returning home the launcher's nodes are visible again.
    let nodes = h.nodes();
    assert!(!nodes.is_empty());
}

// ── with_db() ────────────────────────────────────────────────────────────────

#[test]
fn with_db_injects_database_into_named_app() {
    use soul_db::Database;

    let mut db = Database::new("notes");
    db.insert(0, b"fixture note".to_vec());

    let mut h = Harness::with_db("notes", db);
    h.launch("notes");
    h.settle().expect("notes with seeded db should settle");

    // The app at least launched without crashing.
    let nodes = h.nodes();
    assert!(!nodes.is_empty());
}

// ── coverage_report() ────────────────────────────────────────────────────────

#[test]
fn coverage_report_on_launcher_has_nodes() {
    let mut h = Harness::new_runner();
    h.settle().ok();

    let report = h.coverage_report();
    assert!(
        !report.nodes.is_empty(),
        "launcher must expose a11y nodes for coverage_report to be useful"
    );
    assert!(
        report.screen_coverage > 0.0,
        "at least some screen area should be covered by a11y nodes"
    );
}

#[test]
fn coverage_report_after_launch_notes() {
    let mut h = Harness::new_runner();
    h.launch("notes");
    h.settle().ok();

    let report = h.coverage_report();
    // Gaps list any quality problems; they don't have to be zero yet
    // (that's what the a11y audit issue tracks), but the report must run.
    assert!(
        !report.nodes.is_empty(),
        "notes must have a11y nodes after launch"
    );
    // Screen coverage should be non-zero.
    assert!(report.screen_coverage > 0.0);
}

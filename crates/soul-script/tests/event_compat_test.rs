use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;

#[test]
fn test_legacy_event_compatibility() {
    let script = r#"
        let app_id = "test.events";
        let app_name = "Event Test";
        let tap_count = 0;

        fn on_draw() {
            clear();
            label(10, 50, "Taps: " + tap_count);
        }

        fn on_event(ev) {
            if ev.type == "PenDown" {
                tap_count += 1;
                invalidate_all();
            }
        }
    "#;

    let db = Database::new("test");
    let app = ScriptedApp::new("test", script, db).expect("Failed to compile script");
    let mut harness = Harness::new(app);

    // Initial draw
    harness.tick();
    harness.settle().expect("App failed to settle");

    // 1. Verify initial tap count is 0
    let count: i32 = harness.app().get_var("tap_count").expect("tap_count should exist");
    assert_eq!(count, 0, "Initial tap count should be 0");
    
    // 2. Perform a tap
    harness.tap(100, 100);
    harness.tick();
    harness.settle().expect("App failed to settle after tap");

    // 3. Verify on_event was called
    let count: i32 = harness.app().get_var("tap_count").expect("tap_count should exist");
    assert_eq!(count, 1, "Tap count should be 1 after tap event");
    
    println!("Legacy event compatibility test passed");
}

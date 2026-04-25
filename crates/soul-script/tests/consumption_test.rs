use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;

#[test]
fn test_egui_pointer_consumption() {
    let script = r#"
        let app_id = "test.consumption";
        let app_name = "Consumption Test";
        let move_count = 0;

        fn on_draw() {
            clear();
            label(10, 50, "Moves: " + move_count);
        }

        fn on_event(ev) {
            if ev.type == "PenMove" {
                move_count += 1;
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

    // 1. Perform a drag
    // 10 steps of drag from (100, 100) to (100, 200)
    harness.drag((100, 100), (100, 200), 10);
    
    // 2. Verify move count
    let count: i32 = harness.app().get_var("move_count").expect("move_count should exist");
    // We expect at least 10 moves if not consumed
    assert!(count >= 10, "Legacy on_event should have received drag moves (got {})", count);
    
    println!("Pointer consumption test passed (count={})", count);
}

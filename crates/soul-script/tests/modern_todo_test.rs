use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;

#[test]
fn test_modern_todo_app_compatibility() {
    let script_path = "../../assets/scripts/todo_with_egui_scroll.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read script");

    let db = Database::new("test");
    let app = ScriptedApp::new("todo_modern", &script, db).expect("Failed to compile script");
    let mut harness = Harness::new(app);

    // Initial draw
    harness.tick();
    harness.settle().expect("Modern Todo app failed to settle");

    println!("Modern Todo app compatibility test passed");
}

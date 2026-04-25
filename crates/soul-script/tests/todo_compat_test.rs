use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;
use embedded_graphics::prelude::GrayColor;

#[test]
fn test_todo_app_compatibility() {
    let script_path = "../../assets/scripts/todo.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read todo.rhai");

    let db = Database::new("test");
    let app = ScriptedApp::new("todo", &script, db).expect("Failed to compile todo.rhai");
    let mut harness = Harness::new(app);

    // Initial draw
    harness.tick();
    harness.settle().expect("Todo app failed to settle");

    // Check if title bar is there
    let title_pixel = harness.pixel(120, 5);
    assert_eq!(title_pixel.luma(), 0, "Todo title bar should be black");

    println!("Todo app compatibility test passed");
}

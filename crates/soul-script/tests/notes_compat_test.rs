use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;
use embedded_graphics::prelude::GrayColor;

#[test]
fn test_notes_app_compatibility() {
    let script_path = "../../assets/scripts/notes.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read notes.rhai");

    let db = Database::new("test");
    let app = ScriptedApp::new("notes", &script, db).expect("Failed to compile notes.rhai");
    let mut harness = Harness::new(app);

    // Initial draw
    harness.tick();
    harness.settle().expect("Notes app failed to settle");

    // Check if title bar is there
    let title_pixel = harness.pixel(120, 5);
    assert_eq!(title_pixel.luma(), 0, "Notes title bar should be black");

    println!("Notes app compatibility test passed");
}

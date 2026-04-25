use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;
use embedded_graphics::prelude::GrayColor;

#[test]
fn test_egui_demo_app_compatibility() {
    let script_path = "../../assets/scripts/egui_demo.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read egui_demo.rhai");

    let db = Database::new("test");
    let app = ScriptedApp::new("egui_demo", &script, db).expect("Failed to compile egui_demo.rhai");
    let mut harness = Harness::new(app);

    // Initial draw
    harness.tick();
    
    // Check for errors
    if let Some(err) = harness.app().last_error() {
        panic!("Script error during on_draw: {:?}", err);
    }
    
    harness.settle().expect("egui_demo app failed to settle");

    println!("egui_demo app compatibility test passed");
}

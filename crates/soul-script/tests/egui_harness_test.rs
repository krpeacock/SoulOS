use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;
use embedded_graphics::prelude::GrayColor;

#[test]
fn test_egui_scrolling_and_interaction() {
    let script = r#"
        let app_id = "test.egui";
        let app_name = "EGUI Test";
        let clicked = false;
        let checked = false;

        fn on_draw() {
            clear();
            egui_scroll_area("test_scroll", 100, |ui| {
                if egui_button(ui, "Click Me") {
                    clicked = true;
                }
                checked = egui_checkbox(ui, checked, "Check Me");
                
                // Add lots of content to ensure scrolling
                for i in 0..20 {
                    egui_label(ui, "Line " + i);
                }
            });
        }

        fn on_event(ev) {}
    "#;

    let db = Database::new("test");
    let app = ScriptedApp::new("test", script, db).expect("Failed to compile script");
    let mut harness = Harness::new(app);

    // Initial draw
    harness.tick();
    harness.settle().expect("App failed to settle");

    // Verify something was rendered (not all white)
    let mut all_white = true;
    for y in 0..320 {
        for x in 0..240 {
            if harness.pixel(x as i16, y as i16).luma() < 255 {
                all_white = false;
                break;
            }
        }
        if !all_white { break; }
    }
    assert!(!all_white, "Framebuffer should not be all white after rendering EGUI elements");

    // Tap where we expect the button to be (approximate)
    harness.tap(50, 40); 
    
    harness.tick();
    harness.settle().expect("App failed to settle after tap");
    
    // Check speech log or other side effects if any
    // Since we don't have a easy way to assert Rhai state yet, 
    // let's just ensure it doesn't crash.
    
    println!("EGUI Harness test completed successfully (no crash)");
}

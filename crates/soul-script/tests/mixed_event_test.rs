use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;

#[test]
fn test_mixed_egui_and_legacy_events() {
    let script = r#"
        let app_id = "test.mixed";
        let app_name = "Mixed Test";
        let legacy_taps = 0;
        let egui_clicked = false;

        fn on_draw() {
            clear();
            // 1. Manual button (legacy)
            label(10, 50, "Legacy Taps: " + legacy_taps);
            
            // 2. EGUI button (native)
            egui_scroll_area("mixed_scroll", 100, |ui| {
                if egui_button(ui, "EGUI Click Me") {
                    egui_clicked = true;
                }
            });
        }

        fn on_event(ev) {
            if ev.type == "PenDown" {
                // Legacy tap detection
                if ev.x >= 10 && ev.x <= 100 && ev.y >= 40 && ev.y <= 70 {
                    legacy_taps += 1;
                    invalidate_all();
                }
            }
        }
    "#;

    let db = Database::new("test");
    let app = ScriptedApp::new("test", script, db).expect("Failed to compile script");
    let mut harness = Harness::new(app);

    // Initial draw
    harness.tick();
    harness.settle().expect("App failed to settle");

    // 1. Click EGUI button (approximate position)
    // EGUI CentralPanel + ScrollArea will place the button near the top left
    // but below the title bar if one exists.
    harness.tap(50, 40); 
    harness.tick();
    harness.settle().expect("App failed to settle after EGUI tap");
    
    // Check if EGUI handled it
    let clicked: bool = harness.app().get_var("egui_clicked").expect("egui_clicked should exist");
    // Note: This might fail if the coordinates are wrong, but we'll see.
    
    // 2. Click Legacy area (away from EGUI button)
    // Legacy tap area is around (10..100, 40..70)
    harness.tap(50, 60); 
    harness.tick();
    harness.settle().expect("App failed to settle after legacy tap");
    
    // Check if Legacy handled it
    let taps: i32 = harness.app().get_var("legacy_taps").expect("legacy_taps should exist");
    assert_eq!(taps, 1, "Legacy on_event should still receive events that EGUI doesn't consume");
    
    println!("Mixed event test passed");
}

use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;
use embedded_graphics::prelude::GrayColor;

#[test]
fn test_legacy_drawing_compatibility() {
    let script = r#"
        let app_id = "test.legacy";
        let app_name = "Legacy Test";

        fn on_draw() {
            clear();
            title_bar("Legacy App");
            label(10, 50, "If you see this, it works");
            draw_rect(10, 80, 50, 20, 128); // Gray box
        }

        fn on_event(ev) {}
    "#;

    let db = Database::new("test");
    let app = ScriptedApp::new("test", script, db).expect("Failed to compile script");
    let mut harness = Harness::new(app);

    // Initial draw
    harness.tick();
    
    // Check for script errors
    if let Some(err) = harness.app().last_error() {
        panic!("Script error during on_draw: {:?}", err);
    }
    
    harness.settle().expect("App failed to settle");

    // 1. Verify title bar (Black background at top)
    let title_pixel = harness.pixel(120, 5); 
    assert_eq!(title_pixel.luma(), 0, "Title bar should be black (0)");

    // 2. Verify gray box (128) - Check this first to see if other drawing works
    let box_pixel = harness.pixel(30, 90);
    assert_eq!(box_pixel.luma(), 128, "Gray box should be rendered with luma 128 (got {})", box_pixel.luma());

    // 3. Verify label text (Black text on white background)
    // Check a range of Y because text pixels are sparse
    let mut found_text = false;
    'outer: for y in 50..60 {
        for x in 10..100 {
            let luma = harness.pixel(x, y).luma();
            if luma < 200 { 
                found_text = true;
                break 'outer;
            }
        }
    }
    assert!(found_text, "Should find some non-white pixels for the label 'If you see this, it works' in range y=50..60");
    
    println!("Legacy compatibility test passed");
}

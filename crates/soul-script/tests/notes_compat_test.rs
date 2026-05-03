use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;
use embedded_graphics::prelude::GrayColor;
use std::path::PathBuf;

fn debug_png_dir() -> PathBuf {
    let dir = PathBuf::from("/tmp/notes_debug");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_notes_app_starts_in_list_view() {
    let script_path = "../../assets/scripts/notes.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read notes.rhai");

    let db = Database::new("test");
    let app = ScriptedApp::new("notes", &script, db).expect("Failed to compile notes.rhai");
    let mut harness = Harness::new(app);

    harness.tick();
    harness.settle().expect("Notes app failed to settle");

    // Title bar should be black.
    let title_pixel = harness.pixel(120, 5);
    assert_eq!(title_pixel.luma(), 0, "Notes title bar should be black");

    // Screen should not be entirely white: the welcome note + list items are rendered.
    let fb = harness.framebuffer();
    let all_white = fb.buffer.iter().all(|&p| (p & 0xFF) == 0xFF);
    assert!(!all_white, "List view should render content, not a blank screen");
}

#[test]
fn test_notes_creates_welcome_note_on_first_launch() {
    let script_path = "../../assets/scripts/notes.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read notes.rhai");

    // Empty DB → app should insert a welcome note on AppStart.
    let db = Database::new("test");
    let app = ScriptedApp::new("notes", &script, db).expect("Failed to compile notes.rhai");
    let mut harness = Harness::new(app);

    harness.tick();
    harness.settle().expect("Notes app failed to settle");

    // Retrieve note count by inspecting what was inserted.
    // The ScriptedApp exposes the DB; after AppStart the welcome note must exist.
    let note_count = harness.app().db.iter_category(0).count();
    assert_eq!(note_count, 1, "A welcome note should be created on first launch");

    let welcome = harness.app().db.iter_category(0).next().unwrap();
    let text = String::from_utf8_lossy(&welcome.data);
    assert!(text.starts_with("Welcome"), "First note should be the welcome message");
}

#[test]
fn test_notes_preserves_existing_notes() {
    let script_path = "../../assets/scripts/notes.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read notes.rhai");

    // Pre-populate DB with two notes.
    let mut db = Database::new("test");
    let id_a = db.insert(0, b"First note".to_vec());
    let id_b = db.insert(0, b"Second note".to_vec());

    let app = ScriptedApp::new("notes", &script, db).expect("Failed to compile notes.rhai");
    let mut harness = Harness::new(app);

    harness.tick();
    harness.settle().expect("Notes app failed to settle with pre-existing notes");

    // App must not create an extra welcome note when notes already exist.
    let count = harness.app().db.iter_category(0).count();
    assert_eq!(count, 2, "Should have exactly the 2 pre-existing notes");

    // Original notes must be intact.
    let text_a = harness.app().db.get(id_a).map(|r| String::from_utf8_lossy(&r.data).to_string());
    let text_b = harness.app().db.get(id_b).map(|r| String::from_utf8_lossy(&r.data).to_string());
    assert_eq!(text_a.as_deref(), Some("First note"));
    assert_eq!(text_b.as_deref(), Some("Second note"));

    // Keyboard events in list mode must not crash the app.
    harness.type_text("ignored");
    harness.settle().expect("App should survive key events in list mode");
}

#[test]
fn test_new_note_button_creates_note() {
    let script_path = "../../assets/scripts/notes.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read notes.rhai");

    let db = Database::new("test");
    let app = ScriptedApp::new("notes", &script, db).expect("Failed to compile notes.rhai");
    let mut harness = Harness::new(app);

    harness.tick();
    harness.settle().unwrap();

    // After AppStart there is 1 welcome note.
    assert_eq!(harness.app().db.iter_category(0).count(), 1);

    // "New" button is at x=2..62, y=282..302 — tap its centre.
    harness.tap(32, 292);
    harness.settle().unwrap();

    // A blank new note must have been created (total = 2).
    assert_eq!(harness.app().db.iter_category(0).count(), 2,
        "Tapping 'New' should insert a new note");

    // Title bar should still be black (edit view rendered successfully).
    let title_pixel = harness.pixel(120, 5);
    assert_eq!(title_pixel.luma(), 0, "Title bar should be black in edit view");
}

/// Traces the "New Note" flow with screenshots at each step.
/// Run with: cargo test -p soul-script --test notes_compat_test debug_new_note -- --nocapture --test-threads=1
/// Screenshots land in /tmp/notes_debug/
#[test]
fn debug_new_note_flow() {
    let dir = debug_png_dir();
    let script_path = "../../assets/scripts/notes.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read notes.rhai");

    let db = Database::new("test");
    let app = ScriptedApp::new("notes", &script, db).expect("Failed to compile notes.rhai");
    let mut harness = Harness::new(app);

    harness.tick();
    harness.settle().unwrap();
    harness.save_png(dir.join("01_list_initial.png")).unwrap();
    println!("01: list view after AppStart");
    println!("    notes in DB: {}", harness.app().db.iter_category(0).count());

    // Full-screen dark-pixel audit: print each row that has any non-white content.
    println!("    full-screen content rows (luma < 200):");
    for y in 0i16..304 {
        let dark_count = (0i16..240).filter(|&x| harness.pixel(x, y).luma() < 200).count();
        if dark_count > 0 {
            println!("      y={:3}: {} dark pixels", y, dark_count);
        }
    }

    // The EGUI "New Note" button lives below the scroll area.
    // Probe the bottom strip (y=270..290) for where non-white pixels are.
    println!("    scanning y=260..295 for non-white pixels:");
    for y in 260i16..295 {
        let mut dark_xs: Vec<i16> = vec![];
        for x in 0i16..240 {
            if harness.pixel(x, y).luma() < 200 {
                dark_xs.push(x);
            }
        }
        if !dark_xs.is_empty() {
            println!("      y={}: dark pixels at x={:?}", y, dark_xs);
        }
    }

    // Try tapping where we expect the "New Note" button.
    // EGUI lays out the button right after the scroll area (height=259).
    // Title bar=15, scroll=259 → button starts around y=274.
    println!("    tapping (60, 278) — expected 'New Note' button");
    harness.tap(60, 278);
    harness.settle().unwrap();
    harness.save_png(dir.join("02_after_tap_new_note.png")).unwrap();
    let count_after = harness.app().db.iter_category(0).count();
    println!("02: after tap — notes in DB: {}", count_after);

    // Check if we're now in edit mode (keyboard visible at y=208..304)
    let mut kb_pixels = 0u32;
    for x in 0i16..240 {
        for y in 210i16..280 {
            if harness.pixel(x, y).luma() < 200 { kb_pixels += 1; }
        }
    }
    println!("    dark pixels in keyboard zone (y=210..280): {}", kb_pixels);

    if count_after > 1 {
        println!("SUCCESS: new note was created (count={})", count_after);
    } else {
        // Button wasn't where we expected — scan all of y=260..304 for dark pixel clusters
        println!("MISS: no new note created, rescanning y=260..304 more carefully:");
        for y in 260i16..304 {
            let row_dark: Vec<i16> = (0i16..240).filter(|&x| harness.pixel(x, y).luma() < 128).collect();
            if !row_dark.is_empty() {
                println!("      y={}: {} dark pixels, first={} last={}",
                    y, row_dark.len(), row_dark[0], row_dark[row_dark.len()-1]);
            }
        }

        // Try a few other y positions to find the button
        for tap_y in [268i16, 272, 276, 280, 284, 288] {
            harness.tap(60, tap_y);
            harness.settle().unwrap();
            let c = harness.app().db.iter_category(0).count();
            if c > 1 {
                println!("    button found at tap_y={} (count now {})", tap_y, c);
                harness.save_png(dir.join(format!("03_found_at_y{}.png", tap_y))).unwrap();
                break;
            }
        }
    }

    println!("Screenshots saved to {:?}", dir);
}

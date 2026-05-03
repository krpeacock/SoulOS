use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;
use embedded_graphics::prelude::GrayColor;

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

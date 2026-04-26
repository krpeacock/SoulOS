use soul_script::ScriptedApp;
use soul_hal_hosted::Harness;
use soul_db::Database;
use embedded_graphics::prelude::GrayColor;

fn make_harness() -> Harness<ScriptedApp> {
    let script_path = "../../assets/scripts/calc.rhai";
    let script = std::fs::read_to_string(script_path).expect("Failed to read calc.rhai");
    let db = Database::new("calc");
    let app = ScriptedApp::new("calc", &script, db).expect("Failed to compile calc.rhai");
    Harness::new(app)
}

#[test]
fn calc_renders_without_crash() {
    let mut h = make_harness();
    h.tick();
    h.settle().expect("calc failed to settle on initial draw");

    let mut has_dark_pixel = false;
    for y in 0..320i16 {
        for x in 0..240i16 {
            if h.pixel(x, y).luma() < 255 {
                has_dark_pixel = true;
                break;
            }
        }
        if has_dark_pixel { break; }
    }
    assert!(has_dark_pixel, "Calculator screen should not be all white");
}

#[test]
fn calc_digit_buttons_do_not_crash() {
    let mut h = make_harness();
    h.tick();
    h.settle().expect("initial settle");

    // Tap digit buttons in approximate positions
    // The egui layout places buttons roughly in the top-left quadrant
    let taps = [(30, 120), (70, 120), (110, 120),  // 7 8 9
                (30, 155), (70, 155), (110, 155),  // 4 5 6
                (30, 190), (70, 190), (110, 190),  // 1 2 3
                (30, 225)];                         // 0

    for (x, y) in taps {
        h.tap(x, y);
        h.tick();
        h.settle().expect("calc crashed after tap");
    }
}

#[test]
fn calc_operator_and_equals_do_not_crash() {
    let mut h = make_harness();
    h.tick();
    h.settle().expect("initial settle");

    // Rough positions: digit buttons, operator buttons, equals
    let steps = [
        (30, 190),  // 1
        (150, 155), // −
        (110, 190), // 3
        (150, 225), // =
    ];
    for (x, y) in steps {
        h.tap(x, y);
        h.tick();
        h.settle().expect("calc crashed during arithmetic sequence");
    }
}

#[test]
fn calc_divide_by_zero_does_not_crash() {
    let mut h = make_harness();
    h.tick();
    h.settle().expect("initial settle");

    // Tap: 5, ÷, 0, =
    let steps = [
        (70, 190),  // 2 (near 1-2-3 row)
        (150, 120), // ÷
        (30, 225),  // 0
        (150, 225), // =
    ];
    for (x, y) in steps {
        h.tap(x, y);
        h.tick();
        h.settle().expect("calc crashed on divide-by-zero sequence");
    }
}

#[test]
fn calc_ac_button_does_not_crash() {
    let mut h = make_harness();
    h.tick();
    h.settle().expect("initial settle");

    // Tap AC (top-left of button area)
    h.tap(30, 87);
    h.tick();
    h.settle().expect("calc crashed after AC");
}

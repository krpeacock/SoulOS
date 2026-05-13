// assets/scripts/button_demo.rhai
// Demonstrates all SoulOS button variants.

let app_id   = "com.soulos.button_demo";
let app_name = "Buttons";
let app_icon = "demo";

let toggle_a = false;
let toggle_b = false;
let last_action = "";

fn on_event(event) {
    if event.type == "AppStart" {
        invalidate_all();
    }
}

fn on_draw() {
    clear();
    title_bar("Button Variants");

    egui_begin();

    // --- Standard buttons ---
    egui_label((), "Standard:");
    egui_horizontal_layout((), |ui| {
        if egui_button(ui, "Normal") { last_action = "Normal"; }
        if egui_small_button(ui, "Small") { last_action = "Small"; }
        if egui_icon_button(ui, "×") { last_action = "Icon ×"; }
        if egui_icon_button(ui, "◀") { last_action = "Icon ◀"; }
    });

    egui_separator(());

    // --- Toggle buttons (tab-bar / filter pattern) ---
    egui_label((), "Toggles:");
    egui_horizontal_layout((), |ui| {
        if egui_toggle_button(ui, "A", toggle_a) {
            toggle_a = !toggle_a;
            last_action = "Toggle A: " + toggle_a;
        }
        if egui_toggle_button(ui, "B", toggle_b) {
            toggle_b = !toggle_b;
            last_action = "Toggle B: " + toggle_b;
        }
    });

    egui_separator(());

    // --- Fill button (primary action) ---
    egui_label((), "Primary action (fills width):");
    if egui_fill_button((), "Save Changes") {
        last_action = "Save";
    }

    egui_separator(());

    // --- Danger button ---
    egui_label((), "Destructive action:");
    egui_horizontal_layout((), |ui| {
        if egui_danger_button(ui, "Delete All") {
            last_action = "Delete All";
        }
    });

    if last_action != "" {
        egui_separator(());
        egui_label((), "Last: " + last_action);
    }

    egui_end();
}

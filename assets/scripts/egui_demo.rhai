// assets/scripts/egui_demo.rhai
// Comprehensive demo of egui components in Rhai

let app_id   = "com.soulos.egui_demo";
let app_name = "egui Demo";
let app_icon = "demo";

// Demo state
let text_input = "Hello egui!";
let password_input = "secret";
let multiline_text = "Line 1\nLine 2\nLine 3";
let slider_value = 50;
let selected_color = "Red";
let checkbox_state = true;
let radio_selection = "Option2";
let combo_selection = "Choice B";
let progress_value = 70;

fn on_event(event) {
    if event.type == "AppStart" {
        invalidate_all();
    }
    
    if event.type == "Key" {
        if event.key != "Other" {
            // Simple text input handling for demo
            if event.key == "Backspace" && text_input.len() > 0 {
                text_input = text_input.sub_string(0, text_input.len() - 1);
            } else if event.key != "Backspace" {
                text_input += event.key;
            }
            invalidate_all();
        }
    }
}

fn on_draw() {
    clear();
    title_bar("egui Components Demo");
    
    ui_begin();
    
    ui_heading("egui Component Showcase");
    ui_separator();
    
    // Text & Input Components
    ui_group("Text & Input");
    ui_label("Basic text label");
    ui_rich_text("Rich text in color", "blue");
    ui_hyperlink("Click me!");
    ui_monospace("Code text");
    
    ui_text_input(text_input, "Type here...");
    ui_text_edit_hint("", "Hint text example");
    ui_text_edit_password(password_input);
    ui_text_edit_multiline(multiline_text, 60);
    ui_code_editor("fn main() { println!(\"Hello\"); }");
    ui_group_end();
    
    ui_space(10);
    
    // Selection & Interaction
    ui_group("Selection & Interaction");
    
    ui_horizontal_begin();
    ui_checkbox(checkbox_state, "Enable feature");
    ui_radio(radio_selection == "Option1", "Option 1");
    ui_radio(radio_selection == "Option2", "Option 2");
    ui_horizontal_end();
    
    ui_slider(slider_value, 0, 100);
    ui_drag_value(42, "Value:");
    ui_combo_box("Choose:", combo_selection);
    ui_progress_bar(progress_value);
    ui_spinner();
    ui_color_edit(selected_color);
    
    ui_group_end();
    
    ui_space(10);
    
    // Layout Examples
    ui_group("Layout Examples");
    
    ui_columns(3);
    ui_button("Col 1");
    ui_next_column();
    ui_button("Col 2");
    ui_next_column();
    ui_button("Col 3");
    ui_columns_end();
    
    ui_horizontal_begin();
    ui_small_button("◀");
    ui_label("Navigation");
    ui_small_button("▶");
    ui_horizontal_end();
    
    ui_group_end();
    
    ui_space(10);
    
    // Visual Components
    ui_group("Visual Components");
    
    ui_image(80, 60, "Demo Image");
    ui_plot("Sample Chart");
    
    ui_table(3);
    ui_table_header("Name");
    ui_table_header("Value");
    ui_table_header("Status");
    ui_table_cell("Item 1");
    ui_table_cell("123");
    ui_table_cell("✓");
    ui_table_cell("Item 2");
    ui_table_cell("456");
    ui_table_cell("○");
    ui_table_end();
    
    ui_group_end();
    
    ui_space(10);
    
    // Interaction Examples
    ui_group("Menus & Windows");
    ui_menu("File Menu");
    ui_menu_item("New");
    ui_menu_item("Open");
    ui_menu_item("Save");
    ui_group_end();
    
    ui_space(10);
    ui_label("All egui components rendered successfully!");
}
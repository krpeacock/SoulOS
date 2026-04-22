// assets/scripts/egui_layout_demo.rhai
// Demonstration of modern EGUI layout containers and widgets

let app_id   = "com.soulos.egui_layout_demo";
let app_name = "EGUI Layout Demo";
let app_icon = "demo";

// Demo state
let button_clicked = false;
let input_text = "Type here...";
let selected_tab = 0;
let checkbox_state = true;
let slider_value = 50;
let scroll_position = 0;

fn on_event(event) {
    if event.type == "AppStart" {
        invalidate_all();
    }
    
    if event.type == "PenDown" {
        button_clicked = !button_clicked;
        invalidate_all();
    }
    
    if event.type == "Key" {
        if event.key != "Other" {
            if event.key == "Backspace" && input_text.len() > 0 {
                input_text = input_text.sub_string(0, input_text.len() - 1);
            } else if event.key != "Backspace" {
                input_text += event.key;
            }
            invalidate_all();
        }
    }
}

fn on_draw() {
    clear();
    title_bar("EGUI Layout Demo");
    
    // Modern layout using EGUI containers
    egui_begin();
    
    // Main layout with responsive design
    egui_central_panel(|ui| {
        
        // Heading section
        egui_heading(ui, "Modern Layout Containers");
        egui_separator(ui);
        
        // Two-column layout example
        egui_columns(ui, 2, |columns| {
            // Left column - Form layout
            egui_group(&mut columns[0], "Form Layout", |ui| {
                egui_form_layout(ui, |ui| {
                    egui_label(ui, "Name:");
                    egui_text_input(ui, input_text);
                    
                    egui_label(ui, "Email:");
                    egui_text_input(ui, "user@example.com");
                    
                    egui_label(ui, "Subscribe:");
                    egui_checkbox(ui, checkbox_state, "Newsletter");
                });
            });
            
            // Right column - Controls
            egui_group(&mut columns[1], "Controls", |ui| {
                egui_vertical_layout(ui, |ui| {
                    if egui_button(ui, "Primary Action") {
                        button_clicked = !button_clicked;
                    }
                    
                    egui_small_button(ui, "Secondary");
                    
                    egui_separator(ui);
                    
                    egui_label(ui, "Value: " + slider_value);
                    egui_slider(ui, slider_value, 0, 100);
                });
            });
        });
        
        egui_space(ui, 20);
        
        // Horizontal toolbar example
        egui_toolbar(ui, 40, |ui| {
            egui_horizontal_layout(ui, |ui| {
                egui_small_button(ui, "◀");
                egui_small_button(ui, "⏸");
                egui_small_button(ui, "▶");
                egui_separator(ui);
                egui_label(ui, "Toolbar Items");
            });
        });
        
        egui_space(ui, 10);
        
        // Responsive grid example
        egui_group(ui, "Responsive Grid", |ui| {
            let items = ["Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6"];
            egui_responsive_grid(ui, items, 80, |ui, item| {
                egui_selectable_label(ui, false, item);
            });
        });
        
        egui_space(ui, 10);
        
        // Scrollable content example
        egui_group(ui, "Scrollable Content", |ui| {
            egui_scroll_area(ui, 150, |ui| {
                for i in range(1, 21) {
                    egui_label(ui, "Scrollable item " + i);
                    egui_separator(ui);
                }
            });
        });
        
        egui_space(ui, 10);
        
        // Tab layout example (simplified)
        egui_group(ui, "Tab Layout", |ui| {
            egui_horizontal_layout(ui, |ui| {
                if egui_selectable_label(ui, selected_tab == 0, "Tab 1") {
                    selected_tab = 0;
                }
                if egui_selectable_label(ui, selected_tab == 1, "Tab 2") {
                    selected_tab = 1;
                }
                if egui_selectable_label(ui, selected_tab == 2, "Tab 3") {
                    selected_tab = 2;
                }
            });
            
            egui_separator(ui);
            
            if selected_tab == 0 {
                egui_label(ui, "Content for Tab 1");
                egui_label(ui, "This demonstrates modern layout management");
            } else if selected_tab == 1 {
                egui_label(ui, "Content for Tab 2");
                egui_label(ui, "Responsive and touch-friendly design");
            } else {
                egui_label(ui, "Content for Tab 3");
                egui_label(ui, "No more manual position calculations!");
            }
        });
        
        egui_space(ui, 10);
        
        // Status section
        if button_clicked {
            egui_colored_label(ui, "✓ Button was clicked!", "green");
        } else {
            egui_label(ui, "Click anywhere to test interaction");
        }
        
    });
    
    egui_end();
}

// Mock EGUI layout functions (these would be implemented in Rust)
fn egui_begin() {
    // Initialize EGUI frame
}

fn egui_end() {
    // Finalize EGUI frame and render
}

fn egui_central_panel(content_fn) {
    // Create central panel and call content function
    content_fn("ui_mock");
}

fn egui_heading(ui, text) {
    ui_label(text); // For now, fallback to existing UI
}

fn egui_separator(ui) {
    ui_separator();
}

fn egui_columns(ui, count, content_fn) {
    ui_columns(count);
    let columns = [];  // Mock columns array
    for i in range(0, count) {
        columns.push("column_" + i);
    }
    content_fn(columns);
    ui_columns_end();
}

fn egui_group(ui, title, content_fn) {
    ui_group(title);
    content_fn(ui);
    ui_group_end();
}

fn egui_form_layout(ui, content_fn) {
    content_fn(ui); // Simplified for now
}

fn egui_vertical_layout(ui, content_fn) {
    content_fn(ui); // Already vertical by default
}

fn egui_horizontal_layout(ui, content_fn) {
    ui_horizontal_begin();
    content_fn(ui);
    ui_horizontal_end();
}

fn egui_toolbar(ui, height, content_fn) {
    content_fn(ui); // Simplified toolbar
}

fn egui_scroll_area(ui, height, content_fn) {
    content_fn(ui); // For now, just render content (no scrolling)
}

fn egui_responsive_grid(ui, items, min_width, item_fn) {
    // Simplified responsive grid
    ui_horizontal_begin();
    for item in items {
        item_fn(ui, item);
    }
    ui_horizontal_end();
}

fn egui_label(ui, text) {
    ui_label(text);
}

fn egui_button(ui, text) {
    ui_button(text);
    return false; // Mock return value
}

fn egui_small_button(ui, text) {
    ui_small_button(text);
    return false; // Mock return value
}

fn egui_text_input(ui, text) {
    ui_text_input(text, "");
}

fn egui_checkbox(ui, checked, label) {
    ui_checkbox(checked, label);
}

fn egui_slider(ui, value, min, max) {
    ui_slider(value, min, max);
}

fn egui_selectable_label(ui, selected, text) {
    ui_selectable(selected, text);
    return false; // Mock return value
}

fn egui_colored_label(ui, text, color) {
    ui_rich_text(text, color);
}

fn egui_space(ui, height) {
    ui_space(height);
}
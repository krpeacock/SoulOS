// assets/scripts/address.rhai
// A simple Address Book app migrated to native EGUI.

let app_id   = "com.soulos.address";
let app_name = "Address";
let app_icon = "address";

let name_text = "";
let phone_text = "";

fn on_event(event) {
    if event.type == "AppStart" {
        invalidate_all();
    }
    
    if event.type == "Key" {
        // Keyboard input is mostly handled by EGUI but we keep it for now
        invalidate_all();
    }
}

fn on_draw() {
    clear();
    title_bar("Address Book");
    
    egui_begin();
    
    // 1. Scrollable list of contacts
    egui_scroll_area("contacts_list", 120, |ui| {
        let ids = db_get_ids_in_category(0);
        if ids.len() == 0 {
            egui_label(ui, "No contacts found.");
        } else {
            for i in 0..ids.len() {
                let id = ids[i];
                let text = to_string(db_get_data(id));
                
                egui_horizontal_layout(ui, |ui| {
                    egui_label(ui, text);
                    if egui_small_button(ui, "Delete") {
                        db_delete(id);
                    }
                });
                egui_separator(ui);
            }
        }
    });
    
    egui_space(ui, 10);
    
    // 2. Add contact form
    egui_group(ui, "Add Contact", |ui| {
        egui_vertical_layout(ui, |ui| {
            egui_horizontal_layout(ui, |ui| {
                egui_label(ui, "Name:");
                // In a real app we'd use egui_text_input, but for now we'll 
                // use buttons to trigger focus or manual entry
                if egui_button(ui, if name_text == "" { "Tap to enter name" } else { name_text }) {
                    // Manual focus logic or exchange could go here
                }
            });
            
            egui_horizontal_layout(ui, |ui| {
                egui_label(ui, "Phone:");
                if egui_button(ui, if phone_text == "" { "Tap to enter phone" } else { phone_text }) {
                }
            });
            
            if egui_button(ui, "Save Contact") {
                if name_text != "" {
                    db_insert(0, to_bytes(name_text + "|" + phone_text));
                    name_text = "";
                    phone_text = "";
                }
            }
        });
    });
    
    egui_end();
}

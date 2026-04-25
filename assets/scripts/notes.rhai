// assets/scripts/notes.rhai
// A simple Notes app migrated to native EGUI.

let app_id      = "com.soulos.notes";
let app_name    = "Notes";
let app_icon    = "notes";
let app_handles = ["open_script"];

let area = new_text_area(0, 15, 240, 190);
let kb   = new_keyboard();
let active_id    = 0;
let exchange_mode = false;  // true when opened via open_script exchange
let menu_open    = false;

fn on_event(event) {
    if event.type == "AppStart" {
        exchange_mode = false;
        menu_open = false;
        let ids = db_get_ids_in_category(0);
        if ids.len() == 0 {
            active_id = db_insert(0, to_bytes("welcome to soulos rhai notes."));
        } else {
            active_id = ids[0];
        }
        area.set_text(to_string(db_get_data(active_id)));
        invalidate_all();
    }

    if event.type == "Exchange" && event.action == "open_script" {
        exchange_mode = true;
        menu_open = false;
        area.set_text(event.payload.text);
        invalidate_all();
    }

    // Menu button toggles the menu overlay.
    if event.type == "Menu" {
        menu_open = !menu_open;
        invalidate_all();
    }

    if event.type == "Key" {
        if event.key == "Backspace" {
            area.backspace();
        } else if event.key == "Enter" {
            area.enter();
        } else if event.key == "ArrowLeft" {
            area.cursor_left();
        } else if event.key == "ArrowRight" {
            area.cursor_right();
        } else if event.key == "ArrowUp" {
            area.cursor_up();
        } else if event.key == "ArrowDown" {
            area.cursor_down();
        } else if event.key != "Other" {
            area.insert_char(event.key);
        }
        if !exchange_mode {
            db_update(active_id, to_bytes(area.get_text()));
        }
        invalidate_all();
    }

    if event.type == "ButtonDown" {
        if event.button == "PageUp" {
            area.page_up();
            invalidate_all();
        } else if event.button == "PageDown" {
            area.page_down();
            invalidate_all();
        }
    }

    if event.type == "PenDown" {
        // EGUI handles its own PenDown events if consumed
        if event.y >= 205 {
            handle_pen(keyboard, event.x, event.y, true);
        } else {
            area.pen_down(event.x, event.y, event.now_ms);
        }
        invalidate_all();
    }

    if event.type == "PenMove" {
        if event.y >= 205 {
            handle_pen(keyboard, event.x, event.y, true);
        } else {
            area.pen_move(event.x, event.y);
        }
        invalidate_all();
    }

    if event.type == "PenUp" {
        if event.y >= 205 {
            let key = handle_pen(keyboard, event.x, event.y, false);
            if key != "" {
                if key == "Backspace" {
                    area.backspace();
                } else if key == "Enter" {
                    area.enter();
                } else {
                    area.insert_char(key);
                }
                if !exchange_mode {
                    db_update(active_id, to_bytes(area.get_text()));
                }
            }
        } else {
            area.pen_up(event.x, event.y);
        }
        invalidate_all();
    }
}

fn on_draw() {
    clear();
    if exchange_mode {
        title_bar("Notes · Edit Script");
    } else {
        title_bar("Notes");
    }
    
    // Draw manual components
    draw_text_area(area);
    draw_keyboard(kb);

    // Overlay EGUI menu if open
    if menu_open {
        egui_begin();
        // Position menu near top
        egui_scroll_area("menu_scroll", 100, |ui| {
            egui_group(ui, "Notes Menu", |ui| {
                egui_vertical_layout(ui, |ui| {
                    if egui_button(ui, "Save") {
                        db_update(active_id, to_bytes(area.get_text()));
                        menu_open = false;
                    }
                    
                    let done_label = if exchange_mode { "Done ▶" } else { "Done" };
                    if egui_button(ui, done_label) {
                        if exchange_mode {
                            system_send_text_result("return_text", area.get_text());
                            exchange_mode = false;
                        } else {
                            db_update(active_id, to_bytes(area.get_text()));
                        }
                        menu_open = false;
                    }
                    
                    if egui_button(ui, "Close Menu") {
                        menu_open = false;
                    }
                });
            });
        });
        egui_end();
    }
}

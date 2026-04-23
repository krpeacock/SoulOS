// assets/scripts/notes.rhai
// A simple Notes app.
// When opened via open_script exchange (from Builder) the user edits the
// supplied text and taps "Done" in the menu to return the result.

let app_id      = "com.soulos.notes";
let app_name    = "Notes";
let app_icon    = "notes";
let app_handles = ["open_script"];

let area = new_text_area(0, 15, 240, 190);
let kb   = new_keyboard();
let active_id    = 0;
let exchange_mode = false;  // true when opened via open_script exchange
let menu_open    = false;
let menu_pressed = -1;      // index of button held during PenDown

// Menu layout — 2-column grid, 3 items: 0=Save  1=Done  2=Close menu
// These are plain functions so they're always in scope inside on_event/on_draw.
fn menu_item_x(i) { 15 + (i % 2) * 105 }
fn menu_item_y(i) { 25 + (i / 2) * 28  }
fn menu_w()       { 100 }
fn menu_h()       { 24  }

fn hit_menu(x, y) {
    for i in range(0, 3) {
        let mx = menu_item_x(i);
        let my = menu_item_y(i);
        if x >= mx && x < mx + menu_w() && y >= my && y < my + menu_h() {
            return i;
        }
    }
    return -1;
}

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

    // Menu button (hardware or system strip) toggles the menu overlay.
    if event.type == "Menu" {
        menu_open = !menu_open;
        menu_pressed = -1;
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

    // PageUp / PageDown hardware buttons scroll the text area.
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
        if menu_open {
            menu_pressed = hit_menu(event.x, event.y);
            invalidate_all();
            return;
        }
        if event.y >= 205 {
            kb.handle_pen(event.x, event.y, true);
        } else {
            area.pen_down(event.x, event.y, event.now_ms);
        }
        invalidate_all();
    }

    if event.type == "PenMove" {
        if menu_open { return; }
        if event.y >= 205 {
            kb.handle_pen(event.x, event.y, true);
        } else {
            area.pen_move(event.x, event.y);
        }
        invalidate_all();
    }

    if event.type == "PenUp" {
        if menu_open {
            let i = hit_menu(event.x, event.y);
            if i >= 0 && i == menu_pressed {
                menu_open = false;
                menu_pressed = -1;
                if i == 0 {
                    // Save
                    db_update(active_id, to_bytes(area.get_text()));
                } else if i == 1 {
                    // Done — in exchange_mode this returns the text to Builder;
                    // outside exchange_mode it just saves.
                    if exchange_mode {
                        system_send_text_result("return_text", area.get_text());
                        exchange_mode = false;
                    } else {
                        db_update(active_id, to_bytes(area.get_text()));
                    }
                }
                // i == 2: Close menu — already closed above
            } else {
                menu_pressed = -1;
            }
            invalidate_all();
            return;
        }

        if event.y >= 205 {
            let key = kb.handle_pen(event.x, event.y, false);
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
    if exchange_mode {
        title_bar("Notes · Edit Script");
    } else {
        title_bar("Notes");
    }
    draw_text_area(area);
    draw_keyboard(kb);

    if menu_open {
        // Draw a white panel with a border.
        draw_rect(10, 15, 220, 90, 255);   // white fill
        draw_rect(10, 15, 220, 1,  0);     // top border
        draw_rect(10, 15, 1,   90, 0);     // left border
        draw_rect(229, 15, 1,  90, 0);     // right border
        draw_rect(10, 104, 220, 1, 0);     // bottom border

        let done_label = if exchange_mode { "Done ▶" } else { "Done" };
        button(menu_item_x(0), menu_item_y(0), menu_w(), menu_h(), "Save",       menu_pressed == 0);
        button(menu_item_x(1), menu_item_y(1), menu_w(), menu_h(), done_label,   menu_pressed == 1);
        button(menu_item_x(2), menu_item_y(2), menu_w(), menu_h(), "Close menu", menu_pressed == 2);
    }
}

// assets/scripts/notes.rhai
// A simple Notes app.

let app_id      = "com.soulos.notes";
let app_name    = "Notes";
let app_icon    = "notes";
let app_handles = ["open_script"];

let area = new_text_area(0, 15, 240, 190);
let kb   = new_keyboard();
let active_id     = 0;
let pref_id       = 0;   // DB record that stores font preference (category 1)
let exchange_mode = false;
let menu_open     = false;
let menu_touch    = -1;  // button index currently pressed, -1 = none
let current_font  = "sans";  // "sans" | "serif" | "mono"

// Menu layout: 6 buttons inside a panel at (10, 25), each 24 px tall, 2 px gap.
// Button x=12, w=216.  Button i has top edge at 28 + i*26.
// Returns 0-5 when (x,y) hits a button, -1 otherwise.
fn menu_btn_idx(x, y) {
    if x < 12 || x > 228 || y < 28 { return -1; }
    let i = (y - 28) / 26;
    if i < 0 || i >= 6 { return -1; }
    if y >= 28 + i * 26 + 24 { return -1; }  // in the 2 px gap
    i
}

fn apply_font() {
    area.set_font(current_font);
}

fn on_event(event) {
    if event.type == "AppStart" {
        exchange_mode = false;
        menu_open     = false;
        menu_touch    = -1;

        let ids = db_get_ids_in_category(0);
        if ids.len() == 0 {
            active_id = db_insert(0, to_bytes("welcome to soulos rhai notes."));
        } else {
            active_id = ids[0];
        }
        area.set_text(to_string(db_get_data(active_id)));

        let pref_ids = db_get_ids_in_category(1);
        if pref_ids.len() == 0 {
            pref_id = db_insert(1, to_bytes("sans"));
        } else {
            pref_id = pref_ids[0];
            current_font = to_string(db_get_data(pref_id));
        }
        apply_font();
        invalidate_all();
    }

    if event.type == "Exchange" && event.action == "open_script" {
        exchange_mode = true;
        menu_open     = false;
        area.set_text(event.payload.text);
        invalidate_all();
    }

    if event.type == "Menu" {
        menu_open  = !menu_open;
        menu_touch = -1;
        invalidate_all();
    }

    if event.type == "Key" {
        if !menu_open {
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
    }

    if event.type == "ButtonDown" {
        if !menu_open {
            if event.button == "PageUp" {
                area.page_up();
                invalidate_all();
            } else if event.button == "PageDown" {
                area.page_down();
                invalidate_all();
            }
        }
    }

    if event.type == "PenDown" {
        if menu_open {
            menu_touch = menu_btn_idx(event.x, event.y);
            invalidate_all();
        } else if event.y >= 205 {
            handle_pen(kb, event.x, event.y, true);
            invalidate_all();
        } else {
            area.pen_down(event.x, event.y, event.now_ms);
            invalidate_all();
        }
    }

    if event.type == "PenMove" {
        if menu_open {
            let i = menu_btn_idx(event.x, event.y);
            if i != menu_touch {
                menu_touch = -1;
                invalidate_all();
            }
        } else if event.y >= 205 {
            handle_pen(kb, event.x, event.y, true);
            invalidate_all();
        } else {
            area.pen_move(event.x, event.y);
            invalidate_all();
        }
    }

    if event.type == "PenUp" {
        if menu_open {
            let i = menu_btn_idx(event.x, event.y);
            if i == menu_touch && i >= 0 {
                if i == 0 {
                    // Save
                    db_update(active_id, to_bytes(area.get_text()));
                    menu_open = false;
                } else if i == 1 {
                    // Done / Done ▶
                    if exchange_mode {
                        system_send_text_result("return_text", area.get_text());
                        exchange_mode = false;
                    } else {
                        db_update(active_id, to_bytes(area.get_text()));
                    }
                    menu_open = false;
                } else if i == 2 {
                    current_font = "sans";
                    apply_font();
                    db_update(pref_id, to_bytes(current_font));
                    menu_open = false;
                } else if i == 3 {
                    current_font = "serif";
                    apply_font();
                    db_update(pref_id, to_bytes(current_font));
                    menu_open = false;
                } else if i == 4 {
                    current_font = "mono";
                    apply_font();
                    db_update(pref_id, to_bytes(current_font));
                    menu_open = false;
                } else if i == 5 {
                    menu_open = false;
                }
            }
            menu_touch = -1;
            invalidate_all();
        } else if event.y >= 205 {
            let key = handle_pen(kb, event.x, event.y, false);
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
            invalidate_all();
        } else {
            area.pen_up(event.x, event.y);
            invalidate_all();
        }
    }
}

fn on_draw() {
    clear();
    if exchange_mode {
        title_bar("Notes · Edit Script");
    } else {
        title_bar("Notes");
    }

    draw_text_area(area);
    draw_keyboard(kb);

    if menu_open {
        // Opaque white panel with 1 px black border.
        draw_rect(9, 24, 222, 160, 0);    // border (black)
        draw_rect(10, 25, 220, 158, 255); // fill (white)

        let done_lbl  = if exchange_mode { "Done >" } else { "Done" };
        let sans_lbl  = if current_font == "sans"  { "> Sans"  } else { "  Sans"  };
        let serif_lbl = if current_font == "serif" { "> Serif" } else { "  Serif" };
        let mono_lbl  = if current_font == "mono"  { "> Mono"  } else { "  Mono"  };

        button(12, 28 + 0 * 26, 216, 24, "Save",     menu_touch == 0);
        button(12, 28 + 1 * 26, 216, 24, done_lbl,   menu_touch == 1);
        button(12, 28 + 2 * 26, 216, 24, sans_lbl,   menu_touch == 2);
        button(12, 28 + 3 * 26, 216, 24, serif_lbl,  menu_touch == 3);
        button(12, 28 + 4 * 26, 216, 24, mono_lbl,   menu_touch == 4);
        button(12, 28 + 5 * 26, 216, 24, "Close",    menu_touch == 5);
    }
}

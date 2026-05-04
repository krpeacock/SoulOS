// assets/scripts/notes.rhai
// Palm-style Memo app: list → edit, multiple notes, font switching.

let app_id      = "com.soulos.notes";
let app_name    = "Notes";
let app_icon    = "notes";
let app_handles = ["open_script"];

// "list" shows all notes; "edit" edits the selected note.
let mode          = "list";
let active_id     = -1;
let pref_id       = 0;   // DB record that stores font preference (category 1)
let current_font  = "sans";  // "sans" | "serif" | "mono"
let exchange_mode = false;
let menu_open     = false;
let menu_touch    = -1;  // button index currently highlighted, -1 = none
let list_page     = 0;   // current page in list view (12 rows/page)

// Text editor and soft keyboard (created once, reused across notes).
// Keyboard sits at y = APP_HEIGHT(304) − KB_HEIGHT(96) = 208.
let area = new_text_area(0, 15, 240, 193);
let kb   = new_keyboard();
const KB_Y = 208;

// --- Helpers ---

fn hit(px, py, bx, by, bw, bh) {
    px >= bx && px < bx + bw && py >= by && py < by + bh
}

// Menu panel: 6 buttons at (12, 28+i*26), each 216×24.  2 px gap between buttons.
// Returns 0–5 for a hit, -1 otherwise.
fn menu_btn_idx(x, y) {
    if x < 12 || x > 228 || y < 28 { return -1; }
    let i = (y - 28) / 26;
    if i < 0 || i >= 6 { return -1; }
    if y >= 28 + i * 26 + 24 { return -1; }
    i
}

fn note_title(text) {
    let nl = text.index_of("\n");
    let first = if nl >= 0 { text.sub_string(0, nl) } else { text };
    if first.len() == 0   { return "(empty)"; }
    if first.len() > 34   { return first.sub_string(0, 34) + "..."; }
    first
}

fn apply_font() {
    area.set_font(current_font);
}

fn save_current() {
    if active_id >= 0 && !exchange_mode {
        db_update(active_id, to_bytes(area.get_text()));
    }
}

fn open_note(id) {
    active_id = id;
    area.set_text(to_string(db_get_data(id)));
    apply_font();
    mode = "edit";
    menu_open = false;
    menu_touch = -1;
    invalidate_all();
}

fn new_note() {
    let id = db_insert(0, to_bytes(""));
    open_note(id);
}

fn go_back() {
    save_current();
    mode = "list";
    menu_open = false;
    menu_touch = -1;
    active_id = -1;
    list_page = 0;
    invalidate_all();
}

// List-view layout constants (rows of height LIST_ROW_H starting at LIST_TOP).
const LIST_TOP    = 16;
const LIST_ROW_H  = 22;
const LIST_ROWS   = 12;
const LIST_BTN_Y  = 282;  // "New" button at bottom

// --- Events ---

fn on_event(event) {
    if event.type == "AppStart" {
        exchange_mode = false;
        menu_open     = false;
        menu_touch    = -1;
        mode          = "list";

        let ids = db_get_ids_in_category(0);
        if ids.len() == 0 {
            db_insert(0, to_bytes("Welcome to Notes.\nTap a note to edit it, or tap New to write."));
        }

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
        menu_touch    = -1;
        area.set_text(event.payload.text);
        apply_font();
        mode = "edit";
        invalidate_all();
    }

    if event.type == "Menu" && mode == "edit" {
        menu_open  = !menu_open;
        menu_touch = -1;
        invalidate_all();
    }

    if event.type == "ButtonDown" {
        if event.button == "PageUp"   && mode == "edit" && !menu_open { area.page_up();   invalidate_all(); }
        if event.button == "PageDown" && mode == "edit" && !menu_open { area.page_down(); invalidate_all(); }

        if mode == "list" {
            let ids = db_get_ids_in_category(0);
            let max_page = if ids.len() == 0 { 0 } else { (ids.len() - 1) / LIST_ROWS };
            if event.button == "PageUp"   && list_page > 0        { list_page -= 1; invalidate_all(); }
            if event.button == "PageDown" && list_page < max_page  { list_page += 1; invalidate_all(); }
        }
    }

    if event.type == "PenUp" && mode == "list" {
        let ids = db_get_ids_in_category(0);
        // "New" button
        if hit(event.x, event.y, 2, LIST_BTN_Y, 60, 20) {
            new_note();
        } else {
            // Note rows
            for i in 0..LIST_ROWS {
                let idx = list_page * LIST_ROWS + i;
                if idx >= ids.len() { break; }
                let row_y = LIST_TOP + i * LIST_ROW_H;
                if hit(event.x, event.y, 2, row_y, 236, LIST_ROW_H - 2) {
                    open_note(ids[idx]);
                    break;
                }
            }
        }
        invalidate_all();
    }

    // Physical keyboard → text area (edit mode only, not while menu is open).
    if event.type == "Key" && mode == "edit" && !menu_open {
        let k = event.key;
        if      k == "Backspace"  { area.backspace(); }
        else if k == "Enter"      { area.enter(); }
        else if k == "ArrowLeft"  { area.cursor_left(); }
        else if k == "ArrowRight" { area.cursor_right(); }
        else if k == "ArrowUp"    { area.cursor_up(); }
        else if k == "ArrowDown"  { area.cursor_down(); }
        else if k != "Other"      { area.insert_char(k); }
        save_current();
        invalidate_all();
    }

    // Pen events in edit mode: routed to menu, keyboard, or text area.
    if mode == "edit" {
        if menu_open {
            if event.type == "PenDown" {
                menu_touch = menu_btn_idx(event.x, event.y);
                invalidate_all();
            }
            if event.type == "PenMove" {
                let i = menu_btn_idx(event.x, event.y);
                if i != menu_touch { menu_touch = -1; invalidate_all(); }
            }
            if event.type == "PenUp" {
                let i = menu_btn_idx(event.x, event.y);
                if i == menu_touch && i >= 0 {
                    if !exchange_mode {
                        if      i == 0 { save_current(); new_note(); }
                        else if i == 1 {
                            if active_id >= 0 { db_delete(active_id); }
                            active_id = -1; mode = "list"; menu_open = false;
                        }
                        else if i == 2 { go_back(); }
                        else if i == 3 { current_font = "sans";  apply_font(); db_update(pref_id, to_bytes(current_font)); menu_open = false; }
                        else if i == 4 { current_font = "serif"; apply_font(); db_update(pref_id, to_bytes(current_font)); menu_open = false; }
                        else if i == 5 { current_font = "mono";  apply_font(); db_update(pref_id, to_bytes(current_font)); menu_open = false; }
                    } else {
                        if      i == 0 { system_send_text_result("return_text", area.get_text()); exchange_mode = false; mode = "list"; menu_open = false; }
                        else if i == 1 { exchange_mode = false; mode = "list"; menu_open = false; }
                        else if i == 3 { current_font = "sans";  apply_font(); db_update(pref_id, to_bytes(current_font)); menu_open = false; }
                        else if i == 4 { current_font = "serif"; apply_font(); db_update(pref_id, to_bytes(current_font)); menu_open = false; }
                        else if i == 5 { current_font = "mono";  apply_font(); db_update(pref_id, to_bytes(current_font)); menu_open = false; }
                    }
                }
                menu_touch = -1;
                invalidate_all();
            }
        } else {
            // Normal edit: route pen to soft keyboard or text area.
            if event.type == "PenDown" {
                if event.y >= KB_Y { handle_pen(kb, event.x, event.y, true); }
                else               { area.pen_down(event.x, event.y, event.now_ms); }
                invalidate_all();
            }
            if event.type == "PenMove" {
                if event.y >= KB_Y { handle_pen(kb, event.x, event.y, true); }
                else               { area.pen_move(event.x, event.y); }
                invalidate_all();
            }
            if event.type == "PenUp" {
                if event.y >= KB_Y {
                    let key = handle_pen(kb, event.x, event.y, false);
                    if      key == "Backspace" { area.backspace(); }
                    else if key == "Enter"     { area.enter(); }
                    else if key != ""          { area.insert_char(key); }
                    save_current();
                } else {
                    area.pen_up(event.x, event.y);
                }
                invalidate_all();
            }
        }
    }
}

// --- Drawing ---

fn on_draw() {
    clear();

    if mode == "list" {
        title_bar("Notes");

        let ids = db_get_ids_in_category(0);
        if ids.len() == 0 {
            label(10, LIST_TOP + 4, "No notes yet. Tap New to start.");
        } else {
            for i in 0..LIST_ROWS {
                let idx = list_page * LIST_ROWS + i;
                if idx >= ids.len() { break; }
                let row_y = LIST_TOP + i * LIST_ROW_H;
                let txt = to_string(db_get_data(ids[idx]));
                button(2, row_y, 236, LIST_ROW_H - 2, note_title(txt), false);
            }
        }

        button(2, LIST_BTN_Y, 60, 20, "New", false);

    } else {
        // Edit view.
        let bar_label = if exchange_mode { "Notes · Script" } else { "Notes" };
        title_bar(bar_label);
        draw_text_area(area);
        draw_keyboard(kb);

        if menu_open {
            // Opaque white panel with 1 px black border.
            draw_rect(9, 24, 222, 160, 0);    // border
            draw_rect(10, 25, 220, 158, 255); // fill

            let sans_lbl  = if current_font == "sans"  { "> Sans"  } else { "  Sans"  };
            let serif_lbl = if current_font == "serif" { "> Serif" } else { "  Serif" };
            let mono_lbl  = if current_font == "mono"  { "> Mono"  } else { "  Mono"  };

            if !exchange_mode {
                button(12, 28 + 0 * 26, 216, 24, "New Note",  menu_touch == 0);
                button(12, 28 + 1 * 26, 216, 24, "Delete",    menu_touch == 1);
                button(12, 28 + 2 * 26, 216, 24, "< Back",    menu_touch == 2);
                button(12, 28 + 3 * 26, 216, 24, sans_lbl,    menu_touch == 3);
                button(12, 28 + 4 * 26, 216, 24, serif_lbl,   menu_touch == 4);
                button(12, 28 + 5 * 26, 216, 24, mono_lbl,    menu_touch == 5);
            } else {
                button(12, 28 + 0 * 26, 216, 24, "Done >",    menu_touch == 0);
                button(12, 28 + 1 * 26, 216, 24, "Cancel",    menu_touch == 1);
                button(12, 28 + 3 * 26, 216, 24, sans_lbl,    menu_touch == 3);
                button(12, 28 + 4 * 26, 216, 24, serif_lbl,   menu_touch == 4);
                button(12, 28 + 5 * 26, 216, 24, mono_lbl,    menu_touch == 5);
            }
        }
    }
}

// assets/scripts/notes.rhai
// A simple Notes app rewritten in Rhai

let area = new_text_area(0, 15, 240, 190);
let kb = new_keyboard();
let active_id = 0;

fn on_event(event) {
    if event.type == "AppStart" {
        let ids = db_get_ids_in_category(0);
        if ids.len() == 0 {
            active_id = db_insert(0, to_bytes("welcome to soulos rhai notes."));
        } else {
            active_id = ids[0];
        }
        area.set_text(to_string(db_get_data(active_id)));
        invalidate_all();
    }
    
    if event.type == "Key" {
        if event.key == "Backspace" {
            area.backspace();
        } else if event.key == "Enter" {
            area.enter();
        } else if event.key != "Other" {
            area.insert_char(event.key);
        }
        db_update(active_id, to_bytes(area.get_text()));
        invalidate_all();
    }
    
    if event.type == "PenDown" {
        if event.y >= 205 {
            let key = kb.handle_pen(event.x, event.y, true);
            // pen_down on keyboard doesn't produce char usually, but we call it for highlights
        } else {
            area.pen_down(event.x, event.y, event.now_ms);
        }
        invalidate_all();
    }
    
    if event.type == "PenMove" {
        if event.y >= 205 {
            kb.handle_pen(event.x, event.y, true);
        } else {
            area.pen_move(event.x, event.y);
        }
        invalidate_all();
    }
    
    if event.type == "PenUp" {
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
                db_update(active_id, to_bytes(area.get_text()));
            }
        } else {
            area.pen_up(event.x, event.y);
        }
        invalidate_all();
    }
}

fn on_draw() {
    title_bar("Notes");
    draw_text_area(area);
    draw_keyboard(kb);
}

// assets/scripts/address.rhai
// A simple Address Book app rewritten in Rhai

let name_input = new_text_input(10, 150, 220, 20);
let phone_input = new_text_input(10, 180, 220, 20);
let focus = "name"; // or "phone"

fn on_event(event) {
    if event.type == "AppStart" {
        invalidate_all();
    }
    
    if event.type == "Key" {
        let active = if focus == "name" { name_input } else { phone_input };
        if event.key == "Backspace" {
            active.backspace();
        } else if event.key == "Enter" {
            if focus == "name" { focus = "phone"; }
            else {
                let name = name_input.get_text();
                let phone = phone_input.get_text();
                if name != "" {
                    db_insert(0, to_bytes(name + "|" + phone));
                    name_input.set_text("");
                    phone_input.set_text("");
                    focus = "name";
                }
            }
        } else if event.key != "Other" {
            active.insert_char(event.key);
        }
        invalidate_all();
    }
    
    if event.type == "PenDown" {
        if event.y >= 150 && event.y < 170 { focus = "name"; }
        if event.y >= 180 && event.y < 200 { focus = "phone"; }
        
        // Add button
        if event.y >= 210 && event.y < 234 && event.x >= 10 && event.x < 100 {
            let name = name_input.get_text();
            let phone = phone_input.get_text();
            if name != "" {
                db_insert(0, to_bytes(name + "|" + phone));
                name_input.set_text("");
                phone_input.set_text("");
                focus = "name";
            }
        }

        let ids = db_get_ids_in_category(0);
        for i in 0..ids.len() {
            let item_y = 30 + i * 20;
            if event.y >= item_y && event.y < item_y + 18 && event.x >= 210 && event.x < 230 {
                db_delete(ids[i]);
                break;
            }
        }
        invalidate_all();
    }
}

fn on_draw() {
    title_bar("Address Book");
    
    let ids = db_get_ids_in_category(0);
    for i in 0..ids.len() {
        let text = to_string(db_get_data(ids[i]));
        label(10, 30 + i * 20, text);
        button(210, 30 + i * 20, 20, 18, "X", false);
    }
    
    label(10, 140, "Name:");
    draw_text_input(name_input);
    label(10, 170, "Phone:");
    draw_text_input(phone_input);
    
    button(10, 210, 90, 24, "Add Contact", false);
    
    // Draw focus indicator
    if focus == "name" { draw_rect(8, 148, 2, 24, 0); }
    else { draw_rect(8, 178, 2, 24, 0); }
}

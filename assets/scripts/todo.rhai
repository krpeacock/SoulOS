// assets/scripts/todo.rhai
// A simple Todo app rewritten in Rhai with Add/Delete support and Text Input

let input = new_text_input(10, 170, 160, 24);

fn on_event(event) {
    if event.type == "AppStart" {
        invalidate_all();
    }
    
    if event.type == "Key" {
        if event.key == "Backspace" {
            input.backspace();
        } else if event.key == "Enter" {
            let text = input.get_text();
            if text != "" {
                db_insert(0, to_bytes("[ ] " + text));
                input.set_text("");
            }
        } else if event.key != "Other" {
            input.insert_char(event.key);
        }
        invalidate_all();
    }
    
    if event.type == "PenDown" {
        let x = event.x;
        let y = event.y;
        
        // Check "Add" button
        if y >= 170 && y < 194 && x >= 180 && x < 230 {
            let text = input.get_text();
            if text != "" {
                db_insert(0, to_bytes("[ ] " + text));
                input.set_text("");
                invalidate_all();
            }
            return;
        }

        let ids = db_get_ids_in_category(0);
        for i in 0..ids.len() {
            let item_y = 40 + i * 25;
            
            // Check delete button (small X on the right)
            if y >= item_y && y < item_y + 20 && x >= 210 && x < 230 {
                db_delete(ids[i]);
                invalidate_all();
                break;
            }

            // Check toggle
            if y >= item_y && y < item_y + 20 && x >= 10 && x < 200 {
                let id = ids[i];
                let data = db_get_data(id);
                let text = to_string(data);
                
                if text.index_of("[x] ") == 0 {
                    db_update(id, to_bytes("[ ] " + text.sub_string(4)));
                } else if text.index_of("[ ] ") == 0 {
                    db_update(id, to_bytes("[x] " + text.sub_string(4)));
                }
                
                invalidate_all();
                break;
            }
        }
    }
}

fn on_draw() {
    clear();
    title_bar("Rhai Todo");
    
    let ids = db_get_ids_in_category(0);
    
    for i in 0..ids.len() {
        let id = ids[i];
        let text = to_string(db_get_data(id));
        
        // Main task button
        button(10, 40 + i * 25, 190, 20, text, text.index_of("[x]") == 0);
        
        // Delete button
        button(210, 40 + i * 25, 20, 20, "X", false);
    }
    
    // Add UI
    draw_text_input(input);
    button(180, 170, 50, 24, "Add", false);
    label(10, 200, "Type task & press Enter or Add");
}

// assets/scripts/todo.rhai
// Modern Todo app using proper EGUI scrolling and layout.

let app_id   = "com.soulos.todo";
let app_name = "Todo";
let app_icon = "todo";

let new_task_text = "";
let task_filter = "all"; // "all", "active", "completed"

fn add_task(text) {
    if text.trim() != "" {
        db_insert(0, to_bytes("[ ] " + text.trim()));
        invalidate_all();
        return true;
    }
    return false;
}

fn toggle_task(id) {
    let data = db_get_data(id);
    let text = to_string(data);
    
    if text.index_of("[x] ") == 0 {
        db_update(id, to_bytes("[ ] " + text.sub_string(4)));
    } else if text.index_of("[ ] ") == 0 {
        db_update(id, to_bytes("[x] " + text.sub_string(4)));
    }
    invalidate_all();
}

fn delete_task(id) {
    db_delete(id);
    invalidate_all();
}

fn clear_completed() {
    let ids = db_get_ids_in_category(0);
    for id in ids {
        let text = to_string(db_get_data(id));
        if text.index_of("[x] ") == 0 {
            db_delete(id);
        }
    }
    invalidate_all();
}

fn get_filtered_tasks(filter) {
    let ids = db_get_ids_in_category(0);
    let filtered = [];
    
    for id in ids {
        let text = to_string(db_get_data(id));
        let is_completed = text.index_of("[x] ") == 0;
        
        if filter == "all" || 
           (filter == "active" && !is_completed) ||
           (filter == "completed" && is_completed) {
            filtered.push(#{id: id, text: text, completed: is_completed});
        }
    }
    return filtered;
}

fn on_event(event) {
    if event.type == "AppStart" {
        invalidate_all();
    }
}

fn on_draw() {
    clear();
    title_bar("Todo");
    
    egui_begin();
    
    // 1. Header with filters
    egui_toolbar(320, 30, |ui| {
        egui_horizontal_layout(ui, |ui| {
            if egui_selectable_label(ui, task_filter == "all", "All") {
                task_filter = "all";
            }
            if egui_selectable_label(ui, task_filter == "active", "Active") {
                task_filter = "active";
            }
            if egui_selectable_label(ui, task_filter == "completed", "Done") {
                task_filter = "completed";
            }
        });
    });
    
    // 2. Scrollable task list
    egui_scroll_area("todo_list", 160, |ui| {
        let tasks = get_filtered_tasks(task_filter);
        
        if tasks.len() == 0 {
            egui_label(ui, "No tasks found.");
        } else {
            for i in 0..tasks.len() {
                let task = tasks[i];
                egui_horizontal_layout(ui, |ui| {
                    let text = task.text.sub_string(4);
                    if egui_selectable_label(ui, task.completed, text) {
                        toggle_task(task.id);
                    }
                    if egui_small_button(ui, "x") {
                        delete_task(task.id);
                    }
                });
                egui_separator(ui);
            }
        }
    });
    
    egui_space(ui, 5);
    
    // 3. Add task area
    egui_group(ui, "Add Task", |ui| {
        egui_horizontal_layout(ui, |ui| {
            if egui_button(ui, if new_task_text == "" { "Tap to type..." } else { new_task_text }) {
                // In a future update, this would trigger the keyboard
            }
            if egui_button(ui, "Add") {
                if add_task(new_task_text) {
                    new_task_text = "";
                }
            }
        });
    });
    
    // 4. Stats
    let all_tasks = db_get_ids_in_category(0);
    if all_tasks.len() > 0 {
        egui_horizontal_layout(ui, |ui| {
            egui_label(ui, all_tasks.len() + " tasks");
            if egui_button(ui, "Clear Done") {
                clear_completed();
            }
        });
    }
    
    egui_end();
}

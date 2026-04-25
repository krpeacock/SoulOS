// assets/scripts/todo_with_egui_scroll.rhai
// Modern Todo app using proper EGUI scrolling instead of manual scroll handling

let app_id   = "com.soulos.todo";
let app_name = "Todo (EGUI Scroll)";
let app_icon = "todo";

// Modern UI state - no more manual scroll variables!
let new_task_text = "";
let task_filter = "all"; // "all", "active", "completed"
let input_focused = false;
let keyboard_visible = false;
let keyboard = new_keyboard();

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
    
    if event.type == "Key" && input_focused {
        if event.key == "Backspace" && new_task_text.len() > 0 {
            new_task_text = new_task_text.sub_string(0, new_task_text.len() - 1);
            invalidate_all();
        } else if event.key == "Enter" {
            if add_task(new_task_text) {
                new_task_text = "";
                input_focused = false;
                keyboard_visible = false;
                invalidate_all();
            }
        } else if event.key == "Escape" {
            input_focused = false;
            keyboard_visible = false;
            invalidate_all();
        } else if event.key != "Other" && event.key != "Backspace" && event.key != "Enter" {
            new_task_text += event.key;
            invalidate_all();
        }
    }
    
    if event.type == "PenMove" {
        let x = event.x;
        let y = event.y;
        
        // Handle keyboard hover if visible
        if keyboard_visible && y >= 220 {
            handle_pen(keyboard, x, y, true);
            invalidate_all();
        }
        // Note: No more manual scroll handling - EGUI ScrollArea handles this automatically!
    }
    
    if event.type == "PenUp" {
        let x = event.x;
        let y = event.y;
        
        // Handle keyboard input if visible
        if keyboard_visible && y >= 220 {
            let key = handle_pen(keyboard, x, y, false);
            if key != "" {
                if key == "Backspace" {
                    if new_task_text.len() > 0 {
                        new_task_text = new_task_text.sub_string(0, new_task_text.len() - 1);
                        invalidate_all();
                    }
                } else if key == "Enter" {
                    if add_task(new_task_text) {
                        new_task_text = "";
                        input_focused = false;
                        keyboard_visible = false;
                        invalidate_all();
                    }
                } else {
                    new_task_text += key;
                    invalidate_all();
                }
            }
            return;
        }
    }
    
    if event.type == "PenDown" {
        let x = event.x;
        let y = event.y;
        
        // Handle keyboard press down if visible
        if keyboard_visible && y >= 220 {
            handle_pen(keyboard, x, y, true);
            invalidate_all();
            return;
        }
        
        // Filter buttons - fixed positions (no scroll offset needed)
        if y >= 30 && y < 50 {
            if x >= 10 && x < 70 {
                task_filter = "all";
                invalidate_all();
            } else if x >= 80 && x < 140 {
                task_filter = "active"; 
                invalidate_all();
            } else if x >= 150 && x < 230 {
                task_filter = "completed";
                invalidate_all();
            }
        }
        
        // Add button - adjust for keyboard mode
        let input_y = if keyboard_visible { 170 } else { 200 };
        if y >= input_y && y < input_y + 24 && x >= 180 && x < 230 {
            if add_task(new_task_text) {
                new_task_text = "";
                invalidate_all();
            }
        }
        
        // Input area (focus for typing)
        if y >= input_y && y < input_y + 24 && x >= 10 && x < 170 {
            if !input_focused {
                input_focused = true;
                keyboard_visible = true;
                invalidate_all();
            }
        } else if y < 220 && !keyboard_visible {
            if input_focused {
                input_focused = false;
                keyboard_visible = false;
                invalidate_all();
            }
        }
        
        // Clear completed button
        if !keyboard_visible && y >= 250 && y < 270 && x >= 10 && x < 110 {
            clear_completed();
        }
        
        // Task list interactions - EGUI ScrollArea will handle coordinate mapping
        let tasks = get_filtered_tasks(task_filter);
        let list_start_y = 60;
        
        for i in 0..tasks.len() {
            let task = tasks[i];
            let y_pos = list_start_y + i * 25;
            
            // Delete button (coordinates will be handled by scroll area)
            if y >= y_pos && y < y_pos + 20 && x >= 210 && x < 230 {
                delete_task(task.id);
                break;
            }
            
            // Task toggle
            if y >= y_pos && y < y_pos + 20 && x >= 10 && x < 200 {
                toggle_task(task.id);
                break;
            }
        }
    }
}

fn on_draw() {
    clear();
    title_bar("Todo (EGUI Scroll)");
    
    // Begin EGUI layout system
    egui_begin();
    
    // Fixed header area (not scrolled)
    egui_toolbar(320, 30, |ui| {
        // Filter buttons in toolbar
        egui_horizontal_layout(ui, |ui| {
            if egui_selectable_label(ui, task_filter == "all", "All") {
                task_filter = "all";
            }
            if egui_selectable_label(ui, task_filter == "active", "Active") {
                task_filter = "active";
            }
            if egui_selectable_label(ui, task_filter == "completed", "Completed") {
                task_filter = "completed";
            }
        });
    });
    
    // Main scrollable content area
    let content_height = if keyboard_visible { 140 } else { 180 };
    egui_scroll_area("todo_list", content_height, |ui| {
        
        // Task list
        let tasks = get_filtered_tasks(task_filter);
        
        if tasks.len() == 0 {
            egui_label(ui, "No tasks to show");
        } else {
            egui_group(ui, "Tasks", |ui| {
                for i in 0..tasks.len() {
                    let task = tasks[i];
                    
                    egui_horizontal_layout(ui, |ui| {
                        // Task checkbox and text
                        let display_text = task.text.sub_string(4); // Remove "[ ] " or "[x] "
                        let prefix = if task.completed { "[X]" } else { "[ ]" };
                        let full_text = prefix + " " + display_text;
                        
                        if egui_selectable_label(ui, task.completed, full_text) {
                            toggle_task(task.id);
                        }
                        
                        // Delete button
                        if egui_small_button(ui, "×") {
                            delete_task(task.id);
                        }
                    });
                    
                    egui_separator(ui);
                }
            });
        }
        
        // Add some extra content to demonstrate scrolling
        if !keyboard_visible {
            egui_space(ui, 10);
            egui_group(ui, "Scroll Test", |ui| {
                for i in 0..15 {
                    egui_label(ui, "This is scroll test line " + i);
                }
            });
        }
    });
    
    // Fixed bottom area (input and stats)
    let input_y = if keyboard_visible { 170 } else { 200 };
    
    // Input area
    egui_group("input_area", |ui| {
        egui_label(ui, "Add new task:");
        
        egui_horizontal_layout(ui, |ui| {
            // Text input with focus styling
            let input_display = if new_task_text == "" { 
                if input_focused { "Type here... |" } else { "Type here..." }
            } else { 
                if input_focused { new_task_text + " |" } else { new_task_text }
            };
            
            if egui_button(ui, input_display) {
                input_focused = !input_focused;
                keyboard_visible = input_focused;
            }
            
            // Add button
            if egui_button(ui, "Add") {
                if add_task(new_task_text) {
                    new_task_text = "";
                    input_focused = false;
                    keyboard_visible = false;
                }
            }
        });
    });
    
    // Stats and clear button (when keyboard not visible)
    if !keyboard_visible {
        let all_tasks = db_get_ids_in_category(0);
        let completed_count = 0;
        for id in all_tasks {
            let text = to_string(db_get_data(id));
            if text.index_of("[x] ") == 0 {
                completed_count += 1;
            }
        }
        
        egui_horizontal_layout(|ui| {
            let stats_text = all_tasks.len() + " total, " + completed_count + " done";
            egui_label(ui, stats_text);
            
            if completed_count > 0 {
                if egui_button(ui, "Clear Done") {
                    clear_completed();
                }
            }
        });
    }
    
    // Show keyboard if input is focused
    if keyboard_visible {
        draw_keyboard(keyboard);
    }
    
    egui_end();
}

// End of script
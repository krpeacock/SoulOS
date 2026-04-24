// assets/scripts/todo.rhai
// A modern Todo app using egui components in Rhai

let app_id   = "com.soulos.todo";
let app_name = "Todo";
let app_icon = "todo";

// Modern UI state
let new_task_text = "";
let task_filter = "all"; // "all", "active", "completed"
let input_focused = false;
let keyboard_visible = false;
let keyboard = new_keyboard();
let scroll_offset = 0;
let content_height = 200; // Height when keyboard is hidden
let keyboard_height = 100; // Approximate keyboard height

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
                scroll_offset = 0;
                invalidate_all();
            }
        } else if event.key == "Escape" {
            // ESC key closes keyboard
            input_focused = false;
            keyboard_visible = false;
            scroll_offset = 0;
            invalidate_all();
        } else if event.key != "Other" && event.key != "Backspace" && event.key != "Enter" {
            new_task_text += event.key;
            invalidate_all();
        }
    }
    
    if event.type == "PenMove" {
        let x = event.x;
        let y = event.y;
        
        // Handle keyboard hover if visible (no key output, just visual feedback)
        if keyboard_visible && y >= 220 {
            handle_pen(keyboard, x, y, true);
            invalidate_all();
        }
    }
    
    if event.type == "PenUp" {
        let x = event.x;
        let y = event.y;
        
        // Handle keyboard input if visible - this is where keys are actually typed
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
                        scroll_offset = 0;
                        invalidate_all();
                    }
                } else {
                    new_task_text += key;
                    invalidate_all();
                }
            }
            return; // Don't process other pen events if keyboard handled it
        }
    }
    
    if event.type == "PenDown" {
        let x = event.x;
        let y = event.y;
        
        // Handle keyboard press down if visible (start of key press)
        if keyboard_visible && y >= 220 {
            handle_pen(keyboard, x, y, true);
            invalidate_all();
            return; // Don't process other UI elements when keyboard is being used
        }
        
        // Filter buttons
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
        
        // Scroll handling for task list (when keyboard visible)
        if keyboard_visible && x >= 200 && x < 220 {
            let list_height = 100; // Reduced height when keyboard shown
            if y >= 60 && y < 70 {
                // Up arrow clicked
                scroll_offset = (scroll_offset - 25).max(0);
                invalidate_all();
            } else if y >= 150 && y < 160 {
                // Down arrow clicked
                let tasks = get_filtered_tasks(task_filter);
                let max_scroll = (tasks.len() * 25 - list_height).max(0);
                scroll_offset = (scroll_offset + 25).min(max_scroll);
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
        
        // Input area (focus for typing) - adjust for keyboard mode
        if y >= input_y && y < input_y + 24 && x >= 10 && x < 170 {
            if !input_focused {
                input_focused = true;
                keyboard_visible = true;
                // Reset scroll when keyboard opens
                scroll_offset = 0;
                invalidate_all();
            }
        } else if y < 220 && !keyboard_visible {
            // Only unfocus if clicking in content area when keyboard not visible
            if input_focused {
                input_focused = false;
                keyboard_visible = false;
                scroll_offset = 0;
                invalidate_all();
            }
        }
        
        // Clear completed button - only show when keyboard hidden
        if !keyboard_visible && y >= 250 && y < 270 && x >= 10 && x < 110 {
            clear_completed();
        }
        
        // Task list interactions - adjust for scroll
        let tasks = get_filtered_tasks(task_filter);
        let list_start_y = 60;
        let list_height = if keyboard_visible { 100 } else { 140 };
        
        for i in 0..tasks.len() {
            let task = tasks[i];
            let y_pos = list_start_y + i * 25 - scroll_offset;
            
            // Only handle clicks for visible tasks
            if y_pos >= list_start_y - 25 && y_pos < list_start_y + list_height {
                // Delete button
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
}

fn on_draw() {
    clear();
    title_bar("Modern Todo");
    
    // Adjust content height based on keyboard visibility
    let available_height = if keyboard_visible { 220 } else { 320 }; // Leave space for keyboard
    let max_content_y = available_height - 30;
    
    // Filter buttons at top (always visible)
    button(10, 30, 60, 20, "All", task_filter == "all");
    button(80, 30, 60, 20, "Active", task_filter == "active");
    button(150, 30, 80, 20, "Complete", task_filter == "completed");
    
    // Scrollable task list area
    let tasks = get_filtered_tasks(task_filter);
    let list_start_y = 60;
    let list_height = if keyboard_visible { 100 } else { 140 }; // Reduced when keyboard shown
    
    // Draw tasks with scroll offset
    if tasks.len() == 0 {
        let y_pos = list_start_y - scroll_offset;
        if y_pos >= list_start_y && y_pos < list_start_y + list_height {
            label(10, y_pos, "No tasks to show");
        }
    } else {
        for i in 0..tasks.len() {
            let task = tasks[i];
            let y_pos = list_start_y + i * 25 - scroll_offset;
            
            // Only draw if within visible area
            if y_pos >= list_start_y - 25 && y_pos < list_start_y + list_height {
                // ASCII checkbox display
                let display_text = task.text.sub_string(4); // Remove "[ ] " or "[x] "
                let prefix = if task.completed { "[X]" } else { "[ ]" };
                let full_text = prefix + " " + display_text;
                
                // Task button (toggles completion)
                button(10, y_pos, 190, 20, full_text, task.completed);
                
                // Delete button
                button(210, y_pos, 20, 20, "x", false);
            }
        }
    }
    
    // Input area - fixed position when keyboard visible
    let input_y = if keyboard_visible { 170 } else { 200 };
    if input_y < max_content_y {
        label(10, input_y - 15, "Add new task:");
        
        // Text input area with focus styling
        let input_display = if new_task_text == "" { 
            if input_focused { "Type here... |" } else { "Type here..." }
        } else { 
            if input_focused { new_task_text + " |" } else { new_task_text }
        };
        
        button(10, input_y, 160, 24, input_display, input_focused);
        
        // Add button
        button(180, input_y, 50, 24, "Add", false);
    }
    
    // Stats area - only show if space available
    if !keyboard_visible {
        let all_tasks = db_get_ids_in_category(0);
        let completed_count = 0;
        for id in all_tasks {
            let text = to_string(db_get_data(id));
            if text.index_of("[x] ") == 0 {
                completed_count += 1;
            }
        }
        
        let stats_text = all_tasks.len() + " total, " + completed_count + " done";
        label(10, 235, stats_text);
        
        if completed_count > 0 {
            button(10, 250, 100, 20, "Clear Done", false);
        }
    }
    
    // Show scroll indicators if needed - positioned to right of todo list
    if keyboard_visible && tasks.len() * 25 > list_height {
        label(205, 60, "^"); // Up arrow - positioned after task text but before delete button
        label(205, list_start_y + list_height - 10, "v"); // Down arrow
    }
    
    // Show actual keyboard widget if input is focused - fixed at bottom
    if keyboard_visible {
        draw_keyboard(keyboard);
    }
}

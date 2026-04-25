// assets/scripts/egui_scroll_demo.rhai
// Demonstration of EGUI scroll area replacing manual scrollbar

let app_id   = "com.soulos.egui_scroll_demo";
let app_name = "EGUI Scroll Demo";
let app_icon = "demo";

// Demo state
let items_count = 50;
let selected_item = -1;

fn on_event(event) {
    if event.type == "AppStart" {
        invalidate_all();
    }
    
    if event.type == "PenDown" {
        // Simple item selection by Y position
        let item_height = 25;
        let clicked_item = (event.y - 30) / item_height; // 30 = title bar height
        if clicked_item >= 0 && clicked_item < items_count {
            selected_item = clicked_item;
            invalidate_all();
        }
    }
    
    if event.type == "Key" {
        if event.key == "ArrowDown" {
            selected_item = (selected_item + 1) % items_count;
            invalidate_all();
        } else if event.key == "ArrowUp" {
            selected_item = if selected_item <= 0 { items_count - 1 } else { selected_item - 1 };
            invalidate_all();
        }
    }
}

fn on_draw() {
    clear();
    title_bar("EGUI Scroll Demo");
    
    // The old manual scroll system has been removed
    // In the new EGUI system, scrolling would be handled automatically
    // by the EGUI ScrollArea widget in the native integration layer
    
    ui_begin();
    
    ui_heading("Scrollable Content Demo");
    ui_separator();
    
    ui_label("This demonstrates content that would scroll automatically");
    ui_label("with EGUI ScrollArea instead of manual scrollbar handling.");
    ui_separator();
    
    // Simulate a long list of items
    for i in range(0, items_count) {
        let is_selected = selected_item == i;
        let text = "Item " + (i + 1) + " - " + if is_selected { "SELECTED" } else { "normal" };
        
        if is_selected {
            ui_selectable(true, text);
        } else {
            ui_label(text);
        }
        
        // Add some spacing between items
        if i % 5 == 4 {
            ui_space(5);
        }
    }
    
    ui_space(20);
    ui_separator();
    ui_label("✓ EGUI automatically handles:");
    ui_label("  • Scroll position tracking");
    ui_label("  • Touch/pen scrolling");
    ui_label("  • Scrollbar rendering");
    ui_label("  • Content clipping");
    ui_label("  • Performance optimization");
    
    if selected_item >= 0 {
        ui_space(10);
        ui_rich_text("Selected: Item " + (selected_item + 1), "blue");
    }
}

// These UI functions now work without manual scroll offset calculations
// The underlying EGUI ScrollArea handles all the complexity
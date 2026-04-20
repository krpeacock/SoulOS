// Scrollbar Test App
// Demonstrates the new scrollbar component with a scrollable list

let title = "Scrollbar Test";
let items = [];

// Generate test content
for i in 1..50 {
    items.push(`Item ${i}: This is test content item number ${i}`);
}

let scroll_view = null;
let item_height = 20;
let visible_items = 10;

fn init() {
    // Create scrollable view
    let area_rect = #{x: 0, y: 15, w: 240, h: visible_items * item_height};
    let total_height = items.len() * item_height;
    scroll_view = new_scrollable_view(area_rect, total_height);
}

fn handle_event(event) {
    if scroll_view != null {
        let result = scroll_view.handle_event(event);
        if result.position_changed {
            return #{invalidate: scroll_view.area()};
        }
    }
    return #{};
}

fn draw(canvas) {
    // Draw title bar
    canvas.title_bar(240, title);
    
    if scroll_view == null {
        init();
    }
    
    // Draw content area background
    let content_area = scroll_view.content_area();
    canvas.fill_rect(content_area, #{r: 255, g: 255, b: 255});
    
    // Calculate which items to draw
    let scroll_offset = scroll_view.scroll_offset();
    let first_item = scroll_offset / item_height;
    let last_item = (first_item + visible_items + 1).min(items.len());
    
    // Draw visible items
    for i in first_item..last_item {
        let y = content_area.y + (i * item_height) - scroll_offset;
        if y >= content_area.y && y < content_area.y + content_area.h {
            let item_text = items[i];
            canvas.text(#{x: content_area.x + 5, y: y + 5}, item_text);
        }
    }
    
    // Draw scrollbar
    scroll_view.draw(canvas);
}

fn get_accessibility_nodes() {
    let nodes = [];
    if scroll_view != null && scroll_view.scrollbar_visible() {
        let scrollbar_info = scroll_view.accessibility_info();
        if scrollbar_info != null {
            nodes.push(#{
                bounds: scroll_view.scrollbar_area(),
                label: scrollbar_info,
                role: "scrollbar"
            });
        }
    }
    return nodes;
}
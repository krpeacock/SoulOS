// assets/scripts/kitchen_sink.rhai
// SoulOS Component Kitchen Sink - A comprehensive component playground
// Like Storybook, but for SoulOS components

let app_id   = "com.soulos.kitchen_sink";
let app_name = "Kitchen Sink";
let app_icon = "demo";

// Navigation state - global variables 
let current_page = "text";
let pages = ["text", "input", "select", "layout", "visual", "examples", "help"];
let page_names = ["Text", "Input", "Select", "Layout", "Visual", "Examples", "Help"];

// Scrollable view for content
let scroll_view = ();
let content_height = 450; // Height needed for page content

// Component states for interactive demos - global variables
let sample_text = "Hello SoulOS!";
let checkbox_state = true;
let radio_selection = "option2";
let slider_value = 75;
let progress_value = 60;
let color_selection = "Blue";
let combo_selection = "Choice B";
let collapsing_open = true;
let input_focused = false;

// Counter for interactive demos - global variables
let counter = 0;
let timer = 0;

fn get_page_index(page) {
    for i in 0..pages.len() {
        if pages[i] == page {
            return i;
        }
    }
    return 0;
}

fn on_event(event) {
    if event.type == "AppStart" {
        // Initialize scroll view - area below navigation tabs
        scroll_view = new_scrollable_view(0, 50, 240, 270, content_height);
        invalidate_all();
    }
    
    if event.type == "PenDown" && scroll_view != () {
        // Handle scrollbar interaction first
        if scrollable_contains_point(scroll_view, event.x, event.y) {
            let changed = scrollable_handle_pen(scroll_view, true, false, false, event.x, event.y);
            if changed {
                invalidate_all();
                return; // Don't process other interactions if scrollbar was used
            }
        }
    }
    
    if event.type == "PenMove" && scroll_view != () {
        if scrollable_contains_point(scroll_view, event.x, event.y) {
            let changed = scrollable_handle_pen(scroll_view, false, true, false, event.x, event.y);
            if changed {
                invalidate_all();
            }
        }
    }
    
    if event.type == "PenUp" && scroll_view != () {
        let changed = scrollable_handle_pen(scroll_view, false, false, true, event.x, event.y);
        if changed {
            invalidate_all();
        }
    }
    
    if event.type == "Key" && input_focused {
        if event.key == "Backspace" && sample_text.len() > 0 {
            sample_text = sample_text.sub_string(0, sample_text.len() - 1);
            invalidate_all();
        } else if event.key != "Other" && event.key != "Backspace" && event.key != "Enter" {
            sample_text += event.key;
            invalidate_all();
        }
    }
    
    if event.type == "PenDown" {
        let x = event.x;
        let y = event.y;
        
        // Navigation tabs
        if y >= 30 && y < 50 {
            let tab_width = 240 / pages.len();
            for i in 0..pages.len() {
                let tab_x = i * tab_width;
                if x >= tab_x && x < tab_x + tab_width {
                    current_page = pages[i];
                    invalidate_all();
                    break;
                }
            }
        }
        
        // Interactive elements based on current page
        if current_page == "input" {
            // Sample text input area
            if y >= 80 && y < 104 && x >= 10 && x < 170 {
                input_focused = !input_focused;
                invalidate_all();
            }
        }
        
        if current_page == "select" {
            // Toggle checkbox
            if y >= 80 && y < 100 && x >= 10 && x < 200 {
                checkbox_state = !checkbox_state;
                invalidate_all();
            }
            
            // Radio buttons  
            if y >= 105 && y < 125 && x >= 10 && x < 100 {
                radio_selection = "option1";
                invalidate_all();
            } else if y >= 105 && y < 125 && x >= 100 && x < 200 {
                radio_selection = "option2";
                invalidate_all();
            }
            
            // Slider area
            if y >= 150 && y < 170 && x >= 10 && x < 200 {
                slider_value = ((x - 10) * 100 / 190).max(0).min(100);
                invalidate_all();
            }
        }
        
        if current_page == "layout" {
            // Toggle collapsing section
            if y >= 80 && y < 100 && x >= 10 && x < 200 {
                collapsing_open = !collapsing_open;
                invalidate_all();
            }
        }
        
        if current_page == "examples" {
            // Counter buttons
            if y >= 80 && y < 100 && x >= 10 && x < 60 {
                counter -= 1;
                invalidate_all();
            } else if y >= 80 && y < 100 && x >= 170 && x < 220 {
                counter += 1;
                invalidate_all();
            }
            
            // Reset button
            if y >= 110 && y < 130 && x >= 10 && x < 80 {
                counter = 0;
                timer = 0;
                invalidate_all();
            }
        }
    }
    
    if event.type == "Tick" {
        // Update timer for examples
        timer += 1;
        if timer % 30 == 0 { // Update every ~0.5 seconds
            if current_page == "examples" {
                invalidate_all();
            }
        }
    }
}


// Simplified kitchen sink with basic components only

fn on_draw() {
    clear();
    title_bar("SoulOS Kitchen Sink");
    
    // Navigation - always visible at top
    let tab_width = 240 / pages.len();
    
    for i in 0..pages.len() {
        let tab_x = i * tab_width;
        let is_active = pages[i] == current_page;
        button(tab_x, 30, tab_width, 20, page_names[i], is_active);
    }
    
    // Draw current page content (simple version without scrolling)
    if current_page == "text" {
        label(10, 60, "Text & Typography Components");
        label(10, 80, "Standard label text");
        label(10, 100, "Rich text with styling");
        label(10, 120, "_This looks like a link_");
        label(10, 140, "[Monospace code text]");
    } else if current_page == "input" {
        label(10, 60, "Input Components");
        label(10, 80, "Text Input:");
        button(10, 100, 160, 24, sample_text, input_focused);
        button(10, 130, 60, 20, "Button", false);
        button(80, 130, 70, 20, "Secondary", false);
    } else if current_page == "select" {
        label(10, 60, "Selection & Interaction");
        let checkbox_text = if checkbox_state { "[X] Enable feature" } else { "[ ] Enable feature" };
        label(10, 80, checkbox_text);
        let radio1 = if radio_selection == "option1" { "(*) Option 1" } else { "( ) Option 1" };
        let radio2 = if radio_selection == "option2" { "(*) Option 2" } else { "( ) Option 2" };
        label(10, 100, radio1);
        label(120, 100, radio2);
        let slider_text = "Slider: " + slider_value + "%";
        label(10, 120, slider_text);
    } else if current_page == "layout" {
        label(10, 60, "Layout & Container Components");
        button(10, 80, 50, 20, "Left", false);
        button(70, 80, 60, 20, "Center", false);
        button(140, 80, 50, 20, "Right", false);
        let collapse_text = if collapsing_open { "▼ Collapsing Section" } else { "▶ Collapsing Section" };
        button(10, 110, 180, 20, collapse_text, false);
    } else if current_page == "visual" {
        label(10, 60, "Visual & Media Components");
        draw_rect(10, 80, 80, 60, 200);
        label(30, 105, "Sample");
        label(10, 150, "Component  Type     Status");
        label(10, 170, "Button     Input    Ready");
    } else if current_page == "examples" {
        label(10, 60, "Interactive Examples");
        button(10, 80, 30, 20, "-", false);
        label(50, 85, "Count: " + counter);
        button(120, 80, 30, 20, "+", false);
        button(10, 110, 60, 20, "Reset", false);
        label(10, 140, "Timer: " + timer);
    } else if current_page == "help" {
        label(10, 60, "SoulOS Kitchen Sink Help");
        label(10, 80, "Component playground for SoulOS");
        label(10, 100, "Use tabs above to explore");
        label(10, 120, "TEXT: Typography and styling");
        label(10, 140, "INPUT: Text inputs & buttons");
        label(10, 160, "SELECT: Checkboxes & sliders");
        label(10, 180, "LAYOUT: Groups & columns");
        label(10, 200, "VISUAL: Images & tables");
        label(10, 220, "EXAMPLES: Interactive demos");
    }
}


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
        invalidate_all();
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

fn draw_navigation() {
    let tab_width = 240 / pages.len();
    
    for i in 0..pages.len() {
        let tab_x = i * tab_width;
        let is_active = pages[i] == current_page;
        button(tab_x, 30, tab_width, 20, page_names[i], is_active);
    }
}

fn draw_text_page() {
    ui_begin();
    
    ui_heading("Text & Typography Components");
    ui_separator();
    
    ui_group("Basic Text");
    ui_label("Standard label text");
    ui_rich_text("Rich text with styling", "red");
    ui_hyperlink("This looks like a link");
    ui_monospace("Monospace code text");
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Headings & Styles");
    ui_heading("This is a heading");
    ui_label("Normal text follows headings");
    ui_label("Multiple lines of text");
    ui_label("Can be displayed together");
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Interactive Text");
    ui_label("Sample text: " + sample_text);
    if input_focused {
        ui_label("Click above to edit text!");
    } else {
        ui_label("(Text is editable on Input page)");
    }
    ui_group_end();
}

fn draw_input_page() {
    ui_begin();
    
    ui_heading("Input Components");
    ui_separator();
    
    ui_group("Text Input");
    ui_text_input(sample_text, "Type something...");
    ui_text_edit_hint("", "Text with hint");
    ui_text_edit_password("secret123");
    ui_text_edit_multiline("Line 1\nLine 2\nLine 3", 60);
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Buttons");
    ui_horizontal_begin();
    ui_button("Primary");
    ui_button("Secondary");
    ui_small_button("×");
    ui_horizontal_end();
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Code Editor");
    ui_code_editor("fn hello() {\n    print(\"Hello SoulOS!\");\n}");
    ui_group_end();
}

fn draw_selection_page() {
    ui_begin();
    
    ui_heading("Selection & Interaction");
    ui_separator();
    
    ui_group("Checkboxes & Radio");
    ui_checkbox(checkbox_state, "Enable feature");
    ui_horizontal_begin();
    ui_radio(radio_selection == "option1", "Option 1");
    ui_radio(radio_selection == "option2", "Option 2");
    ui_horizontal_end();
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Value Controls");
    ui_slider(slider_value, 0, 100);
    ui_drag_value(slider_value, "Value:");
    ui_combo_box("Select:", combo_selection);
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Progress & Status");
    ui_progress_bar(progress_value);
    ui_spinner();
    ui_color_edit(color_selection);
    ui_group_end();
}

fn draw_layout_page() {
    ui_begin();
    
    ui_heading("Layout & Container Components");
    ui_separator();
    
    ui_group("Horizontal Layout");
    ui_horizontal_begin();
    ui_button("Left");
    ui_button("Center");
    ui_button("Right");
    ui_horizontal_end();
    ui_group_end();
    
    ui_space(10);
    
    ui_collapsing("Collapsing Section", collapsing_open);
    if collapsing_open {
        ui_label("This content can be hidden");
        ui_label("by clicking the header above");
        ui_button("Button inside collapsing area");
    }
    ui_collapsing_end(collapsing_open);
    
    ui_space(10);
    
    ui_group("Columns");
    ui_columns(3);
    ui_label("Col 1");
    ui_next_column();
    ui_label("Col 2");
    ui_next_column();
    ui_label("Col 3");
    ui_columns_end();
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Grid Layout");
    ui_grid(2);
    ui_button("A"); ui_grid_next();
    ui_button("B"); ui_grid_next();
    ui_button("C"); ui_grid_next();
    ui_button("D"); ui_grid_next();
    ui_group_end();
}

fn draw_visual_page() {
    ui_begin();
    
    ui_heading("Visual & Media Components");
    ui_separator();
    
    ui_group("Images & Media");
    ui_image(80, 60, "Sample");
    ui_image(100, 40, "Wide");
    ui_plot("Performance Chart");
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Data Tables");
    ui_table(3);
    ui_table_header("Component");
    ui_table_header("Type");
    ui_table_header("Status");
    ui_table_cell("Button");
    ui_table_cell("Input");
    ui_table_cell("Ready");
    ui_table_cell("Checkbox");
    ui_table_cell("Select");
    ui_table_cell("Active");
    ui_table_cell("Slider");
    ui_table_cell("Control");
    ui_table_cell("Working");
    ui_table_end();
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Menu Systems");
    ui_horizontal_begin();
    ui_menu("File");
    ui_menu("Edit");
    ui_menu("View");
    ui_horizontal_end();
    
    ui_label("Menu Items:");
    ui_menu_item("New File");
    ui_menu_item("Open File");
    ui_menu_item("Save As...");
    ui_menu_item("Exit");
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Visual Feedback");
    ui_tooltip("This is a tooltip example");
    ui_separator();
    ui_label("Separators divide content sections");
    ui_group_end();
}

fn draw_examples_page() {
    ui_begin();
    
    ui_heading("Interactive Examples");
    ui_separator();
    
    ui_group("Counter Example");
    ui_horizontal_begin();
    ui_button("-");
    ui_label("Count: " + counter);
    ui_button("+");
    ui_horizontal_end();
    ui_button("Reset");
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Live Updates");
    ui_label("Timer: " + timer);
    ui_progress_bar((timer % 100) / 100);
    ui_label("Progress cycles every 100 ticks");
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Component States");
    ui_label("Checkbox: " + if checkbox_state { "ON" } else { "OFF" });
    ui_label("Radio: " + radio_selection);
    ui_label("Slider: " + slider_value + "%");
    ui_label("Color: " + color_selection);
    ui_group_end();
    
    ui_space(10);
    
    ui_group("Complex Layout Example");
    ui_columns(2);
    
    ui_label("Left Column:");
    ui_checkbox(true, "Feature A");
    ui_checkbox(false, "Feature B");
    ui_next_column();
    
    ui_label("Right Column:");
    ui_radio(true, "Option X");
    ui_radio(false, "Option Y");
    ui_columns_end();
    ui_group_end();
}

fn draw_help_page() {
    ui_begin();
    
    ui_heading("SoulOS Kitchen Sink Help");
    ui_separator();
    
    ui_group("About This App");
    ui_label("This is a component playground for SoulOS");
    ui_label("Similar to Storybook for web development");
    ui_label("Use the tabs above to explore components");
    ui_group_end();
    
    ui_space(5);
    
    ui_group("Page Guide");
    ui_label("TEXT: Typography and text styling");
    ui_label("INPUT: Text inputs and code editor");
    ui_label("SELECT: Checkboxes, radio, sliders");
    ui_label("LAYOUT: Groups, columns, grids");
    ui_label("VISUAL: Images, charts, tables");
    ui_label("EXAMPLES: Interactive demos");
    ui_group_end();
    
    ui_space(5);
    
    ui_group("Available Components");
    ui_monospace("ui_label()");
    ui_monospace("ui_button()");
    ui_monospace("ui_checkbox()");
    ui_monospace("ui_text_input()");
    ui_monospace("ui_horizontal_begin/end()");
    ui_monospace("ui_group()");
    ui_label("...and many more!");
    ui_group_end();
    
    ui_space(5);
    
    ui_group("Traditional SoulOS Components");
    ui_label("Also available:");
    ui_monospace("button()");
    ui_monospace("label()");
    ui_monospace("title_bar()");
    ui_monospace("clear()");
    ui_group_end();
    
    ui_space(5);
    
    ui_group("Development Tips");
    ui_label("- Components auto-layout vertically");
    ui_label("- Use ui_horizontal_*() for side-by-side");
    ui_label("- ui_group() creates visual containers");
    ui_label("- ui_separator() adds visual breaks");
    ui_label("- Remember to call invalidate_all()");
    ui_group_end();
}

fn on_draw() {
    clear();
    title_bar("SoulOS Kitchen Sink");
    
    // Navigation
    call!(draw_navigation);
    
    // Current page content
    if current_page == "text" {
        call!(draw_text_page);
    } else if current_page == "input" {
        call!(draw_input_page);
    } else if current_page == "select" {
        call!(draw_selection_page);
    } else if current_page == "layout" {
        call!(draw_layout_page);
    } else if current_page == "visual" {
        call!(draw_visual_page);
    } else if current_page == "examples" {
        call!(draw_examples_page);
    } else if current_page == "help" {
        call!(draw_help_page);
    }
}
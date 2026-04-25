// assets/scripts/calc.rhai
// A functional Calculator app using native EGUI grid.

let app_id   = "com.soulos.calc";
let app_name = "Calc";
let app_icon = "calc";

let display = "0";
let last_val = 0;
let op = "";
let clear_on_next = true;

fn do_op() {
    let current = display.to_int();
    if op == "+" { last_val += current; }
    else if op == "-" { last_val -= current; }
    else if op == "*" { last_val *= current; }
    else if op == "/" { if current != 0 { last_val /= current; } }
    else { last_val = current; }
    display = last_val.to_string();
    clear_on_next = true;
}

fn add_digit(d) {
    if clear_on_next {
        display = d.to_string();
        clear_on_next = false;
    } else {
        if display == "0" { display = d.to_string(); }
        else { display += d.to_string(); }
    }
}

fn on_draw() {
    clear();
    title_bar(app_name);
    
    egui_begin();
    
    egui_group(ui, "", |ui| {
        egui_vertical_layout(ui, |ui| {
            // 1. Display
            egui_label(ui, display);
            egui_separator(ui);
            
            // 2. Buttons in a simulated grid using horizontal layouts
            // (In a more advanced bridge we'd have egui_grid)
            
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "7") { add_digit(7); }
                if egui_button(ui, "8") { add_digit(8); }
                if egui_button(ui, "9") { add_digit(9); }
                if egui_button(ui, "/") { do_op(); op = "/"; }
            });
            
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "4") { add_digit(4); }
                if egui_button(ui, "5") { add_digit(5); }
                if egui_button(ui, "6") { add_digit(6); }
                if egui_button(ui, "*") { do_op(); op = "*"; }
            });
            
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "1") { add_digit(1); }
                if egui_button(ui, "2") { add_digit(2); }
                if egui_button(ui, "3") { add_digit(3); }
                if egui_button(ui, "-") { do_op(); op = "-"; }
            });
            
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "0") { add_digit(0); }
                if egui_button(ui, "C") { display = "0"; last_val = 0; op = ""; clear_on_next = true; }
                if egui_button(ui, "=") { do_op(); op = ""; }
                if egui_button(ui, "+") { do_op(); op = "+"; }
            });
        });
    });
    
    egui_end();
}

fn on_event(ev) {}

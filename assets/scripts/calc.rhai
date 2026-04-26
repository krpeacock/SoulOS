// assets/scripts/calc.rhai
// Calculator app: integer arithmetic with backspace, sign toggle, and pending-op display.

let app_id   = "com.soulos.calc";
let app_name = "Calc";
let app_icon = "calc";

let display      = "0";   // number currently shown
let last_val     = 0;     // accumulated operand
let op           = "";    // pending operator: "", "+", "-", "*", "/", "%"
let clear_on_next = true; // next digit starts a fresh entry
let error        = false; // divide-by-zero or similar
let context_line = "";    // "12 +" shown above the main display

fn do_op() {
    if error { return; }
    let current = display.to_int();
    if op == "+" { last_val += current; }
    else if op == "-" { last_val -= current; }
    else if op == "*" { last_val *= current; }
    else if op == "/" {
        if current == 0 {
            error        = true;
            display      = "Error";
            context_line = "";
            clear_on_next = true;
            return;
        }
        last_val /= current;
    }
    else if op == "%" {
        if current == 0 {
            error        = true;
            display      = "Error";
            context_line = "";
            clear_on_next = true;
            return;
        }
        last_val %= current;
    }
    else { last_val = current; }

    display      = last_val.to_string();
    clear_on_next = true;
}

fn add_digit(d) {
    if error { return; }
    if clear_on_next {
        display       = d.to_string();
        clear_on_next = false;
    } else {
        if display == "0" { display = d.to_string(); }
        else              { display += d.to_string(); }
    }
}

fn add_double_zero() {
    if error { return; }
    if clear_on_next {
        display       = "0";
        clear_on_next = false;
    } else if display != "0" {
        display += "00";
    }
}

fn backspace() {
    if error {
        all_clear();
        return;
    }
    if clear_on_next {
        display       = "0";
        clear_on_next = false;
        return;
    }
    let is_neg = display.len() > 1 && display.sub_string(0, 1) == "-";
    let min_len = if is_neg { 2 } else { 1 };
    if display.len() <= min_len {
        display = "0";
    } else {
        display = display.sub_string(0, display.len() - 1);
    }
}

fn toggle_sign() {
    if error { return; }
    if display.len() > 0 && display.sub_string(0, 1) == "-" {
        display = display.sub_string(1);
    } else if display != "0" {
        display = "-" + display;
    }
}

fn all_clear() {
    display       = "0";
    last_val      = 0;
    op            = "";
    context_line  = "";
    clear_on_next = true;
    error         = false;
}

fn set_op(new_op) {
    if error { return; }
    do_op();
    if !error {
        context_line = display + " " + new_op;
        op           = new_op;
    }
}

fn press_equals() {
    do_op();
    context_line = "";
    op           = "";
}

fn on_draw() {
    clear();
    title_bar(app_name);

    egui_begin();

    egui_group(ui, "", |ui| {
        egui_vertical_layout(ui, |ui| {

            // Context line: e.g. "12 +"
            egui_label(ui, context_line);
            // Main display
            egui_label(ui, display);
            egui_separator(ui);

            // Row 0: AC  ±  ⌫  ÷
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "AC") { all_clear(); }
                if egui_button(ui, "±")  { toggle_sign(); }
                if egui_button(ui, "⌫")  { backspace(); }
                if egui_button(ui, "÷")  { set_op("/"); }
            });

            // Row 1: 7  8  9  ×
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "7") { add_digit(7); }
                if egui_button(ui, "8") { add_digit(8); }
                if egui_button(ui, "9") { add_digit(9); }
                if egui_button(ui, "×") { set_op("*"); }
            });

            // Row 2: 4  5  6  −
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "4") { add_digit(4); }
                if egui_button(ui, "5") { add_digit(5); }
                if egui_button(ui, "6") { add_digit(6); }
                if egui_button(ui, "−") { set_op("-"); }
            });

            // Row 3: 1  2  3  +
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "1") { add_digit(1); }
                if egui_button(ui, "2") { add_digit(2); }
                if egui_button(ui, "3") { add_digit(3); }
                if egui_button(ui, "+") { set_op("+"); }
            });

            // Row 4: 0  00  %  =
            egui_horizontal_layout(ui, |ui| {
                if egui_button(ui, "0")  { add_digit(0); }
                if egui_button(ui, "00") { add_double_zero(); }
                if egui_button(ui, "%")  { set_op("%"); }
                if egui_button(ui, "=")  { press_equals(); }
            });
        });
    });

    egui_end();
}

fn on_event(ev) {
    if ev.type == "Key" {
        let k = ev.key;
        if      k == "0"         { add_digit(0); }
        else if k == "1"         { add_digit(1); }
        else if k == "2"         { add_digit(2); }
        else if k == "3"         { add_digit(3); }
        else if k == "4"         { add_digit(4); }
        else if k == "5"         { add_digit(5); }
        else if k == "6"         { add_digit(6); }
        else if k == "7"         { add_digit(7); }
        else if k == "8"         { add_digit(8); }
        else if k == "9"         { add_digit(9); }
        else if k == "+"         { set_op("+"); }
        else if k == "-"         { set_op("-"); }
        else if k == "*"         { set_op("*"); }
        else if k == "/"         { set_op("/"); }
        else if k == "%"         { set_op("%"); }
        else if k == "Enter"     { press_equals(); }
        else if k == "Backspace" { backspace(); }
        else if k == "Escape"    { all_clear(); }
        invalidate_all();
    }
}

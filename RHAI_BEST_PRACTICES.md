# Rhai Scripting Best Practices for SoulOS

## Overview

Rhai is the scripting language for creating applications in SoulOS. This document outlines best practices for writing clean, efficient, and correct Rhai scripts that integrate smoothly with the SoulOS Egui-based UI system.

## 1. Calling Egui Bridge Functions

The most critical pattern to understand is how to call Egui layout and widget functions from Rhai. These functions are exposed through a "bridge" and have a specific, non-obvious syntax.

### The `()` Placeholder Rule

Most Egui functions that organize UI elements (like layouts and groups) or that don't belong to a specific Egui closure (`egui_space`) require a `()` placeholder as their **first argument**. This is because the underlying Rust function registration includes a `_ui: Dynamic` parameter that is ignored, but Rhai still needs a value for it.

**Correct Syntax:**

```rhai
// For layout containers that take a closure
egui_horizontal_layout((), |ui| {
    // The 'ui' variable is automatically available inside this closure.
    if egui_button(ui, "OK") {
        // ...
    }
});

egui_group((), "My Group", |ui| {
    egui_label(ui, "A label inside a group.");
});

// For standalone functions that need the UI context
egui_space((), 10);
```

**Incorrect Syntax (will cause errors):**

```rhai
// ❌ Error: "Variable not found: ui"
// 'ui' is not defined in the main on_draw scope.
egui_horizontal_layout(ui, |ui| { ... }); 

// ❌ Error: "Function not found"
// The function signature doesn't match the one registered in Rust.
egui_group("My Group", |ui| { ... });

// ❌ Error: "Variable not found: ui"
egui_space(ui, 10);

// ❌ Error: "Function not found"
egui_space(10);
```

### Functions Inside Egui Closures

Functions that are called *inside* a layout closure (like `egui_button`, `egui_label`, `egui_selectable_label`) **do** take the `ui` object as their first argument. This `ui` variable is the one provided by the closure (`|ui|`).

```rhai
egui_vertical_layout((), |ui| {
    // Correct: 'ui' is passed to functions inside the closure.
    egui_label(ui, "Enter your name:");
    if egui_button(ui, "Submit").clicked() {
        // ...
    }
});
```

## 2. UI Structure in `on_draw`

To use the Egui system, your `on_draw` function must be structured correctly.

1.  Call `egui_begin()` at the start.
2.  Perform all your Egui layout and widget calls.
3.  Call `egui_end()` at the end.

```rhai
fn on_draw() {
    clear();
    title_bar("My App");

    egui_begin();

    // --- All your UI code goes here ---
    egui_vertical_layout((), |ui| {
        egui_label(ui, "Hello, Egui!");
    });
    // ---

    egui_end();
}
```

## 3. State Management

- **Simple State:** Use global `let` variables for simple, transient UI state (e.g., text in an input box, which tab is selected).

  ```rhai
  let new_task_text = "";
  let task_filter = "all";
  ```

- **Persistent State:** For data that needs to be saved, use the `db_*` functions (`db_insert`, `db_get_data`, `db_update`, `db_delete`). This stores the data in SoulOS's central database.

  ```rhai
  fn add_task(text) {
      if text.trim() != "" {
          // Store the new task in the database
          db_insert(0, to_bytes("[ ] " + text.trim()));
          invalidate_all(); // Redraw to show the new task
      }
  }
  ```

## 4. Performance and Redrawing

- **`invalidate_all()`:** Call this function whenever your application's state changes in a way that requires a visual update. This tells the system to redraw the screen. Use it after adding/deleting data, or when UI state changes.

- **Avoid Work in `on_draw`:** The `on_draw` function is called every time the screen redraws. Avoid doing heavy computations, complex data processing, or reading from the database inside `on_draw`. Fetch your data in `on_event` (e.g., on `AppStart`) and store it in global variables if needed.

## 5. Event Handling (`on_event`)

The `on_event(event)` function is where you respond to user input and system events.

```rhai
fn on_event(event) {
    if event.type == "AppStart" {
        // Initialization code here
        invalidate_all();
    }

    if event.type == "PenDown" {
        // Handle touch/stylus input
    }
}
```

## 6. Migrating from Old Primitives

The old UI system involved manual coordinate-based drawing (`button(x, y, w, h, ...)`). The new Egui system uses a layout-based approach.

- **REPLACE** `button(x, y, ...)` with `egui_button(ui, ...)` inside a layout.
- **REPLACE** `label(x, y, ...)` with `egui_label(ui, ...)` inside a layout.
- **REPLACE** manual scroll logic with `egui_scroll_area`.

This migration is essential for creating modern, maintainable, and responsive UIs in SoulOS.

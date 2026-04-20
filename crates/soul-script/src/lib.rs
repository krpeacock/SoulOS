#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use embedded_graphics::pixelcolor::Gray8;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use rhai::{Dynamic, Engine, EvalAltResult, Map, Position, Scope, AST};
use soul_core::{App, Ctx, Event, APP_HEIGHT, SCREEN_WIDTH};
use soul_db::Database;
use soul_ui::Form;
use soul_ui::TextInput;
use soul_ui::{Keyboard, TextArea, TypedKey, KEYBOARD_HEIGHT};

/// Object-safe drawing trait to bridge Rhai and DrawTarget.
pub trait ObjectSafeDraw {
    fn title_bar(&mut self, title: &str);
    fn button(&mut self, x: i32, y: i32, w: u32, h: u32, label: &str, pressed: bool);
    fn label(&mut self, x: i32, y: i32, text: &str);
    fn clear(&mut self);
    fn draw_form(&mut self, form: &Form);
    fn draw_text_input(&mut self, input: &TextInput);
    fn draw_text_area(&mut self, area: &TextArea);
    fn draw_keyboard(&mut self, kb: &Keyboard);
    fn rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: u8);
}

impl<D> ObjectSafeDraw for D
where
    D: DrawTarget<Color = Gray8>,
{
    fn title_bar(&mut self, title: &str) {
        let _ = soul_ui::title_bar(self, SCREEN_WIDTH as u32, title);
    }

    fn button(&mut self, x: i32, y: i32, w: u32, h: u32, label: &str, pressed: bool) {
        let rect = Rectangle::new(Point::new(x, y), Size::new(w, h));
        let _ = soul_ui::button(self, rect, label, pressed);
    }

    fn label(&mut self, x: i32, y: i32, text: &str) {
        let _ = soul_ui::label(self, Point::new(x, y), text);
    }

    fn clear(&mut self) {
        let _ = soul_ui::clear(self, SCREEN_WIDTH as u32, APP_HEIGHT as u32);
    }

    fn draw_form(&mut self, form: &Form) {
        let _ = form.draw(self, None);
    }

    fn draw_text_input(&mut self, input: &TextInput) {
        let _ = input.draw(self);
    }

    fn draw_text_area(&mut self, area: &TextArea) {
        let _ = area.draw(self);
    }

    fn draw_keyboard(&mut self, kb: &Keyboard) {
        let _ = kb.draw(self);
    }

    fn rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: u8) {
        let rect = Rectangle::new(Point::new(x, y), Size::new(w, h));
        let _ = self.fill_solid(&rect, Gray8::new(color));
    }
}

// --- System call protocol -----------------------------------------------

/// A request from a scripted app to the runtime kernel.
///
/// Scripts emit these via `system_launch_by_id(id)` or `system_return()`.
/// The kernel reads them via [`take_system_request`] after each event
/// dispatch and updates the app stack accordingly.
#[derive(Debug)]
pub enum SystemRequest {
    /// Push `apps[idx]` onto the navigation stack (legacy numeric form).
    Launch(usize),
    /// Push the app with the given stable ID onto the navigation stack.
    LaunchById(String),
    /// Pop the current app from the navigation stack, returning to the caller.
    Return,
}

/// One entry in the runtime app registry, shared with scripts via the global pointer.
pub struct AppEntry {
    pub app_id: String,
    pub name: String,
    pub slot_idx: usize,
    /// Icon file stem for loading `assets/sprites/{icon_stem}_icon.pgm`.
    /// Empty string means no icon is available.
    pub icon_stem: String,
}

static mut PENDING_SYSTEM: Option<SystemRequest> = None;
/// Stable pointer to the runtime's app registry.
/// Set once by the runner after all apps are loaded; valid for process lifetime.
static mut ACTIVE_APP_LIST: Option<*const Vec<AppEntry>> = None;

/// Register the app list so scripts can call `system_list_apps()`.
/// The pointer must remain valid for the lifetime of the process.
/// # Safety
/// Caller must ensure `list` outlives all script execution.
pub unsafe fn set_app_list(list: *const Vec<AppEntry>) {
    ACTIVE_APP_LIST = Some(list);
}

/// Consume a pending system request emitted by the last script call.
pub fn take_system_request() -> Option<SystemRequest> {
    // SAFETY: single-threaded cooperative runtime; no concurrent access.
    #[allow(static_mut_refs)]
    unsafe {
        PENDING_SYSTEM.take()
    }
}

/// Native equivalent of the `system_list_apps()` Rhai function.
/// Returns a slice of all registered non-Launcher apps.
/// Returns an empty slice if the list has not been set yet.
pub fn app_list() -> &'static [AppEntry] {
    // SAFETY: pointer is set once at startup and valid for process lifetime.
    #[allow(static_mut_refs)]
    unsafe {
        ACTIVE_APP_LIST.map(|ptr| (*ptr).as_slice()).unwrap_or(&[])
    }
}

// --- Global pointers to the active canvas, database, and context --------
/// Only safe in a single-threaded SoulOS environment.
static mut ACTIVE_CANVAS: Option<*mut dyn ObjectSafeDraw> = None;
static mut ACTIVE_DB: Option<*mut Database> = None;
static mut ACTIVE_CTX: Option<*mut ()> = None;

/// Enhanced error information for debugging (no_std compatible)
#[derive(Debug)]
pub struct ScriptError {
    pub script_name: String,
    pub function_name: String,
    pub error_message: String,
    pub line: Option<usize>,
    pub position: Option<Position>,
}

impl ScriptError {
    pub fn from_rhai_error(script_name: &str, function_name: &str, error: &EvalAltResult) -> Self {
        let (line, position) = match error.position() {
            Position::NONE => (None, None),
            pos => {
                let line_num = pos.line();
                (line_num, Some(pos))
            }
        };

        ScriptError {
            script_name: script_name.to_string(),
            function_name: function_name.to_string(),
            error_message: error.to_string(),
            line,
            position,
        }
    }
}

pub struct ScriptedApp {
    engine: Engine,
    ast: AST,
    scope: Scope<'static>,
    pub db: Database,
    script_name: String,
    script_source: String,
    last_error: Option<ScriptError>,
}

impl ScriptedApp {
    pub fn new(name: &str, script: &str, db: Database) -> Result<Self, Box<rhai::ParseError>> {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(50, 50);

        // Register TextInput type
        engine.register_type_with_name::<TextInput>("TextInput");
        engine.register_fn("new_text_input", |x: i32, y: i32, w: i32, h: i32| {
            TextInput::new(Rectangle::new(
                Point::new(x, y),
                Size::new(w as u32, h as u32),
            ))
        });
        engine.register_fn("get_text", |input: &mut TextInput| input.text().to_string());
        engine.register_fn("set_text", |input: &mut TextInput, text: String| {
            input.set_text(text);
        });
        engine.register_fn("insert_char", |input: &mut TextInput, c: String| {
            if let Some(ch) = c.chars().next() {
                let _ = input.insert_char(ch);
            }
        });
        engine.register_fn("backspace", |input: &mut TextInput| {
            let _ = input.backspace();
        });

        // Register TextArea type
        engine.register_type_with_name::<TextArea>("TextArea");
        engine.register_fn("new_text_area", |x: i32, y: i32, w: i32, h: i32| {
            TextArea::new(Rectangle::new(
                Point::new(x, y),
                Size::new(w as u32, h as u32),
            ))
        });
        engine.register_fn("get_text", |area: &mut TextArea| area.text().to_string());
        engine.register_fn("set_text", |area: &mut TextArea, text: String| {
            area.set_text(text);
        });
        engine.register_fn("insert_char", |area: &mut TextArea, c: String| {
            if let Some(ch) = c.chars().next() {
                let _ = area.insert_char(ch);
            }
        });
        engine.register_fn("backspace", |area: &mut TextArea| {
            let _ = area.backspace();
        });
        engine.register_fn("enter", |area: &mut TextArea| {
            let _ = area.enter();
        });
        engine.register_fn(
            "pen_down",
            |area: &mut TextArea, x: i32, y: i32, now_ms: i32| {
                area.pen_down(x as i16, y as i16, now_ms as u64);
            },
        );
        engine.register_fn("pen_move", |area: &mut TextArea, x: i32, y: i32| {
            area.pen_moved(x as i16, y as i16);
        });
        engine.register_fn("pen_up", |area: &mut TextArea, x: i32, y: i32| {
            area.pen_released(x as i16, y as i16);
        });

        // Register Keyboard type
        engine.register_type_with_name::<Keyboard>("Keyboard");
        engine.register_fn("new_keyboard", || {
            Keyboard::new(APP_HEIGHT as i32 - KEYBOARD_HEIGHT as i32)
        });
        engine.register_fn(
            "handle_pen",
            |kb: &mut Keyboard, x: i32, y: i32, down: bool| -> String {
                let res = if down {
                    kb.pen_moved(x as i16, y as i16);
                    None
                } else {
                    kb.pen_released(x as i16, y as i16).typed
                };
                match res {
                    Some(TypedKey::Char(c)) => c.to_string(),
                    Some(TypedKey::Backspace) => "Backspace".to_string(),
                    Some(TypedKey::Enter) => "Enter".to_string(),
                    None => "".to_string(),
                }
            },
        );

        // Register Form type
        engine.register_type_with_name::<Form>("Form");
        engine.register_fn("new_form", |name: String| Form::new(&name));
        engine.register_fn("from_json", |json: String| {
            Form::from_json(&json).unwrap_or_else(|| Form::new("error"))
        });
        engine.register_fn("to_json", |form: &mut Form| form.to_json());

        // Register Database methods (Global)
        engine.register_fn("db_insert", |category: i32, data: Vec<u8>| unsafe {
            ACTIVE_DB
                .map(|db| (*db).insert(category as u8, data) as i32)
                .unwrap_or(0)
        });
        engine.register_fn("db_get_data", |id: i32| -> Vec<u8> {
            unsafe {
                ACTIVE_DB
                    .and_then(|db| (*db).get(id as u32).map(|r| r.data.clone()))
                    .unwrap_or_default()
            }
        });
        engine.register_fn("db_get_ids_in_category", |category: i32| -> rhai::Array {
            unsafe {
                ACTIVE_DB
                    .map(|db| {
                        (*db)
                            .iter_category(category as u8)
                            .map(|r| Dynamic::from(r.id as i32))
                            .collect()
                    })
                    .unwrap_or_default()
            }
        });
        engine.register_fn("db_update", |id: i32, data: Vec<u8>| unsafe {
            if let Some(db) = ACTIVE_DB {
                (*db).update(id as u32, data);
            }
        });
        engine.register_fn("db_delete", |id: i32| unsafe {
            if let Some(db) = ACTIVE_DB {
                (*db).delete(id as u32);
            }
        });

        // System call: push app[idx] onto the navigation stack (legacy, prefer system_launch_by_id)
        engine.register_fn("system_launch", |idx: i32| unsafe {
            PENDING_SYSTEM = Some(SystemRequest::Launch(idx as usize));
        });
        // System call: push the app with the given stable ID onto the navigation stack
        engine.register_fn("system_launch_by_id", |id: String| unsafe {
            PENDING_SYSTEM = Some(SystemRequest::LaunchById(id));
        });
        // System call: pop the current app, returning to the caller
        engine.register_fn("system_return", || unsafe {
            PENDING_SYSTEM = Some(SystemRequest::Return);
        });
        // System call: get the list of launchable apps as [{id, name, idx, icon}]
        engine.register_fn("system_list_apps", || -> rhai::Array {
            unsafe {
                ACTIVE_APP_LIST
                    .map(|ptr| {
                        (*ptr)
                            .iter()
                            .map(|entry| {
                                let mut m = Map::new();
                                m.insert("id".into(), Dynamic::from(entry.app_id.clone()));
                                m.insert("name".into(), Dynamic::from(entry.name.clone()));
                                m.insert("idx".into(), Dynamic::from(entry.slot_idx as i32));
                                m.insert("icon".into(), Dynamic::from(entry.icon_stem.clone()));
                                Dynamic::from_map(m)
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
        });

        // Simple print function for scripts (no logging infrastructure in no_std)
        engine.register_fn("print", |_s: String| {
            // In no_std environment, print is a no-op
            // Logging will be handled at the soul-runner level
        });
        engine.register_fn("to_string", |bytes: Vec<u8>| {
            String::from_utf8_lossy(&bytes).into_owned()
        });
        engine.register_fn("to_bytes", |s: String| s.into_bytes());

        // Register Context methods
        engine.register_fn("invalidate", |x: i32, y: i32, w: i32, h: i32| unsafe {
            if let Some(ctx_ptr) = ACTIVE_CTX {
                let ctx = &mut *(ctx_ptr as *mut Ctx);
                ctx.invalidate(Rectangle::new(
                    Point::new(x, y),
                    Size::new(w as u32, h as u32),
                ));
            }
        });
        engine.register_fn("invalidate_all", || unsafe {
            if let Some(ctx_ptr) = ACTIVE_CTX {
                let ctx = &mut *(ctx_ptr as *mut Ctx);
                ctx.invalidate_all();
            }
        });

        // Register Global drawing functions
        engine.register_fn("title_bar", |title: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).title_bar(&title);
            }
        });
        engine.register_fn(
            "button",
            |x: i32, y: i32, w: i32, h: i32, label: String, pressed: bool| unsafe {
                if let Some(canvas) = ACTIVE_CANVAS {
                    (*canvas).button(x, y, w as u32, h as u32, &label, pressed);
                }
            },
        );
        engine.register_fn("label", |x: i32, y: i32, text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).label(x, y, &text);
            }
        });
        engine.register_fn("clear", || unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).clear();
            }
        });
        engine.register_fn("draw_form", |form: Form| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).draw_form(&form);
            }
        });
        engine.register_fn("draw_text_input", |input: TextInput| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).draw_text_input(&input);
            }
        });
        engine.register_fn("draw_text_area", |area: TextArea| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).draw_text_area(&area);
            }
        });
        engine.register_fn("draw_keyboard", |kb: Keyboard| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).draw_keyboard(&kb);
            }
        });
        engine.register_fn(
            "draw_rect",
            |x: i32, y: i32, w: i32, h: i32, color: i32| unsafe {
                if let Some(canvas) = ACTIVE_CANVAS {
                    (*canvas).rect(x, y, w as u32, h as u32, color as u8);
                }
            },
        );
        engine.register_fn("form_tap", |form: &mut Form, x: i32, y: i32| -> String {
            for comp in &form.components {
                let rect = comp.bounds.to_eg_rect();
                if soul_ui::hit_test(&rect, x as i16, y as i16) {
                    return comp.id.clone();
                }
            }
            "".to_string()
        });

        // --- Modern layout system ---
        // Global layout state for proper positioning
        static mut LAYOUT_Y: i32 = 30;
        static mut LAYOUT_X: i32 = 10;
        static mut LAYOUT_MAX_WIDTH: i32 = 220;
        static mut IN_HORIZONTAL: bool = false;
        static mut HORIZONTAL_START_X: i32 = 10;
        
        // Input focus and keyboard state
        static mut INPUT_FOCUSED: bool = false;
        static mut SHOW_KEYBOARD: bool = false;
        static mut FOCUSED_INPUT_BOUNDS: (i32, i32, i32, i32) = (0, 0, 0, 0); // x, y, w, h

        engine.register_fn("ui_begin", || unsafe {
            LAYOUT_Y = 30;
            LAYOUT_X = 10;
            LAYOUT_MAX_WIDTH = 220;
            IN_HORIZONTAL = false;
        });

        engine.register_fn("ui_heading", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &text);
                LAYOUT_Y += 25;
            }
        });

        engine.register_fn("ui_label", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &text);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += text.len() as i32 * 6 + 10; // Approximate text width
                }
            }
        });

        engine.register_fn("ui_button", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let width = (text.len() as i32 * 7 + 16).max(60);
                (*canvas).button(LAYOUT_X, LAYOUT_Y, width as u32, 20, &text, false);
                
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 25;
                } else {
                    LAYOUT_X += width + 5;
                }
            }
        });


        engine.register_fn("ui_small_button", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let width = 20;
                (*canvas).button(LAYOUT_X, LAYOUT_Y, width, 15, &text, false);
                
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += width as i32 + 5;
                }
            }
        });

        engine.register_fn("ui_text_input", |current: String, placeholder: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let display_text = if current.is_empty() { &placeholder } else { &current };
                let is_focused = INPUT_FOCUSED && 
                    FOCUSED_INPUT_BOUNDS.0 == LAYOUT_X &&
                    FOCUSED_INPUT_BOUNDS.1 == LAYOUT_Y;
                
                // Store bounds for focus detection
                FOCUSED_INPUT_BOUNDS = (LAYOUT_X, LAYOUT_Y, 160, 24);
                
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 160, 24, display_text, is_focused);
                
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 30;
                } else {
                    LAYOUT_X += 165;
                }
            }
        });

        // Functions to manage input focus and keyboard
        engine.register_fn("ui_set_input_focus", |x: i32, y: i32, w: i32, h: i32| unsafe {
            INPUT_FOCUSED = true;
            SHOW_KEYBOARD = true;
            FOCUSED_INPUT_BOUNDS = (x, y, w, h);
        });

        engine.register_fn("ui_clear_input_focus", || unsafe {
            INPUT_FOCUSED = false;
            SHOW_KEYBOARD = false;
        });

        engine.register_fn("ui_is_input_focused", || -> bool {
            unsafe { INPUT_FOCUSED }
        });

        engine.register_fn("ui_should_show_keyboard", || -> bool {
            unsafe { SHOW_KEYBOARD }
        });

        engine.register_fn("ui_checkbox", |checked: bool, label: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let symbol = if checked { "[X]" } else { "[ ]" };
                let text = if label.is_empty() { 
                    symbol.to_string() 
                } else { 
                    let mut result = String::from(symbol);
                    result.push(' ');
                    result.push_str(&label);
                    result
                };
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &text);
                
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += text.len() as i32 * 6 + 10;
                }
            }
        });

        engine.register_fn("ui_selectable", |selected: bool, text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let width = (text.len() as i32 * 7 + 16).max(60);
                (*canvas).button(LAYOUT_X, LAYOUT_Y, width as u32, 20, &text, selected);
                
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 25;
                } else {
                    LAYOUT_X += width + 5;
                }
            }
        });

        engine.register_fn("ui_separator", || unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).rect(LAYOUT_X, LAYOUT_Y, LAYOUT_MAX_WIDTH as u32, 1, 128);
                LAYOUT_Y += 10;
            }
        });

        engine.register_fn("ui_space", |height: i32| unsafe {
            LAYOUT_Y += height;
        });

        engine.register_fn("ui_horizontal_begin", || unsafe {
            IN_HORIZONTAL = true;
            HORIZONTAL_START_X = LAYOUT_X;
        });

        engine.register_fn("ui_horizontal_end", || unsafe {
            IN_HORIZONTAL = false;
            LAYOUT_X = HORIZONTAL_START_X;
            LAYOUT_Y += 25;
        });

        engine.register_fn("ui_same_line", || unsafe {
            // Keep on same line for next element
            if !IN_HORIZONTAL {
                LAYOUT_Y -= 25; // Undo the last Y advance
            }
        });

        // === TEXT & INPUT COMPONENTS ===

        // Rich text label with formatting options
        engine.register_fn("ui_rich_text", |text: String, _color: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                // For now, just render as regular label - could add color support later
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &text);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += text.len() as i32 * 6 + 10;
                }
            }
        });

        // Hyperlink label
        engine.register_fn("ui_hyperlink", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                // Render as underlined-style text
                let mut underlined = String::from("_");
                underlined.push_str(&text);
                underlined.push('_');
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &underlined);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += text.len() as i32 * 6 + 10;
                }
            }
        });

        // Monospace text
        engine.register_fn("ui_monospace", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut mono_text = String::from("[");
                mono_text.push_str(&text);
                mono_text.push(']');
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &mono_text);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += text.len() as i32 * 6 + 10;
                }
            }
        });

        // Text input with hint
        engine.register_fn("ui_text_edit_hint", |current: String, hint: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let display_text = if current.is_empty() { 
                    let mut result = String::from("(");
                    result.push_str(&hint);
                    result.push(')');
                    result
                } else { 
                    current.clone()
                };
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 160, 24, &display_text, false);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 30;
                } else {
                    LAYOUT_X += 165;
                }
            }
        });

        // Password input
        engine.register_fn("ui_text_edit_password", |current: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let masked = "*".repeat(current.len());
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 160, 24, &masked, false);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 30;
                } else {
                    LAYOUT_X += 165;
                }
            }
        });

        // Multiline text edit
        engine.register_fn("ui_text_edit_multiline", |current: String, height: i32| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let h = if height <= 0 { 60 } else { height };
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 200, h as u32, &current, false);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += h + 10;
                } else {
                    LAYOUT_X += 205;
                }
            }
        });

        // Code editor (simplified)
        engine.register_fn("ui_code_editor", |current: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut code_text = String::from("{ ");
                code_text.push_str(&current);
                code_text.push_str(" }");
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 200, 60, &code_text, false);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 70;
                } else {
                    LAYOUT_X += 205;
                }
            }
        });

        // === SELECTION & INTERACTION COMPONENTS ===

        // Radio button
        engine.register_fn("ui_radio", |selected: bool, text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let symbol = if selected { "(*)" } else { "( )" };
                let mut radio_text = String::from(symbol);
                radio_text.push(' ');
                radio_text.push_str(&text);
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &radio_text);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += radio_text.len() as i32 * 6 + 10;
                }
            }
        });

        // Radio button with value
        engine.register_fn("ui_radio_value", |current_value: String, button_value: String, text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let selected = current_value == button_value;
                let symbol = if selected { "(*)" } else { "( )" };
                let mut radio_text = String::from(symbol);
                radio_text.push(' ');
                radio_text.push_str(&text);
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &radio_text);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += radio_text.len() as i32 * 6 + 10;
                }
            }
        });

        // Slider (horizontal)
        engine.register_fn("ui_slider", |value: i32, min: i32, max: i32| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut slider_text = String::from("◀");
                let range = max - min;
                let position = if range > 0 { (value - min) * 10 / range } else { 0 };
                for i in 0..10 {
                    if i == position {
                        slider_text.push('●');
                    } else {
                        slider_text.push('─');
                    }
                }
                slider_text.push_str("▶ ");
                slider_text.push_str(&value.to_string());
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &slider_text);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 25;
                } else {
                    LAYOUT_X += 100;
                }
            }
        });

        // Drag value (click and drag to change)
        engine.register_fn("ui_drag_value", |value: i32, prefix: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut drag_text = String::new();
                if !prefix.is_empty() {
                    drag_text.push_str(&prefix);
                    drag_text.push(' ');
                }
                drag_text.push_str(&value.to_string());
                drag_text.push_str(" ↔");
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 80, 20, &drag_text, false);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 25;
                } else {
                    LAYOUT_X += 85;
                }
            }
        });

        // ComboBox / Dropdown
        engine.register_fn("ui_combo_box", |label: String, selected: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut combo_text = String::new();
                combo_text.push_str(&selected);
                combo_text.push_str(" ▼");
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 120, 20, &combo_text, false);
                if !label.is_empty() {
                    (*canvas).label(LAYOUT_X - 60, LAYOUT_Y, &label);
                }
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 25;
                } else {
                    LAYOUT_X += 125;
                }
            }
        });

        // Progress bar
        engine.register_fn("ui_progress_bar", |progress: f32| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut bar_text = String::from("[");
                let filled = (progress * 10.0) as i32;
                for i in 0..10 {
                    if i < filled {
                        bar_text.push('█');
                    } else {
                        bar_text.push('░');
                    }
                }
                bar_text.push(']');
                bar_text.push(' ');
                bar_text.push_str(&((progress * 100.0) as i32).to_string());
                bar_text.push('%');
                (*canvas).label(LAYOUT_X, LAYOUT_Y, &bar_text);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += bar_text.len() as i32 * 6 + 10;
                }
            }
        });

        // Spinner (loading indicator)
        engine.register_fn("ui_spinner", || unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                (*canvas).label(LAYOUT_X, LAYOUT_Y, "⟲ Loading...");
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 20;
                } else {
                    LAYOUT_X += 80;
                }
            }
        });

        // Color picker (simplified)
        engine.register_fn("ui_color_edit", |color_name: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut color_text = String::from("■ ");
                color_text.push_str(&color_name);
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 100, 20, &color_text, false);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 25;
                } else {
                    LAYOUT_X += 105;
                }
            }
        });

        // === LAYOUT & CONTAINER COMPONENTS ===

        static mut GROUP_DEPTH: i32 = 0;
        static mut GROUP_START_Y: i32 = 0;

        // Group (visual container)
        engine.register_fn("ui_group", |title: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                if !title.is_empty() {
                    (*canvas).label(LAYOUT_X, LAYOUT_Y, &title);
                    LAYOUT_Y += 20;
                }
                // Draw group border
                (*canvas).rect(LAYOUT_X - 2, LAYOUT_Y, 220, 1, 128);
                GROUP_START_Y = LAYOUT_Y;
                GROUP_DEPTH += 1;
                LAYOUT_X += 10; // Indent group contents
                LAYOUT_Y += 5;
            }
        });

        engine.register_fn("ui_group_end", || unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                if GROUP_DEPTH > 0 {
                    LAYOUT_X -= 10; // Restore indent
                    LAYOUT_Y += 5;
                    // Draw bottom border
                    (*canvas).rect(LAYOUT_X - 2, LAYOUT_Y, 220, 1, 128);
                    GROUP_DEPTH -= 1;
                    LAYOUT_Y += 10;
                }
            }
        });

        // Collapsing header
        engine.register_fn("ui_collapsing", |title: String, open: bool| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let arrow = if open { "▼" } else { "▶" };
                let mut header_text = String::from(arrow);
                header_text.push(' ');
                header_text.push_str(&title);
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 200, 20, &header_text, false);
                LAYOUT_Y += 25;
                if open {
                    LAYOUT_X += 15; // Indent collapsed content
                }
            }
        });

        engine.register_fn("ui_collapsing_end", |was_open: bool| unsafe {
            if was_open {
                LAYOUT_X -= 15; // Restore indent
            }
        });

        // Vertical layout
        engine.register_fn("ui_vertical", || {
            // Already vertical by default, no-op
        });

        // Columns layout
        static mut COLUMN_COUNT: i32 = 1;
        static mut COLUMN_WIDTH: i32 = 220;
        static mut CURRENT_COLUMN: i32 = 0;
        static mut COLUMN_START_X: i32 = 10;
        static mut COLUMN_START_Y: i32 = 30;

        engine.register_fn("ui_columns", |count: i32| unsafe {
            COLUMN_COUNT = count.max(1);
            COLUMN_WIDTH = LAYOUT_MAX_WIDTH / COLUMN_COUNT;
            CURRENT_COLUMN = 0;
            COLUMN_START_X = LAYOUT_X;
            COLUMN_START_Y = LAYOUT_Y;
        });

        engine.register_fn("ui_next_column", || unsafe {
            CURRENT_COLUMN += 1;
            if CURRENT_COLUMN < COLUMN_COUNT {
                LAYOUT_X = COLUMN_START_X + (CURRENT_COLUMN * COLUMN_WIDTH);
                LAYOUT_Y = COLUMN_START_Y;
            }
        });

        engine.register_fn("ui_columns_end", || unsafe {
            LAYOUT_X = COLUMN_START_X;
            // Find the maximum Y across all columns
            LAYOUT_Y += 20; // Approximate spacing after columns
            COLUMN_COUNT = 1;
            CURRENT_COLUMN = 0;
        });

        // Grid layout (simplified)
        static mut GRID_COLUMNS: i32 = 2;
        static mut GRID_CURRENT_COL: i32 = 0;
        static mut GRID_ROW_HEIGHT: i32 = 25;

        engine.register_fn("ui_grid", |columns: i32| unsafe {
            GRID_COLUMNS = columns.max(1);
            GRID_CURRENT_COL = 0;
        });

        engine.register_fn("ui_grid_next", || unsafe {
            GRID_CURRENT_COL += 1;
            if GRID_CURRENT_COL >= GRID_COLUMNS {
                GRID_CURRENT_COL = 0;
                LAYOUT_Y += GRID_ROW_HEIGHT;
                LAYOUT_X = 10;
            } else {
                LAYOUT_X += LAYOUT_MAX_WIDTH / GRID_COLUMNS;
            }
        });

        // Indent/Unindent
        engine.register_fn("ui_indent", |amount: i32| unsafe {
            LAYOUT_X += amount;
        });

        engine.register_fn("ui_unindent", |amount: i32| unsafe {
            LAYOUT_X = (LAYOUT_X - amount).max(10);
        });

        // Scope (temporary layout state)
        static mut SCOPE_X: i32 = 10;
        static mut SCOPE_Y: i32 = 30;

        engine.register_fn("ui_scope_begin", || unsafe {
            SCOPE_X = LAYOUT_X;
            SCOPE_Y = LAYOUT_Y;
        });

        engine.register_fn("ui_scope_end", || unsafe {
            LAYOUT_X = SCOPE_X;
            LAYOUT_Y = SCOPE_Y;
        });

        // === VISUAL & STYLING COMPONENTS ===

        // Image placeholder
        engine.register_fn("ui_image", |width: i32, height: i32, alt_text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let w = if width <= 0 { 64 } else { width };
                let h = if height <= 0 { 64 } else { height };
                
                // Draw image placeholder border
                (*canvas).rect(LAYOUT_X, LAYOUT_Y, w as u32, h as u32, 200);
                (*canvas).rect(LAYOUT_X + 1, LAYOUT_Y + 1, (w-2) as u32, (h-2) as u32, 240);
                
                // Add alt text in center
                if !alt_text.is_empty() {
                    let text_x = LAYOUT_X + w / 2 - (alt_text.len() as i32 * 3);
                    let text_y = LAYOUT_Y + h / 2 - 5;
                    (*canvas).label(text_x, text_y, &alt_text);
                }
                
                if !IN_HORIZONTAL {
                    LAYOUT_Y += h + 10;
                } else {
                    LAYOUT_X += w + 10;
                }
            }
        });

        // Plot/Chart placeholder
        engine.register_fn("ui_plot", |title: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                if !title.is_empty() {
                    (*canvas).label(LAYOUT_X, LAYOUT_Y, &title);
                    LAYOUT_Y += 15;
                }
                
                // Draw plot area
                (*canvas).rect(LAYOUT_X, LAYOUT_Y, 180, 100, 220);
                (*canvas).rect(LAYOUT_X + 1, LAYOUT_Y + 1, 178, 98, 250);
                
                // Add grid lines
                for i in 1..4 {
                    let grid_y = LAYOUT_Y + (i * 25);
                    (*canvas).rect(LAYOUT_X + 5, grid_y, 170, 1, 230);
                }
                for i in 1..6 {
                    let grid_x = LAYOUT_X + (i * 30);
                    (*canvas).rect(grid_x, LAYOUT_Y + 5, 1, 90, 230);
                }
                
                // Add sample line
                (*canvas).label(LAYOUT_X + 80, LAYOUT_Y + 45, "📈");
                
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 110;
                } else {
                    LAYOUT_X += 190;
                }
            }
        });

        // Table
        static mut TABLE_ROWS: i32 = 0;
        static mut TABLE_COLS: i32 = 0;
        static mut TABLE_COL_WIDTH: i32 = 50;
        static mut TABLE_START_X: i32 = 10;
        static mut TABLE_START_Y: i32 = 30;

        engine.register_fn("ui_table", |columns: i32| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                TABLE_COLS = columns.max(1);
                TABLE_ROWS = 0;
                TABLE_COL_WIDTH = LAYOUT_MAX_WIDTH / TABLE_COLS;
                TABLE_START_X = LAYOUT_X;
                TABLE_START_Y = LAYOUT_Y;
                
                // Draw table header line
                (*canvas).rect(LAYOUT_X, LAYOUT_Y, (TABLE_COLS * TABLE_COL_WIDTH) as u32, 1, 128);
                LAYOUT_Y += 5;
            }
        });

        engine.register_fn("ui_table_header", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let col = TABLE_ROWS % TABLE_COLS;
                let cell_x = TABLE_START_X + (col * TABLE_COL_WIDTH);
                (*canvas).label(cell_x + 2, LAYOUT_Y, &text);
                
                // Draw vertical separator
                if col < TABLE_COLS - 1 {
                    (*canvas).rect(cell_x + TABLE_COL_WIDTH, LAYOUT_Y - 2, 1, 18, 128);
                }
                
                TABLE_ROWS += 1;
                if TABLE_ROWS % TABLE_COLS == 0 {
                    LAYOUT_Y += 20;
                    // Draw horizontal line after header row
                    (*canvas).rect(TABLE_START_X, LAYOUT_Y, (TABLE_COLS * TABLE_COL_WIDTH) as u32, 1, 128);
                    LAYOUT_Y += 5;
                }
            }
        });

        engine.register_fn("ui_table_cell", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let col = TABLE_ROWS % TABLE_COLS;
                let cell_x = TABLE_START_X + (col * TABLE_COL_WIDTH);
                (*canvas).label(cell_x + 2, LAYOUT_Y, &text);
                
                // Draw vertical separator
                if col < TABLE_COLS - 1 {
                    (*canvas).rect(cell_x + TABLE_COL_WIDTH, LAYOUT_Y - 2, 1, 18, 192);
                }
                
                TABLE_ROWS += 1;
                if TABLE_ROWS % TABLE_COLS == 0 {
                    LAYOUT_Y += 20;
                }
            }
        });

        engine.register_fn("ui_table_end", || unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                // Draw bottom border
                (*canvas).rect(TABLE_START_X, LAYOUT_Y, (TABLE_COLS * TABLE_COL_WIDTH) as u32, 1, 128);
                LAYOUT_Y += 10;
            }
        });

        // Menu and menu items
        engine.register_fn("ui_menu", |title: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut menu_text = String::from("≡ ");
                menu_text.push_str(&title);
                (*canvas).button(LAYOUT_X, LAYOUT_Y, 80, 20, &menu_text, false);
                if !IN_HORIZONTAL {
                    LAYOUT_Y += 25;
                } else {
                    LAYOUT_X += 85;
                }
            }
        });

        engine.register_fn("ui_menu_item", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let mut item_text = String::from("  ");
                item_text.push_str(&text);
                (*canvas).button(LAYOUT_X + 10, LAYOUT_Y, 120, 18, &item_text, false);
                LAYOUT_Y += 20;
            }
        });

        // Tooltip simulation
        engine.register_fn("ui_tooltip", |text: String| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                // Draw tooltip box
                let tooltip_width = (text.len() as i32 * 6 + 10).min(200);
                (*canvas).rect(LAYOUT_X, LAYOUT_Y - 25, tooltip_width as u32, 20, 100);
                (*canvas).rect(LAYOUT_X + 1, LAYOUT_Y - 24, (tooltip_width - 2) as u32, 18, 255);
                (*canvas).label(LAYOUT_X + 5, LAYOUT_Y - 20, &text);
            }
        });

        // Window (simplified)
        engine.register_fn("ui_window", |title: String, width: i32, height: i32| unsafe {
            if let Some(canvas) = ACTIVE_CANVAS {
                let w = if width <= 0 { 200 } else { width };
                let h = if height <= 0 { 150 } else { height };
                
                // Window border
                (*canvas).rect(LAYOUT_X, LAYOUT_Y, w as u32, h as u32, 128);
                (*canvas).rect(LAYOUT_X + 1, LAYOUT_Y + 1, (w-2) as u32, (h-2) as u32, 240);
                
                // Title bar
                (*canvas).rect(LAYOUT_X + 2, LAYOUT_Y + 2, (w-4) as u32, 20, 200);
                (*canvas).label(LAYOUT_X + 5, LAYOUT_Y + 6, &title);
                
                // Close button
                (*canvas).label(LAYOUT_X + w - 15, LAYOUT_Y + 6, "×");
                
                // Set content area
                LAYOUT_X += 5;
                LAYOUT_Y += 25;
            }
        });

        engine.register_fn("ui_window_end", |width: i32, height: i32| unsafe {
            let _w = if width <= 0 { 200 } else { width };
            let h = if height <= 0 { 150 } else { height };
            LAYOUT_X -= 5;
            LAYOUT_Y = LAYOUT_Y - 25 + h + 10; // Reset to after window
        });

        let ast = engine.compile(script)?;
        let mut scope = Scope::new();

        // Execute the script once to initialize global variables
        // Ignore initialization errors in no_std - they'll be caught at runtime
        let _ = engine.run_with_scope(&mut scope, script);

        Ok(Self {
            engine,
            ast,
            scope,
            db,
            script_name: name.to_string(),
            script_source: script.to_string(),
            last_error: None,
        })
    }

    /// Get the last error that occurred, if any (for debugging in std environments)
    pub fn last_error(&self) -> Option<&ScriptError> {
        self.last_error.as_ref()
    }

    /// Clear the last error
    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    /// Get script name for debugging
    pub fn script_name(&self) -> &str {
        &self.script_name
    }

    /// Get script source for debugging  
    pub fn script_source(&self) -> &str {
        &self.script_source
    }

    // --- Self-describing app identity -----------------------------------
    // Scripts declare their identity as top-level variables:
    //   let app_id   = "com.soulos.notes";
    //   let app_name = "Notes";
    //   let app_icon = "notes";   // → assets/sprites/notes_icon.pgm

    /// The stable, app-assigned identifier declared by the script.
    pub fn declared_app_id(&self) -> Option<String> {
        self.scope.get_value::<String>("app_id")
    }

    /// The display name declared by the script.
    pub fn declared_name(&self) -> Option<String> {
        self.scope.get_value::<String>("app_name")
    }

    /// The icon stem declared by the script (loaded as `{stem}_icon.pgm`).
    pub fn declared_icon_name(&self) -> Option<String> {
        self.scope.get_value::<String>("app_icon")
    }
}

impl App for ScriptedApp {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        let mut map = Map::new();
        match event {
            Event::AppStart => {
                map.insert("type".into(), "AppStart".into());
            }
            Event::AppStop => {
                map.insert("type".into(), "AppStop".into());
            }
            Event::PenDown { x, y } => {
                map.insert("type".into(), "PenDown".into());
                map.insert("x".into(), Dynamic::from(x as i32));
                map.insert("y".into(), Dynamic::from(y as i32));
            }
            Event::PenMove { x, y } => {
                map.insert("type".into(), "PenMove".into());
                map.insert("x".into(), Dynamic::from(x as i32));
                map.insert("y".into(), Dynamic::from(y as i32));
            }
            Event::PenUp { x, y } => {
                map.insert("type".into(), "PenUp".into());
                map.insert("x".into(), Dynamic::from(x as i32));
                map.insert("y".into(), Dynamic::from(y as i32));
            }
            Event::Tick(ms) => {
                map.insert("type".into(), "Tick".into());
                map.insert("ms".into(), Dynamic::from(ms as i32));
            }
            Event::Key(code) => {
                map.insert("type".into(), "Key".into());
                match code {
                    soul_core::KeyCode::Char(c) => map.insert("key".into(), c.to_string().into()),
                    soul_core::KeyCode::Backspace => map.insert("key".into(), "Backspace".into()),
                    soul_core::KeyCode::Enter => map.insert("key".into(), "Enter".into()),
                    _ => map.insert("key".into(), "Other".into()),
                };
            }
            _ => {
                map.insert("type".into(), "Other".into());
            }
        }
        map.insert("now_ms".into(), Dynamic::from(ctx.now_ms as i32));

        unsafe {
            ACTIVE_DB = Some(&mut self.db as *mut Database);
            ACTIVE_CTX = Some(ctx as *mut Ctx as *mut ());

            // Execute on_event and capture any errors for std environments to log
            if let Err(e) =
                self.engine
                    .call_fn::<()>(&mut self.scope, &self.ast, "on_event", (map,))
            {
                self.last_error = Some(ScriptError::from_rhai_error(
                    &self.script_name,
                    "on_event",
                    &e,
                ));
            }

            ACTIVE_DB = None;
            ACTIVE_CTX = None;
        }
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        unsafe {
            let bridge: &mut dyn ObjectSafeDraw = canvas;
            // Erase lifetime for storage in static
            let erased =
                core::mem::transmute::<&mut dyn ObjectSafeDraw, *mut dyn ObjectSafeDraw>(bridge);
            ACTIVE_CANVAS = Some(erased);
            ACTIVE_DB = Some(&mut self.db as *mut Database);

            // Execute on_draw and capture any errors for std environments to log
            if let Err(e) = self
                .engine
                .call_fn::<()>(&mut self.scope, &self.ast, "on_draw", ())
            {
                self.last_error = Some(ScriptError::from_rhai_error(
                    &self.script_name,
                    "on_draw",
                    &e,
                ));
            }

            ACTIVE_CANVAS = None;
            ACTIVE_DB = None;
        }
    }
}

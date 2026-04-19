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
                let line_num = pos.line().map(|l| l as usize);
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

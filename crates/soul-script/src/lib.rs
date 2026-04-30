#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::pixelcolor::Gray8;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use rhai::{Dynamic, Engine, EvalAltResult, Map, Position, Scope, AST, FnPtr};
use soul_core::{App, Ctx, Event, APP_HEIGHT, SCREEN_WIDTH};
use soul_db::Database;
use soul_ui::Form;
use soul_ui::TextInput;
use soul_ui::font_aa::FontFace;
use soul_ui::{Keyboard, TextArea, TypedKey, KEYBOARD_HEIGHT, TITLE_BAR_H};

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
    /// Draw a raw grayscale pixel buffer at `(x, y)`.
    /// `pixels` must contain exactly `w * h` bytes (one Gray8 luma per pixel).
    /// Pixels outside the canvas bounds are silently clipped.
    fn draw_pixels(&mut self, x: i32, y: i32, w: u32, pixels: &[u8]);
    /// Like `draw_pixels` but inverts each luma value (255 − p) before drawing.
    /// Used for press-highlight effects without allocating in the script.
    fn draw_pixels_inverted(&mut self, x: i32, y: i32, w: u32, pixels: &[u8]);
    fn draw_scrollbar(&mut self, scroll_offset: i32, content_height: i32, viewport_height: i32);
    /// Render the output of an EGUI frame onto the canvas.
    fn render_egui_frame(&mut self, egui_output: egui::FullOutput);
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
        unsafe {
            let content_bottom = (y + h as i32) as u32;
            ACTIVE_CONTENT_HEIGHT = ACTIVE_CONTENT_HEIGHT.max(content_bottom);
        }
    }

    fn label(&mut self, x: i32, y: i32, text: &str) {
        let _ = soul_ui::label(self, Point::new(x, y), text);
        unsafe {
            let content_bottom = (y + 10) as u32; // Assume ~10px text height
            ACTIVE_CONTENT_HEIGHT = ACTIVE_CONTENT_HEIGHT.max(content_bottom);
        }
    }

    fn clear(&mut self) {
        let _ = soul_ui::clear(self, SCREEN_WIDTH as u32, APP_HEIGHT as u32);
        unsafe {
            ACTIVE_CONTENT_HEIGHT = 0; // Reset content height on clear
        }
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
        unsafe {
            let content_bottom = (y + h as i32) as u32;
            ACTIVE_CONTENT_HEIGHT = ACTIVE_CONTENT_HEIGHT.max(content_bottom);
        }
    }

    fn draw_scrollbar(&mut self, _scroll_offset: i32, _content_height: i32, _viewport_height: i32) {
        // Manual scrollbar drawing removed - now handled by EGUI ScrollArea
    }

    fn draw_pixels(&mut self, x: i32, y: i32, w: u32, pixels: &[u8]) {
        if w == 0 || pixels.is_empty() { return; }
        let raw = ImageRaw::<Gray8>::new(pixels, w);
        let _ = Image::new(&raw, Point::new(x, y)).draw(self);
    }

    fn draw_pixels_inverted(&mut self, x: i32, y: i32, w: u32, pixels: &[u8]) {
        if w == 0 || pixels.is_empty() { return; }
        let inv: alloc::vec::Vec<u8> = pixels.iter().map(|&p| 255 - p).collect();
        let raw = ImageRaw::<Gray8>::new(&inv, w);
        let _ = Image::new(&raw, Point::new(x, y)).draw(self);
    }

    fn render_egui_frame(&mut self, egui_output: egui::FullOutput) {
        for clipped_shape in egui_output.shapes {
            match clipped_shape.shape {
                egui::Shape::Rect(rect_shape) => {
                    // Skip fully transparent shapes
                    if rect_shape.fill.a() == 0 {
                        continue;
                    }
                    
                    let rect = rect_shape.rect;
                    let color = color_to_gray8(rect_shape.fill);
                    let eg_rect = Rectangle::new(
                        Point::new(rect.min.x as i32, rect.min.y as i32),
                        Size::new(rect.width() as u32, rect.height() as u32),
                    );
                    let _ = self.fill_solid(&eg_rect, color);
                    
                    // Draw stroke if present
                    if rect_shape.stroke.width > 0.0 && rect_shape.stroke.color.a() > 0 {
                        let _stroke_color = color_to_gray8(rect_shape.stroke.color);
                        // Simplified stroke as a border rect for now
                        // Proper stroke would need primitives::Styled
                    }
                }
                egui::Shape::Text(text_shape) => {
                    if text_shape.fallback_color.a() == 0 && text_shape.galley.job.sections.iter().all(|s| s.format.color.a() == 0) {
                        continue;
                    }
                    
                    let pos = text_shape.pos;
                    let text = text_shape.galley.text();
                    
                    // Skip very short text fragments that might be causing overlap
                    if text.len() < 2 && text.chars().all(|c| c.is_whitespace()) {
                        continue;
                    }
                    
                    // Simple approach: render the text only once per galley
                    if !text.trim().is_empty() {
                        let _ = soul_ui::label(self, Point::new(pos.x as i32, pos.y as i32), &text);
                    }
                }
                _ => {}
            }
        }
    }
}

fn color_to_gray8(c: egui::Color32) -> Gray8 {
    // Simple luma conversion: (r + g + b) / 3
    let luma = (c.r() as u32 + c.g() as u32 + c.b() as u32) / 3;
    Gray8::new(luma as u8)
}

// --- System call protocol -----------------------------------------------

/// A request from a scripted app to the runtime kernel.
///
/// Scripts emit these via system call functions (`system_launch_by_id`,
/// `system_return`, `system_send`, `system_request`). The kernel reads
/// them via [`take_system_request`] after each event dispatch.
#[derive(Debug)]
pub enum SystemRequest {
    /// Push `apps[idx]` onto the navigation stack (legacy numeric form).
    Launch(usize),
    /// Push the app with the given stable ID onto the navigation stack.
    LaunchById(String),
    /// Pop the current app from the navigation stack, returning to the caller.
    Return,
    /// Deliver a payload to another app (or to a kernel-chosen handler).
    ///
    /// `action` names the kind of transfer ("open_bitmap", "set_icon", …).
    /// `target` is an optional destination app ID; when absent the kernel
    /// picks the registered handler (or presents a picker if there are
    /// multiple). `payload` is the data being sent.
    Send {
        action: String,
        payload: soul_core::ExchangePayload,
        target: Option<String>,
    },
    /// Ask the kernel to launch the registered handler for `action` and
    /// deliver its result back to this app as [`soul_core::Event::Exchange`].
    /// `payload` is forwarded to the handler in the opening Exchange event;
    /// scripted apps use [`soul_core::ExchangePayload::Text`] with an empty
    /// string (the Rhai `system_request` function).  Native apps can pass
    /// richer payloads (e.g. a `Bitmap` when opening Draw for icon editing).
    Request {
        action: String,
        payload: soul_core::ExchangePayload,
    },
    /// Return a result payload to the app that `Request`-ed this one.
    ///
    /// Emitted by a handler app when it's done; the kernel routes the
    /// payload back to the requester and then pops this app off the stack.
    SendResult {
        action: String,
        payload: soul_core::ExchangePayload,
    },
    /// Dispatch a payload directly to a named target app **without** pushing
    /// it onto the navigation stack or calling its `draw`. Used for
    /// kernel-mediated background services (e.g. resource get/set via the
    /// Launcher). The target app handles the event synchronously and any
    /// `SendResult` it emits is delivered back to the caller in the same
    /// dispatch cycle.
    BackgroundSend {
        action: String,
        payload: soul_core::ExchangePayload,
        /// Target app ID. Empty string routes to the first registered
        /// capability-index handler (same as `Send`).
        target: String,
    },
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
// Content height tracking for scroll detection
static mut ACTIVE_CONTENT_HEIGHT: u32 = 0;
// EGUI bridge for native scrolling
static mut ACTIVE_EGUI_BRIDGE: Option<*mut soul_ui::EguiRhaiBridge> = None;

// Rhai engine, scope, and AST for executing FnPtrs within EGUI closures
static mut ACTIVE_RHAI_ENGINE: Option<*mut Engine> = None;
static mut ACTIVE_RHAI_SCOPE: Option<*mut Scope<'static>> = None;
static mut ACTIVE_RHAI_AST: Option<*const AST> = None;

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

// --- Icon loading (std only) --------------------------------------------

/// Load and decode a PGM icon, returning only the raw pixel bytes.
/// Looks for `assets/sprites/{stem}_icon.pgm` relative to the working
/// directory.  Returns `None` on any I/O or parse error.
#[cfg(feature = "std")]
fn load_pgm_pixels(stem: &str) -> Option<Vec<u8>> {
    use std::io::{BufRead, BufReader, Read};
    let path = std::path::PathBuf::from("assets/sprites")
        .join(alloc::format!("{stem}_icon.pgm"));
    let f = std::fs::File::open(&path).ok()?;
    let mut r = BufReader::new(f);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    if line.trim() != "P5" { return None; }
    // Skip comments and read width/height
    let (w, h) = read_pgm_pair(&mut r)?;
    // Read and discard max-value line
    let _maxv = read_pgm_value(&mut r)?;
    let mut pixels = alloc::vec![0u8; w * h];
    r.read_exact(&mut pixels).ok()?;
    Some(pixels)
}

#[cfg(feature = "std")]
fn read_pgm_pair<R: std::io::BufRead>(r: &mut R) -> Option<(usize, usize)> {
    let mut line = String::new();
    loop {
        line.clear();
        r.read_line(&mut line).ok()?;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        let mut it = t.splitn(2, ' ');
        let w = it.next()?.parse().ok()?;
        let h = it.next()?.parse().ok()?;
        return Some((w, h));
    }
}

#[cfg(feature = "std")]
fn read_pgm_value<R: std::io::BufRead>(r: &mut R) -> Option<usize> {
    let mut line = String::new();
    loop {
        line.clear();
        r.read_line(&mut line).ok()?;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        return t.parse().ok();
    }
}

// --- ScriptedApp --------------------------------------------------------

pub struct ScriptedApp {
    engine: Engine,
    ast: AST,
    scope: Scope<'static>,
    pub db: Database,
    script_name: String,
    script_source: String,
    last_error: Option<ScriptError>,
    // Simple state tracking
    last_content_height: u32,
    egui_context: egui::Context,
    egui_bridge: soul_ui::EguiRhaiBridge,
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
        engine.register_fn("cursor_left",  |area: &mut TextArea| { let _ = area.cursor_left(); });
        engine.register_fn("cursor_right", |area: &mut TextArea| { let _ = area.cursor_right(); });
        engine.register_fn("cursor_up",    |area: &mut TextArea| { let _ = area.cursor_up(); });
        engine.register_fn("cursor_down",  |area: &mut TextArea| { let _ = area.cursor_down(); });
        engine.register_fn("page_up",      |area: &mut TextArea| { let _ = area.page_up(); });
        engine.register_fn("page_down",    |area: &mut TextArea| { let _ = area.page_down(); });
        engine.register_fn("set_font", |area: &mut TextArea, name: String| {
            let face = match name.to_lowercase().as_str() {
                "serif" => FontFace::Serif,
                "mono"  => FontFace::Mono,
                _       => FontFace::Sans,
            };
            area.set_face(face);
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
        // System call: send a payload to another app (or a kernel-chosen handler).
        // In Rhai: system_send("open_bitmap", pixels_blob, "com.soulos.draw")
        //          system_send("open_bitmap", pixels_blob, "")   ← kernel picks handler
        engine.register_fn(
            "system_send",
            |action: String, pixels: Vec<u8>, target: String| unsafe {
                let payload = soul_core::ExchangePayload::Bitmap {
                    width: 0,
                    height: 0,
                    pixels,
                };
                PENDING_SYSTEM = Some(SystemRequest::Send {
                    action,
                    payload,
                    target: if target.is_empty() { None } else { Some(target) },
                });
            },
        );
        // System call: send a bitmap with explicit dimensions.
        // In Rhai: system_send_bitmap("open_bitmap", w, h, pixels, "")
        engine.register_fn(
            "system_send_bitmap",
            |action: String, width: i32, height: i32, pixels: Vec<u8>, target: String| unsafe {
                let payload = soul_core::ExchangePayload::Bitmap {
                    width: width as u16,
                    height: height as u16,
                    pixels,
                };
                PENDING_SYSTEM = Some(SystemRequest::Send {
                    action,
                    payload,
                    target: if target.is_empty() { None } else { Some(target) },
                });
            },
        );
        // System call: send a text payload.
        // In Rhai: system_send_text("open_script", my_text, "")
        engine.register_fn(
            "system_send_text",
            |action: String, text: String, target: String| unsafe {
                let payload = soul_core::ExchangePayload::Text(text);
                PENDING_SYSTEM = Some(SystemRequest::Send {
                    action,
                    payload,
                    target: if target.is_empty() { None } else { Some(target) },
                });
            },
        );
        // System call: ask the kernel to fulfil an action and deliver the
        // result back to this app as an Exchange event.
        // In Rhai: system_request("pick_contact")
        engine.register_fn("system_request", |action: String| unsafe {
            PENDING_SYSTEM = Some(SystemRequest::Request {
                action,
                payload: soul_core::ExchangePayload::Text(String::new()),
            });
        });
        // System call: return a bitmap result to the app that request-ed this one.
        // In Rhai: system_send_result("return_bitmap", w, h, pixels)
        engine.register_fn(
            "system_send_result",
            |action: String, width: i32, height: i32, pixels: Vec<u8>| unsafe {
                let payload = soul_core::ExchangePayload::Bitmap {
                    width: width as u16,
                    height: height as u16,
                    pixels,
                };
                PENDING_SYSTEM = Some(SystemRequest::SendResult { action, payload });
            },
        );
        // System call: return a text result to the app that request-ed this one.
        // In Rhai: system_send_text_result("return_script", source_text)
        engine.register_fn(
            "system_send_text_result",
            |action: String, text: String| unsafe {
                let payload = soul_core::ExchangePayload::Text(text);
                PENDING_SYSTEM = Some(SystemRequest::SendResult { action, payload });
            },
        );

        // --- Kernel resource API -----------------------------------------------
        // Background calls — dispatched to the Launcher without showing any UI.
        // The result arrives as an Exchange event (action "return_resource") in the
        // next on_event call.
        //
        // In Rhai: system_get_resource("com.soulos.notes", "icon")
        //   → ev.type=="Exchange", ev.action=="return_resource",
        //     ev.payload.resource=="icon", ev.payload.pixels==[...]
        engine.register_fn(
            "system_get_resource",
            |app_id: String, kind: String| unsafe {
                let payload = soul_core::ExchangePayload::Resource {
                    app_id,
                    kind,
                    width: 0,
                    height: 0,
                    pixels: alloc::vec![],
                    text: String::new(),
                };
                PENDING_SYSTEM = Some(SystemRequest::BackgroundSend {
                    action: "get_resource".to_string(),
                    payload,
                    target: String::new(),
                });
            },
        );
        // In Rhai: system_set_resource_bitmap("com.soulos.notes", "icon", w, h, pixels)
        engine.register_fn(
            "system_set_resource_bitmap",
            |app_id: String, kind: String, width: i32, height: i32, pixels: Vec<u8>| unsafe {
                let payload = soul_core::ExchangePayload::Resource {
                    app_id,
                    kind,
                    width: width as u16,
                    height: height as u16,
                    pixels,
                    text: String::new(),
                };
                PENDING_SYSTEM = Some(SystemRequest::BackgroundSend {
                    action: "set_resource".to_string(),
                    payload,
                    target: String::new(),
                });
            },
        );
        // In Rhai: system_set_resource_text("com.soulos.notes", "script", source)
        engine.register_fn(
            "system_set_resource_text",
            |app_id: String, kind: String, text: String| unsafe {
                let payload = soul_core::ExchangePayload::Resource {
                    app_id,
                    kind,
                    width: 0,
                    height: 0,
                    pixels: alloc::vec![],
                    text,
                };
                PENDING_SYSTEM = Some(SystemRequest::BackgroundSend {
                    action: "set_resource".to_string(),
                    payload,
                    target: String::new(),
                });
            },
        );
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

        // Screen / layout dimension queries — preferred over hard-coded numbers.
        engine.register_fn("screen_width",    || SCREEN_WIDTH as i32);
        engine.register_fn("app_height",      || APP_HEIGHT as i32);
        engine.register_fn("title_bar_height", || TITLE_BAR_H as i32);
        engine.register_fn("keyboard_height", || KEYBOARD_HEIGHT as i32);
        engine.register_fn("icon_size",       || 32i32);

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
        
        // Register scroll helper functions
        engine.register_fn("get_content_height", || -> i32 {
            unsafe { ACTIVE_CONTENT_HEIGHT as i32 }
        });
        
        engine.register_fn("needs_scrolling", || -> bool {
            unsafe { ACTIVE_CONTENT_HEIGHT > APP_HEIGHT as u32 }
        });
        
        engine.register_fn("get_viewport_height", || -> i32 {
            APP_HEIGHT as i32
        });

        // Register EGUI Bridge functions for native scrolling
        Self::register_egui_bridge_functions(&mut engine);
        
        // Register native EGUI widgets
        engine.register_fn("egui_label", |_ui: Dynamic, text: String| unsafe {
            if let Some(ui_ptr) = soul_ui::ACTIVE_UI {
                (*ui_ptr).label(text);
            }
        });
        engine.register_fn("egui_button", |_ui: Dynamic, text: String| -> bool {
            unsafe {
                if let Some(ui_ptr) = soul_ui::ACTIVE_UI {
                    (*ui_ptr).button(text).clicked()
                } else {
                    false
                }
            }
        });
        engine.register_fn("egui_checkbox", |_ui: Dynamic, checked: bool, text: String| -> bool {
            unsafe {
                if let Some(ui_ptr) = soul_ui::ACTIVE_UI {
                    let mut val = checked;
                    (*ui_ptr).checkbox(&mut val, text);
                    val
                } else {
                    checked
                }
            }
        });
        engine.register_fn("egui_separator", |_ui: Dynamic| unsafe {
            if let Some(ui_ptr) = soul_ui::ACTIVE_UI {
                (*ui_ptr).separator();
            }
        });
        engine.register_fn("egui_title_bar", |_ui: Dynamic, title: String| unsafe {
            if let Some(ui_ptr) = soul_ui::ACTIVE_UI {
                (*ui_ptr).add(soul_ui::SoulOSTitleBar::new(title, soul_core::SCREEN_WIDTH as f32));
            }
        });
        engine.register_fn("egui_space", |_ui: Dynamic, amount: i32| unsafe {
            if let Some(ui_ptr) = soul_ui::ACTIVE_UI {
                (*ui_ptr).add_space(amount as f32);
            }
        });
        engine.register_fn("egui_begin", || {});
        engine.register_fn("egui_end", || {});

        engine.register_fn("egui_small_button", |_ui: Dynamic, text: String| -> bool {
            unsafe {
                if let Some(ui_ptr) = soul_ui::ACTIVE_UI {
                    // Small button style
                    (*ui_ptr).add(egui::Button::new(text).small()).clicked()
                } else {
                    false
                }
            }
        });

        engine.register_fn("egui_toolbar", |_w: i32, _h: i32, content: FnPtr| {
            // Toolbars are usually at the top/bottom. 
            // In SoulOS, we can just treat it as a group or just run the content.
            // For now, let's just run the content.
            if let (Some(engine_ptr), Some(ast_ptr)) = unsafe { (ACTIVE_RHAI_ENGINE, ACTIVE_RHAI_AST) } {
                let engine = unsafe { &*engine_ptr };
                let ast = unsafe { &*ast_ptr };
                let _ = content.call::<()>(engine, ast, (Dynamic::from(()),));
            }
        });
        engine.register_fn("egui_selectable_label", |_ui: Dynamic, selected: bool, text: String| -> bool {
            unsafe {
                if let Some(ui_ptr) = soul_ui::ACTIVE_UI {
                    (*ui_ptr).selectable_label(selected, text).clicked()
                } else {
                    false
                }
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

        // Draw a raw grayscale pixel buffer at (x, y) with the given width.
        // The height is inferred from pixels.len() / w.
        // In Rhai: draw_pixels(x, y, w, pixels_blob)
        engine.register_fn(
            "draw_pixels",
            |x: i32, y: i32, w: i32, pixels: Vec<u8>| unsafe {
                if let Some(canvas) = ACTIVE_CANVAS {
                    (*canvas).draw_pixels(x, y, w as u32, &pixels);
                }
            },
        );
        // In Rhai: draw_pixels_inverted(x, y, w, pixels_blob)
        // Draws with each luma inverted (255 − p) — useful for press highlights.
        engine.register_fn(
            "draw_pixels_inverted",
            |x: i32, y: i32, w: i32, pixels: Vec<u8>| unsafe {
                if let Some(canvas) = ACTIVE_CANVAS {
                    (*canvas).draw_pixels_inverted(x, y, w as u32, &pixels);
                }
            },
        );

        // Font metrics for the default label font (FONT_6X10).
        // Use these to center or truncate labels without hard-coding pixel values.
        // In Rhai: let x = cx - (text.len() * font_char_width()) / 2;
        engine.register_fn("font_char_width",  || 6i32);
        engine.register_fn("font_char_height", || 10i32);

        // Load the raw pixel data for a named icon stem.
        // Reads `assets/sprites/{stem}_icon.pgm` and returns the decoded pixel
        // bytes as a blob.  Falls back to `default_icon.pgm` when the file is
        // missing or the stem is empty; returns an empty blob only if the
        // fallback is also unavailable.
        // In Rhai: let px = load_icon("notes");  // Vec<u8>, 32×32 = 1024 bytes
        #[cfg(feature = "std")]
        engine.register_fn("load_icon", |stem: String| -> Vec<u8> {
            if !stem.is_empty() {
                if let Some(px) = load_pgm_pixels(&stem) {
                    return px;
                }
            }
            load_pgm_pixels("default").unwrap_or_default()
        });

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

        let egui_context = egui::Context::default();
        let egui_bridge = soul_ui::EguiRhaiBridge::new(egui_context.clone());

        Ok(Self {
            engine,
            ast,
            scope,
            db,
            script_name: name.to_string(),
            script_source: script.to_string(),
            last_error: None,
            last_content_height: 0,
            egui_context,
            egui_bridge,
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

    /// Get a value from the script's global scope.
    pub fn get_var<T: 'static + Clone>(&self, name: &str) -> Option<T> {
        self.scope.get_value::<T>(name)
    }

    /// Get script source for debugging  
    pub fn script_source(&self) -> &str {
        &self.script_source
    }

    // --- Self-describing app identity -----------------------------------
    // Scripts declare their identity as top-level variables:
    //   let app_id      = "com.soulos.notes";
    //   let app_name    = "Notes";
    //   let app_icon    = "notes";          // → assets/sprites/notes_icon.pgm
    //   let app_handles = ["open_script"];  // exchange actions this app handles

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

    /// Exchange actions this app can handle, declared as `let app_handles = ["open_bitmap"];`.
    pub fn declared_handles(&self) -> Vec<String> {
        self.scope
            .get_value::<rhai::Array>("app_handles")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.try_cast::<String>())
            .collect()
    }

    /// Register EGUI bridge functions with the Rhai engine
    fn register_egui_bridge_functions(engine: &mut Engine) {
        // Register native EGUI scroll area function
        engine.register_fn("egui_scroll_area", |id: String, max_height: i32, content: FnPtr| {
            unsafe {
                if let Some(bridge_ptr) = ACTIVE_EGUI_BRIDGE {
                    let bridge = &*bridge_ptr;
                    (*bridge).create_scroll_area(&id, max_height as f32, |_ui| {
                        if let (Some(engine_ptr), Some(ast_ptr)) = (ACTIVE_RHAI_ENGINE, ACTIVE_RHAI_AST) {
                            let engine = &*engine_ptr;
                            let ast = &*ast_ptr;
                            // Pass mock ui object for compatibility
                            let _ = content.call::<()>(engine, ast, (Dynamic::from(()),));
                        }
                    });
                }
            }
        });

        // Register EGUI group function
        engine.register_fn("egui_group", |_ui: Dynamic, title: String, content: FnPtr| {
            unsafe {
                if let Some(bridge_ptr) = ACTIVE_EGUI_BRIDGE {
                    let bridge = &*bridge_ptr;
                    (*bridge).group(&title, |_ui| {
                        if let (Some(engine_ptr), Some(ast_ptr)) = (ACTIVE_RHAI_ENGINE, ACTIVE_RHAI_AST) {
                            let engine = &*engine_ptr;
                            let ast = &*ast_ptr;
                            let _ = content.call::<()>(engine, ast, (Dynamic::from(()),));
                        }
                    });
                }
            }
        });

        // Register horizontal layout function
        engine.register_fn("egui_horizontal_layout", |_ui: Dynamic, content: FnPtr| {
            unsafe {
                if let Some(bridge_ptr) = ACTIVE_EGUI_BRIDGE {
                    let bridge = &*bridge_ptr;
                    bridge.horizontal_layout(|_ui| {
                        if let (Some(engine_ptr), Some(ast_ptr)) = (ACTIVE_RHAI_ENGINE, ACTIVE_RHAI_AST) {
                            let engine = &*engine_ptr;
                            let ast = &*ast_ptr;
                            let _ = content.call::<()>(engine, ast, (Dynamic::from(()),));
                        }
                    });
                }
            }
        });

        // Register vertical layout function
        engine.register_fn("egui_vertical_layout", |_ui: Dynamic, content: FnPtr| {
            unsafe {
                if let Some(bridge_ptr) = ACTIVE_EGUI_BRIDGE {
                    let bridge = &*bridge_ptr;
                    bridge.vertical_layout(|_ui| {
                        if let (Some(engine_ptr), Some(ast_ptr)) = (ACTIVE_RHAI_ENGINE, ACTIVE_RHAI_AST) {
                            let engine = &*engine_ptr;
                            let ast = &*ast_ptr;
                            let _ = content.call::<()>(engine, ast, (Dynamic::from(()),));
                        }
                    });
                }
            }
        });
    }
}

impl App for ScriptedApp {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        // Convert SoulOS event to EGUI event
        let egui_event = match event {
            Event::PenDown { x, y } => Some(egui::Event::PointerButton {
                pos: egui::Pos2::new(x as f32, y as f32),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }),
            Event::PenMove { x, y } => Some(egui::Event::PointerMoved(egui::Pos2::new(x as f32, y as f32))),
            Event::PenUp { x, y } => Some(egui::Event::PointerButton {
                pos: egui::Pos2::new(x as f32, y as f32),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }),
            _ => None,
        };

        // Let EGUI handle the event
        let mut consumed = false;
        if let Some(e) = egui_event {
            self.egui_context.input_mut(|i| i.events.push(e));
            // Check if any widget consumed the pointer in the last frame
            // This is a heuristic; proper consumption check happens during `run()`
            consumed = self.egui_context.wants_pointer_input();
        }

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
                    soul_core::KeyCode::Char(c)  => map.insert("key".into(), c.to_string().into()),
                    soul_core::KeyCode::Backspace => map.insert("key".into(), "Backspace".into()),
                    soul_core::KeyCode::Enter     => map.insert("key".into(), "Enter".into()),
                    soul_core::KeyCode::ArrowLeft  => map.insert("key".into(), "ArrowLeft".into()),
                    soul_core::KeyCode::ArrowRight => map.insert("key".into(), "ArrowRight".into()),
                    soul_core::KeyCode::ArrowUp    => map.insert("key".into(), "ArrowUp".into()),
                    soul_core::KeyCode::ArrowDown  => map.insert("key".into(), "ArrowDown".into()),
                    _ => map.insert("key".into(), "Other".into()),
                };
            }
            Event::Exchange { action, payload, sender } => {
                map.insert("type".into(), "Exchange".into());
                map.insert("action".into(), Dynamic::from(action));
                map.insert("sender".into(), Dynamic::from(sender));
                match payload {
                    soul_core::ExchangePayload::Bitmap { width, height, pixels } => {
                        let mut p = Map::new();
                        p.insert("kind".into(),   Dynamic::from("bitmap".to_string()));
                        p.insert("width".into(),  Dynamic::from(width as i32));
                        p.insert("height".into(), Dynamic::from(height as i32));
                        p.insert("pixels".into(), Dynamic::from(pixels));
                        map.insert("payload".into(), Dynamic::from_map(p));
                    }
                    soul_core::ExchangePayload::Text(text) => {
                        let mut p = Map::new();
                        p.insert("kind".into(), Dynamic::from("text".to_string()));
                        p.insert("text".into(), Dynamic::from(text));
                        map.insert("payload".into(), Dynamic::from_map(p));
                    }
                    soul_core::ExchangePayload::Resource { app_id, kind, width, height, pixels, text } => {
                        let mut p = Map::new();
                        p.insert("kind".into(),   Dynamic::from("resource".to_string()));
                        p.insert("app_id".into(), Dynamic::from(app_id));
                        p.insert("resource".into(), Dynamic::from(kind));
                        p.insert("width".into(),  Dynamic::from(width as i32));
                        p.insert("height".into(), Dynamic::from(height as i32));
                        p.insert("pixels".into(), Dynamic::from(pixels));
                        p.insert("text".into(),   Dynamic::from(text));
                        map.insert("payload".into(), Dynamic::from_map(p));
                    }
                }
            }
            Event::Menu => {
                map.insert("type".into(), "Menu".into());
            }
            Event::ButtonDown(btn) => {
                map.insert("type".into(), "ButtonDown".into());
                let name = match btn {
                    soul_core::HardButton::AppA     => "AppA",
                    soul_core::HardButton::AppB     => "AppB",
                    soul_core::HardButton::AppC     => "AppC",
                    soul_core::HardButton::AppD     => "AppD",
                    soul_core::HardButton::Home     => "Home",
                    soul_core::HardButton::Menu     => "Menu",
                    soul_core::HardButton::Power    => "Power",
                    soul_core::HardButton::PageUp   => "PageUp",
                    soul_core::HardButton::PageDown => "PageDown",
                    soul_core::HardButton::VolumeUp   => "VolumeUp",
                    soul_core::HardButton::VolumeDown => "VolumeDown",
                };
                map.insert("button".into(), Dynamic::from(name.to_string()));
            }
            _ => {
                map.insert("type".into(), "Other".into());
            }
        }
        map.insert("now_ms".into(), Dynamic::from(ctx.now_ms as i32));

        if !consumed {
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
    }

    fn draw<D>(&mut self, canvas: &mut D, _dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>,
    {
        unsafe {
            let bridge_interface: &mut dyn ObjectSafeDraw = canvas;
            // Erase lifetime for storage in static
            let erased =
                core::mem::transmute::<&mut dyn ObjectSafeDraw, *mut dyn ObjectSafeDraw>(bridge_interface);
            ACTIVE_CANVAS = Some(erased);
            ACTIVE_DB = Some(&mut self.db as *mut Database);
            ACTIVE_CONTENT_HEIGHT = 0;

            // Set global Rhai engine, scope, and EGUI bridge pointers
            ACTIVE_RHAI_ENGINE = Some(&mut self.engine as *mut Engine);
            ACTIVE_RHAI_SCOPE = Some(&mut self.scope as *mut Scope<'static>);
            ACTIVE_RHAI_AST = Some(&self.ast as *const AST);
            ACTIVE_EGUI_BRIDGE = Some(&mut self.egui_bridge as *mut soul_ui::EguiRhaiBridge);

            // Run EGUI frame and capture output
            let egui_output = self.egui_bridge.run(|_ui| {
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
            });

            // Render EGUI output to the canvas
            canvas.render_egui_frame(egui_output);

            // Clear global pointers
            ACTIVE_RHAI_ENGINE = None;
            ACTIVE_RHAI_SCOPE = None;
            ACTIVE_RHAI_AST = None;
            ACTIVE_EGUI_BRIDGE = None;

            // Update content height tracking
            self.last_content_height = ACTIVE_CONTENT_HEIGHT;

            ACTIVE_CANVAS = None;
            ACTIVE_DB = None;
        }
    }
}

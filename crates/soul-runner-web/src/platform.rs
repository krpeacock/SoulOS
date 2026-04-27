//! Web `Platform` implementation: a 240×320 Gray8 framebuffer that
//! blits to an HTML `<canvas>` via `ImageData`, plus mouse → stylus
//! and keyboard input wiring.
//!
//! Mirrors the structure of `soul-hal-hosted` (desktop minifb) and
//! `soul-hal-android`: virtual buffer in 0x00RRGGBB packing, owned
//! input queue, monotonic clock derived from `performance.now()`.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::Gray8,
    prelude::*,
};
use soul_hal::{HardButton, InputEvent, KeyCode, Platform};
use wasm_bindgen::{prelude::*, Clamped, JsCast};
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, HtmlInputElement, ImageData, KeyboardEvent,
    MouseEvent,
};

use soul_core::{SCREEN_HEIGHT, SCREEN_WIDTH};

const VIRT_W: u32 = SCREEN_WIDTH as u32;
const VIRT_H: u32 = SCREEN_HEIGHT as u32;

/// Gray8 `DrawTarget` over a `Vec<u32>` buffer in `0x00RRGGBB`
/// (R == G == B == luma) packing. Same encoding the desktop and
/// Android HALs use; lets the present step share code shape.
pub struct WebDisplay {
    pub width: u32,
    pub height: u32,
    pub buffer: Vec<u32>,
}

impl WebDisplay {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            buffer: vec![0x00FF_FFFFu32; (width * height) as usize],
        }
    }
}

impl OriginDimensions for WebDisplay {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for WebDisplay {
    type Color = Gray8;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(Point { x, y }, color) in pixels {
            if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
                let idx = (y as u32 * self.width + x as u32) as usize;
                let l = color.luma() as u32;
                self.buffer[idx] = (l << 16) | (l << 8) | l;
            }
        }
        Ok(())
    }
}

/// `Platform` impl backed by an HTML canvas.
///
/// Pointer events arrive asynchronously from the browser; they're
/// pushed onto a shared queue by JS-side closures and drained by the
/// runtime each frame via [`Platform::poll_event`].
pub struct WebPlatform {
    pub display: WebDisplay,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    /// Scratch RGBA buffer reused each `flush` to feed `ImageData`.
    rgba: Vec<u8>,
    /// Shared with all event closures so they can enqueue input
    /// without holding any borrow on `WebPlatform`.
    queue: Rc<RefCell<VecDeque<InputEvent>>>,
    /// Closures must be held so the JS GC doesn't collect them.
    _on_down: Closure<dyn FnMut(MouseEvent)>,
    _on_move: Closure<dyn FnMut(MouseEvent)>,
    _on_up: Closure<dyn FnMut(MouseEvent)>,
    _on_keydown: Closure<dyn FnMut(KeyboardEvent)>,
    _on_keyup: Closure<dyn FnMut(KeyboardEvent)>,
    /// Catches characters from the mobile virtual keyboard. The
    /// browser delivers IME-composed characters via `input` events on
    /// a focused text field rather than `keydown`, so we keep a hidden
    /// `<input>` in the DOM and forward its value to the queue.
    _hidden_input: HtmlInputElement,
    _on_input: Closure<dyn FnMut(web_sys::Event)>,
    /// Focuses `_hidden_input` on canvas tap so the OS shows its
    /// soft keyboard.
    _on_canvas_focus: Closure<dyn FnMut(web_sys::Event)>,
    start_ms: f64,
    _pointer_down: Rc<RefCell<bool>>,
}

impl WebPlatform {
    pub fn new(canvas_id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas: HtmlCanvasElement = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("missing canvas element"))?
            .dyn_into()?;
        canvas.set_width(VIRT_W);
        canvas.set_height(VIRT_H);

        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("2d context unavailable"))?
            .dyn_into()?;
        // Crisp upscaling: the canvas backing store is 240×320 and CSS
        // scales it up. Disable the browser's smoothing so the pixel
        // grid stays visible — matches the e-ink target's character.
        ctx.set_image_smoothing_enabled(false);

        let display = WebDisplay::new(VIRT_W, VIRT_H);
        let rgba = vec![0u8; (VIRT_W * VIRT_H * 4) as usize];
        let queue: Rc<RefCell<VecDeque<InputEvent>>> = Rc::new(RefCell::new(VecDeque::new()));
        let pointer_down: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let on_down = mouse_listener(&canvas, &queue, &pointer_down, MouseKind::Down)?;
        let on_move = mouse_listener(&canvas, &queue, &pointer_down, MouseKind::Move)?;
        let on_up = mouse_listener(&canvas, &queue, &pointer_down, MouseKind::Up)?;

        // Hidden off-screen text field: focused on canvas tap so the OS
        // virtual keyboard appears. Its `input` event delivers chars that
        // mobile IMEs bypass `keydown` for.
        let hidden_input: HtmlInputElement = document
            .create_element("input")?
            .dyn_into()?;
        hidden_input.set_attribute("type", "text")?;
        // Positioned off-screen so it never renders, but still focusable.
        hidden_input.set_attribute(
            "style",
            "position:absolute;opacity:0;top:-9999px;left:-9999px;width:1px;height:1px;",
        )?;
        document
            .body()
            .ok_or_else(|| JsValue::from_str("no body"))?
            .append_child(&hidden_input)?;

        let on_keydown = keyboard_listener(&window, &queue, KeyKind::Down)?;
        let on_keyup = keyboard_listener(&window, &queue, KeyKind::Up)?;
        let on_input = input_listener(&hidden_input, &queue)?;
        let on_canvas_focus = focus_listener(&canvas, &hidden_input)?;

        let start_ms = window
            .performance()
            .map(|p| p.now())
            .unwrap_or(0.0);

        Ok(Self {
            display,
            canvas,
            ctx,
            rgba,
            queue,
            _on_down: on_down,
            _on_move: on_move,
            _on_up: on_up,
            _on_keydown: on_keydown,
            _on_keyup: on_keyup,
            _hidden_input: hidden_input,
            _on_input: on_input,
            _on_canvas_focus: on_canvas_focus,
            start_ms,
            _pointer_down: pointer_down,
        })
    }
}

impl Platform for WebPlatform {
    type Display = WebDisplay;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.queue.borrow_mut().pop_front()
    }

    fn now_ms(&self) -> u64 {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        (now - self.start_ms).max(0.0) as u64
    }

    fn flush(&mut self) {
        // Pack 0x00RRGGBB → RRGGBBFF for ImageData (RGBA, A=255).
        for (i, px) in self.display.buffer.iter().enumerate() {
            let r = ((px >> 16) & 0xFF) as u8;
            let g = ((px >> 8) & 0xFF) as u8;
            let b = (px & 0xFF) as u8;
            let base = i * 4;
            self.rgba[base] = r;
            self.rgba[base + 1] = g;
            self.rgba[base + 2] = b;
            self.rgba[base + 3] = 0xFF;
        }
        if let Ok(image) = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&self.rgba[..]),
            VIRT_W,
            VIRT_H,
        ) {
            let _ = self.ctx.put_image_data(&image, 0.0, 0.0);
        }
        let _ = &self.canvas; // keep field used
    }

    fn sleep_ms(&mut self, _ms: u32) {
        // Wasm has no thread to block; the rAF-driven frame loop in
        // lib.rs is the throttle.
    }

    fn speak(&mut self, _text: &str) {
        // Speech synthesis is intentionally unimplemented for the
        // initial web build. Plumb to `SpeechSynthesisUtterance` once
        // a11y on the web becomes a goal.
    }
}

// ── Mouse ────────────────────────────────────────────────────────────────────

#[derive(Copy, Clone)]
enum MouseKind {
    Down,
    Move,
    Up,
}

fn mouse_listener(
    canvas: &HtmlCanvasElement,
    queue: &Rc<RefCell<VecDeque<InputEvent>>>,
    pointer_down: &Rc<RefCell<bool>>,
    kind: MouseKind,
) -> Result<Closure<dyn FnMut(MouseEvent)>, JsValue> {
    let queue = queue.clone();
    let pointer_down = pointer_down.clone();
    let target_canvas = canvas.clone();

    let closure = Closure::wrap(Box::new(move |event: MouseEvent| {
        let rect = target_canvas.get_bounding_client_rect();
        // CSS pixels → canvas-pixel coordinates. The backing store is
        // VIRT_W×VIRT_H regardless of CSS-displayed size, so we scale.
        let scale_x = (target_canvas.width() as f64) / rect.width().max(1.0);
        let scale_y = (target_canvas.height() as f64) / rect.height().max(1.0);
        let cx = ((event.client_x() as f64 - rect.left()) * scale_x).round() as i32;
        let cy = ((event.client_y() as f64 - rect.top()) * scale_y).round() as i32;
        let x = cx.clamp(0, VIRT_W as i32 - 1) as i16;
        let y = cy.clamp(0, VIRT_H as i32 - 1) as i16;

        let mut q = queue.borrow_mut();
        let mut down = pointer_down.borrow_mut();
        match kind {
            MouseKind::Down => {
                *down = true;
                q.push_back(InputEvent::StylusDown { x, y });
            }
            MouseKind::Move if *down => {
                q.push_back(InputEvent::StylusMove { x, y });
            }
            MouseKind::Move => {}
            MouseKind::Up => {
                if *down {
                    q.push_back(InputEvent::StylusUp { x, y });
                    *down = false;
                }
            }
        }
    }) as Box<dyn FnMut(MouseEvent)>);

    let event_name = match kind {
        MouseKind::Down => "mousedown",
        MouseKind::Move => "mousemove",
        MouseKind::Up => "mouseup",
    };
    canvas.add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())?;
    Ok(closure)
}

// ── Keyboard ─────────────────────────────────────────────────────────────────

#[derive(Copy, Clone)]
enum KeyKind {
    Down,
    Up,
}

fn keyboard_listener(
    window: &web_sys::Window,
    queue: &Rc<RefCell<VecDeque<InputEvent>>>,
    kind: KeyKind,
) -> Result<Closure<dyn FnMut(KeyboardEvent)>, JsValue> {
    let queue = queue.clone();
    let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
        let key = event.key();
        match kind {
            KeyKind::Down => {
                if let Some(button) = key_to_hard_button(&key) {
                    // Suppress OS auto-repeat for hard button presses.
                    if !event.repeat() {
                        queue.borrow_mut().push_back(InputEvent::ButtonDown(button));
                    }
                    event.prevent_default();
                } else if let Some(kc) = key_to_keycode(&key) {
                    queue.borrow_mut().push_back(InputEvent::Key(kc));
                    // prevent_default stops:
                    //  - arrow/page keys from scrolling the page
                    //  - tab from shifting focus out of the hidden input
                    //  - printable chars from reaching the hidden input
                    //    (which would fire an `input` event and
                    //     double-deliver the character)
                    event.prevent_default();
                }
                // key == "Process" or "Unidentified" means the mobile
                // IME is composing; don't prevent_default so the char
                // reaches the hidden input's `input` listener.
            }
            KeyKind::Up => {
                if let Some(button) = key_to_hard_button(&key) {
                    queue.borrow_mut().push_back(InputEvent::ButtonUp(button));
                    event.prevent_default();
                }
            }
        }
    }) as Box<dyn FnMut(KeyboardEvent)>);

    let event_name = match kind {
        KeyKind::Down => "keydown",
        KeyKind::Up => "keyup",
    };
    window.add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())?;
    Ok(closure)
}

/// Drain chars from the hidden input element. Fires when the mobile
/// virtual keyboard commits a character through the DOM `input` event
/// rather than via `keydown` (which we intercept for printable chars
/// on physical keyboards via `prevent_default`).
fn input_listener(
    input: &HtmlInputElement,
    queue: &Rc<RefCell<VecDeque<InputEvent>>>,
) -> Result<Closure<dyn FnMut(web_sys::Event)>, JsValue> {
    let queue = queue.clone();
    let input_ref = input.clone();
    let closure = Closure::wrap(Box::new(move |_ev: web_sys::Event| {
        let value = input_ref.value();
        if !value.is_empty() {
            let mut q = queue.borrow_mut();
            for c in value.chars() {
                q.push_back(InputEvent::Key(KeyCode::Char(c)));
            }
            // Clear so the next `input` event only contains the new delta.
            input_ref.set_value("");
        }
    }) as Box<dyn FnMut(web_sys::Event)>);
    input.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref())?;
    Ok(closure)
}

/// Register mousedown + touchstart on the canvas to keep the hidden
/// input focused, which is what tells the OS to display its virtual
/// keyboard on mobile devices.
fn focus_listener(
    canvas: &HtmlCanvasElement,
    hidden_input: &HtmlInputElement,
) -> Result<Closure<dyn FnMut(web_sys::Event)>, JsValue> {
    let input_ref = hidden_input.clone();
    let closure = Closure::wrap(Box::new(move |_ev: web_sys::Event| {
        let _ = input_ref.focus();
    }) as Box<dyn FnMut(web_sys::Event)>);
    let fn_ref: &js_sys::Function = closure.as_ref().unchecked_ref();
    canvas.add_event_listener_with_callback("mousedown", fn_ref)?;
    canvas.add_event_listener_with_callback("touchstart", fn_ref)?;
    Ok(closure)
}

// ── Key translation ───────────────────────────────────────────────────────────

fn key_to_hard_button(key: &str) -> Option<HardButton> {
    Some(match key {
        "Escape" => HardButton::Power,
        "F1" => HardButton::AppA,
        "F2" => HardButton::AppB,
        "F3" => HardButton::AppC,
        "F4" => HardButton::AppD,
        // F5 and the named Home key both map to the Home hard button,
        // matching the desktop HAL's mapping.
        "F5" | "Home" => HardButton::Home,
        "F6" => HardButton::Menu,
        "PageUp" => HardButton::PageUp,
        "PageDown" => HardButton::PageDown,
        _ => return None,
    })
}

fn key_to_keycode(key: &str) -> Option<KeyCode> {
    Some(match key {
        "Backspace" => KeyCode::Backspace,
        "Enter" => KeyCode::Enter,
        "Tab" => KeyCode::Tab,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        // The browser already resolves shift/caps/alt modifiers into the
        // correct Unicode scalar, so a single-char key string maps directly.
        k if k.chars().count() == 1 => KeyCode::Char(k.chars().next().unwrap()),
        _ => return None,
    })
}

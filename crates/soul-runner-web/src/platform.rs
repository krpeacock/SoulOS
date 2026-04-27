//! Web `Platform` implementation: a 240×320 Gray8 framebuffer that
//! blits to an HTML `<canvas>` via `ImageData`, plus mouse → stylus
//! input wiring.
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
use soul_hal::{InputEvent, Platform};
use wasm_bindgen::{prelude::*, Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, MouseEvent};

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
    /// Shared with mouse-event closures so they can enqueue input
    /// without holding any borrow on `WebPlatform`.
    queue: Rc<RefCell<VecDeque<InputEvent>>>,
    /// Closures live as long as the platform; `forget` would leak,
    /// so we hold them so `remove_event_listener_*` could be wired
    /// later if we ever tear the platform down.
    _on_down: Closure<dyn FnMut(MouseEvent)>,
    _on_move: Closure<dyn FnMut(MouseEvent)>,
    _on_up: Closure<dyn FnMut(MouseEvent)>,
    start_ms: f64,
    /// Tracks whether the primary pointer is currently down. Read by
    /// the mouse-event closures (move events fire constantly and we
    /// only forward them while the button is held, matching StylusMove
    /// semantics); the field exists on the struct to keep the Rc alive
    /// for the closures' lifetime.
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

        let on_down = mouse_listener(&canvas, &queue, &pointer_down, EventKind::Down)?;
        let on_move = mouse_listener(&canvas, &queue, &pointer_down, EventKind::Move)?;
        let on_up = mouse_listener(&canvas, &queue, &pointer_down, EventKind::Up)?;

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

#[derive(Copy, Clone)]
enum EventKind {
    Down,
    Move,
    Up,
}

fn mouse_listener(
    canvas: &HtmlCanvasElement,
    queue: &Rc<RefCell<VecDeque<InputEvent>>>,
    pointer_down: &Rc<RefCell<bool>>,
    kind: EventKind,
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
            EventKind::Down => {
                *down = true;
                q.push_back(InputEvent::StylusDown { x, y });
            }
            EventKind::Move if *down => {
                q.push_back(InputEvent::StylusMove { x, y });
            }
            EventKind::Move => {}
            EventKind::Up => {
                if *down {
                    q.push_back(InputEvent::StylusUp { x, y });
                    *down = false;
                }
            }
        }
    }) as Box<dyn FnMut(MouseEvent)>);

    let event_name = match kind {
        EventKind::Down => "mousedown",
        EventKind::Move => "mousemove",
        EventKind::Up => "mouseup",
    };
    canvas.add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())?;
    Ok(closure)
}

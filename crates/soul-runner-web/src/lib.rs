//! Web wasm runner for SoulOS.
//!
//! Wires a wasm-side `Platform` (240×320 framebuffer that blits to an
//! HTML canvas via `ImageData`) to the same `soul_core` event loop the
//! desktop and Android runners use. Because wasm has no thread to
//! block, we can't call `soul_core::run` directly — its main loop ends
//! in `sleep_ms`. Instead, the `frame` function below performs one
//! iteration of that loop and re-arms itself via `requestAnimationFrame`,
//! which is also what gates our refresh rate.

extern crate alloc;

mod platform;

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;

use embedded_graphics::{
    draw_target::DrawTargetExt,
    pixelcolor::Gray8,
    prelude::*,
    primitives::PrimitiveStyle,
};
use soul_core::{a11y::A11yManager, App, Ctx, Dirty, Event};
use soul_hal::{InputEvent, Platform};
use soul_runner::Host;
use wasm_bindgen::{prelude::*, JsCast};

use platform::WebPlatform;

const CANVAS_ID: &str = "soulos-canvas";

/// `requestAnimationFrame` requires a `&Function`; this thin wrapper
/// hides the `web_sys` ceremony at every call site.
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .expect("no window")
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("rAF");
}

struct RunState {
    platform: WebPlatform,
    app: Host,
    dirty: Dirty,
    a11y: A11yManager,
    quit: bool,
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let platform = WebPlatform::new(CANVAS_ID)?;
    let mut app = Host::new();
    let mut dirty = Dirty::full();
    let mut a11y = A11yManager::new();

    // Deliver AppStart before the first frame, mirroring `soul_core::run`.
    {
        let now = platform.now_ms();
        let mut ctx = Ctx {
            now_ms: now,
            dirty: &mut dirty,
            a11y: &mut a11y,
        };
        app.handle(Event::AppStart, &mut ctx);
    }

    let state = Rc::new(RefCell::new(RunState {
        platform,
        app,
        dirty,
        a11y,
        quit: false,
    }));

    // The standard wasm-bindgen rAF self-rescheduling pattern: the
    // outer Rc<RefCell<Option<Closure>>> holds the closure; the closure
    // re-arms itself via the same Rc. This avoids leaking a fresh
    // closure on every frame.
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let state_for_frame = state.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let mut s = state_for_frame.borrow_mut();
        if s.quit {
            return;
        }
        run_frame(&mut s);
        // Re-schedule (skip if quit was set this frame).
        if !s.quit {
            if let Some(cb) = f.borrow().as_ref() {
                request_animation_frame(cb);
            }
        }
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
    Ok(())
}

/// One iteration of `soul_core::run`'s main loop. The shape mirrors
/// the canonical loop exactly so the wasm runner stays a thin glue
/// layer rather than a divergent reimplementation.
fn run_frame(s: &mut RunState) {
    // Drain pending input.
    while let Some(ev) = s.platform.poll_event() {
        if matches!(ev, InputEvent::Quit) {
            let now = s.platform.now_ms();
            let mut ctx = Ctx {
                now_ms: now,
                dirty: &mut s.dirty,
                a11y: &mut s.a11y,
            };
            s.app.handle(Event::AppStop, &mut ctx);
            s.quit = true;
            return;
        }
        if let Some(translated) = translate(ev) {
            let now = s.platform.now_ms();
            let mut ctx = Ctx {
                now_ms: now,
                dirty: &mut s.dirty,
                a11y: &mut s.a11y,
            };
            s.app.handle(translated, &mut ctx);
        }
    }

    // Tick.
    {
        let now = s.platform.now_ms();
        let mut ctx = Ctx {
            now_ms: now,
            dirty: &mut s.dirty,
            a11y: &mut s.a11y,
        };
        s.app.handle(Event::Tick(now), &mut ctx);
    }

    // Repaint dirty region (clip + clear-to-white + app draw).
    if let Some(rect) = s.dirty.take() {
        let mut clip = s.platform.display().clipped(&rect);
        let _ = rect
            .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
            .draw(&mut clip);
        s.app.draw(&mut clip, rect);
    }

    // Drain accessibility speech, then present.
    let rate = s.a11y.rate_wpm;
    let punctuation = s.a11y.punctuation;
    for text in s.a11y.pending_speech.drain(..) {
        s.platform.speak(soul_hal::SpeechRequest {
            text: &text,
            rate_wpm: rate,
            interrupt: true,
            punctuation,
        });
    }
    s.platform.flush();
}

fn translate(input: InputEvent) -> Option<Event> {
    match input {
        InputEvent::StylusDown { x, y } => Some(Event::PenDown { x, y }),
        InputEvent::StylusMove { x, y } => Some(Event::PenMove { x, y }),
        InputEvent::StylusUp { x, y } => Some(Event::PenUp { x, y }),
        InputEvent::Wheel { dx, dy } => Some(Event::Wheel { dx, dy }),
        InputEvent::ButtonDown(b) => Some(Event::ButtonDown(b)),
        InputEvent::ButtonUp(b) => Some(Event::ButtonUp(b)),
        InputEvent::Key(k) => Some(Event::Key(k)),
        InputEvent::Quit => None,
    }
}

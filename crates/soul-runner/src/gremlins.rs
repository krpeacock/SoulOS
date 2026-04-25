//! Gremlins — deterministic random-input stress tester, modelled on the
//! PalmOS Gremlins tool.
//!
//! A Gremlin session fires a stream of random pen strokes, hard-button
//! taps, and key presses at the running app, rendering every frame to a
//! live window so you can watch chaos unfold.
//!
//! The stream is fully determined by a 64-bit seed, so any crash can be
//! reproduced exactly:
//!
//! ```text
//! # First run — crash reported after N gremlins with seed S
//! soul-runner --gremlins S
//!
//! # Reproduce — identical event stream, same crash at same N
//! soul-runner --gremlins S 20000
//! ```
//!
//! ## Controls (real keyboard, not injected into the app)
//!
//! - **P** — pause / unpause (app stays live, display keeps refreshing)
//! - **Q** — quit cleanly, printing final count
//! - **Window close** — same as Q
//!
//! ## Text input
//!
//! Random keystrokes use an English letter-frequency distribution
//! (ETAOIN SHRDLU order) so the noise looks vaguely prose-like.
//! Occasionally a full Shakespeare quote is injected character-by-character,
//! producing the recognisable bursts of real text that made PalmOS gremlins
//! famous.

use embedded_graphics::{
    draw_target::DrawTargetExt,
    pixelcolor::Gray8,
    prelude::*,
    primitives::PrimitiveStyle,
};
use soul_core::{
    a11y::A11yManager, App, Ctx, Dirty, Event, KeyCode,
    APP_HEIGHT, SCREEN_HEIGHT, SCREEN_WIDTH,
};
use soul_hal::{HardButton, InputEvent, Platform};
use std::panic::{self, AssertUnwindSafe};

// ---------------------------------------------------------------------------
// PRNG — xorshift64
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0xdeadbeef_cafebabe } else { seed })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn range(&mut self, lo: i16, hi: i16) -> i16 {
        let span = (hi as i32 - lo as i32 + 1) as u64;
        lo + (self.next() % span) as i16
    }
}

// ---------------------------------------------------------------------------
// Text content
// ---------------------------------------------------------------------------

/// Cumulative frequency table (ETAOIN SHRDLU order). Total weight = 1000.
/// Space is the most common entry — it keeps the noise word-boundary-shaped.
const CHAR_FREQ: &[(u64, char)] = &[
    (130, ' '),
    (243, 'e'),
    (334, 't'),
    (412, 'a'),
    (484, 'o'),
    (549, 'i'),
    (610, 'n'),
    (666, 's'),
    (718, 'h'),
    (766, 'r'),
    (810, 'd'),
    (847, 'l'),
    (880, 'u'),
    (908, 'c'),
    (933, 'm'),
    (955, 'w'),
    (973, 'f'),
    (986, 'g'),
    (993, 'y'),
    (997, 'p'),
    (999, 'b'),
    (1000, 'v'),
];

/// Shakespeare quotes injected wholesale as character bursts.
const QUOTES: &[&str] = &[
    "To be, or not to be, that is the question.",
    "All the world's a stage.",
    "The lady doth protest too much, methinks.",
    "What's in a name? That which we call a rose by any other name would smell as sweet.",
    "We know what we are, but know not what we may be.",
    "The course of true love never did run smooth.",
    "Brevity is the soul of wit.",
    "All that glitters is not gold.",
    "This above all: to thine own self be true.",
    "Cowards die many times before their deaths; the valiant never taste of death but once.",
    "Good night, good night! Parting is such sweet sorrow.",
    "Some are born great, some achieve greatness, and some have greatness thrust upon them.",
    "What a piece of work is a man!",
    "The robbed that smiles, steals something from the thief.",
    "Hell is empty and all the devils are here.",
];

fn weighted_char(rng: &mut Rng) -> char {
    let w = rng.below(1000);
    for &(cum, ch) in CHAR_FREQ {
        if w < cum {
            return ch;
        }
    }
    'e'
}

fn key_events_for_text(rng: &mut Rng) -> Vec<Event> {
    match rng.below(20) {
        // 75%: single frequency-weighted character
        0..=14 => {
            let ch = weighted_char(rng);
            let ch = if ch != ' ' && rng.below(8) == 0 {
                ch.to_ascii_uppercase()
            } else {
                ch
            };
            vec![Event::Key(KeyCode::Char(ch))]
        }
        // 5%: full Shakespeare quote, character by character
        15 => {
            let idx = rng.below(QUOTES.len() as u64) as usize;
            QUOTES[idx]
                .chars()
                .map(|c| Event::Key(KeyCode::Char(c)))
                .collect()
        }
        // 10%: editing keys (Tab included as a11y escape valve)
        16..=17 => match rng.below(12) {
            0 => vec![Event::Key(KeyCode::Backspace)],
            1 => vec![Event::Key(KeyCode::Enter)],
            2 => vec![Event::Key(KeyCode::ArrowLeft)],
            3 => vec![Event::Key(KeyCode::ArrowRight)],
            4 => vec![Event::Key(KeyCode::ArrowUp)],
            5 => vec![Event::Key(KeyCode::ArrowDown)],
            _ => vec![Event::Key(KeyCode::Tab)],
        },
        // 10%: punctuation and digits
        _ => {
            let ch = match rng.below(12) {
                0 => '.',
                1 => ',',
                2 => '!',
                3 => '?',
                4 => '\'',
                5 => '-',
                _ => (b'0' + rng.below(10) as u8) as char,
            };
            vec![Event::Key(KeyCode::Char(ch))]
        }
    }
}

// ---------------------------------------------------------------------------
// Gremlin action generation
// ---------------------------------------------------------------------------

const SCREEN_W: i16 = SCREEN_WIDTH as i16;
const SCREEN_H: i16 = SCREEN_HEIGHT as i16;
const APP_H: i16 = APP_HEIGHT as i16;

fn next_gremlin(rng: &mut Rng) -> Vec<Event> {
    match rng.below(20) {
        0..=11 => pen_stroke(rng),      // 60%: pen gesture on the canvas
        12..=13 => strip_tap(rng),      // 10%: tap the system strip
        14..=16 => button_tap(rng),     // 15%: hard button
        _ => key_events_for_text(rng),  // 15%: text / quotes
    }
}

fn pen_stroke(rng: &mut Rng) -> Vec<Event> {
    let x0 = rng.range(0, SCREEN_W - 1);
    let y0 = rng.range(0, APP_H - 1);

    // 50% tap, 30% short drag, 20% long drag — so buttons actually get hit.
    let (moves, jitter): (i16, i16) = match rng.below(10) {
        0..=4 => (0, 4),
        5..=7 => (rng.range(1, 6), 8),
        _     => (rng.range(7, 15), 16),
    };

    let mut events = vec![Event::PenDown { x: x0, y: y0 }];
    let mut x = x0;
    let mut y = y0;
    for _ in 0..moves {
        x = (x + rng.range(-jitter, jitter)).clamp(0, SCREEN_W - 1);
        y = (y + rng.range(-jitter, jitter)).clamp(0, APP_H - 1);
        events.push(Event::PenMove { x, y });
    }
    if moves == 0 {
        // Keep PenUp within the 10 px tap threshold.
        x = (x + rng.range(-3, 3)).clamp(0, SCREEN_W - 1);
        y = (y + rng.range(-3, 3)).clamp(0, APP_H - 1);
    }
    events.push(Event::PenUp { x, y });
    events
}

fn strip_tap(rng: &mut Rng) -> Vec<Event> {
    let x = rng.range(0, SCREEN_W - 1);
    let y = rng.range(APP_H, SCREEN_H - 1);
    vec![Event::PenDown { x, y }, Event::PenUp { x, y }]
}

fn button_tap(rng: &mut Rng) -> Vec<Event> {
    // AppA–D weighted 3× over scroll/menu. Home excluded — it just returns to
    // Launcher and stalls the session without exercising any app logic.
    let btn = match rng.below(11) {
        0..=2 => HardButton::AppA,
        3..=5 => HardButton::AppB,
        6..=7 => HardButton::AppC,
        8     => HardButton::AppD,
        9     => HardButton::PageUp,
        _     => HardButton::Menu,
    };
    vec![Event::ButtonDown(btn), Event::ButtonUp(btn)]
}

// ---------------------------------------------------------------------------
// Run loop
// ---------------------------------------------------------------------------

/// Translate a real `InputEvent` into a `soul_core::Event` for pass-through
/// while gremlins are stopped.  Mirrors `soul_core::translate` (private there).
fn translate_input(ev: InputEvent) -> Option<Event> {
    match ev {
        InputEvent::StylusDown { x, y } => Some(Event::PenDown { x, y }),
        InputEvent::StylusMove { x, y } => Some(Event::PenMove { x, y }),
        InputEvent::StylusUp   { x, y } => Some(Event::PenUp   { x, y }),
        InputEvent::ButtonDown(HardButton::Menu) => Some(Event::Menu),
        InputEvent::ButtonUp  (HardButton::Menu) => None,
        InputEvent::ButtonDown(b) => Some(Event::ButtonDown(b)),
        InputEvent::ButtonUp  (b) => Some(Event::ButtonUp(b)),
        InputEvent::Key(k)        => Some(Event::Key(k)),
        InputEvent::Quit          => None,
    }
}

/// Run gremlins against `app` on `platform`, rendering every action live.
///
/// Real keyboard controls (intercepted before any synthetic input is injected):
/// - **P** — stop / resume  
///   When stopped, gremlins step aside completely: real mouse and keyboard
///   input flows through to the app normally so you can tap around and
///   examine state.  Press P again to resume synthetic injection.
/// - **Q** or window-close — quit the session
///
/// `seed`  — PRNG seed; same seed → same event stream → same crash.  
/// `limit` — stop after this many gremlins (0 = run until crash or quit).
pub fn run_gremlins<A: App, P: Platform>(mut app: A, mut platform: P, seed: u64, limit: u64) {
    println!(
        "🐛 Gremlins starting — seed={seed} limit={} (P=stop/resume  Q=quit)",
        if limit == 0 { "∞".to_string() } else { limit.to_string() }
    );

    let mut rng = Rng::new(seed);
    let mut dirty = Dirty::full();
    let mut a11y = A11yManager::new();
    let mut count: u64 = 0;
    let mut stopped = false;

    {
        let mut ctx = Ctx { now_ms: 0, dirty: &mut dirty, a11y: &mut a11y };
        app.handle(Event::AppStart, &mut ctx);
    }

    render(&mut app, &mut platform, &mut dirty);

    loop {
        // Drain real platform events.  In stopped mode these are passed
        // through to the app so the user can interact normally.
        // In running mode P and Q are intercepted; everything else is discarded
        // (the app only sees synthetic input while gremlins are running).
        while let Some(ev) = platform.poll_event() {
            match ev {
                InputEvent::Quit => {
                    println!("🐛 Gremlins stopped by user after {count} actions. Seed={seed}");
                    return;
                }
                InputEvent::Key(KeyCode::Char('p')) | InputEvent::Key(KeyCode::Char('P')) => {
                    stopped = !stopped;
                    if stopped {
                        println!("⏹  Stopped after {count} gremlins — you have control. Press P to resume, Q to quit.");
                    } else {
                        println!("▶  Resuming from gremlin {count}.");
                    }
                }
                InputEvent::Key(KeyCode::Char('q')) | InputEvent::Key(KeyCode::Char('Q')) => {
                    println!("🐛 Gremlins quit after {count} actions. Seed={seed}");
                    return;
                }
                real_ev if stopped => {
                    // Pass real input through to the app while stopped.
                    if let Some(core_ev) = translate_input(real_ev) {
                        let now_ms = count * 16;
                        let mut ctx = Ctx { now_ms, dirty: &mut dirty, a11y: &mut a11y };
                        app.handle(core_ev, &mut ctx);
                    }
                }
                _ => {} // discard real input while gremlins are running
            }
        }

        if stopped {
            render(&mut app, &mut platform, &mut dirty);
            continue;
        }

        count += 1;
        if limit > 0 && count > limit {
            println!("🐛 Gremlins finished {count} actions without a crash. Seed={seed}");
            return;
        }

        let events = next_gremlin(&mut rng);
        let now_ms = count * 16;

        for event in events {
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                let mut ctx = Ctx { now_ms, dirty: &mut dirty, a11y: &mut a11y };
                app.handle(event, &mut ctx);
            }));

            if let Err(e) = result {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "(unknown panic payload)".to_string()
                };
                eprintln!("\n💥 CRASH after {count} gremlins!");
                eprintln!("   seed  = {seed}");
                eprintln!("   panic = {msg}");
                eprintln!();
                eprintln!("To reproduce:");
                eprintln!("   soul-runner --gremlins {seed} {count}");
                render(&mut app, &mut platform, &mut dirty);
                std::process::exit(1);
            }
        }

        render(&mut app, &mut platform, &mut dirty);

        if count % 1_000 == 0 {
            println!("🐛 {count} gremlins — still running (seed={seed})");
        }
    }
}

fn render<A: App, P: Platform>(app: &mut A, platform: &mut P, dirty: &mut Dirty) {
    if let Some(rect) = dirty.take() {
        let display = platform.display();
        let mut clip = display.clipped(&rect);
        let _ = rect
            .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
            .draw(&mut clip);
        app.draw(&mut clip, rect);
    }
    platform.flush();
}

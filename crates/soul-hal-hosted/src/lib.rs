//! Hosted desktop HAL backed by minifb (pure-Rust, no SDL2 required).
//! Provides a window, a Gray8 framebuffer, and keyboard/mouse input.

use std::collections::VecDeque;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::Gray8,
    prelude::*,
};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Scale, Window, WindowOptions};
// KeyRepeat::No is used in pump() to guarantee single-fire per keypress.
use soul_hal::{HardButton, InputEvent, KeyCode, Platform, SpeechRequest};

pub mod harness;

// Re-export key types from harness for convenience
pub use harness::{SettleTimeout, Harness, HeadlessPlatform, VirtualClock};

// ── Framebuffer ──────────────────────────────────────────────────────────────

/// Each logical pixel is rendered as a PIXEL_SCALE×PIXEL_SCALE block so
/// the physical window has 4× the pixel density of the 240×320 virtual canvas.
/// Exported so callers that need physical dimensions can compute them.
pub const PIXEL_SCALE: u32 = 4;

/// A `DrawTarget<Color = Gray8>` backed by a `Vec<u32>` pixel buffer.
///
/// `width` / `height` are the *logical* dimensions (240×320) reported by
/// `size()` — all embedded-graphics drawing and app layout happens in this
/// space.  `buffer` is *physical*: each logical pixel occupies a
/// `PIXEL_SCALE×PIXEL_SCALE` block, so `buffer.len() == width * PIXEL_SCALE *
/// height * PIXEL_SCALE`.  `draw_iter` writes every block atomically, which
/// means callers that know about physical pixels can also write individual
/// `buffer` entries for sub-logical-pixel rendering.
#[derive(Clone)]
pub struct MiniFbDisplay {
    pub width: u32,
    pub height: u32,
    /// Physical pixel buffer: row-major, stride = `width * PIXEL_SCALE`.
    /// Format: `0x00RRGGBB` where R == G == B == luma.
    pub buffer: Vec<u32>,
}

impl MiniFbDisplay {
    pub fn new(width: u32, height: u32) -> Self {
        let phys = (width * PIXEL_SCALE * height * PIXEL_SCALE) as usize;
        Self {
            width,
            height,
            buffer: vec![0x00FF_FFFFu32; phys],
        }
    }

    /// Physical width in pixels (= `width * PIXEL_SCALE`).
    #[inline]
    pub fn phys_width(&self) -> u32 {
        self.width * PIXEL_SCALE
    }

    /// Physical height in pixels (= `height * PIXEL_SCALE`).
    #[inline]
    pub fn phys_height(&self) -> u32 {
        self.height * PIXEL_SCALE
    }

    /// Render `text` with true sub-pixel anti-aliasing by writing individual
    /// physical pixels directly into `self.buffer`.
    ///
    /// `(x, y)` are **logical** coordinates (top of cap-height).  Internally
    /// the glyph is rasterized at `size_px * PIXEL_SCALE` physical pixels so
    /// every fontdue coverage value lands in exactly one entry of the physical
    /// buffer — no 4×4 block expansion.  The result is visibly sharper than
    /// the gray-AA path in `soul_ui::font_aa` which still operates at logical
    /// resolution.
    ///
    /// `luma = 0` → black text; `luma = 255` → white text.
    pub fn draw_text_aa_phys(&mut self, x: i32, y: i32, text: &str, size_px: f32, luma: u8) {
        let font = phys_font();
        let phys_size = size_px * PIXEL_SCALE as f32;
        let cap_h = font.rasterize('H', phys_size).0.height as i32;
        let baseline_y = y * PIXEL_SCALE as i32 + cap_h;
        let stride = self.phys_width() as i32;
        let phys_w = self.phys_width() as i32;
        let phys_h = self.phys_height() as i32;

        let mut cursor_x = (x * PIXEL_SCALE as i32) as f32;
        for c in text.chars() {
            let (metrics, bitmap) = font.rasterize(c, phys_size);
            let glyph_left = cursor_x as i32 + metrics.xmin;
            let glyph_top  = baseline_y - (metrics.height as i32 + metrics.ymin);

            for row in 0..metrics.height as i32 {
                for col in 0..metrics.width as i32 {
                    let coverage = bitmap[(row * metrics.width as i32 + col) as usize];
                    if coverage == 0 {
                        continue;
                    }
                    let px = glyph_left + col;
                    let py = glyph_top  + row;
                    if px < 0 || py < 0 || px >= phys_w || py >= phys_h {
                        continue;
                    }
                    let a  = coverage as u32;
                    let fg = luma as u32;
                    let blended = ((fg * a + 255 * (255 - a)) / 255) as u8;
                    let v = blended as u32;
                    self.buffer[(py * stride + px) as usize] = (v << 16) | (v << 8) | v;
                }
            }
            cursor_x += metrics.advance_width;
        }
    }
}

static PHYS_FONT: OnceLock<fontdue::Font> = OnceLock::new();

fn phys_font() -> &'static fontdue::Font {
    PHYS_FONT.get_or_init(|| {
        // Re-use the same Liberation Sans bytes that soul-ui bundles.
        // We embed them here too so soul-hal-hosted has no dependency on soul-ui.
        static FONT_DATA: &[u8] =
            include_bytes!("../../soul-ui/assets/fonts/LiberationSans-Regular.ttf");
        fontdue::Font::from_bytes(FONT_DATA, fontdue::FontSettings::default())
            .expect("bundled font is valid")
    })
}

impl OriginDimensions for MiniFbDisplay {
    fn size(&self) -> Size {
        Size::new(self.width, self.height) // logical — apps draw here
    }
}

impl DrawTarget for MiniFbDisplay {
    type Color = Gray8;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let phys_stride = self.phys_width();
        for Pixel(Point { x, y }, color) in pixels {
            if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
                let px = x as u32 * PIXEL_SCALE;
                let py = y as u32 * PIXEL_SCALE;
                let l = color.luma() as u32;
                let pv = (l << 16) | (l << 8) | l;
                for dy in 0..PIXEL_SCALE {
                    for dx in 0..PIXEL_SCALE {
                        self.buffer[((py + dy) * phys_stride + (px + dx)) as usize] = pv;
                    }
                }
            }
        }
        Ok(())
    }
}

// ── Platform ─────────────────────────────────────────────────────────────────

pub struct HostedPlatform {
    pub display: MiniFbDisplay,
    window: Window,
    start: Instant,
    pub pending: VecDeque<InputEvent>,
    /// Tracks whether left mouse button was down on the previous pump.
    prev_mouse_down: bool,
    /// Previous logical mouse position (buffer coords, not window coords).
    prev_mouse_pos: Option<(f32, f32)>,
    /// Keys held on the previous pump — used to generate ButtonDown/Up events.
    prev_keys: Vec<Key>,
    /// Currently speaking TTS subprocess. Killed on a new request with
    /// `interrupt: true` so navigation never falls behind.
    tts_child: Option<std::process::Child>,
    /// Set true on the first call to a missing TTS engine (e.g. `espeak-ng`
    /// not on PATH) so we log a single warning instead of one per utterance.
    tts_warned: bool,
}

impl HostedPlatform {
    pub fn new(title: &str, width: u32, height: u32) -> Self {
        let display = MiniFbDisplay::new(width, height);
        let phys_w = display.phys_width() as usize;
        let phys_h = display.phys_height() as usize;
        let window = Window::new(
            title,
            phys_w,
            phys_h,
            WindowOptions {
                scale: Scale::X1,
                resize: true,
                ..Default::default()
            },
        )
        .expect("Failed to create minifb window");

        Self {
            display,
            window,
            start: Instant::now(),
            pending: VecDeque::new(),
            prev_mouse_down: false,
            prev_mouse_pos: None,
            prev_keys: Vec::new(),
            tts_child: None,
            tts_warned: false,
        }
    }

    /// Collect mouse and keyboard events into `self.pending`.
    /// Called after `window.update_with_buffer()` so minifb's internal state
    /// (key-press lists, mouse position) reflects the latest frame.
    fn pump(&mut self) {
        // ── Window close ─────────────────────────────────────────────────────
        if !self.window.is_open() {
            self.pending.push_back(InputEvent::Quit);
            return;
        }

        // ── Mouse ─────────────────────────────────────────────────────────────
        // Window is PIXEL_SCALE× the logical buffer; convert to logical coords.
        let mouse_down = self.window.get_mouse_down(MouseButton::Left);
        let mouse_pos  = self.window.get_mouse_pos(MouseMode::Discard)
            .map(|(mx, my)| (mx / PIXEL_SCALE as f32, my / PIXEL_SCALE as f32));

        match (mouse_pos, mouse_down, self.prev_mouse_down) {
            (Some((mx, my)), true, false) => {
                self.pending.push_back(InputEvent::StylusDown { x: mx as i16, y: my as i16 });
            }
            (Some((mx, my)), false, true) => {
                self.pending.push_back(InputEvent::StylusUp { x: mx as i16, y: my as i16 });
            }
            (Some((mx, my)), true, true) if self.prev_mouse_pos != Some((mx, my)) => {
                self.pending.push_back(InputEvent::StylusMove { x: mx as i16, y: my as i16 });
            }
            (None, false, true) => {
                // Cursor left the window while held — synthesise a release.
                if let Some((px, py)) = self.prev_mouse_pos {
                    self.pending.push_back(InputEvent::StylusUp { x: px as i16, y: py as i16 });
                }
            }
            _ => {}
        }
        self.prev_mouse_down = mouse_down;
        self.prev_mouse_pos  = mouse_pos;

        // ── Scroll wheel / two-finger swipe ───────────────────────────────────
        // minifb returns positive y when the wheel rolls forward (toward the
        // screen) — i.e., the user wants to scroll *up*. We invert so positive
        // dy in our event means "scroll content down". Multiply by a line-
        // height-ish factor so a single wheel notch moves a useful distance.
        if let Some((sx, sy)) = self.window.get_scroll_wheel() {
            let dx = (sx * 16.0) as i16;
            let dy = (-sy * 16.0) as i16;
            if dx != 0 || dy != 0 {
                self.pending.push_back(InputEvent::Wheel { dx, dy });
            }
        }

        // ── Keyboard ──────────────────────────────────────────────────────────
        // minifb provides two views after each update_with_buffer():
        //   get_keys_pressed(No)  → keys whose *first* press occurred this frame
        //   get_keys_pressed(Yes) → keys that have an event this frame (first press
        //                           OR OS key-repeat fire)
        //
        // Split: initial presses from No, repeat-only from Yes − No.
        // This guarantees the initial press fires exactly once even when Yes also
        // includes it (which it does on the same frame).
        let current_keys  = self.window.get_keys();
        let pressed_new   = self.window.get_keys_pressed(KeyRepeat::No);
        let pressed_all   = self.window.get_keys_pressed(KeyRepeat::Yes);

        // Initial key-down (fires exactly once per physical press).
        for key in &pressed_new {
            if let Some(b) = map_hard_button(*key) {
                self.pending.push_back(InputEvent::ButtonDown(b));
            } else if let Some(kc) = map_keycode(*key, &current_keys) {
                self.pending.push_back(InputEvent::Key(kc));
            }
        }

        // Key repeat: in Yes but not in No (pure repeats, no initial press).
        for key in &pressed_all {
            if !pressed_new.contains(key) && map_hard_button(*key).is_none() {
                if let Some(kc) = map_keycode(*key, &current_keys) {
                    self.pending.push_back(InputEvent::Key(kc));
                }
            }
        }

        // Key-up: only needed for hardware buttons (apps care about ButtonUp).
        for &key in &self.prev_keys {
            if !current_keys.contains(&key) {
                if let Some(b) = map_hard_button(key) {
                    self.pending.push_back(InputEvent::ButtonUp(b));
                }
            }
        }

        self.prev_keys = current_keys;
    }
}

impl Platform for HostedPlatform {
    type Display = MiniFbDisplay;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.pending.pop_front()
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn flush(&mut self) {
        let pw = self.display.phys_width() as usize;
        let ph = self.display.phys_height() as usize;
        let _ = self.window.update_with_buffer(&self.display.buffer, pw, ph);
        self.pump();
    }

    fn sleep_ms(&mut self, ms: u32) {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }

    fn speak(&mut self, req: SpeechRequest<'_>) {
        // Reap any zombie from the previous utterance, regardless of
        // whether we'll interrupt it.
        if let Some(mut prev) = self.tts_child.take() {
            let still_running = matches!(prev.try_wait(), Ok(None));
            if still_running && req.interrupt {
                let _ = prev.kill();
                let _ = prev.wait();
            } else if still_running {
                // Caller didn't ask to interrupt; put it back.
                self.tts_child = Some(prev);
                return;
            }
        }

        let mut command = build_tts_command(req.rate_wpm, req.text);
        match command.as_mut().map(|c| c.spawn()) {
            Some(Ok(child)) => self.tts_child = Some(child),
            Some(Err(e)) => {
                if !self.tts_warned {
                    self.tts_warned = true;
                    eprintln!(
                        "[TTS] failed to launch engine: {e}. Speech output disabled. \
                         Install `espeak-ng` (Linux) or run on macOS for native TTS."
                    );
                }
                println!("[TTS]: {}", req.text);
            }
            None => {
                if !self.tts_warned {
                    self.tts_warned = true;
                    eprintln!(
                        "[TTS] no TTS engine configured for this platform. Speech \
                         output disabled. Install `espeak-ng` and add it to PATH."
                    );
                }
                println!("[TTS]: {}", req.text);
            }
        }
    }
}

/// Build the OS-specific TTS subprocess command, or `None` when the
/// platform has no supported engine.
fn build_tts_command(rate_wpm: u16, text: &str) -> Option<std::process::Command> {
    let rate = rate_wpm.clamp(80, 400);
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("say");
        cmd.arg("-r").arg(rate.to_string()).arg(text);
        return Some(cmd);
    }
    #[cfg(target_os = "linux")]
    {
        let mut cmd = std::process::Command::new("espeak-ng");
        cmd.arg("-s").arg(rate.to_string()).arg("--").arg(text);
        return Some(cmd);
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (rate, text);
        None
    }
}

// ── Key mapping ───────────────────────────────────────────────────────────────

/// Map F-keys and special keys to hardware buttons.
fn map_hard_button(k: Key) -> Option<HardButton> {
    Some(match k {
        Key::Escape => HardButton::Power,
        Key::F1 => HardButton::AppA,
        Key::F2 => HardButton::AppB,
        Key::F3 => HardButton::AppC,
        Key::F4 => HardButton::AppD,
        Key::F5 | Key::Home => HardButton::Home,
        Key::F6 => HardButton::Menu,
        Key::PageUp => HardButton::PageUp,
        Key::PageDown => HardButton::PageDown,
        _ => return None,
    })
}

fn map_keycode(k: Key, held: &[Key]) -> Option<KeyCode> {
    let shift = held.contains(&Key::LeftShift) || held.contains(&Key::RightShift);
    let caps  = held.contains(&Key::CapsLock);

    match k {
        Key::Backspace => Some(KeyCode::Backspace),
        Key::Enter | Key::NumPadEnter => Some(KeyCode::Enter),
        Key::Tab => Some(KeyCode::Tab),
        Key::Left  => Some(KeyCode::ArrowLeft),
        Key::Right => Some(KeyCode::ArrowRight),
        Key::Up    => Some(KeyCode::ArrowUp),
        Key::Down  => Some(KeyCode::ArrowDown),
        _ => key_to_char(k, shift).map(|c| {
            if c.is_ascii_alphabetic() && (shift ^ caps) {
                KeyCode::Char(c.to_ascii_uppercase())
            } else {
                KeyCode::Char(c)
            }
        }),
    }
}

fn key_to_char(k: Key, shift: bool) -> Option<char> {
    Some(match k {
        Key::A => 'a', Key::B => 'b', Key::C => 'c', Key::D => 'd',
        Key::E => 'e', Key::F => 'f', Key::G => 'g', Key::H => 'h',
        Key::I => 'i', Key::J => 'j', Key::K => 'k', Key::L => 'l',
        Key::M => 'm', Key::N => 'n', Key::O => 'o', Key::P => 'p',
        Key::Q => 'q', Key::R => 'r', Key::S => 's', Key::T => 't',
        Key::U => 'u', Key::V => 'v', Key::W => 'w', Key::X => 'x',
        Key::Y => 'y', Key::Z => 'z',
        Key::Key0 => if shift { ')' } else { '0' },
        Key::Key1 => if shift { '!' } else { '1' },
        Key::Key2 => if shift { '@' } else { '2' },
        Key::Key3 => if shift { '#' } else { '3' },
        Key::Key4 => if shift { '$' } else { '4' },
        Key::Key5 => if shift { '%' } else { '5' },
        Key::Key6 => if shift { '^' } else { '6' },
        Key::Key7 => if shift { '&' } else { '7' },
        Key::Key8 => if shift { '*' } else { '8' },
        Key::Key9 => if shift { '(' } else { '9' },
        Key::Space => ' ',
        Key::Period    => if shift { '>' } else { '.' },
        Key::Comma     => if shift { '<' } else { ',' },
        Key::Minus     => if shift { '_' } else { '-' },
        Key::Equal     => if shift { '+' } else { '=' },
        Key::Slash     => if shift { '?' } else { '/' },
        Key::Backslash => if shift { '|' } else { '\\' },
        Key::Semicolon => if shift { ':' } else { ';' },
        Key::Apostrophe => if shift { '"' } else { '\'' },
        Key::Backquote => if shift { '~' } else { '`' },
        Key::LeftBracket  => if shift { '{' } else { '[' },
        Key::RightBracket => if shift { '}' } else { ']' },
        _ => return None,
    })
}

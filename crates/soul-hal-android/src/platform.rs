use std::collections::VecDeque;
use std::ffi::CString;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::Instant;

use android_activity::input::{InputEvent, KeyAction, Keycode, MotionAction};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::Gray8,
    prelude::*,
};
use ndk::asset::AssetManager;
use ndk::native_window::NativeWindow;
use raw_window_handle::{
    AndroidDisplayHandle, DisplayHandle, HandleError, HasDisplayHandle, RawDisplayHandle,
};
use soul_hal::{HardButton, KeyCode, Platform};

/// Empty stand-in for an Android display handle.
///
/// Android's window system has no separate display object — `softbuffer`
/// still wants a `HasDisplayHandle` for the context, so we hand it this.
#[derive(Clone, Copy, Default)]
struct AndroidDisplayContext;

impl HasDisplayHandle for AndroidDisplayContext {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let raw = RawDisplayHandle::Android(AndroidDisplayHandle::new());
        // SAFETY: `AndroidDisplayHandle` carries no resource — the borrow
        // is purely nominal and outlives any frame we'd present.
        Ok(unsafe { DisplayHandle::borrow_raw(raw) })
    }
}

// SoulOS virtual screen dimensions; mirrored from soul-core to avoid pulling
// it in through soul-hal-android's deps. Kept in sync by convention.
const VIRT_W: u32 = 240;
const VIRT_H: u32 = 320;

/// `DrawTarget<Color = Gray8>` over a `Vec<u32>` matching the virtual
/// 240×320 SoulOS canvas. `flush()` upscales this into the actual phone
/// surface — this struct never knows the physical size.
pub struct AndroidDisplay {
    width: u32,
    height: u32,
    /// 0x00RRGGBB pixels (R==G==B==luma) — same encoding the desktop HAL
    /// uses, so downstream pixel handling is identical.
    pub buffer: Vec<u32>,
}

impl AndroidDisplay {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            buffer: vec![0x00FF_FFFFu32; (width * height) as usize],
        }
    }
}

impl OriginDimensions for AndroidDisplay {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for AndroidDisplay {
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

/// Thin wrapper around an `AndroidApp` that implements `Platform`.
///
/// Holds a softbuffer surface that exists only while the Activity has a
/// window (Resumed → Stopped lifecycle). All drawing falls back to no-op
/// while the surface is gone.
pub struct AndroidPlatform {
    app: AndroidApp,
    pub display: AndroidDisplay,
    start: Instant,
    pending: VecDeque<soul_hal::InputEvent>,

    sb_context: Option<softbuffer::Context<AndroidDisplayContext>>,
    sb_surface: Option<softbuffer::Surface<AndroidDisplayContext, NativeWindow>>,

    /// Current physical surface size. Updated on InitWindow / WindowResized.
    phys_w: u32,
    phys_h: u32,

    /// Cached scale + centring offsets used to map between phone-pixel
    /// coordinates and virtual SoulOS coordinates.
    scale: u32,
    offset_x: u32,
    offset_y: u32,

    /// True once we've delivered InputEvent::Quit (set when the Activity
    /// requests destruction via MainEvent::Destroy).
    quit_requested: bool,
}

impl AndroidPlatform {
    fn new(app: AndroidApp) -> Self {
        Self {
            app,
            display: AndroidDisplay::new(VIRT_W, VIRT_H),
            start: Instant::now(),
            pending: VecDeque::new(),
            sb_context: None,
            sb_surface: None,
            phys_w: 0,
            phys_h: 0,
            scale: 1,
            offset_x: 0,
            offset_y: 0,
            quit_requested: false,
        }
    }

    fn recompute_layout(&mut self) {
        let sx = self.phys_w / VIRT_W;
        let sy = self.phys_h / VIRT_H;
        self.scale = sx.min(sy).max(1);
        let used_w = VIRT_W * self.scale;
        let used_h = VIRT_H * self.scale;
        self.offset_x = self.phys_w.saturating_sub(used_w) / 2;
        // Push content to the bottom so the UI stays near the thumb.
        self.offset_y = self.phys_h.saturating_sub(used_h);
    }

    /// Acquire / reacquire the softbuffer surface against the current native window.
    fn rebuild_surface(&mut self) {
        let Some(window) = self.app.native_window() else {
            return;
        };
        self.phys_w = window.width() as u32;
        self.phys_h = window.height() as u32;
        self.recompute_layout();

        // softbuffer needs `HasDisplayHandle` for the context (Android's
        // display handle is empty — see `AndroidDisplayContext`) and
        // `HasWindowHandle` for the surface (the ndk `NativeWindow`).
        let context = match softbuffer::Context::new(AndroidDisplayContext) {
            Ok(c) => c,
            Err(e) => {
                log::error!("softbuffer context: {e}");
                return;
            }
        };
        let mut surface = match softbuffer::Surface::new(&context, window) {
            Ok(s) => s,
            Err(e) => {
                log::error!("softbuffer surface: {e}");
                self.sb_context = Some(context);
                return;
            }
        };
        if let (Some(w), Some(h)) = (NonZeroU32::new(self.phys_w), NonZeroU32::new(self.phys_h)) {
            if let Err(e) = surface.resize(w, h) {
                log::error!("softbuffer resize: {e}");
            }
        }
        self.sb_context = Some(context);
        self.sb_surface = Some(surface);
    }

    fn drop_surface(&mut self) {
        self.sb_surface = None;
        self.sb_context = None;
    }

    /// Drain Android input + lifecycle events with a 0-timeout poll.
    /// Pushes translated `soul_hal::InputEvent`s onto `self.pending`.
    fn pump(&mut self) {
        let app = self.app.clone();
        // Snapshot the current physical→virtual mapping for input translation.
        // Doing it inside the closure would borrow `self` immutably while the
        // outer `poll_events` already holds it mutably for the surface side.
        let scale = self.scale.max(1) as f32;
        let off_x = self.offset_x as f32;
        let off_y = self.offset_y as f32;

        let mut window_changed = false;
        let mut redraw_needed = false;

        app.poll_events(Some(std::time::Duration::ZERO), |event| match event {
            PollEvent::Wake => {}
            PollEvent::Timeout => {}
            PollEvent::Main(main) => match main {
                MainEvent::InitWindow { .. } | MainEvent::WindowResized { .. } => {
                    window_changed = true;
                    redraw_needed = true;
                }
                MainEvent::TerminateWindow { .. } => {
                    window_changed = true;
                }
                MainEvent::RedrawNeeded { .. } => {
                    redraw_needed = true;
                }
                MainEvent::Destroy => {
                    self.pending.push_back(soul_hal::InputEvent::Quit);
                    self.quit_requested = true;
                }
                _ => {}
            },
            _ => {}
        });

        if window_changed {
            if self.app.native_window().is_some() {
                self.rebuild_surface();
            } else {
                self.drop_surface();
            }
        }
        if redraw_needed {
            // Force a full repaint next frame: the runtime owns the dirty
            // accumulator, so we just request a flush via an empty event.
            // (soul_core::run always issues one Tick per frame, and the
            // first frame after InitWindow already starts dirty-full.)
        }

        if let Ok(mut iter) = self.app.input_events_iter() {
            let pending = &mut self.pending;
            loop {
                let consumed = iter.next(|ev| {
                    match ev {
                        InputEvent::MotionEvent(motion) => {
                            if let Some(pointer) = motion.pointers().next() {
                                let vx = ((pointer.x() - off_x) / scale).round() as i32;
                                let vy = ((pointer.y() - off_y) / scale).round() as i32;
                                let vx = vx.clamp(0, VIRT_W as i32 - 1) as i16;
                                let vy = vy.clamp(0, VIRT_H as i32 - 1) as i16;
                                match motion.action() {
                                    MotionAction::Down | MotionAction::PointerDown => {
                                        pending.push_back(soul_hal::InputEvent::StylusDown {
                                            x: vx,
                                            y: vy,
                                        });
                                    }
                                    MotionAction::Move => {
                                        pending.push_back(soul_hal::InputEvent::StylusMove {
                                            x: vx,
                                            y: vy,
                                        });
                                    }
                                    MotionAction::Up
                                    | MotionAction::PointerUp
                                    | MotionAction::Cancel => {
                                        pending.push_back(soul_hal::InputEvent::StylusUp {
                                            x: vx,
                                            y: vy,
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                        InputEvent::KeyEvent(key) => match key.action() {
                            KeyAction::Down => {
                                if let Some(translated) = translate_keycode(key.key_code()) {
                                    match translated {
                                        Translated::Hard(b) => pending
                                            .push_back(soul_hal::InputEvent::ButtonDown(b)),
                                        Translated::Key(k) => {
                                            pending.push_back(soul_hal::InputEvent::Key(k))
                                        }
                                    }
                                }
                            }
                            KeyAction::Up => {
                                if let Some(Translated::Hard(b)) =
                                    translate_keycode(key.key_code())
                                {
                                    pending.push_back(soul_hal::InputEvent::ButtonUp(b));
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                    InputStatus::Handled
                });
                if !consumed {
                    break;
                }
            }
        }
    }
}

impl Platform for AndroidPlatform {
    type Display = AndroidDisplay;

    fn display(&mut self) -> &mut Self::Display {
        &mut self.display
    }

    fn poll_event(&mut self) -> Option<soul_hal::InputEvent> {
        self.pending.pop_front()
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn flush(&mut self) {
        if let Some(surface) = self.sb_surface.as_mut() {
            match surface.buffer_mut() {
                Ok(mut buf) => {
                    let pw = self.phys_w as usize;
                    let ph = self.phys_h as usize;
                    if buf.len() == pw * ph {
                        let scale = self.scale.max(1) as usize;
                        let ox = self.offset_x as usize;
                        let oy = self.offset_y as usize;
                        let vw = VIRT_W as usize;
                        let vh = VIRT_H as usize;
                        // Black letterbox; nearest-neighbour upscale of the
                        // 240×320 virtual buffer into the physical surface.
                        buf.fill(0x0000_0000);
                        for vy in 0..vh {
                            let src_row = &self.display.buffer[vy * vw..(vy + 1) * vw];
                            for sy in 0..scale {
                                let dst_y = oy + vy * scale + sy;
                                if dst_y >= ph {
                                    break;
                                }
                                for vx in 0..vw {
                                    let pixel = src_row[vx];
                                    for sx in 0..scale {
                                        let dst_x = ox + vx * scale + sx;
                                        if dst_x < pw {
                                            buf[dst_y * pw + dst_x] = pixel;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Err(e) = buf.present() {
                        log::warn!("softbuffer present: {e}");
                    }
                }
                Err(e) => log::warn!("softbuffer buffer_mut: {e}"),
            }
        }
        self.pump();
    }

    fn sleep_ms(&mut self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }

    fn speak(&mut self, text: &str) {
        // TTS over the JNI bridge is significant scope; log for now so the
        // accessibility code path still drains its queue without leaking.
        log::info!("[TTS] {text}");
    }
}

// --- Bootstrap --------------------------------------------------------------

/// Build an `AndroidPlatform` and prepare the working directory so the
/// existing `std::fs` paths in soul-runner resolve correctly.
///
/// On first launch this extracts every file under the APK's `assets/`
/// directory into `<internal_data_path>/assets/`, then chdirs into the
/// internal data path. Subsequent launches re-extract (cheap, idempotent)
/// so that updated APKs ship updated scripts.
pub fn bootstrap(app: AndroidApp) -> AndroidPlatform {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("soulos"),
    );
    log::info!("🚀 SoulOS bootstrap on Android");

    let data_dir = app
        .internal_data_path()
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/soulos"));
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        log::error!("internal_data_path create: {e}");
    }

    let assets_root = data_dir.join("assets");
    let am = app.asset_manager();
    if let Err(e) = extract_assets(&am, Path::new(""), &assets_root) {
        log::error!("asset extraction failed: {e}");
    }

    if let Err(e) = std::env::set_current_dir(&data_dir) {
        log::error!("set_current_dir({}): {e}", data_dir.display());
    } else {
        log::info!("cwd → {}", data_dir.display());
    }

    AndroidPlatform::new(app)
}

/// Recursively copy every entry under `src_rel` (relative to the APK's
/// `assets/` root) into `dst_root/<src_rel>`. Files that fail to extract
/// are logged and skipped — partial assets are better than no assets.
fn extract_assets(am: &AssetManager, src_rel: &Path, dst_root: &Path) -> std::io::Result<()> {
    let dst_dir = dst_root.join(src_rel);
    std::fs::create_dir_all(&dst_dir)?;

    let dir_cstr = path_to_cstring(src_rel);
    let Some(mut dir) = am.open_dir(&dir_cstr) else {
        return Ok(());
    };

    while let Some(entry) = dir.next() {
        let name = entry.to_string_lossy().to_string();
        let child_rel = if src_rel.as_os_str().is_empty() {
            PathBuf::from(&name)
        } else {
            src_rel.join(&name)
        };
        let child_cstr = path_to_cstring(&child_rel);

        if let Some(mut asset) = am.open(&child_cstr) {
            // It's a file — stream bytes out. `buffer()` / `AAsset_getBuffer`
            // only works for uncompressed assets; use Read to handle both.
            let mut bytes = Vec::new();
            if let Err(e) = std::io::Read::read_to_end(&mut asset, &mut bytes) {
                log::warn!("read {}: {e}", child_rel.display());
            }
            let dst_file = dst_root.join(&child_rel);
            if let Err(e) = std::fs::write(&dst_file, &bytes) {
                log::warn!("write {}: {e}", dst_file.display());
            }
        } else {
            // Treat as a subdirectory and recurse.
            if let Err(e) = extract_assets(am, &child_rel, dst_root) {
                log::warn!("extract {}: {e}", child_rel.display());
            }
        }
    }
    Ok(())
}

fn path_to_cstring(p: &Path) -> CString {
    let s = p.to_string_lossy().replace('\\', "/");
    CString::new(s).unwrap_or_else(|_| CString::new("").unwrap())
}

// --- Key translation --------------------------------------------------------

enum Translated {
    Hard(HardButton),
    Key(KeyCode),
}

fn translate_keycode(kc: Keycode) -> Option<Translated> {
    Some(match kc {
        Keycode::Home => Translated::Hard(HardButton::Home),
        Keycode::Menu => Translated::Hard(HardButton::Menu),
        Keycode::Power => Translated::Hard(HardButton::Power),
        Keycode::VolumeUp => Translated::Hard(HardButton::VolumeUp),
        Keycode::VolumeDown => Translated::Hard(HardButton::VolumeDown),
        Keycode::PageUp => Translated::Hard(HardButton::PageUp),
        Keycode::PageDown => Translated::Hard(HardButton::PageDown),
        Keycode::Back => Translated::Hard(HardButton::Home),
        Keycode::DpadUp => Translated::Key(KeyCode::ArrowUp),
        Keycode::DpadDown => Translated::Key(KeyCode::ArrowDown),
        Keycode::DpadLeft => Translated::Key(KeyCode::ArrowLeft),
        Keycode::DpadRight => Translated::Key(KeyCode::ArrowRight),
        Keycode::Enter | Keycode::DpadCenter => Translated::Key(KeyCode::Enter),
        Keycode::Del => Translated::Key(KeyCode::Backspace),
        Keycode::Tab => Translated::Key(KeyCode::Tab),
        Keycode::Space => Translated::Key(KeyCode::Char(' ')),
        _ => return None,
    })
}

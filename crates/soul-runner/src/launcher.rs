//! Launcher — the home-screen app that lists and launches other apps.
//!
//! The Launcher is a first-class app stored at `apps[0]` in the Host.
//! On `AppStart` it queries [`soul_script::app_list`] for the current
//! registry and loads each app's PGM icon from `assets/sprites/`.
//! This means Host does not need to pre-build any snapshot or pass any
//! data into the Launcher at construction time.

use embedded_graphics::{
    draw_target::DrawTarget,
    image::{Image, ImageRaw},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};
use soul_core::{Ctx, Event, HardButton, APP_HEIGHT, SCREEN_WIDTH};
use soul_script::SystemRequest;
use soul_ui::{hit_test, title_bar, BLACK, TITLE_BAR_H};

// --- Layout constants ---------------------------------------------------

const ICON_CELL: u32 = 32;
const LABEL_FONT_W: i32 = 6;
const LABEL_FONT_H: i32 = 10;
const ICON_LABEL_GAP: i32 = 1;
const LAUNCHER_COLS: i32 = 4;
const LAUNCHER_ROWS: i32 = 6;
const LAUNCHER_H_GAP: i32 = 4;
const LAUNCHER_V_GAP: i32 = 3;
const LAUNCHER_TOP_PAD: i32 = 4;

// --- Internal app entry -------------------------------------------------

struct AppEntry {
    app_id: String,
    name: String,
    icon: Vec<u8>, // raw 32×32 pixels, or empty for blank tile
}

// --- Launcher -----------------------------------------------------------

pub struct Launcher {
    apps: Vec<AppEntry>,
    touched: Option<usize>,
}

impl Launcher {
    pub const APP_ID: &'static str = "com.soulos.launcher";
    pub const NAME: &'static str = "Launcher";

    pub fn new() -> Self {
        Self {
            apps: vec![],
            touched: None,
        }
    }

    // --- Self-initialisation -------------------------------------------

    fn refresh_app_list(&mut self) {
        let cell = ICON_CELL as usize;
        self.apps = soul_script::app_list()
            .iter()
            .map(|entry| {
                let icon = load_icon(&entry.icon_stem, cell);
                AppEntry {
                    app_id: entry.app_id.clone(),
                    name: entry.name.clone(),
                    icon,
                }
            })
            .collect();
    }

    // --- Layout helpers -------------------------------------------------

    fn tile_origin(idx: usize) -> (i32, i32) {
        let tile_w = ICON_CELL as i32;
        let tile_h = ICON_CELL as i32 + ICON_LABEL_GAP + LABEL_FONT_H;
        let grid_w = LAUNCHER_COLS * tile_w + (LAUNCHER_COLS - 1) * LAUNCHER_H_GAP;
        let x_off = (SCREEN_WIDTH as i32 - grid_w) / 2;
        let col = idx as i32 % LAUNCHER_COLS;
        let row = idx as i32 / LAUNCHER_COLS;
        let x = x_off + col * (tile_w + LAUNCHER_H_GAP);
        let avail_h = APP_HEIGHT as i32 - TITLE_BAR_H as i32 - LAUNCHER_TOP_PAD;
        let row_pitch = (avail_h - (LAUNCHER_ROWS - 1) * LAUNCHER_V_GAP) / LAUNCHER_ROWS;
        let y_slot = TITLE_BAR_H as i32 + LAUNCHER_TOP_PAD + row * (row_pitch + LAUNCHER_V_GAP);
        (x, y_slot + (row_pitch - tile_h) / 2)
    }

    fn icon_rect(idx: usize) -> Rectangle {
        let (x, y) = Self::tile_origin(idx);
        Rectangle::new(Point::new(x, y), Size::new(ICON_CELL, ICON_CELL))
    }

    fn tile_rect(idx: usize) -> Rectangle {
        let (x, y) = Self::tile_origin(idx);
        let h = ICON_CELL as i32 + ICON_LABEL_GAP + LABEL_FONT_H;
        Rectangle::new(Point::new(x, y), Size::new(ICON_CELL, h as u32))
    }

    fn find_hit(&self, x: i16, y: i16) -> Option<usize> {
        (0..self.apps.len()).find(|&i| hit_test(&Self::tile_rect(i), x, y))
    }

    fn set_touched(&mut self, new: Option<usize>, ctx: &mut Ctx<'_>) {
        if new == self.touched {
            return;
        }
        if let Some(i) = self.touched {
            ctx.invalidate(Self::tile_rect(i));
        }
        if let Some(i) = new {
            ctx.invalidate(Self::tile_rect(i));
        }
        self.touched = new;
    }

    fn label_text(name: &str) -> String {
        let max_chars = (ICON_CELL as i32 / LABEL_FONT_W).max(1) as usize;
        let n = name.chars().count();
        if n <= max_chars {
            return name.to_string();
        }
        let take = max_chars.saturating_sub(1);
        name.chars().take(take).collect::<String>() + "…"
    }

    fn launch_by_display_idx(&self, display_idx: usize) -> Option<SystemRequest> {
        self.apps
            .get(display_idx)
            .map(|e| SystemRequest::LaunchById(e.app_id.clone()))
    }

    // --- App interface --------------------------------------------------

    pub fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match event {
            Event::AppStart => {
                self.refresh_app_list();
                ctx.invalidate_all();
                None
            }
            Event::PenDown { x, y } | Event::PenMove { x, y } => {
                let hit = self.find_hit(x, y);
                self.set_touched(hit, ctx);
                None
            }
            Event::PenUp { x, y } => {
                let hit = self.find_hit(x, y);
                let was = self.touched;
                self.set_touched(None, ctx);
                if hit.is_some() && hit == was {
                    hit.and_then(|i| self.launch_by_display_idx(i))
                } else {
                    None
                }
            }
            Event::ButtonDown(HardButton::AppA) => self.launch_by_display_idx(0),
            Event::ButtonDown(HardButton::AppB) => self.launch_by_display_idx(1),
            Event::ButtonDown(HardButton::AppC) => self.launch_by_display_idx(2),
            Event::ButtonDown(HardButton::AppD) => self.launch_by_display_idx(3),
            _ => None,
        }
    }

    pub fn draw<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D) {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, Self::NAME);
        let label_style = MonoTextStyle::new(&FONT_6X10, BLACK);

        for (display_idx, entry) in self.apps.iter().enumerate() {
            let icon_r = Self::icon_rect(display_idx);
            let pressed = self.touched == Some(display_idx);
            let expected = (ICON_CELL * ICON_CELL) as usize;

            if entry.icon.len() == expected {
                if pressed {
                    let inv: Vec<u8> = entry.icon.iter().map(|&p| 255 - p).collect();
                    let raw = ImageRaw::<Gray8>::new(&inv, ICON_CELL);
                    let _ = Image::new(&raw, icon_r.top_left).draw(canvas);
                } else {
                    let raw = ImageRaw::<Gray8>::new(&entry.icon, ICON_CELL);
                    let _ = Image::new(&raw, icon_r.top_left).draw(canvas);
                }
            } else {
                let _ = canvas.fill_solid(&icon_r, Gray8::new(if pressed { 128 } else { 255 }));
            }

            let lbl = Self::label_text(&entry.name);
            let nw = lbl.chars().count() as i32 * LABEL_FONT_W;
            let tx = icon_r.top_left.x + (ICON_CELL as i32 - nw) / 2;
            let ty = icon_r.top_left.y + ICON_CELL as i32 + ICON_LABEL_GAP;
            let _ = Text::with_baseline(&lbl, Point::new(tx, ty), label_style, Baseline::Top)
                .draw(canvas);
        }
    }

    pub fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        vec![]
    }
}

// --- PGM icon loader ----------------------------------------------------

fn load_icon(stem: &str, cell: usize) -> Vec<u8> {
    if stem.is_empty() {
        return vec![];
    }
    let path = std::path::PathBuf::from("assets/sprites").join(format!("{stem}_icon.pgm"));
    match load_pgm(&path) {
        Ok((w, h, pix)) if w == cell && h == cell => pix,
        _ => vec![],
    }
}

fn load_pgm(path: &std::path::Path) -> std::io::Result<(usize, usize, Vec<u8>)> {
    use std::io::{BufRead, BufReader, Read};
    let f = std::fs::File::open(path)?;
    let mut r = BufReader::new(f);
    let mut line = String::new();
    r.read_line(&mut line)?;
    if line.trim() != "P5" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected P5 PGM",
        ));
    }
    let (w, h) = read_pair(&mut r)?;
    let maxv = read_value(&mut r)?;
    if maxv != 255 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "only maxval 255 supported",
        ));
    }
    let mut pixels = vec![0u8; w * h];
    r.read_exact(&mut pixels)?;
    Ok((w, h, pixels))
}

fn read_pair<R: std::io::BufRead>(r: &mut R) -> std::io::Result<(usize, usize)> {
    let mut line = String::new();
    loop {
        line.clear();
        r.read_line(&mut line)?;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut p = t.splitn(2, ' ');
        let w = p
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad width"))?;
        let h = p
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad height"))?;
        return Ok((w, h));
    }
}

fn read_value<R: std::io::BufRead>(r: &mut R) -> std::io::Result<usize> {
    let mut line = String::new();
    loop {
        line.clear();
        r.read_line(&mut line)?;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        return t
            .parse()
            .ok()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad value"));
    }
}

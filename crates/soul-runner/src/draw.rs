//! Pixel editor for the hosted simulator: fat-pixel canvas, stylus
//! painting, and PGM ([`P5`]) assets under a configurable directory.
//!
//! Eight fixed gray levels; **Pen**, **Fill**, and **Erase** tools; brush
//! size; **Undo** (stack of prior `fg` + `written`); optional background
//! reference; and a written-pixel mask so white ink and clears work.
//!
//! Default directory: `assets/draw/` (override with `SOUL_DRAW_DIR`).
//! Use **Menu** (system strip or **F6**) for file and background actions.
//!
//! Launcher icons live in the `launcher_icons` database, persisted under
//! `.soulos/launcher_icons.sdb` (see `launcher_store`). The DB is seeded
//! lazily from `assets/sprites/` the first time the icon picker is opened.
//! Use **Menu → Open icon…** to edit the launcher-sized icon; **Save** writes
//! back and persists the cache.

use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
};
use soul_core::{App, Ctx, Event, HardButton, KeyCode, APP_HEIGHT, SCREEN_WIDTH};
use soul_ui::{button, hit_test, label, title_bar, TextInput, TextInputOutput, BLACK, TITLE_BAR_H};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use crate::launcher_store::LauncherIconStore;
use crate::ICON_CELL;

const LOG_W: usize = 48;
const LOG_H: usize = 48;
const ICON_OX: usize = (LOG_W - ICON_CELL as usize) / 2;
const ICON_OY: usize = (LOG_H - ICON_CELL as usize) / 2;
const SCALE: i32 = 5;
const CANVAS_PX: i32 = (LOG_W as i32) * SCALE;

/// Eight evenly spaced levels from black to white (3-bit display feel).
pub const GRAY_LEVELS: [u8; 8] = [0, 36, 73, 109, 146, 182, 218, 255];

const ROW1_Y: i32 = TITLE_BAR_H as i32 + CANVAS_PX;
const ROW2_Y: i32 = ROW1_Y + 26;

const BRUSH_RADIUS_MIN: i32 = 0;
const BRUSH_RADIUS_MAX: i32 = 3;

const MENU_ITEMS: &[&str] = &[
    "New",
    "Save",
    "Save as...",
    "Open...",
    "Open icon...",
    "Load bg...",
    "Clear bg",
    "Edit Layout",
    "Delete Element",
    "Reset Layout",
    "Close menu",
];

const OPEN_ROW_H: i32 = 24;
const OPEN_VISIBLE: usize = 6;

const UNDO_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Brush,
    Fill,
    Eraser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaintTarget {
    None,
    Canvas,
    Ink(usize),
    ToolBrush,
    ToolFill,
    ToolEraser,
    BrushMinus,
    BrushPlus,
    UndoBtn,
    ClearBtn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenPurpose {
    Document,
    Background,
    LauncherIcon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditTarget {
    Document,
    Icon(usize),
}

enum Mode {
    Normal,
    SaveAs(TextInput),
    OpenList {
        files: Vec<String>,
        scroll: usize,
        purpose: OpenPurpose,
    },
}

pub struct Draw {
    launcher_icons: LauncherIconStore,
    edit: EditTarget,
    fg: Vec<u8>,
    written: Vec<bool>,
    bg: Option<Vec<u8>>,
    draw_dir: PathBuf,
    doc_name: String,
    brush: u8,
    brush_radius: i32,
    tool: Tool,
    paint_touch: PaintTarget,
    last_cell: Option<(usize, usize)>,
    menu_open: bool,
    menu_touch: Option<usize>,
    mode: Mode,
    undo_stack: Vec<(Vec<u8>, Vec<bool>)>,

    // MobileBuilder integration
    ui_form: soul_ui::Form,
    edit_overlay: soul_ui::EditOverlay,
    builder_mode: bool,
    ui_db_path: PathBuf,
}

impl Draw {
    pub const APP_ID: &'static str = "com.soulos.draw";
    pub const NAME: &'static str = "Draw";

    fn validate_background(&mut self) {
        if let Some(ref bg) = self.bg {
            if bg.len() != LOG_W * LOG_H {
                // Background size doesn't match current canvas - clear it
                self.bg = None;
            }
        }
    }

    pub fn new() -> Self {
        let launcher_icons = LauncherIconStore::load_or_empty();
        let draw_dir = std::env::var("SOUL_DRAW_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("assets/draw"));
        let doc_name = String::from("canvas");
        let path = path_for(&draw_dir, &doc_name);
        let (fg, written) = match load_pgm(&path) {
            Ok((w, h, data)) if w == LOG_W && h == LOG_H => {
                let written = data.iter().map(|&p| p != 255).collect();
                (data, written)
            }
            Ok((w, h, _)) => {
                eprintln!(
                    "draw: {} is {}×{}, expected {}×{} — starting blank",
                    path.display(),
                    w,
                    h,
                    LOG_W,
                    LOG_H
                );
                (vec![255; LOG_W * LOG_H], vec![false; LOG_W * LOG_H])
            }
            Err(e) => {
                eprintln!(
                    "draw: could not load {} ({e}) — blank canvas",
                    path.display()
                );
                (vec![255; LOG_W * LOG_H], vec![false; LOG_W * LOG_H])
            }
        };

        let ui_db_path = std::env::var("SOUL_DRAW_UI_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".soulos/draw_ui.sdb"));

        let mut instance = Self {
            launcher_icons,
            edit: EditTarget::Document,
            fg,
            written,
            bg: None,
            draw_dir,
            doc_name,
            brush: GRAY_LEVELS[0],
            brush_radius: BRUSH_RADIUS_MIN,
            tool: Tool::Brush,
            paint_touch: PaintTarget::None,
            last_cell: None,
            menu_open: false,
            menu_touch: None,
            mode: Mode::Normal,
            undo_stack: Vec::new(),

            ui_form: Self::load_ui(&ui_db_path),
            edit_overlay: soul_ui::EditOverlay::new(),
            builder_mode: false,
            ui_db_path,
        };
        instance.validate_background();
        instance
    }

    fn load_ui(path: &Path) -> soul_ui::Form {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(db) = soul_db::Database::decode(&bytes) {
                if let Some(rec) = db.iter().next() {
                    if let Ok(json) = std::str::from_utf8(&rec.data) {
                        if let Some(form) = soul_ui::Form::from_json(json) {
                            return form;
                        }
                    }
                }
            }
        }
        Self::default_draw_ui()
    }

    fn persist_ui(&self) {
        if let Some(parent) = self.ui_db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut db = soul_db::Database::new("draw_ui");
        db.insert(0, self.ui_form.to_json().into_bytes());
        let _ = std::fs::write(&self.ui_db_path, db.encode());
    }

    fn default_draw_ui() -> soul_ui::Form {
        use soul_ui::{A11yHints, Component, ComponentType, Form, Rect};
        use std::collections::BTreeMap;
        let mut form = Form::new("draw_ui");

        let row1_y = 15 + 240;
        let row2_y = row1_y + 26;

        form.components.push(Component {
            id: "tool_brush".into(),
            class: "tool".into(),
            type_: ComponentType::Button,
            bounds: Rect {
                x: 4,
                y: row1_y + 2,
                w: 32,
                h: 18,
            },
            properties: BTreeMap::from([("label".into(), "Pen".into())]),
            a11y: A11yHints {
                label: "Pen tool".into(),
                role: "button".into(),
            },
            interactions: Vec::new(),
            binding: None,
        });
        form.components.push(Component {
            id: "tool_fill".into(),
            class: "tool".into(),
            type_: ComponentType::Button,
            bounds: Rect {
                x: 40,
                y: row1_y + 2,
                w: 32,
                h: 18,
            },
            properties: BTreeMap::from([("label".into(), "Fill".into())]),
            a11y: A11yHints {
                label: "Fill tool".into(),
                role: "button".into(),
            },
            interactions: Vec::new(),
            binding: None,
        });
        form.components.push(Component {
            id: "tool_eraser".into(),
            class: "tool".into(),
            type_: ComponentType::Button,
            bounds: Rect {
                x: 60,
                y: row1_y + 2,
                w: 38,
                h: 18,
            },
            properties: BTreeMap::from([("label".into(), "Erase".into())]),
            a11y: A11yHints {
                label: "Eraser tool".into(),
                role: "button".into(),
            },
            interactions: Vec::new(),
            binding: None,
        });

        form.components.push(Component {
            id: "brush_minus".into(),
            class: "brush-ctrl".into(),
            type_: ComponentType::Button,
            bounds: Rect {
                x: 102,
                y: row1_y + 2,
                w: 20,
                h: 18,
            },
            properties: BTreeMap::from([("label".into(), "-".into())]),
            a11y: A11yHints {
                label: "Decrease brush size".into(),
                role: "button".into(),
            },
            interactions: Vec::new(),
            binding: None,
        });
        form.components.push(Component {
            id: "brush_plus".into(),
            class: "brush-ctrl".into(),
            type_: ComponentType::Button,
            bounds: Rect {
                x: 126,
                y: row1_y + 2,
                w: 20,
                h: 18,
            },
            properties: BTreeMap::from([("label".into(), "+".into())]),
            a11y: A11yHints {
                label: "Increase brush size".into(),
                role: "button".into(),
            },
            interactions: Vec::new(),
            binding: None,
        });
        form.components.push(Component {
            id: "undo".into(),
            class: "action".into(),
            type_: ComponentType::Button,
            bounds: Rect {
                x: 154,
                y: row1_y + 2,
                w: 36,
                h: 18,
            },
            properties: BTreeMap::from([("label".into(), "Undo".into())]),
            a11y: A11yHints {
                label: "Undo last stroke".into(),
                role: "button".into(),
            },
            interactions: Vec::new(),
            binding: None,
        });
        form.components.push(Component {
            id: "clear".into(),
            class: "action".into(),
            type_: ComponentType::Button,
            bounds: Rect {
                x: 196,
                y: row1_y + 2,
                w: 40,
                h: 18,
            },
            properties: BTreeMap::from([("label".into(), "Clear".into())]),
            a11y: A11yHints {
                label: "Clear canvas".into(),
                role: "button".into(),
            },
            interactions: Vec::new(),
            binding: None,
        });

        for (i, g) in GRAY_LEVELS.iter().enumerate() {
            let x = 4 + (i as i32) * 28;
            form.components.push(Component {
                id: format!("ink_{}", i),
                class: "ink".into(),
                type_: ComponentType::Button,
                bounds: Rect {
                    x,
                    y: row2_y + 2,
                    w: 24,
                    h: 16,
                },
                properties: BTreeMap::from([("color".into(), (*g as i64).into())]),
                a11y: A11yHints {
                    label: format!("Gray level {}", i),
                    role: "button".into(),
                },
                interactions: Vec::new(),
                binding: None,
            });
        }

        form
    }

    fn push_undo(&mut self) {
        if self.undo_stack.len() >= UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.undo_stack
            .push((self.fg.clone(), self.written.clone()));
    }

    fn pop_undo(&mut self, ctx: &mut Ctx<'_>) {
        if let Some((prev_fg, prev_written)) = self.undo_stack.pop() {
            self.fg = prev_fg;
            self.written = prev_written;
            ctx.invalidate(Self::canvas_screen_rect());
        }
    }

    fn path_for_doc(&self) -> PathBuf {
        path_for(&self.draw_dir, &self.doc_name)
    }

    fn canvas_screen_rect() -> Rectangle {
        Rectangle::new(
            Point::new(0, TITLE_BAR_H as i32),
            Size::new(CANVAS_PX as u32, CANVAS_PX as u32),
        )
    }

    fn screen_to_cell(&self, x: i16, y: i16) -> Option<(usize, usize)> {
        let r = Self::canvas_screen_rect();
        if !hit_test(&r, x, y) {
            return None;
        }
        let lx = ((x as i32 - r.top_left.x) / SCALE) as usize;
        let ly = ((y as i32 - r.top_left.y) / SCALE) as usize;
        if lx >= LOG_W || ly >= LOG_H {
            return None;
        }
        if matches!(self.edit, EditTarget::Icon(_)) {
            let cell = ICON_CELL as usize;
            if lx < ICON_OX || lx >= ICON_OX + cell || ly < ICON_OY || ly >= ICON_OY + cell {
                return None;
            }
        }
        Some((lx, ly))
    }

    fn rect_menu_entry(i: usize) -> Rectangle {
        let col = (i % 2) as i32;
        let row = (i / 2) as i32;
        Rectangle::new(
            Point::new(15 + col * 105, 60 + row * 26),
            Size::new(100, 22),
        )
    }

    fn rect_save_as_input() -> Rectangle {
        Rectangle::new(Point::new(16, 98), Size::new(208, 20))
    }

    fn rect_save_as_ok() -> Rectangle {
        Rectangle::new(Point::new(24, 130), Size::new(80, 28))
    }

    fn rect_save_as_cancel() -> Rectangle {
        Rectangle::new(Point::new(120, 130), Size::new(96, 28))
    }

    fn rect_open_row(i: usize) -> Rectangle {
        Rectangle::new(
            Point::new(16, 52 + i as i32 * OPEN_ROW_H),
            Size::new(208, (OPEN_ROW_H - 2) as u32),
        )
    }

    fn rect_open_cancel() -> Rectangle {
        Rectangle::new(Point::new(70, 226), Size::new(100, 28))
    }

    fn app_content_rect() -> Rectangle {
        let h = (APP_HEIGHT as u32).saturating_sub(TITLE_BAR_H);
        Rectangle::new(
            Point::new(0, TITLE_BAR_H as i32),
            Size::new(SCREEN_WIDTH as u32, h),
        )
    }

    fn display_value(&self, i: usize) -> u8 {
        if self.written[i] {
            return self.fg[i];
        }
        if let Some(ref bg) = self.bg {
            if i < bg.len() {
                faint_background(bg[i])
            } else {
                255
            }
        } else {
            255
        }
    }

    fn flatten_for_save(&self) -> Vec<u8> {
        (0..LOG_W * LOG_H).map(|i| self.display_value(i)).collect()
    }

    fn invalidate_cell(ctx: &mut Ctx<'_>, lx: usize, ly: usize) {
        let r = Rectangle::new(
            Point::new(
                (lx as i32) * SCALE,
                TITLE_BAR_H as i32 + (ly as i32) * SCALE,
            ),
            Size::new(SCALE as u32, SCALE as u32),
        );
        ctx.invalidate(r);
    }

    fn stamp(&mut self, cx: i32, cy: i32, ctx: &mut Ctx<'_>) {
        let r = self.brush_radius;
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                let nx = cx + dx;
                let ny = cy + dy;
                if nx < 0 || ny < 0 {
                    continue;
                }
                let lx = nx as usize;
                let ly = ny as usize;
                if lx >= LOG_W || ly >= LOG_H {
                    continue;
                }

                // In icon editing mode, only paint within icon bounds
                if matches!(self.edit, EditTarget::Icon(_)) {
                    let cell = ICON_CELL as usize;
                    if lx < ICON_OX || lx >= ICON_OX + cell || ly < ICON_OY || ly >= ICON_OY + cell
                    {
                        continue;
                    }
                }

                let i = ly * LOG_W + lx;
                if self.written[i] && self.fg[i] == self.brush {
                    continue;
                }
                self.written[i] = true;
                self.fg[i] = self.brush;
                Self::invalidate_cell(ctx, lx, ly);
            }
        }
    }

    fn erase_stamp(&mut self, cx: i32, cy: i32, ctx: &mut Ctx<'_>) {
        let r = self.brush_radius;
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                let nx = cx + dx;
                let ny = cy + dy;
                if nx < 0 || ny < 0 {
                    continue;
                }
                let lx = nx as usize;
                let ly = ny as usize;
                if lx >= LOG_W || ly >= LOG_H {
                    continue;
                }

                // In icon editing mode, only erase within icon bounds
                if matches!(self.edit, EditTarget::Icon(_)) {
                    let cell = ICON_CELL as usize;
                    if lx < ICON_OX || lx >= ICON_OX + cell || ly < ICON_OY || ly >= ICON_OY + cell
                    {
                        continue;
                    }
                }

                let i = ly * LOG_W + lx;
                if !self.written[i] {
                    continue;
                }
                self.written[i] = false;
                Self::invalidate_cell(ctx, lx, ly);
            }
        }
    }

    fn paint_at(&mut self, x: i32, y: i32, ctx: &mut Ctx<'_>) {
        match self.tool {
            Tool::Brush => self.stamp(x, y, ctx),
            Tool::Eraser => self.erase_stamp(x, y, ctx),
            Tool::Fill => {
                // Fill tool only works on pen down, not during moves
            }
        }
    }

    fn flood_fill(&mut self, sx: usize, sy: usize, ctx: &mut Ctx<'_>) {
        let target = self.display_value(sy * LOG_W + sx);
        // Don't fill if target color is same as brush color
        if target == self.brush {
            return;
        }

        self.push_undo();
        let mut seen = vec![false; LOG_W * LOG_H];
        let mut q = VecDeque::new();
        q.push_back((sx, sy));
        while let Some((x, y)) = q.pop_front() {
            let i = y * LOG_W + x;
            if seen[i] {
                continue;
            }
            if self.display_value(i) != target {
                continue;
            }
            seen[i] = true;
            self.written[i] = true;
            self.fg[i] = self.brush;
            Self::invalidate_cell(ctx, x, y);
            if x > 0 {
                let nx = x - 1;
                let ni = y * LOG_W + nx;
                if !seen[ni] && self.display_value(ni) == target {
                    q.push_back((nx, y));
                }
            }
            if x + 1 < LOG_W {
                let nx = x + 1;
                let ni = y * LOG_W + nx;
                if !seen[ni] && self.display_value(ni) == target {
                    q.push_back((nx, y));
                }
            }
            if y > 0 {
                let ny = y - 1;
                let ni = ny * LOG_W + x;
                if !seen[ni] && self.display_value(ni) == target {
                    q.push_back((x, ny));
                }
            }
            if y + 1 < LOG_H {
                let ny = y + 1;
                let ni = ny * LOG_W + x;
                if !seen[ni] && self.display_value(ni) == target {
                    q.push_back((x, ny));
                }
            }
        }
    }

    fn plot_line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, ctx: &mut Ctx<'_>) {
        let mut x0 = x0 as i32;
        let mut y0 = y0 as i32;
        let x1 = x1 as i32;
        let y1 = y1 as i32;
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx as i32 - dy as i32;
        loop {
            self.paint_at(x0, y0, ctx);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x0 += sx;
            }
            if e2 < dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn clear_canvas(&mut self, ctx: &mut Ctx<'_>) {
        self.push_undo();
        for w in &mut self.written {
            *w = false;
        }
        ctx.invalidate(Self::canvas_screen_rect());
    }

    fn clear_background(&mut self, ctx: &mut Ctx<'_>) {
        self.bg = None;
        ctx.invalidate(Self::canvas_screen_rect());
    }

    fn try_save_to_path(&self, path: &Path) -> bool {
        let flat = self.flatten_for_save();
        match save_pgm(path, LOG_W, LOG_H, &flat) {
            Ok(()) => {
                eprintln!("draw: saved {}", path.display());
                true
            }
            Err(e) => {
                eprintln!("draw: save {} failed: {e}", path.display());
                false
            }
        }
    }

    fn try_load_doc_path(&mut self, path: &Path, ctx: &mut Ctx<'_>) -> bool {
        match load_pgm(path) {
            Ok((w, h, data)) if w == LOG_W && h == LOG_H => {
                self.undo_stack.clear();
                self.fg = data;
                self.written = vec![true; LOG_W * LOG_H];
                self.bg = None;
                self.edit = EditTarget::Document;
                self.validate_background();
                ctx.invalidate(Self::canvas_screen_rect());
                eprintln!("draw: loaded {}", path.display());
                true
            }
            Ok((w, h, _)) => {
                eprintln!(
                    "draw: {} is {}×{}, need {}×{}",
                    path.display(),
                    w,
                    h,
                    LOG_W,
                    LOG_H
                );
                false
            }
            Err(e) => {
                eprintln!("draw: load {} failed: {e}", path.display());
                false
            }
        }
    }

    fn try_load_background_path(&mut self, path: &Path, ctx: &mut Ctx<'_>) -> bool {
        match load_pgm(path) {
            Ok((w, h, data)) if w == LOG_W && h == LOG_H => {
                self.bg = Some(data);
                ctx.invalidate(Self::canvas_screen_rect());
                eprintln!("draw: background {}", path.display());
                true
            }
            Ok((w, h, data)) => {
                let scaled_data = scale_image_to_canvas(&data, w, h, LOG_W, LOG_H);
                self.bg = Some(scaled_data);
                ctx.invalidate(Self::canvas_screen_rect());
                eprintln!(
                    "draw: background {} scaled from {}×{} to {}×{}",
                    path.display(),
                    w,
                    h,
                    LOG_W,
                    LOG_H
                );
                true
            }
            Err(e) => {
                eprintln!("draw: load bg failed: {e}");
                false
            }
        }
    }

    fn apply_text_out(&mut self, out: TextInputOutput, ctx: &mut Ctx<'_>) {
        if let Some(r) = out.dirty {
            ctx.invalidate(r);
        }
        if out.submitted {
            self.commit_save_as(ctx);
        }
    }

    fn commit_save_as(&mut self, ctx: &mut Ctx<'_>) {
        let Mode::SaveAs(input) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return;
        };
        let raw = input.text().to_string();
        if let Some(name) = sanitize_name(&raw) {
            self.doc_name = name;
            self.edit = EditTarget::Document;
            let _ = self.try_save_to_path(&self.path_for_doc());
        } else {
            eprintln!("draw: invalid name (use letters, digits, _ -)");
        }
        ctx.invalidate_all();
    }

    fn cancel_modal(&mut self, ctx: &mut Ctx<'_>) {
        self.mode = Mode::Normal;
        self.menu_open = false;
        ctx.invalidate_all();
    }

    fn refresh_open_list(&mut self, purpose: OpenPurpose) {
        self.mode = Mode::OpenList {
            files: list_pgm_stems(&self.draw_dir).unwrap_or_else(|e| {
                eprintln!("draw: list {} failed: {e}", self.draw_dir.display());
                Vec::new()
            }),
            scroll: 0,
            purpose,
        };
    }

    fn ensure_icons_seeded(&mut self) {
        let list = soul_script::app_list();
        if self.launcher_icons.is_valid_for(list.len()) {
            return;
        }
        let cell = ICON_CELL as usize;
        for (i, entry) in list.iter().enumerate() {
            let data = if !entry.icon_stem.is_empty() {
                let path = std::path::PathBuf::from("assets/sprites")
                    .join(format!("{}_icon.pgm", entry.icon_stem));
                match load_pgm(&path) {
                    Ok((w, h, pix)) if w == cell && h == cell => pix,
                    _ => vec![255u8; cell * cell],
                }
            } else {
                vec![255u8; cell * cell]
            };
            self.launcher_icons.db.insert(i as u8, data);
        }
        if let Err(e) = self.launcher_icons.persist() {
            eprintln!("draw: could not persist seeded icons: {e}");
        }
    }

    fn refresh_open_icon_list(&mut self) {
        self.ensure_icons_seeded();
        self.mode = Mode::OpenList {
            files: soul_script::app_list()
                .iter()
                .map(|e| e.name.clone())
                .collect(),
            scroll: 0,
            purpose: OpenPurpose::LauncherIcon,
        };
    }

    /// Persist the launcher-icon cache to disk (called on `AppStop`).
    pub fn persist(&self) {
        if let Err(e) = self.launcher_icons.persist() {
            eprintln!("draw: could not persist icon cache: {e}");
        }
    }

    fn load_icon_from_db(&mut self, idx: usize, ctx: &mut Ctx<'_>) -> bool {
        self.ensure_icons_seeded();
        let cell = ICON_CELL as usize;
        let area = cell * cell;
        let data = {
            let Some(rec) = self.launcher_icons.db.iter_category(idx as u8).next() else {
                return false;
            };
            if rec.data.len() != area {
                return false;
            }
            rec.data.clone()
        };
        self.undo_stack.clear();
        self.fg.fill(255);
        self.written.fill(false);
        self.bg = None;
        for y in 0..cell {
            for x in 0..cell {
                let i = (ICON_OY + y) * LOG_W + (ICON_OX + x);
                let p = data[y * cell + x];
                self.fg[i] = p;
                self.written[i] = p != 255;
            }
        }
        self.edit = EditTarget::Icon(idx);
        self.doc_name = format!(
            "icon:{}",
            soul_script::app_list()
                .get(idx)
                .map(|e| e.name.as_str())
                .unwrap_or("?")
        );
        ctx.invalidate(Self::canvas_screen_rect());
        true
    }

    fn save_icon_to_db(&mut self, idx: usize) -> bool {
        let cell = ICON_CELL as usize;
        let mut buf = Vec::with_capacity(cell * cell);
        for y in 0..cell {
            for x in 0..cell {
                let i = (ICON_OY + y) * LOG_W + (ICON_OX + x);
                buf.push(self.display_value(i));
            }
        }
        let Some(rec) = self.launcher_icons.db.iter_category(idx as u8).next() else {
            return false;
        };
        let id = rec.id;
        let ok = self.launcher_icons.db.update(id, buf);
        if ok {
            if let Err(e) = self.launcher_icons.persist() {
                eprintln!("draw: could not persist launcher icon cache: {e}");
            }
        }
        ok
    }

    fn menu_action(&mut self, idx: usize, ctx: &mut Ctx<'_>) {
        self.menu_open = false;
        match idx {
            0 => {
                self.clear_canvas(ctx);
                self.doc_name = String::from("untitled");
                self.bg = None;
                self.edit = EditTarget::Document;
                ctx.invalidate_all();
            }
            1 => {
                match self.edit {
                    EditTarget::Icon(i) => {
                        if self.save_icon_to_db(i) {
                            eprintln!(
                                "draw: saved launcher icon {}",
                                soul_script::app_list()
                                    .get(i)
                                    .map(|e| e.name.as_str())
                                    .unwrap_or("?")
                            );
                        }
                    }
                    EditTarget::Document => {
                        let _ = self.try_save_to_path(&self.path_for_doc());
                    }
                }
                ctx.invalidate_all();
            }
            2 => {
                let mut input = TextInput::with_placeholder(Self::rect_save_as_input(), "name");
                let _ = input.set_text(self.doc_name.clone());
                self.mode = Mode::SaveAs(input);
                ctx.invalidate_all();
            }
            3 => {
                self.refresh_open_list(OpenPurpose::Document);
                ctx.invalidate_all();
            }
            4 => {
                self.refresh_open_icon_list();
                ctx.invalidate_all();
            }
            5 => {
                self.refresh_open_list(OpenPurpose::Background);
                ctx.invalidate_all();
            }
            6 => {
                self.clear_background(ctx);
                ctx.invalidate_all();
            }
            7 => {
                self.builder_mode = !self.builder_mode;
                ctx.invalidate_all();
            }
            8 => {
                self.edit_overlay.delete_selected(&mut self.ui_form);
                self.persist_ui();
                ctx.invalidate_all();
            }
            9 => {
                self.ui_form = Self::default_draw_ui();
                self.persist_ui();
                ctx.invalidate_all();
            }
            _ => {
                ctx.invalidate_all();
            }
        }
    }

    fn open_pick(&mut self, stem: &str, ctx: &mut Ctx<'_>) {
        let purpose = match &self.mode {
            Mode::OpenList { purpose, .. } => *purpose,
            _ => OpenPurpose::Document,
        };
        match purpose {
            OpenPurpose::Document => {
                let path = path_for(&self.draw_dir, stem);
                self.doc_name = stem.to_string();
                let _ = self.try_load_doc_path(&path, ctx);
            }
            OpenPurpose::Background => {
                let path = path_for(&self.draw_dir, stem);
                let _ = self.try_load_background_path(&path, ctx);
            }
            OpenPurpose::LauncherIcon => {
                if let Some(idx) = soul_script::app_list().iter().position(|e| e.name == stem) {
                    let _ = self.load_icon_from_db(idx, ctx);
                }
            }
        }
        self.mode = Mode::Normal;
        ctx.invalidate_all();
    }

    fn paint_zone_at(&self, x: i16, y: i16) -> PaintTarget {
        if self.screen_to_cell(x, y).is_some() {
            return PaintTarget::Canvas;
        }

        if let Some(comp) = self.ui_form.hit_test(x, y) {
            match comp.id.as_str() {
                "tool_brush" => return PaintTarget::ToolBrush,
                "tool_fill" => return PaintTarget::ToolFill,
                "tool_eraser" => return PaintTarget::ToolEraser,
                "brush_minus" => return PaintTarget::BrushMinus,
                "brush_plus" => return PaintTarget::BrushPlus,
                "undo" => return PaintTarget::UndoBtn,
                "clear" => return PaintTarget::ClearBtn,
                id if id.starts_with("ink_") => {
                    if let Ok(i) = id[4..].parse::<usize>() {
                        return PaintTarget::Ink(i);
                    }
                }
                _ => {}
            }
        }

        PaintTarget::None
    }

    fn handle_menu_pen(
        &mut self,
        down: bool,
        move_: bool,
        up: bool,
        x: i16,
        y: i16,
        ctx: &mut Ctx<'_>,
    ) {
        if down {
            self.menu_touch =
                (0..MENU_ITEMS.len()).find(|&i| hit_test(&Self::rect_menu_entry(i), x, y));
            if self.menu_touch.is_some() {
                ctx.invalidate(Rectangle::new(Point::new(16, 48), Size::new(208, 240)));
            }
        } else if move_ {
        } else if up {
            let end = (0..MENU_ITEMS.len()).find(|&i| hit_test(&Self::rect_menu_entry(i), x, y));
            if self.menu_touch.is_some() && end == self.menu_touch {
                if let Some(i) = end {
                    self.menu_action(i, ctx);
                }
            }
            self.menu_touch = None;
        }
    }

    fn handle_save_as_pen(&mut self, down: bool, up: bool, x: i16, y: i16, ctx: &mut Ctx<'_>) {
        let Mode::SaveAs(ref mut input) = &mut self.mode else {
            return;
        };
        if down {
            if hit_test(&Self::rect_save_as_ok(), x, y)
                || hit_test(&Self::rect_save_as_cancel(), x, y)
            {
                return;
            }
            if input.contains(x, y) {
                let _ = input.pen_released(x, y);
                ctx.invalidate(input.area());
            }
        } else if up {
            if hit_test(&Self::rect_save_as_ok(), x, y) {
                self.commit_save_as(ctx);
            } else if hit_test(&Self::rect_save_as_cancel(), x, y) {
                self.mode = Mode::Normal;
                ctx.invalidate_all();
            }
        }
    }

    fn handle_open_pen(&mut self, down: bool, up: bool, x: i16, y: i16, ctx: &mut Ctx<'_>) {
        if !matches!(self.mode, Mode::OpenList { .. }) {
            return;
        }
        if down {
            return;
        }
        if !up {
            return;
        }
        if hit_test(&Self::rect_open_cancel(), x, y) {
            self.mode = Mode::Normal;
            ctx.invalidate_all();
            return;
        }
        let Mode::OpenList { files, scroll, .. } = &mut self.mode else {
            return;
        };
        let visible = OPEN_VISIBLE.min(files.len().saturating_sub(*scroll));
        for i in 0..visible {
            if hit_test(&Self::rect_open_row(i), x, y) {
                let idx = *scroll + i;
                if let Some(stem) = files.get(idx).cloned() {
                    self.open_pick(&stem, ctx);
                }
                return;
            }
        }
    }
}

impl App for Draw {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::Menu => match &self.mode {
                Mode::SaveAs(_) | Mode::OpenList { .. } => {
                    self.cancel_modal(ctx);
                }
                Mode::Normal => {
                    self.menu_open = !self.menu_open;
                    ctx.invalidate_all();
                }
            },
            Event::Key(KeyCode::Char(c)) => {
                if let Mode::SaveAs(ref mut input) = self.mode {
                    let out = input.insert_char(c);
                    self.apply_text_out(out, ctx);
                }
            }
            Event::Key(KeyCode::Backspace) => {
                if let Mode::SaveAs(ref mut input) = self.mode {
                    let out = input.backspace();
                    self.apply_text_out(out, ctx);
                }
            }
            Event::Key(KeyCode::Enter) => {
                if let Mode::SaveAs(ref mut input) = self.mode {
                    let out = input.enter();
                    self.apply_text_out(out, ctx);
                }
            }
            Event::Key(KeyCode::ArrowLeft) => {
                if let Mode::SaveAs(ref mut input) = self.mode {
                    if let Some(r) = input.cursor_left() {
                        ctx.invalidate(r);
                    }
                }
            }
            Event::Key(KeyCode::ArrowRight) => {
                if let Mode::SaveAs(ref mut input) = self.mode {
                    if let Some(r) = input.cursor_right() {
                        ctx.invalidate(r);
                    }
                }
            }
            Event::ButtonDown(HardButton::PageUp) => {
                if let Mode::OpenList { scroll, .. } = &mut self.mode {
                    *scroll = scroll.saturating_sub(1);
                    ctx.invalidate(Rectangle::new(Point::new(8, 44), Size::new(224, 200)));
                }
            }
            Event::ButtonDown(HardButton::PageDown) => {
                if let Mode::OpenList { scroll, files, .. } = &mut self.mode {
                    let max_scroll = files.len().saturating_sub(OPEN_VISIBLE);
                    *scroll = (*scroll + 1).min(max_scroll);
                    ctx.invalidate(Rectangle::new(Point::new(8, 44), Size::new(224, 200)));
                }
            }
            Event::PenDown { x, y } => match &mut self.mode {
                Mode::SaveAs(_) => self.handle_save_as_pen(true, false, x, y, ctx),
                Mode::OpenList { .. } => self.handle_open_pen(true, false, x, y, ctx),
                Mode::Normal => {
                    if self.builder_mode {
                        if self.edit_overlay.pen_down(&self.ui_form, x, y) {
                            ctx.invalidate_all();
                            return;
                        }
                    }
                    if self.menu_open {
                        self.handle_menu_pen(true, false, false, x, y, ctx);
                        return;
                    }
                    let z = self.paint_zone_at(x, y);
                    self.paint_touch = z;
                    match z {
                        PaintTarget::Canvas => {
                            if let Some((lx, ly)) = self.screen_to_cell(x, y) {
                                match self.tool {
                                    Tool::Brush => {
                                        self.push_undo();
                                        self.last_cell = Some((lx, ly));
                                        self.stamp(lx as i32, ly as i32, ctx);
                                    }
                                    Tool::Eraser => {
                                        self.push_undo();
                                        self.last_cell = Some((lx, ly));
                                        self.erase_stamp(lx as i32, ly as i32, ctx);
                                    }
                                    Tool::Fill => {
                                        self.flood_fill(lx, ly, ctx);
                                    }
                                }
                            }
                        }
                        PaintTarget::Ink(i) => {
                            self.brush = GRAY_LEVELS[i];
                            ctx.invalidate(Rectangle::new(
                                Point::new(0, ROW2_Y),
                                Size::new(SCREEN_WIDTH as u32, 24),
                            ));
                        }
                        PaintTarget::BrushMinus
                        | PaintTarget::BrushPlus
                        | PaintTarget::ClearBtn
                        | PaintTarget::UndoBtn
                        | PaintTarget::ToolBrush
                        | PaintTarget::ToolFill
                        | PaintTarget::ToolEraser => {}
                        PaintTarget::None => {}
                    }
                }
            },
            Event::PenMove { x, y } => {
                if self.builder_mode && matches!(self.mode, Mode::Normal) && !self.menu_open {
                    if self.edit_overlay.pen_move(&mut self.ui_form, x, y) {
                        ctx.invalidate_all();
                        return;
                    }
                }
                if matches!(self.mode, Mode::Normal) && self.menu_open {
                    return;
                }
                if !matches!(self.mode, Mode::Normal) {
                    return;
                }
                if self.paint_touch == PaintTarget::Canvas
                    && matches!(self.tool, Tool::Brush | Tool::Eraser)
                {
                    if let Some((lx, ly)) = self.screen_to_cell(x, y) {
                        if let Some((ox, oy)) = self.last_cell {
                            if (ox, oy) != (lx, ly) {
                                self.plot_line(ox, oy, lx, ly, ctx);
                            }
                        } else {
                            self.paint_at(lx as i32, ly as i32, ctx);
                        }
                        self.last_cell = Some((lx, ly));
                    }
                }
            }
            Event::PenUp { x, y } => match &mut self.mode {
                Mode::SaveAs(_) => self.handle_save_as_pen(false, true, x, y, ctx),
                Mode::OpenList { .. } => self.handle_open_pen(false, true, x, y, ctx),
                Mode::Normal => {
                    if self.builder_mode {
                        self.edit_overlay.pen_up();
                        self.persist_ui();
                        ctx.invalidate_all();
                    }
                    if self.menu_open {
                        self.handle_menu_pen(false, false, true, x, y, ctx);
                    } else {
                        let end = self.paint_zone_at(x, y);
                        if self.paint_touch == end {
                            match end {
                                PaintTarget::ToolBrush => {
                                    self.tool = Tool::Brush;
                                    ctx.invalidate(Rectangle::new(
                                        Point::new(0, ROW1_Y),
                                        Size::new(80, 24),
                                    ));
                                }
                                PaintTarget::ToolFill => {
                                    self.tool = Tool::Fill;
                                    ctx.invalidate(Rectangle::new(
                                        Point::new(0, ROW1_Y),
                                        Size::new(120, 24),
                                    ));
                                }
                                PaintTarget::ToolEraser => {
                                    self.tool = Tool::Eraser;
                                    ctx.invalidate(Rectangle::new(
                                        Point::new(0, ROW1_Y),
                                        Size::new(120, 24),
                                    ));
                                }
                                PaintTarget::UndoBtn => {
                                    self.pop_undo(ctx);
                                }
                                PaintTarget::BrushMinus => {
                                    self.brush_radius =
                                        (self.brush_radius - 1).max(BRUSH_RADIUS_MIN);
                                    ctx.invalidate(Rectangle::new(
                                        Point::new(70, ROW1_Y),
                                        Size::new(80, 28),
                                    ));
                                }
                                PaintTarget::BrushPlus => {
                                    self.brush_radius =
                                        (self.brush_radius + 1).min(BRUSH_RADIUS_MAX);
                                    ctx.invalidate(Rectangle::new(
                                        Point::new(70, ROW1_Y),
                                        Size::new(80, 28),
                                    ));
                                }
                                PaintTarget::ClearBtn => self.clear_canvas(ctx),
                                _ => {}
                            }
                        }
                    }
                    self.paint_touch = PaintTarget::None;
                    self.last_cell = None;
                }
            },
            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let title = format!("Draw · {}", truncate_name(&self.doc_name, 22));
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, &title);

        let r = Self::canvas_screen_rect();
        let border = PrimitiveStyle::with_stroke(BLACK, 1);
        let _ = r.into_styled(border).draw(canvas);

        for ly in 0..LOG_H {
            for lx in 0..LOG_W {
                let i = ly * LOG_W + lx;
                let v = self.display_value(i);
                let px = Rectangle::new(
                    Point::new(
                        (lx as i32) * SCALE,
                        TITLE_BAR_H as i32 + (ly as i32) * SCALE,
                    ),
                    Size::new(SCALE as u32, SCALE as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(Gray8::new(v)));
                let _ = px.draw(canvas);
            }
        }

        if matches!(self.edit, EditTarget::Icon(_)) {
            let ir = Rectangle::new(
                Point::new(
                    (ICON_OX as i32) * SCALE,
                    TITLE_BAR_H as i32 + (ICON_OY as i32) * SCALE,
                ),
                Size::new(
                    (ICON_CELL as u32) * (SCALE as u32),
                    (ICON_CELL as u32) * (SCALE as u32),
                ),
            );
            let _ = ir
                .into_styled(
                    PrimitiveStyleBuilder::new()
                        .stroke_color(BLACK)
                        .stroke_width(1)
                        .build(),
                )
                .draw(canvas);
        }

        // Draw the dynamic UI form
        let pressed_id = match self.paint_touch {
            PaintTarget::ToolBrush => Some("tool_brush"),
            PaintTarget::ToolFill => Some("tool_fill"),
            PaintTarget::ToolEraser => Some("tool_eraser"),
            PaintTarget::BrushMinus => Some("brush_minus"),
            PaintTarget::BrushPlus => Some("brush_plus"),
            PaintTarget::UndoBtn => Some("undo"),
            PaintTarget::ClearBtn => Some("clear"),
            PaintTarget::Ink(0) => Some("ink_0"),
            PaintTarget::Ink(1) => Some("ink_1"),
            PaintTarget::Ink(2) => Some("ink_2"),
            PaintTarget::Ink(3) => Some("ink_3"),
            PaintTarget::Ink(4) => Some("ink_4"),
            PaintTarget::Ink(5) => Some("ink_5"),
            PaintTarget::Ink(6) => Some("ink_6"),
            PaintTarget::Ink(7) => Some("ink_7"),
            _ => None,
        };
        let _ = self.ui_form.draw(canvas, pressed_id);

        // Draw selection highlight for current gray level
        for (i, g) in GRAY_LEVELS.iter().enumerate() {
            if self.brush == *g {
                if let Some(comp) = self
                    .ui_form
                    .components
                    .iter()
                    .find(|c| c.id == format!("ink_{}", i))
                {
                    let rect = comp.bounds.to_eg_rect();
                    let _ = rect
                        .into_styled(PrimitiveStyle::with_stroke(BLACK, 2))
                        .draw(canvas);
                }
            }
        }

        // Overlay brush radius label
        if let Some(comp) = self
            .ui_form
            .components
            .iter()
            .find(|c| c.id == "brush_minus")
        {
            let _ = label(
                canvas,
                Point::new(comp.bounds.x + comp.bounds.w as i32 + 6, comp.bounds.y + 4),
                &format!("{}", self.brush_radius),
            );
        }

        if self.builder_mode {
            let _ = self.edit_overlay.draw(canvas, &self.ui_form);
        }

        match &self.mode {
            Mode::SaveAs(input) => {
                let _ = Self::app_content_rect()
                    .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
                    .draw(canvas);
                let _ = label(canvas, Point::new(12, 76), "Save as (stem only):");
                let _ = input.draw(canvas);
                let _ = button(canvas, Self::rect_save_as_ok(), "OK", false);
                let _ = button(canvas, Self::rect_save_as_cancel(), "Cancel", false);
            }
            Mode::OpenList {
                files,
                scroll,
                purpose,
            } => {
                let _ = Self::app_content_rect()
                    .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
                    .draw(canvas);
                let hdr = match purpose {
                    OpenPurpose::Document => "Open",
                    OpenPurpose::Background => "Background (PGM)",
                    OpenPurpose::LauncherIcon => "Launcher icon",
                };
                let _ = label(canvas, Point::new(12, 28), hdr);
                if files.is_empty() {
                    let _ = label(canvas, Point::new(16, 56), "No .pgm files in folder.");
                } else {
                    let pg = format!(
                        "{}-{} / {}",
                        *scroll + 1,
                        (*scroll + OPEN_VISIBLE).min(files.len()),
                        files.len()
                    );
                    let _ = label(canvas, Point::new(120, 28), &pg);
                    let visible = OPEN_VISIBLE.min(files.len().saturating_sub(*scroll));
                    for i in 0..visible {
                        let idx = *scroll + i;
                        if let Some(name) = files.get(idx) {
                            let _ = button(
                                canvas,
                                Self::rect_open_row(i),
                                &truncate_name(name, 28),
                                false,
                            );
                        }
                    }
                }
                let _ = label(canvas, Point::new(12, 200), "PgUp / PgDn scroll");
                let _ = button(canvas, Self::rect_open_cancel(), "Cancel", false);
            }
            Mode::Normal => {
                if self.menu_open {
                    let rect = Rectangle::new(Point::new(10, 30), Size::new(220, 240));
                    let _ = rect
                        .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
                        .draw(canvas);
                    let _ = rect
                        .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
                        .draw(canvas);
                    let _ = label(canvas, Point::new(15, 38), "Menu");
                    for i in 0..MENU_ITEMS.len() {
                        let pressed = self.menu_touch == Some(i);
                        let _ = button(canvas, Self::rect_menu_entry(i), MENU_ITEMS[i], pressed);
                    }
                }
            }
        }
    }

    fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        let mut nodes = self.ui_form.a11y_nodes();
        match &self.mode {
            Mode::Normal => {
                if self.menu_open {
                    for (i, item) in MENU_ITEMS.iter().enumerate() {
                        nodes.push(soul_core::a11y::A11yNode {
                            bounds: Self::rect_menu_entry(i),
                            label: item.to_string(),
                            role: "menuitem".into(),
                        });
                    }
                }
            }
            Mode::SaveAs(_) => {
                nodes.push(soul_core::a11y::A11yNode {
                    bounds: Self::rect_save_as_input(),
                    label: "Filename input".into(),
                    role: "textinput".into(),
                });
            }
            Mode::OpenList { files, scroll, .. } => {
                let visible = OPEN_VISIBLE.min(files.len().saturating_sub(*scroll));
                for i in 0..visible {
                    let idx = *scroll + i;
                    if let Some(name) = files.get(idx) {
                        nodes.push(soul_core::a11y::A11yNode {
                            bounds: Self::rect_open_row(i),
                            label: name.clone(),
                            role: "file".into(),
                        });
                    }
                }
            }
        }
        nodes
    }
}

fn scale_image_to_canvas(
    data: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    let mut result = vec![255u8; dst_w * dst_h];

    // Calculate scale factor to fit image within canvas while maintaining aspect ratio
    let scale_x = dst_w as f32 / src_w as f32;
    let scale_y = dst_h as f32 / src_h as f32;
    let scale = scale_x.min(scale_y); // Use the smaller scale to ensure it fits

    let scaled_w = (src_w as f32 * scale) as usize;
    let scaled_h = (src_h as f32 * scale) as usize;

    // Center the scaled image in the canvas
    let offset_x = (dst_w - scaled_w) / 2;
    let offset_y = (dst_h - scaled_h) / 2;

    for dy in 0..scaled_h {
        for dx in 0..scaled_w {
            let src_x = (dx as f32 / scale) as usize;
            let src_y = (dy as f32 / scale) as usize;

            if src_x < src_w && src_y < src_h {
                let src_idx = src_y * src_w + src_x;
                let dst_x = offset_x + dx;
                let dst_y = offset_y + dy;

                if dst_x < dst_w && dst_y < dst_h {
                    let dst_idx = dst_y * dst_w + dst_x;
                    result[dst_idx] = data[src_idx];
                }
            }
        }
    }

    result
}

fn faint_background(b: u8) -> u8 {
    let x = b as u16;
    ((x * 85 + 255 * 170) / 255) as u8
}

fn path_for(dir: &Path, stem: &str) -> PathBuf {
    dir.join(format!("{stem}.pgm"))
}

fn sanitize_name(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.len() > 64 {
        return None;
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    Some(t.to_string())
}

fn truncate_name(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars()
            .take(max_chars.saturating_sub(1))
            .collect::<String>()
            + "..."
    }
}

fn list_pgm_stems(dir: &Path) -> io::Result<Vec<String>> {
    let mut v = Vec::new();
    if !dir.exists() {
        return Ok(v);
    }
    for ent in fs::read_dir(dir)? {
        let ent = ent?;
        let p = ent.path();
        if p.extension().and_then(|e| e.to_str()) == Some("pgm") {
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                v.push(stem.to_string());
            }
        }
    }
    v.sort();
    Ok(v)
}

fn load_pgm(path: &Path) -> io::Result<(usize, usize, Vec<u8>)> {
    let f = File::open(path)?;
    let mut r = BufReader::new(f);
    let mut line = String::new();
    r.read_line(&mut line)?;
    if line.trim() != "P5" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected P5 PGM",
        ));
    }
    let (w, h) = read_pgm_whitespace_line(&mut r)?;
    let maxv = read_pgm_whitespace_line_value(&mut r)?;
    if maxv != 255 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "only maxval 255 is supported",
        ));
    }
    let mut pixels = vec![0u8; w * h];
    r.read_exact(&mut pixels)?;
    Ok((w, h, pixels))
}

fn read_pgm_whitespace_line<R: BufRead>(r: &mut R) -> io::Result<(usize, usize)> {
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "pgm header"));
        }
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.split_whitespace();
        let w: usize = it
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "width"))?
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let h: usize = it
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "height"))?
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        return Ok((w, h));
    }
}

fn read_pgm_whitespace_line_value<R: BufRead>(r: &mut R) -> io::Result<u32> {
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "pgm maxval"));
        }
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let v: u32 = t
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        return Ok(v);
    }
}

fn save_pgm(path: &Path, w: usize, h: usize, pixels: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = File::create(path)?;
    writeln!(f, "P5")?;
    writeln!(f, "{w} {h}")?;
    writeln!(f, "255")?;
    f.write_all(pixels)?;
    Ok(())
}

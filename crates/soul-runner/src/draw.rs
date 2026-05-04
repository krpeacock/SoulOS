//! Pixel editor — database-centric, no filesystem access.
//!
//! Canvases are stored as records in the Draw app's own [`soul_db::Database`].
//! Category 0 holds canvas images (each record = name + pixel data).
//! Category 1 holds the MobileBuilder UI form JSON (single record).
//!
//! App icons are imported/exported through the Exchange layer (background calls
//! to the Launcher), keeping Draw decoupled from other apps' storage.

use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
};
use soul_core::{App, Ctx, Event, HardButton, KeyCode, APP_HEIGHT, SCREEN_WIDTH};
use soul_script::SystemRequest;
use soul_ui::{button, hit_test, label, title_bar, TextInput, TextInputOutput, BLACK, TITLE_BAR_H};
use std::collections::VecDeque;
use std::path::PathBuf;

use crate::ICON_CELL;

const LOG_W: usize = 48;
const LOG_H: usize = 48;
const ICON_OX: usize = (LOG_W - ICON_CELL as usize) / 2;
const ICON_OY: usize = (LOG_H - ICON_CELL as usize) / 2;
const SCALE: i32 = 5;
const CANVAS_PX: i32 = (LOG_W as i32) * SCALE;

/// Eight evenly spaced levels from black to white.
pub const GRAY_LEVELS: [u8; 8] = [0, 36, 73, 109, 146, 182, 218, 255];

const ROW1_Y: i32 = TITLE_BAR_H as i32 + CANVAS_PX;
const ROW2_Y: i32 = ROW1_Y + 26;

const BRUSH_RADIUS_MIN: i32 = 0;
const BRUSH_RADIUS_MAX: i32 = 3;

/// Category 0: canvas image records.
const CAT_CANVAS: u8 = 0;
/// Category 1: UI form JSON (single record).
const CAT_FORM: u8 = 1;

const MENU_ITEMS: &[&str] = &[
    "New",
    "Save",
    "Name...",
    "Gallery",
    "Done",      // returns edited bitmap to Builder; no-op when not in exchange mode
    "Load bg",
    "Clear bg",
    "Edit Layout",
    "Delete Elem",
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum GalleryPurpose {
    Open,
    Background,
}

enum Mode {
    Normal,
    NameInput(TextInput),
    Gallery {
        records: Vec<(u32, String)>,
        scroll: usize,
        purpose: GalleryPurpose,
    },
}

pub struct Draw {
    db: soul_db::Database,
    db_path: PathBuf,
    current_record: Option<u32>,
    current_name: String,
    /// True when the canvas holds an icon-sized image for editing.
    icon_mode: bool,
    /// When set, "Done" in the menu returns a SendResult with this action name
    /// (e.g. "return_bitmap") back to the app that opened Draw via exchange.
    return_action: Option<String>,
    fg: Vec<u8>,
    written: Vec<bool>,
    bg: Option<Vec<u8>>,
    brush: u8,
    brush_radius: i32,
    tool: Tool,
    paint_touch: PaintTarget,
    last_cell: Option<(usize, usize)>,
    menu_open: bool,
    menu_touch: Option<usize>,
    mode: Mode,
    undo_stack: Vec<(Vec<u8>, Vec<bool>)>,
    ui_form: soul_ui::Form,
    edit_overlay: soul_ui::EditOverlay,
    builder_mode: bool,
}

impl Draw {
    pub const APP_ID: &'static str = "com.soulos.draw";
    pub const NAME: &'static str = "Draw";

    pub fn new(db_path: PathBuf) -> Self {
        let (db, current_record) = load_db(&db_path);
        let (fg, written, current_name) = load_first_canvas(&db);
        let ui_form = load_form(&db);
        Self {
            db,
            db_path,
            current_record,
            current_name,
            icon_mode: false,
            return_action: None,
            fg,
            written,
            bg: None,
            brush: GRAY_LEVELS[0],
            brush_radius: BRUSH_RADIUS_MIN,
            tool: Tool::Brush,
            paint_touch: PaintTarget::None,
            last_cell: None,
            menu_open: false,
            menu_touch: None,
            mode: Mode::Normal,
            undo_stack: Vec::new(),
            ui_form,
            edit_overlay: soul_ui::EditOverlay::new(),
            builder_mode: false,
        }
    }

    /// Persist the database to disk (called on `AppStop`).
    pub fn persist(&mut self) {
        // Autosave current canvas if it's modified.
        self.save_canvas();
        save_db(&self.db, &self.db_path);
    }

    // --- Canvas DB helpers -----------------------------------------------

    fn save_canvas(&mut self) {
        let flat = self.flatten_for_save();
        let data = encode_canvas(&self.current_name, &flat);
        match self.current_record {
            Some(id) => {
                self.db.update(id, data);
            }
            None => {
                let id = self.db.insert(CAT_CANVAS, data);
                self.current_record = Some(id);
            }
        }
    }

    fn list_gallery(&self) -> Vec<(u32, String)> {
        self.db
            .iter_category(CAT_CANVAS)
            .map(|r| (r.id, decode_canvas_name(&r.data)))
            .collect()
    }

    fn load_canvas_record(&mut self, id: u32, purpose: GalleryPurpose, ctx: &mut Ctx<'_>) {
        let Some(rec) = self.db.get(id) else { return };
        let Some(pixels) = decode_canvas_pixels(&rec.data) else { return };
        match purpose {
            GalleryPurpose::Open => {
                self.undo_stack.clear();
                self.fg = pixels;
                self.written = vec![true; LOG_W * LOG_H];
                self.bg = None;
                self.current_record = Some(id);
                self.current_name = decode_canvas_name(&rec.data.clone());
                self.icon_mode = false;
                ctx.invalidate(Self::canvas_screen_rect());
            }
            GalleryPurpose::Background => {
                self.bg = Some(pixels);
                ctx.invalidate(Self::canvas_screen_rect());
            }
        }
    }

    fn delete_canvas_record(&mut self, id: u32, ctx: &mut Ctx<'_>) {
        self.db.delete(id);
        if self.current_record == Some(id) {
            self.current_record = None;
            self.current_name = "untitled".to_string();
            self.fg.fill(255);
            self.written.fill(false);
            self.icon_mode = false;
            ctx.invalidate(Self::canvas_screen_rect());
        }
    }

    // --- Exchange helpers -------------------------------------------------

    fn handle_exchange(&mut self, action: &str, payload: soul_core::ExchangePayload, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match action {
            // Builder opened Draw for icon editing and supplied the pixels directly.
            "open_bitmap" => {
                if let soul_core::ExchangePayload::Bitmap { width, height, ref pixels } = payload {
                    let w = width as usize;
                    let h = height as usize;
                    if !pixels.is_empty() && w > 0 && h > 0 {
                        self.load_icon_pixels(pixels, w, h, ctx);
                    }
                }
                // Mark that "Done" should return the result to the caller.
                self.return_action = Some("return_bitmap".to_string());
                None
            }
            _ => None,
        }
    }

    fn load_icon_pixels(&mut self, pixels: &[u8], w: usize, h: usize, ctx: &mut Ctx<'_>) {
        self.undo_stack.clear();
        self.fg.fill(255);
        self.written.fill(false);
        self.bg = None;
        let cell = ICON_CELL as usize;
        // Center the icon in the canvas; scale if needed.
        let data = if w == cell && h == cell {
            pixels.to_vec()
        } else {
            scale_image_to_canvas(pixels, w, h, cell, cell)
        };
        for y in 0..cell {
            for x in 0..cell {
                let i = (ICON_OY + y) * LOG_W + (ICON_OX + x);
                let p = data[y * cell + x];
                self.fg[i] = p;
                self.written[i] = p != 255;
            }
        }
        self.icon_mode = true;
        self.current_record = None;
        self.current_name = "icon".to_string();
        // Auto-save immediately — Palm principle: data is never lost on switch.
        self.save_canvas();
        save_db(&self.db, &self.db_path);
        ctx.invalidate(Self::canvas_screen_rect());
    }

    // --- Geometry --------------------------------------------------------

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
        if self.icon_mode {
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

    fn rect_name_input() -> Rectangle {
        Rectangle::new(Point::new(16, 98), Size::new(208, 20))
    }

    fn rect_name_ok() -> Rectangle {
        Rectangle::new(Point::new(24, 130), Size::new(80, 28))
    }

    fn rect_name_cancel() -> Rectangle {
        Rectangle::new(Point::new(120, 130), Size::new(96, 28))
    }

    fn rect_gallery_row(i: usize) -> Rectangle {
        Rectangle::new(
            Point::new(16, 52 + i as i32 * OPEN_ROW_H),
            Size::new(208, (OPEN_ROW_H - 2) as u32),
        )
    }

    fn rect_gallery_cancel() -> Rectangle {
        Rectangle::new(Point::new(70, 226), Size::new(100, 28))
    }

    fn app_content_rect() -> Rectangle {
        let h = (APP_HEIGHT as u32).saturating_sub(TITLE_BAR_H);
        Rectangle::new(
            Point::new(0, TITLE_BAR_H as i32),
            Size::new(SCREEN_WIDTH as u32, h),
        )
    }

    // --- Painting --------------------------------------------------------

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
                if self.icon_mode {
                    let cell = ICON_CELL as usize;
                    if lx < ICON_OX || lx >= ICON_OX + cell || ly < ICON_OY || ly >= ICON_OY + cell {
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
                if self.icon_mode {
                    let cell = ICON_CELL as usize;
                    if lx < ICON_OX || lx >= ICON_OX + cell || ly < ICON_OY || ly >= ICON_OY + cell {
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
            Tool::Fill => {}
        }
    }

    fn flood_fill(&mut self, sx: usize, sy: usize, ctx: &mut Ctx<'_>) {
        let target = self.display_value(sy * LOG_W + sx);
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
                let ni = y * LOG_W + (x - 1);
                if !seen[ni] && self.display_value(ni) == target {
                    q.push_back((x - 1, y));
                }
            }
            if x + 1 < LOG_W {
                let ni = y * LOG_W + (x + 1);
                if !seen[ni] && self.display_value(ni) == target {
                    q.push_back((x + 1, y));
                }
            }
            if y > 0 {
                let ni = (y - 1) * LOG_W + x;
                if !seen[ni] && self.display_value(ni) == target {
                    q.push_back((x, y - 1));
                }
            }
            if y + 1 < LOG_H {
                let ni = (y + 1) * LOG_W + x;
                if !seen[ni] && self.display_value(ni) == target {
                    q.push_back((x, y + 1));
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
        let mut err = dx - dy;
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

    // --- Persist UI form -------------------------------------------------

    fn persist_form(&mut self) {
        let json = self.ui_form.to_json().into_bytes();
        let existing_id = self.db.iter_category(CAT_FORM).next().map(|r| r.id);
        if let Some(id) = existing_id {
            self.db.update(id, json);
        } else {
            self.db.insert(CAT_FORM, json);
        }
    }

    // --- Menu / modal helpers --------------------------------------------

    fn menu_action(&mut self, idx: usize, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        self.menu_open = false;
        match idx {
            0 => {
                // New
                self.clear_canvas(ctx);
                self.current_record = None;
                self.current_name = "untitled".to_string();
                self.bg = None;
                self.icon_mode = false;
                self.return_action = None;
                ctx.invalidate_all();
                None
            }
            1 => {
                // Save
                self.save_canvas();
                save_db(&self.db, &self.db_path);
                ctx.invalidate_all();
                None
            }
            2 => {
                // Name...
                let mut input = TextInput::with_placeholder(Self::rect_name_input(), "name");
                let _ = input.set_text(self.current_name.clone());
                self.mode = Mode::NameInput(input);
                ctx.invalidate_all();
                None
            }
            3 => {
                // Gallery
                let records = self.list_gallery();
                self.mode = Mode::Gallery {
                    records,
                    scroll: 0,
                    purpose: GalleryPurpose::Open,
                };
                ctx.invalidate_all();
                None
            }
            4 => {
                // Done — return edited bitmap to the app that opened Draw via exchange.
                if let Some(action) = self.return_action.take() {
                    // Extract just the icon cell from the canvas (fg is the full
                    // LOG_W×LOG_H buffer; the icon lives at [ICON_OY..][ICON_OX..]).
                    let cell = ICON_CELL as usize;
                    let mut buf = Vec::with_capacity(cell * cell);
                    for y in 0..cell {
                        for x in 0..cell {
                            let i = (ICON_OY + y) * LOG_W + (ICON_OX + x);
                            buf.push(self.display_value(i));
                        }
                    }
                    return Some(soul_script::SystemRequest::SendResult {
                        action,
                        payload: soul_core::ExchangePayload::Bitmap {
                            width: ICON_CELL as u16,
                            height: ICON_CELL as u16,
                            pixels: buf,
                        },
                    });
                }
                ctx.invalidate_all();
                None
            }
            5 => {
                // Load bg — open gallery as background picker
                let records = self.list_gallery();
                self.mode = Mode::Gallery {
                    records,
                    scroll: 0,
                    purpose: GalleryPurpose::Background,
                };
                ctx.invalidate_all();
                None
            }
            6 => {
                self.clear_background(ctx);
                ctx.invalidate_all();
                None
            }
            7 => {
                self.builder_mode = !self.builder_mode;
                ctx.invalidate_all();
                None
            }
            8 => {
                self.edit_overlay.delete_selected(&mut self.ui_form);
                self.persist_form();
                ctx.invalidate_all();
                None
            }
            9 => {
                self.ui_form = Self::default_draw_ui();
                self.persist_form();
                ctx.invalidate_all();
                None
            }
            _ => {
                ctx.invalidate_all();
                None
            }
        }
    }

    fn apply_text_out(&mut self, out: TextInputOutput, ctx: &mut Ctx<'_>) {
        if let Some(r) = out.dirty {
            ctx.invalidate(r);
        }
        if out.submitted {
            self.commit_name(ctx);
        }
    }

    fn commit_name(&mut self, ctx: &mut Ctx<'_>) {
        let Mode::NameInput(input) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            return;
        };
        let raw = input.text().to_string();
        if let Some(name) = sanitize_name(&raw) {
            self.current_name = name;
        }
        ctx.invalidate_all();
    }

    fn cancel_modal(&mut self, ctx: &mut Ctx<'_>) {
        self.mode = Mode::Normal;
        self.menu_open = false;
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

    fn handle_menu_pen(&mut self, down: bool, move_: bool, up: bool, x: i16, y: i16, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        if down {
            self.menu_touch =
                (0..MENU_ITEMS.len()).find(|&i| hit_test(&Self::rect_menu_entry(i), x, y));
            if self.menu_touch.is_some() {
                ctx.invalidate(Rectangle::new(Point::new(16, 48), Size::new(208, 240)));
            }
            None
        } else if move_ {
            None
        } else if up {
            let end = (0..MENU_ITEMS.len()).find(|&i| hit_test(&Self::rect_menu_entry(i), x, y));
            let req = if self.menu_touch.is_some() && end == self.menu_touch {
                if let Some(i) = end {
                    self.menu_action(i, ctx)
                } else {
                    None
                }
            } else {
                None
            };
            self.menu_touch = None;
            req
        } else {
            None
        }
    }

    fn handle_name_pen(&mut self, down: bool, up: bool, x: i16, y: i16, ctx: &mut Ctx<'_>) {
        let Mode::NameInput(ref mut input) = &mut self.mode else { return };
        if down {
            if hit_test(&Self::rect_name_ok(), x, y)
                || hit_test(&Self::rect_name_cancel(), x, y)
            {
                return;
            }
            if input.contains(x, y) {
                let _ = input.pen_released(x, y);
                ctx.invalidate(input.area());
            }
        } else if up {
            if hit_test(&Self::rect_name_ok(), x, y) {
                self.commit_name(ctx);
            } else if hit_test(&Self::rect_name_cancel(), x, y) {
                self.mode = Mode::Normal;
                ctx.invalidate_all();
            }
        }
    }

    fn handle_gallery_pen(&mut self, down: bool, up: bool, x: i16, y: i16, ctx: &mut Ctx<'_>) {
        if !matches!(self.mode, Mode::Gallery { .. }) {
            return;
        }
        if down || !up {
            return;
        }
        if hit_test(&Self::rect_gallery_cancel(), x, y) {
            self.mode = Mode::Normal;
            ctx.invalidate_all();
            return;
        }
        let Mode::Gallery { records, scroll, purpose } = &mut self.mode else { return };
        let visible = OPEN_VISIBLE.min(records.len().saturating_sub(*scroll));
        for i in 0..visible {
            if hit_test(&Self::rect_gallery_row(i), x, y) {
                let idx = *scroll + i;
                if let Some(&(id, _)) = records.get(idx) {
                    let purpose = *purpose;
                    self.load_canvas_record(id, purpose, ctx);
                    self.mode = Mode::Normal;
                    ctx.invalidate_all();
                }
                return;
            }
        }
    }

    /// The main event handler — returns any system request to emit.
    pub fn handle_event(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match event {
            Event::Exchange { action, payload, .. } => {
                self.handle_exchange(&action, payload, ctx)
            }
            Event::Menu => {
                match &self.mode {
                    Mode::NameInput(_) | Mode::Gallery { .. } => self.cancel_modal(ctx),
                    Mode::Normal => {
                        self.menu_open = !self.menu_open;
                        ctx.invalidate_all();
                    }
                }
                None
            }
            Event::Key(KeyCode::Char(c)) => {
                if let Mode::NameInput(ref mut input) = self.mode {
                    let out = input.insert_char(c);
                    self.apply_text_out(out, ctx);
                }
                None
            }
            Event::Key(KeyCode::Backspace) => {
                if let Mode::NameInput(ref mut input) = self.mode {
                    let out = input.backspace();
                    self.apply_text_out(out, ctx);
                }
                None
            }
            Event::Key(KeyCode::Enter) => {
                if let Mode::NameInput(ref mut input) = self.mode {
                    let out = input.enter();
                    self.apply_text_out(out, ctx);
                }
                None
            }
            Event::Key(KeyCode::ArrowLeft) => {
                if let Mode::NameInput(ref mut input) = self.mode {
                    if let Some(r) = input.cursor_left() {
                        ctx.invalidate(r);
                    }
                }
                None
            }
            Event::Key(KeyCode::ArrowRight) => {
                if let Mode::NameInput(ref mut input) = self.mode {
                    if let Some(r) = input.cursor_right() {
                        ctx.invalidate(r);
                    }
                }
                None
            }
            Event::ButtonDown(HardButton::PageUp) => {
                if let Mode::Gallery { scroll, .. } = &mut self.mode {
                    *scroll = scroll.saturating_sub(1);
                    ctx.invalidate(Rectangle::new(Point::new(8, 44), Size::new(224, 200)));
                }
                None
            }
            Event::ButtonDown(HardButton::PageDown) => {
                if let Mode::Gallery { scroll, records, .. } = &mut self.mode {
                    let max_scroll = records.len().saturating_sub(OPEN_VISIBLE);
                    *scroll = (*scroll + 1).min(max_scroll);
                    ctx.invalidate(Rectangle::new(Point::new(8, 44), Size::new(224, 200)));
                }
                None
            }
            Event::PenDown { x, y } => match &mut self.mode {
                Mode::NameInput(_) => { self.handle_name_pen(true, false, x, y, ctx); None }
                Mode::Gallery { .. } => { self.handle_gallery_pen(true, false, x, y, ctx); None }
                Mode::Normal => {
                    if self.builder_mode
                        && self.edit_overlay.pen_down(&self.ui_form, x, y) {
                            ctx.invalidate_all();
                            return None;
                        }
                    if self.menu_open {
                        return self.handle_menu_pen(true, false, false, x, y, ctx);
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
                        _ => {}
                    }
                    None
                }
            },
            Event::PenMove { x, y } => {
                if self.builder_mode && matches!(self.mode, Mode::Normal) && !self.menu_open {
                    if self.edit_overlay.pen_move(&mut self.ui_form, x, y) {
                        ctx.invalidate_all();
                        return None;
                    }
                }
                if !matches!(self.mode, Mode::Normal) || self.menu_open {
                    return None;
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
                None
            }
            Event::PenUp { x, y } => match &mut self.mode {
                Mode::NameInput(_) => { self.handle_name_pen(false, true, x, y, ctx); None }
                Mode::Gallery { .. } => { self.handle_gallery_pen(false, true, x, y, ctx); None }
                Mode::Normal => {
                    if self.builder_mode {
                        self.edit_overlay.pen_up();
                        self.persist_form();
                        ctx.invalidate_all();
                    }
                    let req = if self.menu_open {
                        self.handle_menu_pen(false, false, true, x, y, ctx)
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
                                PaintTarget::UndoBtn => self.pop_undo(ctx),
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
                        None
                    };
                    self.paint_touch = PaintTarget::None;
                    self.last_cell = None;
                    req
                }
            },
            _ => None,
        }
    }

    // --- Default UI form -------------------------------------------------

    fn default_draw_ui() -> soul_ui::Form {
        use soul_ui::{A11yHints, Component, ComponentType, Form, Rect};
        use std::collections::BTreeMap;
        let mut form = Form::new("draw_ui");

        let row1_y = 15 + 240;
        let row2_y = row1_y + 26;

        for (id, label, x, w) in &[
            ("tool_brush", "Pen", 4i32, 32i32),
            ("tool_fill", "Fill", 40, 32),
            ("tool_eraser", "Erase", 76, 38),
        ] {
            form.components.push(Component {
                id: id.to_string(),
                class: "tool".into(),
                type_: ComponentType::Button,
                bounds: Rect { x: *x, y: row1_y + 2, w: *w as u32, h: 18 },
                properties: BTreeMap::from([("label".into(), (*label).into())]),
                a11y: A11yHints {
                    label: format!("{label} tool"),
                    role: "button".into(),
                },
                interactions: Vec::new(),
                binding: None,
            });
        }

        for (id, lbl, x, w) in &[
            ("brush_minus", "-", 118i32, 20i32),
            ("brush_plus", "+", 142, 20),
            ("undo", "Undo", 166, 36),
            ("clear", "Clear", 206, 34),
        ] {
            form.components.push(Component {
                id: id.to_string(),
                class: "action".into(),
                type_: ComponentType::Button,
                bounds: Rect { x: *x, y: row1_y + 2, w: *w as u32, h: 18 },
                properties: BTreeMap::from([("label".into(), (*lbl).into())]),
                a11y: A11yHints {
                    label: lbl.to_string(),
                    role: "button".into(),
                },
                interactions: Vec::new(),
                binding: None,
            });
        }

        for (i, g) in GRAY_LEVELS.iter().enumerate() {
            let x = 4 + (i as i32) * 28;
            form.components.push(Component {
                id: format!("ink_{}", i),
                class: "ink".into(),
                type_: ComponentType::Button,
                bounds: Rect { x, y: row2_y + 2, w: 24, h: 16 },
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
}

impl App for Draw {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        // Delegates to handle_event; System requests are routed via NativeKind.
        self.handle_event(event, ctx);
    }

    fn draw<D>(&mut self, canvas: &mut D, _dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let title = format!("Draw · {}", truncate_name(&self.current_name, 22));
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

        if self.icon_mode {
            let ir = Rectangle::new(
                Point::new(
                    (ICON_OX as i32) * SCALE,
                    TITLE_BAR_H as i32 + (ICON_OY as i32) * SCALE,
                ),
                Size::new(
                    ICON_CELL * (SCALE as u32),
                    ICON_CELL * (SCALE as u32),
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
                break;
            }
        }

        if let Some(comp) = self.ui_form.components.iter().find(|c| c.id == "brush_minus") {
            let _ = label(
                canvas,
                Point::new(comp.bounds.x + comp.bounds.w as i32 + 4, comp.bounds.y + 4),
                &format!("{}", self.brush_radius),
            );
        }

        if self.builder_mode {
            let _ = self.edit_overlay.draw(canvas, &self.ui_form);
        }

        match &self.mode {
            Mode::NameInput(input) => {
                let _ = Self::app_content_rect()
                    .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
                    .draw(canvas);
                let _ = label(canvas, Point::new(12, 76), "Canvas name:");
                let _ = input.draw(canvas);
                let _ = button(canvas, Self::rect_name_ok(), "OK", false);
                let _ = button(canvas, Self::rect_name_cancel(), "Cancel", false);
            }
            Mode::Gallery { records, scroll, purpose } => {
                let _ = Self::app_content_rect()
                    .into_styled(PrimitiveStyle::with_fill(Gray8::WHITE))
                    .draw(canvas);
                let hdr = match purpose {
                    GalleryPurpose::Open => "Open canvas",
                    GalleryPurpose::Background => "Load background",
                };
                let _ = label(canvas, Point::new(12, 28), hdr);
                if records.is_empty() {
                    let _ = label(canvas, Point::new(16, 56), "No saved canvases.");
                } else {
                    let pg = format!(
                        "{}-{} / {}",
                        *scroll + 1,
                        (*scroll + OPEN_VISIBLE).min(records.len()),
                        records.len()
                    );
                    let _ = label(canvas, Point::new(120, 28), &pg);
                    let visible = OPEN_VISIBLE.min(records.len().saturating_sub(*scroll));
                    for i in 0..visible {
                        let idx = *scroll + i;
                        if let Some((_, name)) = records.get(idx) {
                            let _ = button(
                                canvas,
                                Self::rect_gallery_row(i),
                                &truncate_name(name, 28),
                                false,
                            );
                        }
                    }
                }
                let _ = label(canvas, Point::new(12, 200), "PgUp / PgDn scroll");
                let _ = button(canvas, Self::rect_gallery_cancel(), "Cancel", false);
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
                    for (i, &item) in MENU_ITEMS.iter().enumerate() {
                        let pressed = self.menu_touch == Some(i);
                        let _ = button(canvas, Self::rect_menu_entry(i), item, pressed);
                    }
                }
            }
        }
    }

    fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        use soul_core::a11y::{A11yNode, A11yRole};
        let mut nodes = self.ui_form.a11y_nodes();
        match &self.mode {
            Mode::Normal => {
                if self.menu_open {
                    for (i, item) in MENU_ITEMS.iter().enumerate() {
                        nodes.push(A11yNode::new(
                            Self::rect_menu_entry(i),
                            *item,
                            A11yRole::MenuItem,
                        ));
                    }
                }
            }
            Mode::NameInput(input) => {
                nodes.push(input.a11y_node("Canvas name"));
            }
            Mode::Gallery { records, scroll, .. } => {
                let visible = OPEN_VISIBLE.min(records.len().saturating_sub(*scroll));
                for i in 0..visible {
                    let idx = *scroll + i;
                    if let Some((_, name)) = records.get(idx) {
                        nodes.push(A11yNode::new(
                            Self::rect_gallery_row(i),
                            name.clone(),
                            A11yRole::ListItem,
                        ));
                    }
                }
            }
        }
        nodes
    }
}

// --- DB persistence helpers ---------------------------------------------

fn load_db(path: &std::path::Path) -> (soul_db::Database, Option<u32>) {
    if let Ok(bytes) = crate::assets::read(path) {
        if let Some(db) = soul_db::Database::decode(&bytes) {
            let first_id = db.iter_category(CAT_CANVAS).next().map(|r| r.id);
            return (db, first_id);
        }
    }
    (soul_db::Database::new("draw"), None)
}

fn load_first_canvas(db: &soul_db::Database) -> (Vec<u8>, Vec<bool>, String) {
    if let Some(rec) = db.iter_category(CAT_CANVAS).next() {
        let name = decode_canvas_name(&rec.data);
        if let Some(pixels) = decode_canvas_pixels(&rec.data) {
            let written = pixels.iter().map(|&p| p != 255).collect();
            return (pixels, written, name);
        }
    }
    (
        vec![255; LOG_W * LOG_H],
        vec![false; LOG_W * LOG_H],
        "untitled".to_string(),
    )
}

fn load_form(db: &soul_db::Database) -> soul_ui::Form {
    if let Some(rec) = db.iter_category(CAT_FORM).next() {
        if let Ok(json) = std::str::from_utf8(&rec.data) {
            if let Some(form) = soul_ui::Form::from_json(json) {
                return form;
            }
        }
    }
    Draw::default_draw_ui()
}

fn save_db(db: &soul_db::Database, path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = crate::assets::create_dir_all(parent);
    }
    let _ = crate::assets::write(path, &db.encode());
}

// --- Canvas record encoding/decoding ------------------------------------

fn encode_canvas(name: &str, pixels: &[u8]) -> Vec<u8> {
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len().min(255) as u8;
    let mut v = Vec::with_capacity(1 + name_len as usize + pixels.len());
    v.push(name_len);
    v.extend_from_slice(&name_bytes[..name_len as usize]);
    v.extend_from_slice(pixels);
    v
}

fn decode_canvas_name(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    let nl = data[0] as usize;
    if data.len() < 1 + nl {
        return String::new();
    }
    String::from_utf8_lossy(&data[1..1 + nl]).into_owned()
}

fn decode_canvas_pixels(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    let nl = data[0] as usize;
    let px_start = 1 + nl;
    if data.len() != px_start + LOG_W * LOG_H {
        return None;
    }
    Some(data[px_start..].to_vec())
}

// --- Utilities ----------------------------------------------------------

fn scale_image_to_canvas(data: &[u8], src_w: usize, src_h: usize, dst_w: usize, dst_h: usize) -> Vec<u8> {
    let mut result = vec![255u8; dst_w * dst_h];
    let scale_x = dst_w as f32 / src_w as f32;
    let scale_y = dst_h as f32 / src_h as f32;
    let scale = scale_x.min(scale_y);
    let scaled_w = (src_w as f32 * scale) as usize;
    let scaled_h = (src_h as f32 * scale) as usize;
    let offset_x = (dst_w - scaled_w) / 2;
    let offset_y = (dst_h - scaled_h) / 2;
    for dy in 0..scaled_h {
        for dx in 0..scaled_w {
            let src_xf = dx as f32 / scale;
            let src_yf = dy as f32 / scale;
            let x0 = src_xf as usize;
            let y0 = src_yf as usize;
            let x1 = (x0 + 1).min(src_w - 1);
            let y1 = (y0 + 1).min(src_h - 1);
            let fx = src_xf - x0 as f32;
            let fy = src_yf - y0 as f32;
            let v00 = data[y0 * src_w + x0] as f32;
            let v10 = data[y0 * src_w + x1] as f32;
            let v01 = data[y1 * src_w + x0] as f32;
            let v11 = data[y1 * src_w + x1] as f32;
            let v = v00 * (1.0 - fx) * (1.0 - fy)
                  + v10 * fx * (1.0 - fy)
                  + v01 * (1.0 - fx) * fy
                  + v11 * fx * fy;
            let dst_x = offset_x + dx;
            let dst_y = offset_y + dy;
            if dst_x < dst_w && dst_y < dst_h {
                result[dst_y * dst_w + dst_x] = v as u8;
            }
        }
    }
    result
}

fn faint_background(b: u8) -> u8 {
    let x = b as u16;
    ((x * 85 + 255 * 170) / 255) as u8
}

fn sanitize_name(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.len() > 64 {
        return None;
    }
    if !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
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
            + "…"
    }
}

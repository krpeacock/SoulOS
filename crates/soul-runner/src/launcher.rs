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
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use soul_core::{App, Ctx, Event, HardButton, KeyCode, APP_HEIGHT, SCREEN_WIDTH};
use soul_script::SystemRequest;
use soul_db::{Database, CATEGORY_UNFILED};
use soul_ui::{
    button, hit_test, title_bar, MenuItem, MenuSheet, TextInput, BLACK, TITLE_BAR_H, WHITE,
};

use crate::assets;

// --- Layout constants ---------------------------------------------------

const ICON_CELL: u32 = 32;
const LABEL_FONT_W: i32 = 6;
const LABEL_FONT_H: i32 = 10;
const ICON_LABEL_GAP: i32 = 1;
const LAUNCHER_COLS: i32 = 6;
const LAUNCHER_ROWS: i32 = 6;
const LAUNCHER_H_GAP: i32 = 4;
const LAUNCHER_V_GAP: i32 = 3;
const LAUNCHER_TOP_PAD: i32 = 4;
const TAB_STRIP_H: i32 = 15;

// --- Categories ---------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq)]
enum Category {
    All,
    Main,
    Util,
    System,
}

impl Category {
    fn label(self) -> &'static str {
        match self {
            Category::All => "All",
            Category::Main => "Main",
            Category::Util => "Util",
            Category::System => "Sys",
        }
    }

    fn matches(self, app_cat: Category) -> bool {
        self == Category::All || self == app_cat
    }
}

const CATEGORIES: &[Category] = &[
    Category::All,
    Category::Main,
    Category::Util,
    Category::System,
];

fn category_for(app_id: &str) -> Category {
    match app_id {
        "com.soulos.settings" | "com.soulos.system_settings" => Category::System,
        "com.soulos.builder"
        | "com.soulos.draw"
        | "com.soulos.paint"
        | "com.soulos.egui_demo_native" => Category::Util,
        _ => Category::Main,
    }
}

// --- Launcher MenuSheet items ------------------------------------------

const LAUNCHER_MENU_ITEMS: &[MenuItem<'static>] = &[
    MenuItem::with_shortcut("Edit Order", 'E'),
    MenuItem::with_shortcut("Search\u{2026}", 'F'),
    MenuItem::with_shortcut("About SoulOS", 'A'),
];

// --- Internal app entry -------------------------------------------------

struct AppEntry {
    app_id: String,
    name: String,
    icon: Vec<u8>, // raw 32×32 pixels, or empty for blank tile
}

// --- Launcher -----------------------------------------------------------

pub struct Launcher {
    apps: Vec<AppEntry>,
    order: Vec<usize>, // indices into apps, representing user order
    visible: Vec<usize>, // indices into `order` that pass the current category filter
    current_cat: Category,
    touched: Option<usize>,
    drag_from: Option<usize>,
    drag_to: Option<usize>,
    picker_mode: bool,
    reorder_mode: bool,
    about_open: bool,
    search: Option<TextInput>,
    menu: MenuSheet,
    db: Option<Database>,
}

impl Launcher {
    pub const APP_ID: &'static str = "com.soulos.launcher";
    pub const NAME: &'static str = "Launcher";

    pub fn new() -> Self {
        let db = Database::new("launcher");
        Self {
            apps: vec![],
            order: vec![],
            visible: vec![],
            current_cat: Category::All,
            touched: None,
            drag_from: None,
            drag_to: None,
            picker_mode: false,
            reorder_mode: false,
            about_open: false,
            search: None,
            menu: MenuSheet::new(),
            db: Some(db),
        }
    }

    fn search_rect() -> Rectangle {
        Rectangle::new(
            Point::new(2, TITLE_BAR_H as i32 + 1),
            Size::new(SCREEN_WIDTH as u32 - 4, TAB_STRIP_H as u32 - 2),
        )
    }

    fn enter_search(&mut self, ctx: &mut Ctx<'_>) {
        self.search = Some(TextInput::with_placeholder(
            Self::search_rect(),
            "Search apps\u{2026}",
        ));
        self.touched = None;
        self.drag_from = None;
        self.drag_to = None;
        self.refresh_visible();
        ctx.invalidate_all();
    }

    fn exit_search(&mut self, ctx: &mut Ctx<'_>) {
        if self.search.is_some() {
            self.search = None;
            self.refresh_visible();
            ctx.invalidate_all();
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
        // Try to load order from DB
        if let Some(db) = &self.db {
            if let Some(rec) = db.iter().next() {
                // Order is stored as Vec<u8> of indices
                let order: Vec<usize> = rec.data.chunks(4)
                    .filter_map(|b| if b.len() == 4 {
                        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
                    } else { None })
                    .collect();
                // Only use if valid
                if order.len() == self.apps.len() && order.iter().all(|&i| i < self.apps.len()) {
                    self.order = order;
                } else {
                    self.order = (0..self.apps.len()).collect();
                }
            } else {
                self.order = (0..self.apps.len()).collect();
            }
        } else {
            self.order = (0..self.apps.len()).collect();
        }
        self.refresh_visible();
    }

    fn refresh_visible(&mut self) {
        let query = self
            .search
            .as_ref()
            .map(|i| i.text().to_lowercase())
            .filter(|q| !q.is_empty());
        self.visible = self
            .order
            .iter()
            .enumerate()
            .filter_map(|(order_idx, &app_idx)| {
                let entry = self.apps.get(app_idx)?;
                let pass = if let Some(q) = &query {
                    entry.name.to_lowercase().contains(q.as_str())
                } else if self.search.is_some() {
                    true
                } else {
                    self.current_cat.matches(category_for(&entry.app_id))
                };
                if pass {
                    Some(order_idx)
                } else {
                    None
                }
            })
            .collect();
    }

    fn set_current_cat(&mut self, cat: Category, ctx: &mut Ctx<'_>) {
        if cat == self.current_cat {
            return;
        }
        self.current_cat = cat;
        self.touched = None;
        self.drag_from = None;
        self.drag_to = None;
        self.refresh_visible();
        ctx.invalidate_all();
    }

    fn save_order(&mut self) {
        if let Some(db) = &mut self.db {
            // Remove all previous records (only one order record is expected)
            let ids: Vec<u32> = db.iter().map(|r| r.id).collect();
            for id in ids { db.delete(id); }
            let mut data = Vec::with_capacity(self.order.len() * 4);
            for &i in &self.order {
                data.extend_from_slice(&(i as u32).to_le_bytes());
            }
            db.insert(CATEGORY_UNFILED, data);
        }
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
        let grid_top = TITLE_BAR_H as i32 + TAB_STRIP_H + LAUNCHER_TOP_PAD;
        let avail_h = APP_HEIGHT as i32 - TITLE_BAR_H as i32 - TAB_STRIP_H - LAUNCHER_TOP_PAD;
        let row_pitch = (avail_h - (LAUNCHER_ROWS - 1) * LAUNCHER_V_GAP) / LAUNCHER_ROWS;
        let y_slot = grid_top + row * (row_pitch + LAUNCHER_V_GAP);
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

    fn tab_rect(idx: usize) -> Rectangle {
        let n = CATEGORIES.len() as i32;
        let tab_w = SCREEN_WIDTH as i32 / n;
        let x = idx as i32 * tab_w;
        let w = if idx as i32 == n - 1 {
            SCREEN_WIDTH as i32 - x
        } else {
            tab_w
        };
        Rectangle::new(
            Point::new(x, TITLE_BAR_H as i32),
            Size::new(w as u32, TAB_STRIP_H as u32),
        )
    }

    fn find_hit(&self, x: i16, y: i16) -> Option<usize> {
        (0..self.visible.len()).find(|&i| hit_test(&Self::tile_rect(i), x, y))
    }

    fn tab_hit_test(x: i16, y: i16) -> Option<usize> {
        (0..CATEGORIES.len()).find(|&i| hit_test(&Self::tab_rect(i), x, y))
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

    fn activate_display_idx(&mut self, display_idx: usize) -> Option<SystemRequest> {
        let order_idx = *self.visible.get(display_idx)?;
        let app_idx = *self.order.get(order_idx)?;
        let entry = self.apps.get(app_idx)?;
        if self.picker_mode {
            self.picker_mode = false;
            Some(SystemRequest::SendResult {
                action: "return_app".to_string(),
                payload: soul_core::ExchangePayload::Text(entry.app_id.clone()),
            })
        } else {
            Some(SystemRequest::LaunchById(entry.app_id.clone()))
        }
    }

    fn launcher_menu_action(&mut self, idx: usize, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match idx {
            0 => {
                self.reorder_mode = !self.reorder_mode;
                ctx.invalidate_all();
                None
            }
            1 => {
                self.enter_search(ctx);
                None
            }
            2 => {
                self.about_open = true;
                ctx.invalidate_all();
                None
            }
            _ => None,
        }
    }

    // --- Background resource management ---------------------------------

    fn handle_get_resource(&self, app_id: &str, kind: &str) -> Option<SystemRequest> {
        match kind {
            "icon" => {
                let entry = self.apps.iter().find(|e| e.app_id == app_id)?;
                let cell = ICON_CELL as u16;
                Some(SystemRequest::SendResult {
                    action: "return_resource".to_string(),
                    payload: soul_core::ExchangePayload::Resource {
                        app_id: app_id.to_string(),
                        kind: "icon".to_string(),
                        width: cell,
                        height: cell,
                        pixels: entry.icon.clone(),
                        text: String::new(),
                    },
                })
            }
            "script" => {
                // Load script source from the registered app list.
                let src = soul_script::app_list()
                    .iter()
                    .find(|e| e.app_id == app_id)
                    .and_then(|e| {
                        // app_list icon_stem is the only path-adjacent field we have;
                        // for now derive script path from the assets convention.
                        // TODO: replace with resource DB lookup.
                        let stem = e.icon_stem.as_str();
                        let path = if stem.is_empty() {
                            return None;
                        } else {
                            std::path::PathBuf::from("assets/scripts")
                                .join(format!("{stem}.rhai"))
                        };
                        assets::read_to_string(&path).ok()
                    })
                    .unwrap_or_default();
                Some(SystemRequest::SendResult {
                    action: "return_resource".to_string(),
                    payload: soul_core::ExchangePayload::Resource {
                        app_id: app_id.to_string(),
                        kind: "script".to_string(),
                        width: 0,
                        height: 0,
                        pixels: vec![],
                        text: src,
                    },
                })
            }
            _ => {
                log::warn!("launcher: unknown resource kind '{kind}' requested for '{app_id}'");
                None
            }
        }
    }

    fn handle_set_resource(
        &mut self,
        app_id: &str,
        kind: &str,
        width: u16,
        height: u16,
        pixels: Vec<u8>,
        _text: String,
    ) {
        match kind {
            "icon" => {
                if let Some(entry) = self.apps.iter_mut().find(|e| e.app_id == app_id) {
                    entry.icon = pixels.clone();
                }
                // Persist the updated icon back to the PGM file so it survives restart.
                let stem = soul_script::app_list()
                    .iter()
                    .find(|e| e.app_id == app_id)
                    .map(|e| e.icon_stem.clone())
                    .unwrap_or_default();
                if !stem.is_empty() && width > 0 && height > 0 {
                    let path = std::path::PathBuf::from("assets/sprites")
                        .join(format!("{stem}_icon.pgm"));
                    if let Err(e) = save_pgm(&path, width as usize, height as usize, &pixels) {
                        log::warn!("launcher: could not save icon for '{app_id}': {e}");
                    } else {
                        log::info!("launcher: saved icon for '{app_id}' → {}", path.display());
                    }
                }
            }
            "script" => {
                // Write the script source back to the .rhai file on disk.
                let path = soul_script::app_list()
                    .iter()
                    .find(|e| e.app_id == app_id)
                    .and_then(|e| {
                        let stem = e.icon_stem.as_str();
                        if stem.is_empty() { None }
                        else {
                            Some(std::path::PathBuf::from("assets/scripts")
                                .join(format!("{stem}.rhai")))
                        }
                    });
                if let Some(p) = path {
                    if let Err(e) = assets::write(&p, _text.as_bytes()) {
                        log::warn!("launcher: could not save script for '{app_id}': {e}");
                    } else {
                        log::info!("launcher: saved script for '{app_id}' → {}", p.display());
                    }
                } else {
                    log::warn!("launcher: set_resource: no script path found for '{app_id}'");
                }
            }
            _ => {
                log::warn!("launcher: set_resource: unknown kind '{kind}' for '{app_id}'");
            }
        }
    }

    // --- App interface --------------------------------------------------

    pub fn handle_event(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        // About modal absorbs all input until dismissed.
        if self.about_open {
            match event {
                Event::PenDown { .. } | Event::Menu | Event::ButtonDown(_) | Event::Key(_) => {
                    self.about_open = false;
                    ctx.invalidate_all();
                    return None;
                }
                Event::AppStop => {
                    self.about_open = false;
                    return None;
                }
                _ => return None,
            }
        }

        // Menu key while search is active exits search.
        if self.search.is_some() && matches!(event, Event::Menu) {
            self.exit_search(ctx);
            return None;
        }

        // Search input owns typing and Enter while active.
        if self.search.is_some() {
            if let Event::Key(k) = &event {
                let input = self.search.as_mut().unwrap();
                match k {
                    KeyCode::Char(c) => {
                        let out = input.insert_char(*c);
                        if let Some(r) = out.dirty {
                            ctx.invalidate(r);
                        }
                        if out.text_changed {
                            self.refresh_visible();
                            ctx.invalidate_all();
                        }
                        return None;
                    }
                    KeyCode::Backspace => {
                        if input.text().is_empty() {
                            self.exit_search(ctx);
                            return None;
                        }
                        let out = input.backspace();
                        if let Some(r) = out.dirty {
                            ctx.invalidate(r);
                        }
                        if out.text_changed {
                            self.refresh_visible();
                            ctx.invalidate_all();
                        }
                        return None;
                    }
                    KeyCode::Enter => {
                        if !self.visible.is_empty() {
                            return self.activate_display_idx(0);
                        }
                        return None;
                    }
                    KeyCode::ArrowLeft => {
                        if let Some(r) = input.cursor_left() {
                            ctx.invalidate(r);
                        }
                        return None;
                    }
                    KeyCode::ArrowRight => {
                        if let Some(r) = input.cursor_right() {
                            ctx.invalidate(r);
                        }
                        return None;
                    }
                    _ => {}
                }
            }
        }

        // MenuSheet gets first crack at events when relevant.
        // While search is active, do NOT route Key events to the menu
        // (the menu's shortcut keys would swallow typing).
        let route_to_menu = matches!(
            event,
            Event::Menu | Event::AppStop | Event::PenDown { .. } | Event::PenMove { .. }
                | Event::PenUp { .. } | Event::ButtonDown(_)
        ) || (self.search.is_none() && matches!(event, Event::Key(_)));
        if route_to_menu {
            let was_open = self.menu.is_open();
            let out = self.menu.handle(&event, LAUNCHER_MENU_ITEMS);
            if let Some(r) = out.dirty {
                ctx.invalidate(r);
            }
            if let Some(idx) = out.committed {
                return self.launcher_menu_action(idx, ctx);
            }
            if self.menu.is_open() || was_open {
                return None;
            }
        }

        match event {
            Event::AppStart => {
                self.picker_mode = false;
                self.refresh_app_list();
                ctx.invalidate_all();
                None
            }
            Event::PenDown { x, y } => {
                if let Some(input) = self.search.as_ref() {
                    if input.contains(x, y) {
                        return None;
                    }
                } else if let Some(tab) = Self::tab_hit_test(x, y) {
                    self.set_current_cat(CATEGORIES[tab], ctx);
                    return None;
                }
                let hit = self.find_hit(x, y);
                self.set_touched(hit, ctx);
                if self.reorder_mode {
                    self.drag_from = hit;
                    self.drag_to = hit;
                }
                None
            }
            Event::PenMove { x, y } => {
                if self.reorder_mode && self.drag_from.is_some() {
                    let hit = self.find_hit(x, y);
                    if hit != self.drag_to {
                        self.drag_to = hit;
                        ctx.invalidate_all();
                    }
                }
                None
            }
            Event::PenUp { x, y } => {
                if let Some(input) = self.search.as_mut() {
                    if input.contains(x, y) {
                        if let Some(r) = input.pen_released(x, y) {
                            ctx.invalidate(r);
                        }
                        return None;
                    }
                }
                let drag_from = self.drag_from;
                let drag_to = self.drag_to;
                let was = self.touched;
                self.set_touched(None, ctx);
                self.drag_from = None;
                self.drag_to = None;
                if self.reorder_mode {
                    if let (Some(from), Some(to)) = (drag_from, drag_to) {
                        if from != to {
                            let from_order = self.visible[from];
                            let to_order = self.visible[to];
                            let mut new_order = self.order.clone();
                            let idx = new_order.remove(from_order);
                            new_order.insert(to_order, idx);
                            self.order = new_order;
                            self.save_order();
                            self.refresh_visible();
                            ctx.invalidate_all();
                            return None;
                        }
                    }
                }
                let hit = self.find_hit(x, y);
                if hit.is_some() && hit == was {
                    hit.and_then(|i| self.activate_display_idx(i))
                } else {
                    None
                }
            }
            Event::ButtonDown(HardButton::AppA) => self.activate_display_idx(0),
            Event::ButtonDown(HardButton::AppB) => self.activate_display_idx(1),
            Event::ButtonDown(HardButton::AppC) => self.activate_display_idx(2),
            Event::ButtonDown(HardButton::AppD) => self.activate_display_idx(3),
            Event::Key(KeyCode::ArrowLeft) => {
                let cur = CATEGORIES.iter().position(|&c| c == self.current_cat).unwrap_or(0);
                if cur > 0 {
                    self.set_current_cat(CATEGORIES[cur - 1], ctx);
                }
                None
            }
            Event::Key(KeyCode::ArrowRight) => {
                let cur = CATEGORIES.iter().position(|&c| c == self.current_cat).unwrap_or(0);
                if cur + 1 < CATEGORIES.len() {
                    self.set_current_cat(CATEGORIES[cur + 1], ctx);
                }
                None
            }
            Event::Exchange { action, payload, .. } => match action.as_str() {
                "pick_app" => {
                    self.picker_mode = true;
                    if self.apps.is_empty() {
                        self.refresh_app_list();
                    }
                    ctx.invalidate_all();
                    None
                }
                "get_resource" => {
                    if let soul_core::ExchangePayload::Resource { app_id, kind, .. } = payload {
                        // Ensure the app list is populated before serving requests.
                        if self.apps.is_empty() {
                            self.refresh_app_list();
                        }
                        return self.handle_get_resource(&app_id, &kind);
                    }
                    None
                }
                "set_resource" => {
                    if let soul_core::ExchangePayload::Resource {
                        app_id, kind, width, height, pixels, text,
                    } = payload
                    {
                        if self.apps.is_empty() {
                            self.refresh_app_list();
                        }
                        self.handle_set_resource(&app_id, &kind, width, height, pixels, text);
                    }
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub fn draw<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D, _dirty: Rectangle) {
        let title = if self.picker_mode {
            "Pick App"
        } else if self.search.is_some() {
            "Search"
        } else if self.reorder_mode {
            "Edit Order"
        } else {
            Self::NAME
        };
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, title);

        if let Some(input) = &self.search {
            let _ = input.draw(canvas);
        } else {
            for (i, &cat) in CATEGORIES.iter().enumerate() {
                let r = Self::tab_rect(i);
                let _ = button(canvas, r, cat.label(), cat == self.current_cat);
            }
        }

        let label_style = MonoTextStyle::new(&FONT_6X10, BLACK);

        for (display_idx, &order_idx) in self.visible.iter().enumerate() {
            let app_idx = self.order[order_idx];
            let entry = &self.apps[app_idx];
            let icon_r = Self::icon_rect(display_idx);
            let pressed = self.touched == Some(display_idx);
            let dragging = self.drag_from == Some(display_idx);
            let drag_target = self.drag_to == Some(display_idx) && self.drag_from != self.drag_to;
            let expected = (ICON_CELL * ICON_CELL) as usize;

            if entry.icon.len() == expected {
                if pressed || dragging {
                    let inv: Vec<u8> = entry.icon.iter().map(|&p| 255 - p).collect();
                    let raw = ImageRaw::<Gray8>::new(&inv, ICON_CELL);
                    let _ = Image::new(&raw, icon_r.top_left).draw(canvas);
                } else {
                    let raw = ImageRaw::<Gray8>::new(&entry.icon, ICON_CELL);
                    let _ = Image::new(&raw, icon_r.top_left).draw(canvas);
                }
            } else {
                let _ = canvas.fill_solid(&icon_r, Gray8::new(if pressed || dragging { 128 } else { 210 }));
            }

            if drag_target {
                use embedded_graphics::Pixel;
                let border = Rectangle::new(icon_r.top_left, icon_r.size);
                let _ = canvas.draw_iter(border.points().map(|p| Pixel(p, Gray8::new(0))));
            }

            let lbl = Self::label_text(&entry.name);
            let nw = lbl.chars().count() as i32 * LABEL_FONT_W;
            let tx = icon_r.top_left.x + (ICON_CELL as i32 - nw) / 2;
            let ty = icon_r.top_left.y + ICON_CELL as i32 + ICON_LABEL_GAP;
            let _ = Text::with_baseline(&lbl, Point::new(tx, ty), label_style, Baseline::Top)
                .draw(canvas);
        }

        self.menu.draw(canvas, LAUNCHER_MENU_ITEMS);

        if self.about_open {
            self.draw_about(canvas);
        }
    }

    fn draw_about<D: DrawTarget<Color = Gray8>>(&self, canvas: &mut D) {
        let w = 180;
        let h = 90;
        let x = (SCREEN_WIDTH as i32 - w) / 2;
        let y = TITLE_BAR_H as i32 + TAB_STRIP_H + 30;
        let r = Rectangle::new(Point::new(x, y), Size::new(w as u32, h as u32));
        let fill = PrimitiveStyleBuilder::new()
            .fill_color(WHITE)
            .stroke_color(BLACK)
            .stroke_width(1)
            .build();
        let _ = r.into_styled(fill).draw(canvas);

        let label_style = MonoTextStyle::new(&FONT_6X10, BLACK);
        let _ = Text::with_baseline(
            "SoulOS",
            Point::new(x + 12, y + 14),
            label_style,
            Baseline::Top,
        )
        .draw(canvas);
        let _ = Text::with_baseline(
            "Zen of Palm, reborn.",
            Point::new(x + 12, y + 32),
            label_style,
            Baseline::Top,
        )
        .draw(canvas);
        let _ = Text::with_baseline(
            "Tap to close",
            Point::new(x + 12, y + 64),
            label_style,
            Baseline::Top,
        )
        .draw(canvas);
    }

    pub fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        use soul_core::a11y::{A11yNode, A11yRole};
        let mut nodes: Vec<A11yNode> = Vec::new();

        if let Some(input) = &self.search {
            nodes.push(input.a11y_node("Search apps"));
        } else {
            for (i, &cat) in CATEGORIES.iter().enumerate() {
                nodes.push(A11yNode::new(
                    Self::tab_rect(i),
                    cat.label().to_string(),
                    A11yRole::Button,
                ));
            }
        }

        for (display_idx, &order_idx) in self.visible.iter().enumerate() {
            let app_idx = match self.order.get(order_idx) {
                Some(&i) => i,
                None => continue,
            };
            if let Some(entry) = self.apps.get(app_idx) {
                nodes.push(A11yNode::new(
                    Self::tile_rect(display_idx),
                    entry.name.clone(),
                    A11yRole::Button,
                ));
            }
        }

        nodes.extend(self.menu.a11y_nodes(LAUNCHER_MENU_ITEMS));
        nodes
    }
}

// --- PGM icon loader ----------------------------------------------------

fn save_pgm(path: &std::path::Path, w: usize, h: usize, pixels: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        assets::create_dir_all(parent)?;
    }
    let mut buf = Vec::with_capacity(pixels.len() + 32);
    writeln!(buf, "P5")?;
    writeln!(buf, "{w} {h}")?;
    writeln!(buf, "255")?;
    buf.extend_from_slice(pixels);
    assets::write(path, &buf)
}

fn load_icon(stem: &str, cell: usize) -> Vec<u8> {
    let try_load = |s: &str| {
        let path = std::path::PathBuf::from("assets/sprites").join(format!("{s}_icon.pgm"));
        match load_pgm(&path) {
            Ok((w, h, pix)) if w == cell && h == cell => Some(pix),
            _ => None,
        }
    };
    if !stem.is_empty() {
        if let Some(pix) = try_load(stem) {
            return pix;
        }
    }
    try_load("default").unwrap_or_default()
}

fn load_pgm(path: &std::path::Path) -> std::io::Result<(usize, usize, Vec<u8>)> {
    use std::io::{BufRead, Read};
    let bytes = assets::read(path)?;
    let mut r = std::io::Cursor::new(bytes);
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

impl App for Launcher {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        self.handle_event(event, ctx);
    }

    fn draw<D>(&mut self, canvas: &mut D, dirty: Rectangle)
    where
        D: DrawTarget<Color = Gray8>,
    {
        self.draw(canvas, dirty);
    }

    fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        self.a11y_nodes()
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

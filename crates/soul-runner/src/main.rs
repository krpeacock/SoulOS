//! Desktop runner: hosts the launcher and the system strip.

mod address;
mod builder;
mod draw;
mod launcher_store;
mod notes;
mod todo;

use embedded_graphics::{
    draw_target::DrawTarget,
    image::{Image, ImageRaw},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use soul_core::{run, App, Ctx, Event, HardButton, KeyCode, APP_HEIGHT, SCREEN_HEIGHT, SCREEN_WIDTH, SYSTEM_STRIP_H};
use soul_hal_hosted::HostedPlatform;
use soul_ui::{hit_test, title_bar, BLACK, TITLE_BAR_H, WHITE};
use std::cell::RefCell;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use address::Address;
use builder::MobileBuilder;
use draw::Draw;
use launcher_store::LauncherIconStore;
use notes::Notes;
use todo::MyTodoApp;

/// Square PGM icon size; must match `generate_icons.py` export size.
pub(crate) const ICON_CELL: u32 = 32;
pub(crate) const APPS: &[&str] = &[
    "Notes", "Address", "Date", "ToDo", "Mail", "Calc", "Prefs", "Draw", "Sync", "Builder", "Todo",
];
const NOTES_IDX: usize = 0;
const ADDRESS_IDX: usize = 1;
const DRAW_IDX: usize = 7;
const BUILDER_IDX: usize = 9;
const TODO_IDX: usize = 10;

const LABEL_FONT_W: i32 = 6;
const LABEL_FONT_H: i32 = 10;
const ICON_LABEL_GAP: i32 = 1;
const LAUNCHER_COLS: i32 = 4;
const LAUNCHER_ROWS: i32 = 6;
const LAUNCHER_H_GAP: i32 = 4;
const LAUNCHER_V_GAP: i32 = 3;
const LAUNCHER_TOP_PAD: i32 = 4;

fn launcher_label_text(name: &str) -> String {
    let max_chars = ((ICON_CELL as i32) / LABEL_FONT_W).max(1) as usize;
    let n = name.chars().count();
    if n <= max_chars {
        return name.to_string();
    }
    let take = max_chars.saturating_sub(1);
    name.chars().take(take).collect::<String>() + "…"
}

pub(crate) fn seed_launcher_icons(db: &mut soul_db::Database) {
    let cell = ICON_CELL as usize;
    let area = cell * cell;
    for i in 0..APPS.len() {
        let mut data = vec![255u8; area];
        let path =
            PathBuf::from("assets/sprites").join(format!("{}_icon.pgm", APPS[i].to_lowercase()));
        if let Ok((w, h, pix)) = load_pgm(&path) {
            if w == cell && h == cell && pix.len() == area {
                data = pix;
            }
        }
        db.insert(i as u8, data);
    }
}

fn build_launcher_atlases(db: &soul_db::Database) -> (Vec<u8>, Vec<u8>) {
    let row_w = APPS.len() * ICON_CELL as usize;
    let cell = ICON_CELL as usize;
    let h = cell;
    let mut normal = vec![255u8; row_w * h];
    let mut inverted = vec![255u8; row_w * h];
    for i in 0..APPS.len() {
        let Some(rec) = db.iter_category(i as u8).next() else {
            continue;
        };
        if rec.data.len() != cell * cell {
            continue;
        }
        for y in 0..h {
            let src_row = y * cell;
            let dst_row = y * row_w + i * cell;
            for x in 0..cell {
                let p = rec.data[src_row + x];
                normal[dst_row + x] = p;
                inverted[dst_row + x] = 255 - p;
            }
        }
    }
    (normal, inverted)
}

// --- Launcher -----------------------------------------------------------

struct Launcher {
    touched: Option<usize>,
    pending: Option<usize>,
    icons: Rc<RefCell<LauncherIconStore>>,
}

impl Launcher {
    fn new(icons: Rc<RefCell<LauncherIconStore>>) -> Self {
        Self {
            touched: None,
            pending: None,
            icons,
        }
    }

    fn take_launch(&mut self) -> Option<usize> {
        self.pending.take()
    }

    fn tile_origin(idx: usize) -> (i32, i32) {
        let tile_w = ICON_CELL as i32;
        let tile_h = ICON_CELL as i32 + ICON_LABEL_GAP + LABEL_FONT_H;
        let grid_w =
            LAUNCHER_COLS * tile_w + (LAUNCHER_COLS - 1) * LAUNCHER_H_GAP;
        let x_off = (SCREEN_WIDTH as i32 - grid_w) / 2;
        let i = idx as i32;
        let col = i % LAUNCHER_COLS;
        let row = i / LAUNCHER_COLS;
        let x = x_off + col * (tile_w + LAUNCHER_H_GAP);

        let avail_h = APP_HEIGHT as i32 - TITLE_BAR_H as i32 - LAUNCHER_TOP_PAD;
        let row_pitch =
            (avail_h - (LAUNCHER_ROWS - 1) * LAUNCHER_V_GAP) / LAUNCHER_ROWS;
        let y_slot = TITLE_BAR_H as i32
            + LAUNCHER_TOP_PAD
            + row * (row_pitch + LAUNCHER_V_GAP);
        let y = y_slot + (row_pitch - tile_h) / 2;
        (x, y)
    }

    /// Sprite only (`ICON_CELL`×`ICON_CELL`); [`Self::tile_rect`] includes the caption below.
    fn icon_rect(idx: usize) -> Rectangle {
        let (x, y) = Self::tile_origin(idx);
        Rectangle::new(
            Point::new(x, y),
            Size::new(ICON_CELL, ICON_CELL),
        )
    }

    /// Icon + label; used for hit-testing and damage rects.
    fn tile_rect(idx: usize) -> Rectangle {
        let (x, y) = Self::tile_origin(idx);
        let h = ICON_CELL as i32 + ICON_LABEL_GAP + LABEL_FONT_H;
        Rectangle::new(
            Point::new(x, y),
            Size::new(ICON_CELL, h as u32),
        )
    }

    fn find_hit(x: i16, y: i16) -> Option<usize> {
        (0..APPS.len()).find(|&i| hit_test(&Self::tile_rect(i), x, y))
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
}

impl App for Launcher {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::PenDown { x, y } | Event::PenMove { x, y } => {
                self.set_touched(Self::find_hit(x, y), ctx);
            }
            Event::PenUp { x, y } => {
                let hit = Self::find_hit(x, y);
                let was = self.touched;
                self.set_touched(None, ctx);
                if hit.is_some() && hit == was {
                    self.pending = hit;
                }
            }
            Event::ButtonDown(HardButton::AppA) => self.pending = Some(0),
            Event::ButtonDown(HardButton::AppB) => self.pending = Some(1),
            Event::ButtonDown(HardButton::AppC) => self.pending = Some(2),
            Event::ButtonDown(HardButton::AppD) => self.pending = Some(3),
            Event::Menu => {}
            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "Launcher");

        let store = self.icons.borrow();
        let (app_icons_pixels, app_icons_pixels_inverted) = build_launcher_atlases(&store.db);
        let atlas_w = (APPS.len() as u32) * ICON_CELL;
        let atlas: ImageRaw<'_, Gray8> = ImageRaw::new(&app_icons_pixels, atlas_w);
        let atlas_inv: ImageRaw<'_, Gray8> = ImageRaw::new(&app_icons_pixels_inverted, atlas_w);

        let label_style = MonoTextStyle::new(&FONT_6X10, BLACK);

        for (i, name) in APPS.iter().enumerate() {
            let icon_r = Self::icon_rect(i);
            let pressed = self.touched == Some(i);
            let src = Rectangle::new(
                Point::new(i as i32 * ICON_CELL as i32, 0),
                Size::new(ICON_CELL, ICON_CELL),
            );
            if pressed {
                let sprite = atlas_inv.sub_image(&src);
                let _ = Image::new(&sprite, icon_r.top_left).draw(canvas);
            } else {
                let sprite = atlas.sub_image(&src);
                let _ = Image::new(&sprite, icon_r.top_left).draw(canvas);
            }

            let label = launcher_label_text(name);
            let nw = label.chars().count() as i32 * LABEL_FONT_W;
            let tx = icon_r.top_left.x + (ICON_CELL as i32 - nw) / 2;
            let ty = icon_r.top_left.y + ICON_CELL as i32 + ICON_LABEL_GAP;
            let _ = Text::with_baseline(
                label.as_str(),
                Point::new(tx, ty),
                label_style,
                Baseline::Top,
            )
            .draw(canvas);
        }
    }
}

// --- System strip -------------------------------------------------------

const STRIP_H: i32 = SYSTEM_STRIP_H as i32;
const STRIP_TOP: i32 = APP_HEIGHT as i32;
const STRIP_SEGMENT_W: i32 = SCREEN_WIDTH as i32 / 3;
const FONT_W: i32 = 6;
const FONT_H: i32 = 10;

fn strip_home_rect() -> Rectangle {
    Rectangle::new(
        Point::new(0, STRIP_TOP),
        Size::new(STRIP_SEGMENT_W as u32, STRIP_H as u32),
    )
}

fn strip_menu_rect() -> Rectangle {
    Rectangle::new(
        Point::new(2 * STRIP_SEGMENT_W, STRIP_TOP),
        Size::new(STRIP_SEGMENT_W as u32, STRIP_H as u32),
    )
}

fn strip_rect() -> Rectangle {
    Rectangle::new(
        Point::new(0, STRIP_TOP),
        Size::new(SCREEN_WIDTH as u32, STRIP_H as u32),
    )
}

fn draw_system_strip<D>(canvas: &mut D, active_label: &str)
where
    D: DrawTarget<Color = Gray8>,
{
    let _ = strip_rect()
        .into_styled(PrimitiveStyle::with_fill(BLACK))
        .draw(canvas);
    let style = MonoTextStyle::new(&FONT_6X10, WHITE);
    let y = STRIP_TOP + (STRIP_H - FONT_H) / 2;

    // Home label, centered in left third.
    let home = "Home";
    let home_x = (STRIP_SEGMENT_W - home.len() as i32 * FONT_W) / 2;
    let _ = Text::with_baseline(home, Point::new(home_x, y), style, Baseline::Top).draw(canvas);

    // Active-app name, centered in middle third.
    let mid_x =
        STRIP_SEGMENT_W + (STRIP_SEGMENT_W - active_label.chars().count() as i32 * FONT_W) / 2;
    let _ =
        Text::with_baseline(active_label, Point::new(mid_x, y), style, Baseline::Top).draw(canvas);

    // Menu label, centered in right third.
    let menu = "Menu";
    let menu_x = 2 * STRIP_SEGMENT_W + (STRIP_SEGMENT_W - menu.len() as i32 * FONT_W) / 2;
    let _ = Text::with_baseline(menu, Point::new(menu_x, y), style, Baseline::Top).draw(canvas);
}

// --- Host ---------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Launcher,
    Notes,
    Address,
    Draw,
    Builder,
    Todo,
}

struct Host {
    launcher_icons: Rc<RefCell<LauncherIconStore>>,
    launcher: Launcher,
    notes: Notes,
    address: Address,
    draw: Draw,
    builder: MobileBuilder,
    todo: MyTodoApp,
    active: Slot,
    /// `true` while a press that began inside the system strip is
    /// in flight. Child apps don't see any event during this window.
    strip_pressed: bool,

    // Accessibility
    a11y_enabled: bool,
    a11y_focus: Option<usize>,
    pen_start: Option<(i16, i16, u64)>,
    last_tap: Option<(i16, i16, u64)>,
    tap_count: u8,
}

impl Host {
    fn new() -> Self {
        let launcher_icons = Rc::new(RefCell::new(LauncherIconStore::load_or_seed()));
        Self {
            launcher_icons: Rc::clone(&launcher_icons),
            launcher: Launcher::new(Rc::clone(&launcher_icons)),
            notes: Notes::new(),
            address: Address::new(),
            draw: Draw::new(Rc::clone(&launcher_icons)),
            builder: MobileBuilder::new(),
            todo: MyTodoApp::new(),
            active: Slot::Launcher,
            strip_pressed: false,
            a11y_enabled: false,
            a11y_focus: None,
            pen_start: None,
            last_tap: None,
            tap_count: 0,
        }
    }

    fn switch_to(&mut self, slot: Slot, ctx: &mut Ctx<'_>) {
        if self.active != slot {
            self.active = slot;
            ctx.invalidate_all();
        }
    }

    fn active_label(&self) -> &'static str {
        match self.active {
            Slot::Launcher => "Launcher",
            Slot::Notes => "Notes",
            Slot::Address => "Address",
            Slot::Draw => "Draw",
            Slot::Builder => "Builder",
            Slot::Todo => "Todo",
        }
    }

    fn active_a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        let mut nodes = match self.active {
            Slot::Launcher => self.launcher.a11y_nodes(),
            Slot::Notes => self.notes.a11y_nodes(),
            Slot::Address => self.address.a11y_nodes(),
            Slot::Draw => self.draw.a11y_nodes(),
            Slot::Builder => self.builder.a11y_nodes(),
            Slot::Todo => self.todo.a11y_nodes(),
        };

        // Add system strip nodes
        nodes.push(soul_core::a11y::A11yNode {
            bounds: strip_home_rect(),
            label: "Home".into(),
            role: "system_button".into(),
        });
        nodes.push(soul_core::a11y::A11yNode {
            bounds: strip_menu_rect(),
            label: "Menu".into(),
            role: "system_button".into(),
        });

        nodes
    }

    fn speak_focused(&self, ctx: &mut Ctx<'_>) {
        if let Some(idx) = self.a11y_focus {
            let nodes = self.active_a11y_nodes();
            if let Some(node) = nodes.get(idx) {
                let msg = format!("{}, {}", node.label, node.role);
                ctx.a11y.speak(&msg);
            }
        }
    }

    fn activate_focused(&mut self, ctx: &mut Ctx<'_>) {
        if let Some(idx) = self.a11y_focus {
            let nodes = self.active_a11y_nodes();
            if let Some(node) = nodes.get(idx) {
                let center = node.bounds.center();
                let x = center.x as i16;
                let y = center.y as i16;
                
                // If it's a system button, handle it here
                if node.role == "system_button" {
                    if node.label == "Home" {
                        self.switch_to(Slot::Launcher, ctx);
                    } else if node.label == "Menu" {
                        self.forward_to_child(Event::Menu, ctx);
                    }
                } else {
                    // Synthesize events for child app
                    self.forward_to_child(Event::PenDown { x, y }, ctx);
                    self.forward_to_child(Event::PenUp { x, y }, ctx);
                }
            }
        }
    }

    fn forward_to_child(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match self.active {
            Slot::Launcher => {
                self.launcher.handle(event, ctx);
                if let Some(idx) = self.launcher.take_launch() {
                    if idx == NOTES_IDX {
                        self.switch_to(Slot::Notes, ctx);
                    } else if idx == ADDRESS_IDX {
                        self.switch_to(Slot::Address, ctx);
                    } else if idx == DRAW_IDX {
                        self.switch_to(Slot::Draw, ctx);
                    } else if idx == BUILDER_IDX {
                        self.switch_to(Slot::Builder, ctx);
                    } else if idx == TODO_IDX {
                        self.switch_to(Slot::Todo, ctx);
                    }
                }
            }
            Slot::Notes => self.notes.handle(event, ctx),
            Slot::Address => self.address.handle(event, ctx),
            Slot::Draw => self.draw.handle(event, ctx),
            Slot::Builder => self.builder.handle(event, ctx),
            Slot::Todo => self.todo.handle(event, ctx),
        }
    }

    fn toggle_a11y(&mut self, ctx: &mut Ctx<'_>) {
        self.a11y_enabled = !self.a11y_enabled;
        if self.a11y_enabled {
            self.a11y_focus = Some(0);
            ctx.a11y.speak("Accessibility mode enabled");
            self.speak_focused(ctx);
        } else {
            self.a11y_focus = None;
            ctx.a11y.speak("Accessibility mode disabled");
        }
        ctx.invalidate_all();
    }
}

impl App for Host {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        if matches!(event, Event::AppStop) {
            if let Err(e) = self.launcher_icons.borrow().persist() {
                eprintln!("launcher: could not persist icon cache on shutdown: {e}");
            }
            self.forward_to_child(event, ctx);
            return;
        }

        // Toggle A11y Mode with Tab
        if matches!(event, Event::Key(KeyCode::Tab)) {
            self.toggle_a11y(ctx);
            return;
        }

        match event {
            Event::PenDown { x, y } => {
                self.pen_start = Some((x, y, ctx.now_ms));
                if (y as i32) >= STRIP_TOP {
                    self.strip_pressed = true;
                }
                if self.a11y_enabled { return; }
            }
            Event::PenUp { x, y } => {
                if let Some((x0, y0, t0)) = self.pen_start.take() {
                    let dx = x - x0;
                    let dy = y - y0;

                    if self.a11y_enabled && dx.abs() > 40 && dy.abs() < 40 {
                        // Swipe
                        let nodes = self.active_a11y_nodes();
                        if !nodes.is_empty() {
                            let mut idx = self.a11y_focus.unwrap_or(0);
                            if dx > 0 {
                                // Swipe Right -> Next
                                idx = (idx + 1) % nodes.len();
                            } else {
                                // Swipe Left -> Prev
                                if idx == 0 { idx = nodes.len() - 1; }
                                else { idx -= 1; }
                            }
                            self.a11y_focus = Some(idx);
                            self.speak_focused(ctx);
                            ctx.invalidate_all();
                        }
                        return;
                    } else if dx.abs() < 10 && dy.abs() < 10 {
                        // Tap detection
                        let mut triple_tap = false;
                        if let Some((lx, ly, lt)) = self.last_tap {
                            if (x - lx).abs() < 20 && (y - ly).abs() < 20 && (ctx.now_ms - lt) < 600 {
                                self.tap_count += 1;
                                if self.tap_count >= 3 {
                                    triple_tap = true;
                                    self.tap_count = 0;
                                }
                            } else {
                                self.tap_count = 1;
                            }
                        } else {
                            self.tap_count = 1;
                        }
                        self.last_tap = Some((x, y, ctx.now_ms));

                        if triple_tap {
                            self.toggle_a11y(ctx);
                            return;
                        }

                        if self.a11y_enabled {
                            // Double tap detection for activation
                            if self.tap_count == 2 {
                                self.activate_focused(ctx);
                            }
                            return;
                        }
                    }
                }
                
                if self.strip_pressed {
                    self.strip_pressed = false;
                    if hit_test(&strip_home_rect(), x, y) {
                        self.switch_to(Slot::Launcher, ctx);
                    } else if hit_test(&strip_menu_rect(), x, y) {
                        self.forward_to_child(Event::Menu, ctx);
                    }
                    return;
                }
            }
            _ => {}
        }

        if self.a11y_enabled { return; }

        // Hardware fallback still works.
        if matches!(event, Event::ButtonDown(HardButton::Home)) {
            self.switch_to(Slot::Launcher, ctx);
            return;
        }

        self.forward_to_child(event, ctx);
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        match self.active {
            Slot::Launcher => self.launcher.draw(canvas),
            Slot::Notes => self.notes.draw(canvas),
            Slot::Address => self.address.draw(canvas),
            Slot::Draw => self.draw.draw(canvas),
            Slot::Builder => self.builder.draw(canvas),
            Slot::Todo => self.todo.draw(canvas),
        }
        draw_system_strip(canvas, self.active_label());

        if self.a11y_enabled {
            if let Some(idx) = self.a11y_focus {
                let nodes = self.active_a11y_nodes();
                if let Some(node) = nodes.get(idx) {
                    let _ = node.bounds.into_styled(
                        PrimitiveStyle::with_stroke(BLACK, 2)
                    ).draw(canvas);
                    let inner = Rectangle::new(node.bounds.top_left + Point::new(1, 1), node.bounds.size.saturating_sub(Size::new(2, 2)));
                    let _ = inner.into_styled(
                        PrimitiveStyle::with_stroke(WHITE, 1)
                    ).draw(canvas);
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 2 && args[1] == "--test" {
        let scenario_name = &args[2];
        run_headless_test(scenario_name);
        return;
    }
    let mut platform = HostedPlatform::new("SoulOS", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
    run(&mut platform, Host::new());
}

fn run_headless_test(name: &str) {
    use soul_hal_hosted::testing::{scenarios, TestingPlatform};
    let scenario = match name {
        "build-todo" => scenarios::build_todo_app(),
        "verify-todo" => scenarios::verify_todo_app(),
        _ => {
            eprintln!("Unknown test scenario: {}", name);
            return;
        }
    };

    println!("Running headless test: {}", scenario.name);
    let mut platform = HostedPlatform::new("SoulOS Test", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
    let mut host = Host::new();
    let mut dirty = soul_core::Dirty::full();
    let mut a11y = soul_core::a11y::A11yManager::new();
    
    for event in scenario.events {
        println!("  → {} (Active: {})", event.description, host.active_label());
        let mut ctx = soul_core::Ctx {
            now_ms: 0, 
            dirty: &mut dirty,
            a11y: &mut a11y,
        };
        
        // Map InputEvent to Event
        let core_event = match event.event {
            soul_hal::InputEvent::StylusDown { x, y } => soul_core::Event::PenDown { x, y },
            soul_hal::InputEvent::StylusMove { x, y } => soul_core::Event::PenMove { x, y },
            soul_hal::InputEvent::StylusUp { x, y } => soul_core::Event::PenUp { x, y },
            soul_hal::InputEvent::Key(k) => soul_core::Event::Key(k),
            soul_hal::InputEvent::ButtonDown(soul_hal::HardButton::Menu) => soul_core::Event::Menu,
            soul_hal::InputEvent::ButtonDown(b) => soul_core::Event::ButtonDown(b),
            soul_hal::InputEvent::ButtonUp(b) => soul_core::Event::ButtonUp(b),
            _ => continue,
        };
        
        host.handle(core_event, &mut ctx);
        // Simulate a draw pass to process logic
        let mut display = platform.get_display_buffer().clone();
        host.draw(&mut display);
        
        std::thread::sleep(std::time::Duration::from_millis(event.delay_ms));
    }
    println!("Headless test finished.");
}

// --- PGM Utilities ------------------------------------------------------
// These functions are copied from `soul-runner/src/draw.rs` to allow `main.rs`
// to load PGM images directly for assets without creating a hard dependency
// between `main.rs` and `draw.rs` modules.

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

fn read_pgm_whitespace_line<R: io::BufRead>(r: &mut R) -> io::Result<(usize, usize)> {
    let mut line = String::new();
    loop {
        line.clear();
        r.read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.splitn(2, ' ');
        let w = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "expected width"))?;
        let h = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "expected height"))?;
        return Ok((w, h));
    }
}

fn read_pgm_whitespace_line_value<R: io::BufRead>(r: &mut R) -> io::Result<usize> {
    let mut line = String::new();
    loop {
        line.clear();
        r.read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        let v = trimmed
            .parse()
            .ok()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "expected value"))?;
        return Ok(v);
    }
}

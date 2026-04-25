//! Paint — a MacPaint / PadPaint-style drawing application for SoulOS.
//!
//! Screen layout (240 × 304 usable):
//!
//!   ┌──────────────────────────────┐  y = 0
//!   │        Title bar (15 px)     │
//!   ├──────┬───────────────────────┤  y = 15
//!   │      │                       │
//!   │ Tool │      Canvas           │
//!   │  pal │   (188 × 289 px)      │
//!   │  (52)│                       │
//!   │ LWid │                       │
//!   │ Patt │                       │
//!   └──────┴───────────────────────┘  y = 303
//!      52px          188px
//!
//! # Tool sprite sheet
//!
//! The tool palette is rendered from a single sprite sheet:
//!
//!   assets/sprites/paint_tools/paint_tools.pgm
//!
//! Layout within the sheet (41 × 189, maxval=1, binary black/white):
//!   Left  column: x = 1..18  (18 px wide)
//!   Right column: x = 20..37 (18 px wide)
//!   8 row bands (y ranges): see TOOL_BANDS below.
//!
//! The mapping of PALETTE_CELLS order → sheet sub-rect is defined by
//! TOOL_BANDS + column selection (cell_idx % 2).  Re-order PALETTE_CELLS
//! or adjust TOOL_BANDS to rearrange the palette.
//!
//! The selected tool is rendered with inverted pixels (white-on-black),
//! matching the classic MacPaint selection indicator.  A short text label
//! is used as a fallback if the sheet file is missing.

use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{ascii::FONT_5X8, MonoTextStyle},
    pixelcolor::Gray8,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use soul_core::{App, Ctx, Event, APP_HEIGHT, SCREEN_WIDTH};
use soul_script::SystemRequest;
use soul_ui::{hit_test, title_bar, BLACK, TITLE_BAR_H, WHITE};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const PALETTE_W: i32 = 40;
const CANVAS_X: i32 = PALETTE_W;
const CANVAS_Y: i32 = TITLE_BAR_H as i32;
const CANVAS_W: i32 = SCREEN_WIDTH as i32 - PALETTE_W;
const CANVAS_H: i32 = APP_HEIGHT as i32 - CANVAS_Y;

const CANVAS_PIXELS: usize = CANVAS_W as usize * CANVAS_H as usize;

/// Each tool cell in the palette — 20×20 px square.
const TOOL_CELL_W: i32 = PALETTE_W / 2; // 20 px
const TOOL_CELL_H: i32 = 20;
const TOOL_ROWS: i32 = 8;
const TOOLS_AREA_H: i32 = TOOL_ROWS * TOOL_CELL_H; // 160 px

/// Line-width selector — four strokes of increasing thickness, stacked.
const LW_Y: i32 = CANVAS_Y + TOOLS_AREA_H + 2;
const LW_COUNT: usize = 4;
const LW_CELL_H: i32 = 12;

/// Pattern strip — 8 patterns in two rows of 4, each cell 20×20 px.
const PAT_Y: i32 = LW_Y + LW_COUNT as i32 * LW_CELL_H + 2;
const PAT_CELL_W: i32 = PALETTE_W / 2; // 20 px
const PAT_CELL_H: i32 = 20;

/// Undo depth limit.
const UNDO_DEPTH: usize = 16;

// ---------------------------------------------------------------------------
// Tool sprite sheet
// ---------------------------------------------------------------------------

/// The full paint_tools.pgm loaded as Gray8 (maxval-normalised to 0/255).
struct ToolSheet {
    w: usize,
    #[allow(dead_code)]
    h: usize,
    /// Gray8 pixels: 0 = black (ink), 255 = white (paper).
    pixels: Vec<u8>,
}

/// X origin and width of the left and right columns inside the sheet.
const SHEET_LEFT_X: usize = 1;
const SHEET_RIGHT_X: usize = 20;
const SHEET_COL_W: usize = 18;

/// (y_start, y_end inclusive) for each of the 8 tool rows in the sheet.
/// Adjust these if the sheet is regenerated at different dimensions.
const TOOL_BANDS: [(usize, usize); 8] = [
    (1,   18),   // row 0: Pen      | Eraser
    (20,  37),   // row 1: Lines    | Selection
    (39,  56),   // row 2: Text     | Scroll
    (58,  75),   // row 3: Brush    | Spray
    (77,  92),   // row 4: SolidRect| RoundRect
    (94,  110),  // row 5: Frame    | RoundFrame
    (112, 129),  // row 6: Undo     | Magnify
    (131, 148),  // row 7: Transparent | (empty)
];

impl ToolSheet {
    fn load() -> Option<Self> {
        let path = PathBuf::from("assets/sprites/paint_tools/paint_tools.pgm");
        let (w, h, pixels) = load_pgm(&path).ok()?;
        Some(ToolSheet { w, h, pixels })
    }

    /// Blit the sub-rectangle defined by `PALETTE_CELLS[cell_idx]` into
    /// `dst`, centred and clipped.  `invert` swaps black/white for the
    /// MacPaint selected-tool highlight.
    fn blit_cell<D: DrawTarget<Color = Gray8>>(
        &self,
        canvas: &mut D,
        cell_idx: usize,
        dst: &Rectangle,
        invert: bool,
    ) {
        let row = cell_idx / 2;
        let col = cell_idx % 2;
        let Some(&(sy0, sy1)) = TOOL_BANDS.get(row) else { return };
        let sx0 = if col == 0 { SHEET_LEFT_X } else { SHEET_RIGHT_X };
        let src_w = SHEET_COL_W;
        let src_h = sy1 - sy0 + 1;

        let cw = dst.size.width as i32;
        let ch = dst.size.height as i32;
        // Centre the sprite in the cell.
        let ox = dst.top_left.x + (cw - src_w as i32) / 2;
        let oy = dst.top_left.y + (ch - src_h as i32) / 2;

        for dy in 0..src_h as i32 {
            for dx in 0..src_w as i32 {
                let screen_x = ox + dx;
                let screen_y = oy + dy;
                // Clip to cell bounds.
                if screen_x < dst.top_left.x
                    || screen_x >= dst.top_left.x + cw
                    || screen_y < dst.top_left.y
                    || screen_y >= dst.top_left.y + ch
                {
                    continue;
                }
                let pi = (sy0 + dy as usize) * self.w + sx0 + dx as usize;
                let v = self.pixels[pi];
                let v = if invert { 255 - v } else { v };
                let _ = Pixel(Point::new(screen_x, screen_y), Gray8::new(v)).draw(canvas);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Eraser,
    Lines,
    Selection,
    Text,
    Scroll,
    Brush,
    Spray,
    SolidRect,
    RoundRect,
    Frame,
    RoundFrame,
    Magnify,
    Transparent,
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::Pen => "Pen",
            Tool::Eraser => "Eraser",
            Tool::Lines => "Lines",
            Tool::Selection => "Select",
            Tool::Text => "Text",
            Tool::Scroll => "Scroll",
            Tool::Brush => "Brush",
            Tool::Spray => "Spray",
            Tool::SolidRect => "SolidRect",
            Tool::RoundRect => "RndRect",
            Tool::Frame => "Frame",
            Tool::RoundFrame => "RndFrame",
            Tool::Magnify => "Magnify",
            Tool::Transparent => "Transp",
        }
    }

    /// Short label used when no sprite sheet is available (≤ 5 chars).
    fn short_label(self) -> &'static str {
        match self {
            Tool::Pen => "Pen",
            Tool::Eraser => "Erase",
            Tool::Lines => "Lines",
            Tool::Selection => "Sel",
            Tool::Text => "Text",
            Tool::Scroll => "Scrl",
            Tool::Brush => "Brush",
            Tool::Spray => "Spray",
            Tool::SolidRect => "SRect",
            Tool::RoundRect => "RRect",
            Tool::Frame => "Frame",
            Tool::RoundFrame => "RFrm",
            Tool::Magnify => "Zoom",
            Tool::Transparent => "Trns",
        }
    }
}

/// Left-to-right, top-to-bottom order of the two-column palette grid.
/// `None` at index 12 = Undo action button (not a persistent tool mode).
const PALETTE_CELLS: &[Option<Tool>; 16] = &[
    Some(Tool::Pen),         Some(Tool::Eraser),
    Some(Tool::Lines),       Some(Tool::Selection),
    Some(Tool::Text),        Some(Tool::Scroll),
    Some(Tool::Brush),       Some(Tool::Spray),
    Some(Tool::SolidRect),   Some(Tool::RoundRect),
    Some(Tool::Frame),       Some(Tool::RoundFrame),
    None,                    Some(Tool::Magnify),   // None = Undo action
    Some(Tool::Transparent), None,
];

// ---------------------------------------------------------------------------
// Patterns
// ---------------------------------------------------------------------------

/// Eight 8×8 fill patterns.
/// Indices 2 and 3 are solid gray shades handled by `pattern_value`;
/// their bit rows are unused.
/// Each element is one row; MSB = leftmost pixel.
pub const PATTERNS: [[u8; 8]; 8] = [
    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], // 0: solid black
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // 1: white
    [0x00; 8],                                          // 2: dark gray (solid 64)
    [0x00; 8],                                          // 3: light gray (solid 192)
    [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01], // 4: diagonal
    [0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55], // 5: 50% dots
    [0xFF, 0x88, 0x88, 0x88, 0xFF, 0x88, 0x88, 0x88], // 6: crosshatch
    [0x88, 0x88, 0x88, 0x88, 0x88, 0x88, 0x88, 0x88], // 7: vert lines
];

/// Gray8 pixel value for a given pattern at canvas position `(x, y)`.
/// Returns 0 (black) through 255 (white). Patterns 2 and 3 are solid
/// gray shades rather than dithered 1-bit tiles.
#[inline]
pub fn pattern_value(pat: usize, x: i32, y: i32) -> u8 {
    match pat {
        2 => 64,   // dark gray
        3 => 192,  // light gray
        _ => {
            let row = PATTERNS[pat][(y.rem_euclid(8)) as usize];
            if (row >> (7 - x.rem_euclid(8))) & 1 == 1 { 0 } else { 255 }
        }
    }
}


// ---------------------------------------------------------------------------
// Touch target
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TouchTarget {
    None,
    Canvas,
    ToolCell(usize),
    UndoBtn,
    ClearBtn,
    LineWidth(usize),
    Pattern(usize),
}

// ---------------------------------------------------------------------------
// PixelRect — minimal canvas sub-region snapshot
// ---------------------------------------------------------------------------

/// A row-major snapshot of a rectangular canvas sub-region.
///
/// Used for both the rubber-band ghost (a small region that moves each frame)
/// and for undo records (only the bounding rect of each stroke is stored,
/// not the full canvas).  This keeps the undo stack tiny: a typical stroke
/// uses a few hundred bytes instead of the full ~54 KB canvas.
#[derive(Clone)]
struct PixelRect {
    x0: i32,
    y0: i32,
    w:  usize,
    h:  usize,
    data: Vec<u8>,
}

impl PixelRect {
    /// Capture a canvas-local rectangle from `src` (the full canvas buffer).
    /// Coordinates are clamped to canvas bounds.  Returns `None` if the
    /// resulting rect is empty.
    fn capture(src: &[u8], bx0: i32, by0: i32, bx1: i32, by1: i32) -> Option<Self> {
        let x0 = bx0.max(0);
        let y0 = by0.max(0);
        let x1 = bx1.min(CANVAS_W - 1);
        let y1 = by1.min(CANVAS_H - 1);
        if x1 < x0 || y1 < y0 { return None; }
        let w  = (x1 - x0 + 1) as usize;
        let h  = (y1 - y0 + 1) as usize;
        let cw = CANVAS_W as usize;
        let mut data = vec![255u8; w * h];
        for row in 0..h {
            let src_off = (y0 as usize + row) * cw + x0 as usize;
            let dst_off = row * w;
            data[dst_off..dst_off + w].copy_from_slice(&src[src_off..src_off + w]);
        }
        Some(Self { x0, y0, w, h, data })
    }

    /// Write this region's pixels back into `dst` (the full canvas buffer).
    fn restore_to(&self, dst: &mut [u8]) {
        let cw = CANVAS_W as usize;
        for row in 0..self.h {
            let dst_off = (self.y0 as usize + row) * cw + self.x0 as usize;
            let src_off = row * self.w;
            dst[dst_off..dst_off + self.w]
                .copy_from_slice(&self.data[src_off..src_off + self.w]);
        }
    }

    /// Canvas-local bounding box (x0, y0, x1, y1).
    fn bounds(&self) -> (i32, i32, i32, i32) {
        (self.x0, self.y0,
         self.x0 + self.w as i32 - 1,
         self.y0 + self.h as i32 - 1)
    }
}

// ---------------------------------------------------------------------------
// Selection state
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Selection {
    x0: i32, y0: i32,
    x1: i32, y1: i32,
    /// Pixels lifted from the canvas, if any.
    #[allow(dead_code)]
    pixels: Option<Vec<u8>>,
}

// ---------------------------------------------------------------------------
// Main struct
// ---------------------------------------------------------------------------

pub struct Paint {
    db: soul_db::Database,
    db_path: PathBuf,

    /// Canvas pixel buffer, Gray8 row-major.  255 = white (paper).
    pixels: Vec<u8>,

    tool: Tool,
    pen_width: usize, // 0 = thinnest (1 px), 3 = widest (4 px)
    pattern: usize,    // index into PATTERNS

    pen_active: bool,
    pen_pos: (i32, i32),
    pen_start: (i32, i32),
    anchor: Option<(i32, i32)>,

    /// Persistent full-canvas "clean" buffer.  Always kept in sync with
    /// `pixels` after each completed operation (imprinted at pen_up).
    ///
    /// Role:
    ///  - Rubber-band erase: copy ghost[rubber_rect] → pixels each frame.
    ///  - Undo source:       capture ghost[dirty_rect] → undo record at pen_up.
    ///  - No capture on pen_down, ever.
    ghost: Vec<u8>,
    /// Canvas-local bounds of the last rubber-band preview draw.
    rubber_rect: Option<(i32, i32, i32, i32)>,
    /// Growing bounding rect of all pixels dirtied in the current freehand stroke.
    stroke_bounds: Option<(i32, i32, i32, i32)>,

    selection: Option<Selection>,
    /// True while the user is dragging (moving) an existing selection.
    sel_dragging: bool,

    /// Undo stack — each entry is a small PixelRect (the bounding box of the
    /// change), never a full-canvas clone.
    undo_stack: Vec<PixelRect>,

    /// Marching-ants animation state.
    ant_phase: usize,   // 0..=6, advances by 2 each step
    ant_last_ms: u64,   // platform time of last phase step

    touch: TouchTarget,
    menu_open: bool,

    /// The paint_tools.pgm sprite sheet; `None` if the file is missing.
    tool_sheet: Option<ToolSheet>,
}

impl Paint {
    pub const APP_ID: &'static str = "com.soulos.paint";
    pub const NAME: &'static str = "Paint";

    pub fn new(db_path: PathBuf) -> Self {
        let (db, pixels) = load_db(&db_path);
        let ghost = pixels.clone();  // ghost starts equal to screen; one clone at startup only
        let tool_sheet = ToolSheet::load();
        Self {
            db,
            db_path,
            pixels,
            tool: Tool::Pen,
            pen_width: 0,
            pattern: 0,
            pen_active: false,
            pen_pos: (0, 0),
            pen_start: (0, 0),
            anchor: None,
            ghost,
            rubber_rect: None,
            stroke_bounds: None,
            selection: None,
            sel_dragging: false,
            undo_stack: Vec::new(),
            ant_phase: 0,
            ant_last_ms: 0,
            touch: TouchTarget::None,
            menu_open: false,
            tool_sheet,
        }
    }

    pub fn persist(&mut self) {
        save_db(&self.db, &self.db_path, &self.pixels);
    }

    // -----------------------------------------------------------------------
    // Geometry helpers
    // -----------------------------------------------------------------------

    fn canvas_rect() -> Rectangle {
        Rectangle::new(
            Point::new(CANVAS_X, CANVAS_Y),
            Size::new(CANVAS_W as u32, CANVAS_H as u32),
        )
    }

    fn screen_to_canvas(sx: i16, sy: i16) -> Option<(i32, i32)> {
        let cx = sx as i32 - CANVAS_X;
        let cy = sy as i32 - CANVAS_Y;
        if cx >= 0 && cy >= 0 && cx < CANVAS_W && cy < CANVAS_H {
            Some((cx, cy))
        } else {
            None
        }
    }

    fn canvas_index(x: i32, y: i32) -> Option<usize> {
        if x >= 0 && y >= 0 && x < CANVAS_W && y < CANVAS_H {
            Some(y as usize * CANVAS_W as usize + x as usize)
        } else {
            None
        }
    }

    fn tool_cell_rect(cell: usize) -> Rectangle {
        let col = (cell % 2) as i32;
        let row = (cell / 2) as i32;
        Rectangle::new(
            Point::new(col * TOOL_CELL_W, CANVAS_Y + row * TOOL_CELL_H),
            Size::new(TOOL_CELL_W as u32, TOOL_CELL_H as u32),
        )
    }

    fn lw_cell_rect(i: usize) -> Rectangle {
        Rectangle::new(
            Point::new(0, LW_Y + i as i32 * LW_CELL_H),
            Size::new(PALETTE_W as u32, LW_CELL_H as u32),
        )
    }

    fn pat_cell_rect(i: usize) -> Rectangle {
        let col = (i % 2) as i32;
        let row = (i / 2) as i32;
        Rectangle::new(
            Point::new(col * PAT_CELL_W, PAT_Y + row * PAT_CELL_H),
            Size::new(PAT_CELL_W as u32, PAT_CELL_H as u32),
        )
    }

    fn hit_palette(sx: i16, sy: i16) -> TouchTarget {
        for i in 0..PALETTE_CELLS.len() {
            if hit_test(&Self::tool_cell_rect(i), sx, sy) {
                return if i == 12 && PALETTE_CELLS[i].is_none() {
                    TouchTarget::UndoBtn
                } else if i == 15 && PALETTE_CELLS[i].is_none() {
                    TouchTarget::ClearBtn
                } else {
                    TouchTarget::ToolCell(i)
                };
            }
        }
        for i in 0..LW_COUNT {
            if hit_test(&Self::lw_cell_rect(i), sx, sy) {
                return TouchTarget::LineWidth(i);
            }
        }
        for i in 0..8 {
            if hit_test(&Self::pat_cell_rect(i), sx, sy) {
                return TouchTarget::Pattern(i);
            }
        }
        TouchTarget::None
    }

    // -----------------------------------------------------------------------
    // Undo
    // -----------------------------------------------------------------------

    /// Push a pre-captured `PixelRect` onto the undo stack.
    fn push_undo_record(&mut self, pr: PixelRect) {
        if self.undo_stack.len() >= UNDO_DEPTH {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(pr);
    }

    fn pop_undo(&mut self, ctx: &mut Ctx<'_>) {
        if let Some(pr) = self.undo_stack.pop() {
            let b = pr.bounds();
            // Restore to BOTH screen and ghost so they stay in sync.
            pr.restore_to(&mut self.pixels);
            pr.restore_to(&mut self.ghost);
            Self::invalidate_bounds(ctx, b);
        }
    }

    fn clear_canvas(&mut self, ctx: &mut Ctx<'_>) {
        // Capture ghost (= pre-op state) for the full canvas.
        if let Some(pr) = PixelRect::capture(&self.ghost, 0, 0, CANVAS_W - 1, CANVAS_H - 1) {
            self.push_undo_record(pr);
        }
        for px in self.pixels.iter_mut() { *px = 255; }
        for px in self.ghost.iter_mut()  { *px = 255; }  // keep in sync
        ctx.invalidate(Self::canvas_rect());
    }

    // -----------------------------------------------------------------------
    // Selection capture / commit
    // -----------------------------------------------------------------------

    /// Copy the pixels under `sel` from the canvas into `sel.pixels`, then
    /// white-out that region so the background is revealed.  Call this once
    /// when a drag starts (analogous to `CaptureToWorkWindow` + erase).
    fn capture_selection(&mut self) {
        let sel = match self.selection.as_mut() {
            Some(s) => s,
            None => return,
        };
        if sel.pixels.is_some() { return; }  // already captured

        let x0 = sel.x0.max(0) as usize;
        let y0 = sel.y0.max(0) as usize;
        let x1 = (sel.x1).min(CANVAS_W - 1) as usize;
        let y1 = (sel.y1).min(CANVAS_H - 1) as usize;
        if x1 < x0 || y1 < y0 { return; }

        let sw = x1 - x0 + 1;
        let sh = y1 - y0 + 1;
        let mut captured = vec![255u8; sw * sh];

        for dy in 0..sh {
            for dx in 0..sw {
                let idx = (y0 + dy) * CANVAS_W as usize + (x0 + dx);
                let v = self.pixels[idx];
                captured[dy * sw + dx] = v;
                self.pixels[idx] = 255; // erase from background
            }
        }
        if let Some(ref mut s) = self.selection {
            s.pixels = Some(captured);
        }
    }

    /// Blit the captured selection pixels back into `self.pixels` at the
    /// current selection position.  Call on drag-end or before discarding
    /// the selection.  White pixels are transparent (not blitted).
    fn commit_selection(&mut self) {
        let (x0, y0, pixels) = match self.selection.as_mut() {
            Some(s) => {
                let cap = match s.pixels.take() {
                    Some(p) => p,
                    None => return,
                };
                (s.x0, s.y0, cap)
            }
            None => return,
        };

        let sw = match self.selection.as_ref() {
            Some(s) => (s.x1 - s.x0 + 1) as usize,
            None => return,
        };
        let sh = pixels.len() / sw;

        for dy in 0..sh {
            for dx in 0..sw {
                let v = pixels[dy * sw + dx];
                if v != 255 {
                    self.put_pixel(x0 + dx as i32, y0 + dy as i32, v);
                }
            }
        }
    }

    /// Return a screen-space `Rectangle` for a canvas-local strip, clamped
    /// to the canvas bounds.  Used for dirty-rect invalidation during drag.
    fn canvas_strip_rect(cx: i32, cy: i32, w: i32, h: i32) -> Option<Rectangle> {
        let x0 = cx.max(0);
        let y0 = cy.max(0);
        let x1 = (cx + w - 1).min(CANVAS_W - 1);
        let y1 = (cy + h - 1).min(CANVAS_H - 1);
        if x1 < x0 || y1 < y0 { return None; }
        Some(Rectangle::new(
            Point::new(CANVAS_X + x0, CANVAS_Y + y0),
            Size::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32),
        ))
    }

    // -----------------------------------------------------------------------
    // Rubber-band helpers  (port of SetRectFromTwoPoints / GetUnionRect)
    // -----------------------------------------------------------------------

    /// Canvas-local bounding box between two arbitrary points.
    /// Returns (x0, y0, x1, y1) with x0≤x1, y0≤y1, expanded by `margin`
    /// pixels to cover thick-pen strokes.
    fn rubber_bounds(ax: i32, ay: i32, bx: i32, by: i32, margin: i32) -> (i32, i32, i32, i32) {
        (
            ax.min(bx) - margin,
            ay.min(by) - margin,
            ax.max(bx) + margin,
            ay.max(by) + margin,
        )
    }

    /// Union of two canvas-local bounding boxes.
    fn union_bounds(
        a: (i32, i32, i32, i32),
        b: (i32, i32, i32, i32),
    ) -> (i32, i32, i32, i32) {
        (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
    }

    /// Invalidate a canvas-local bounding box in screen space.
    fn invalidate_bounds(ctx: &mut Ctx<'_>, b: (i32, i32, i32, i32)) {
        let x0 = (b.0).max(0);
        let y0 = (b.1).max(0);
        let x1 = (b.2).min(CANVAS_W - 1);
        let y1 = (b.3).min(CANVAS_H - 1);
        if x1 < x0 || y1 < y0 { return; }
        ctx.invalidate(Rectangle::new(
            Point::new(CANVAS_X + x0, CANVAS_Y + y0),
            Size::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32),
        ));
    }

    /// Copy rows of `src` into `dst` for a canvas-local bounding box.
    /// Used for both ghost→pixels (rubber-band erase) and pixels→ghost (imprint).
    /// Takes separate slice refs so field-level borrow splitting works at call sites.
    fn copy_region(src: &[u8], dst: &mut [u8], b: (i32, i32, i32, i32)) {
        let x0 = b.0.max(0) as usize;
        let y0 = b.1.max(0) as usize;
        let x1 = b.2.min(CANVAS_W - 1) as usize;
        let y1 = b.3.min(CANVAS_H - 1) as usize;
        if x1 < x0 || y1 < y0 { return; }
        let w   = CANVAS_W as usize;
        let len = x1 - x0 + 1;
        for y in y0..=y1 {
            let off = y * w + x0;
            dst[off..off + len].copy_from_slice(&src[off..off + len]);
        }
    }

    fn set_pen_width(&mut self, width: usize) -> usize {
        let old = self.pen_width;
        self.pen_width = width;
        old
    }


    // Maps CnvRoundRect — draws a rounded rectangle or circle.
    ///
    /// `diam_x` / `diam_y` control the corner ellipse diameters.
    /// Pass 32767 / 32767 for a full circle/ellipse.
    pub fn draw_round_rect(&mut self, ctx: &mut Ctx<'_> , r: &Rectangle, mut diam_x: i32, mut diam_y: i32) {
        let width = r.size.width as i32;
        let height = r.size.height as i32;
        let mut a0: i32;
        let mut b0: i32;
        let mut x:  i32;
        let mut y:  i32;
        let mut x1: i32;
        let mut y1: i32;

        // long (32-bit) precision in the original; use i64 for safety
        let asquared:      i64;
        let two_asquared:  i64;
        let bsquared:      i64;
        let two_bsquared:  i64;
        let mut d:  i64;
        let mut dx: i64;
        let mut dy: i64;

        let asquared1:     i64;
        let two_asquared1: i64;
        let bsquared1:     i64;
        let two_bsquared1: i64;
        let mut d1:  i64;
        let mut dx1: i64;
        let mut dy1: i64;

        let mut ry = Rectangle::default();
        let square_edge: bool;
        let solid_fill:  bool;

        // Save the caller's border-thickness (actual pixels) and set the
        // internal draw radius to 0 (= 1-pixel spans).  In the C original
        // WinSetPenWidth(1) meant "1 pixel"; our draw_line uses pen_width as
        // a circle radius, so 0 = single pixel, 1 = 3-pixel stamp.
        let pen_width = self.set_pen_width(0) as i32;

        if diam_x != 0 {
            diam_x += pen_width << 1;
        }
        if diam_y != 0 {
            diam_y += pen_width << 1;
        }

        if diam_x > width { diam_x = width; }
        if diam_y > height { diam_y = height; }
        if diam_x < 2 { diam_x = 0; }
        if diam_y < 2 { diam_y = 0; }

        square_edge = (diam_x | diam_y) == 0;
        solid_fill  = (pen_width > (width >> 1)) || (pen_width > (height >> 1));

        let extra_x: i32;
        let extra_y = height - diam_y;
        a0 = diam_x >> 1;
        b0 = diam_y >> 1;
        let xc_left   = r.top_left.x + a0;
        let xc_right  = xc_left + width - diam_x - 1;
        let yc_top    = r.top_left.y + b0;
        let yc_bottom = yc_top + height - diam_y - 1;

        if extra_y != 0 {
            if solid_fill {
                extra_x = width;
            } else {
                extra_x = pen_width;
            }
            // Left (or full-width when solid) vertical strip along the straight edge.
            self.draw_rect_solid(
                r.top_left.x,
                r.top_left.y + b0,
                r.top_left.x + extra_x - 1,
                r.top_left.y + b0 + extra_y - 1,
                ctx,
            );
            if !solid_fill {
                // Right vertical strip.
                self.draw_rect_solid(
                    r.top_left.x + width - extra_x,
                    r.top_left.y + b0,
                    r.top_left.x + width - 1,
                    r.top_left.y + b0 + extra_y - 1,
                    ctx,
                );
            }
            if square_edge {
                if !solid_fill {
                    // Top horizontal strip.
                    self.draw_rect_solid(
                        r.top_left.x + pen_width,
                        r.top_left.y,
                        r.top_left.x + width - pen_width - 1,
                        r.top_left.y + pen_width - 1,
                        ctx,
                    );
                    // Bottom horizontal strip.
                    self.draw_rect_solid(
                        r.top_left.x + pen_width,
                        r.top_left.y + height - pen_width,
                        r.top_left.x + width - pen_width - 1,
                        r.top_left.y + height - 1,
                        ctx,
                    );
                }
                self.set_pen_width(pen_width as usize);
                return;
            }
        }

        // ── Outer arc ─────────────────────────────────────────────────────────────
        x = 0;
        y = b0;
        asquared     = (a0 as i64) * (a0 as i64);
        two_asquared = asquared << 1;
        bsquared     = (b0 as i64) * (b0 as i64);
        two_bsquared = bsquared << 1;
        d  = bsquared - asquared * (b0 as i64) + (asquared >> 2);
        dx = 0;
        dy = two_asquared * (b0 as i64);

        // ── Inner arc (inset by pen_width − 1) ────────────────────────────────────
        a0 -= pen_width - 1;
        b0 -= pen_width - 1;
        if a0 < 0 { a0 = 0; }
        if b0 < 0 { b0 = 0; }
        x1 = 0;
        y1 = b0;
        asquared1     = (a0 as i64) * (a0 as i64);
        two_asquared1 = asquared1 << 1;
        bsquared1     = (b0 as i64) * (b0 as i64);
        two_bsquared1 = bsquared1 << 1;
        d1  = bsquared1 - asquared1 * (b0 as i64) + (asquared1 >> 2);
        dx1 = 0;
        dy1 = two_asquared1 * (b0 as i64);

        // ── Phase 1: upper-half region (dx < dy) ──────────────────────────────────
        while dx < dy {
            if d > 0 {
                self.set4_pixels(ctx, xc_left, xc_right, yc_top, yc_bottom, x1, x, y);
                y -= 1;
                while (y < y1) && (dx1 < dy1) {
                    if d1 > 0 {
                        y1  -= 1;
                        dy1 -= two_asquared1;
                        d1  -= dy1;
                    }
                    x1  += 1;
                    dx1 += two_bsquared1;
                    d1  += bsquared1 + dx1;
                }
                dy -= two_asquared;
                d  -= dy;
            }
            x  += 1;
            dx += two_bsquared;
            d  += bsquared + dx;
        }

        d += ((3 * (asquared - bsquared) >> 1) - (dx + dy)) >> 1;

        // ── Phase 2: lower-half region, outer still ahead of inner ────────────────
        while (y >= 0) && (dx1 < dy1) {
            self.set4_pixels(ctx, xc_left, xc_right, yc_top, yc_bottom, x1, x, y);
            if d < 0 {
                x  += 1;
                dx += two_bsquared;
                d  += dx;
            }
            y -= 1;
            while (y < y1) && (dx1 < dy1) {
                if d1 > 0 {
                    y1  -= 1;
                    dy1 -= two_asquared1;
                    d1  -= dy1;
                }
                x1  += 1;
                dx1 += two_bsquared1;
                d1  += bsquared1 + dx1;
            }
            dy -= two_asquared;
            d  += asquared - dy;
        }

        d1 += ((3 * (asquared1 - bsquared1) >> 1) - (dx1 + dy1)) >> 1;

        // ── Phase 3: finish outer arc after inner arc is complete ─────────────────
        while y >= 0 {
            self.set4_pixels(ctx, xc_left, xc_right, yc_top, yc_bottom, x1, x, y);
            if d < 0 {
                x  += 1;
                dx += two_bsquared;
                d  += dx;
            }
            y -= 1;
            if y < y1 {
                y1 -= 1;
                if d1 < 0 {
                    x1  += 1;
                    dx1 += two_bsquared1;
                    d1  += dx1;
                }
                dy1 -= two_asquared1;
                d1  += asquared1 - dy1;
            }
            dy -= two_asquared;
            d  += asquared - dy;
        }

        self.set_pen_width(pen_width as usize);
    }

    // ── Internal helper ───────────────────────────────────────────────────────────

    /// Maps Set4Pixels — draws symmetric horizontal spans in all four quadrants.
    fn set4_pixels(&mut self, ctx: &mut Ctx<'_>,
        xc_left: i32, xc_right: i32,
        yc_top: i32,  yc_bottom: i32,
        x1: i32, x: i32, y: i32,
    ) {
        let y_top         = yc_top    - y;
        let y_bottom      = yc_bottom + y;
        let x_left_start  = xc_left   - x;
        let x_left_end    = xc_left   - x1;
        let x_right_start = xc_right  + x1;
        let x_right_end   = xc_right  + x;

        if x1 != 0 {
            self.draw_line(x_right_start, y_bottom, x_right_end,  y_bottom, ctx);
            self.draw_line(x_left_start,  y_bottom, x_left_end,   y_bottom, ctx);
            // api.win_pattern_line(x_right_start, y_bottom, x_right_end,  y_bottom);
            // api.win_pattern_line(x_left_start,  y_bottom, x_left_end,   y_bottom);
            if y != 0 {
                self.draw_line(x_right_start, y_top, x_right_end, y_top, ctx);
                self.draw_line(x_left_start,  y_top, x_left_end,  y_top, ctx);
                // api.win_pattern_line(x_right_start, y_top, x_right_end, y_top);
                // api.win_pattern_line(x_left_start,  y_top, x_left_end,  y_top);
            }
        } else {
            self.draw_line(x_left_start, y_bottom, x_right_end, y_bottom, ctx);
            //api.win_pattern_line(x_left_start, y_bottom, x_right_end, y_bottom);
            if y != 0 {
                self.draw_line(x_left_start, y_top, x_right_end, y_top, ctx);
                //api.win_pattern_line(x_left_start, y_top, x_right_end, y_top);
            }
        }
    }




    // -----------------------------------------------------------------------
    // Pixel helpers
    // -----------------------------------------------------------------------

    pub fn put_pixel(&mut self, x: i32, y: i32, value: u8) {
        if let Some(i) = Self::canvas_index(x, y) {
            self.pixels[i] = value;
        }
    }

    pub fn get_pixel(&self, x: i32, y: i32) -> u8 {
        Self::canvas_index(x, y).map(|i| self.pixels[i]).unwrap_or(255)
    }

    /// Filled circle at `(cx, cy)` of radius `r`, drawn with the current pattern.
    pub fn stamp_circle(&mut self, cx: i32, cy: i32, r: i32, ctx: &mut Ctx<'_>) {
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r2 {
                    let px = cx + dx;
                    let py = cy + dy;
                    let v = pattern_value(self.pattern, px, py);
                    self.put_pixel(px, py, v);
                    invalidate_pixel(ctx, px, py);
                }
            }
        }
    }

    /// Bresenham line, stamping a circle of radius `line_width` at each step.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, ctx: &mut Ctx<'_>) {
        let r = self.pen_width as i32;
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        loop {
            self.stamp_circle(x, y, r, ctx);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 > -dy { err -= dy; x += sx; }
            if e2 < dx  { err += dx; y += sy; }
        }
    }

    /// Erase a circle of radius `r` centred at `(cx, cy)`.
    pub fn erase_circle(&mut self, cx: i32, cy: i32, r: i32, ctx: &mut Ctx<'_>) {
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r2 {
                    self.put_pixel(cx + dx, cy + dy, 255);
                    invalidate_pixel(ctx, cx + dx, cy + dy);
                }
            }
        }
    }

    /// Erase a square of half-width `r` centred at `(cx, cy)`.
    pub fn erase_square(&mut self, cx: i32, cy: i32, r: i32, ctx: &mut Ctx<'_>) {
        for dy in -r..=r {
            for dx in -r..=r {
                self.put_pixel(cx + dx, cy + dy, 255);
                invalidate_pixel(ctx, cx + dx, cy + dy);
            }
        }
    }

    /// Port of the original C Bresenham variant.
    ///
    /// Rather than stamping at every pixel, it accumulates runs of non-diagonal
    /// steps and emits a (x1,y1)→(x,y) rectangle segment each time a diagonal
    /// step is taken (and once more for the final run).  Segments are collected
    /// first to satisfy the borrow-checker, then painted.
    fn bresenham_segments(xstart: i32, ystart: i32, xend: i32, yend: i32) -> Vec<(i32, i32, i32, i32)> {
        let mut segs: Vec<(i32, i32, i32, i32)> = Vec::new();
        let mut x = xstart;
        let mut y = ystart;
        let mut a = xend - xstart;
        let mut b = yend - ystart;

        let dx_diag = if a < 0 { a = -a; -1 } else { 1 };
        let dy_diag = if b < 0 { b = -b; -1 } else { 1 };

        let (dx_nondiag, dy_nondiag) = if a < b {
            let t = a; a = b; b = t;
            (0, dy_diag)
        } else {
            (dx_diag, 0)
        };

        let mut d = b + b - a;
        let nondiag_inc = b + b;
        let diag_inc    = 2 * (b - a);

        let (mut x1, mut y1) = (x, y);

        let mut steps = a;
        while steps > 0 {
            steps -= 1;
            if d < 0 {
                x += dx_nondiag;
                y += dy_nondiag;
                d += nondiag_inc;
            } else {
                segs.push((x1, y1, x, y));
                x += dx_diag;
                y += dy_diag;
                x1 = x;
                y1 = y;
                d += diag_inc;
            }
        }
        segs.push((x1, y1, x, y));
        segs
    }

    /// Draw a 1-px solid pen line using the original C Bresenham variant.
    /// Each emitted segment is painted as a filled rectangle of solid `color`.
    pub fn draw_line_pen(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, ctx: &mut Ctx<'_>) {
        let segs = Self::bresenham_segments(x0, y0, x1, y1);
        for (ax, ay, bx, by) in segs {
            let lx = ax.min(bx);
            let ly = ay.min(by);
            let rx = ax.max(bx);
            let ry = ay.max(by);
            for py in ly..=ry {
                for px in lx..=rx {
                    self.put_pixel(px, py, 0);
                    invalidate_pixel(ctx, px, py);
                }
            }
        }
    }

    /// Erase along the original C Bresenham variant, painting each segment
    /// as a white rectangle expanded by `half` pixels on all sides.
    pub fn erase_line_square(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, half: i32, ctx: &mut Ctx<'_>) {
        let segs = Self::bresenham_segments(x0, y0, x1, y1);
        for (ax, ay, bx, by) in segs {
            let lx = ax.min(bx) - half;
            let ly = ay.min(by) - half;
            let rx = ax.max(bx) + half;
            let ry = ay.max(by) + half;
            for py in ly..=ry {
                for px in lx..=rx {
                    self.put_pixel(px, py, 255);
                    invalidate_pixel(ctx, px, py);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Per-tool pen handlers — port your C logic here
    // -----------------------------------------------------------------------

    fn on_pen_down(&mut self, cx: i32, cy: i32, ctx: &mut Ctx<'_>) {
        match self.tool {
            Tool::Pen => {
                // Track from the very first pixel — no capture.
                self.stroke_bounds = Some(Self::rubber_bounds(cx, cy, cx, cy, 0));
                self.put_pixel(cx, cy, 0);
                invalidate_pixel(ctx, cx, cy);
            }
            Tool::Eraser => {
                const ER: i32 = 8;
                self.stroke_bounds = Some(Self::rubber_bounds(cx, cy, cx, cy, ER));
                self.erase_square(cx, cy, ER, ctx);
            }
            Tool::Lines | Tool::Frame | Tool::SolidRect | Tool::RoundRect | Tool::RoundFrame => {
                // Nothing to capture — ghost already holds the clean state.
                self.rubber_rect = None;
            }
            Tool::Selection => {
                let inside = self.selection.as_ref().map_or(false, |s| {
                    cx >= s.x0 && cx <= s.x1 && cy >= s.y0 && cy <= s.y1
                });
                if inside {
                    // Ghost = pre-drag state (pixels and ghost are in sync here).
                    // Full canvas capture since destination is unknown at drag-start.
                    if let Some(pr) = PixelRect::capture(
                        &self.ghost, 0, 0, CANVAS_W - 1, CANVAS_H - 1,
                    ) {
                        self.push_undo_record(pr);
                    }
                    self.capture_selection();
                    self.sel_dragging = true;
                } else {
                    // Commit any floating selection back to canvas first.
                    self.commit_selection();
                    self.sel_dragging = false;
                    self.selection = None;
                    ctx.invalidate(Self::canvas_rect());
                }
            }
            Tool::Brush => {
                let r = self.pen_width as i32 + 1;
                self.stroke_bounds = Some(Self::rubber_bounds(cx, cy, cx, cy, r));
                self.draw_line(cx, cy, cx, cy, ctx);
            }
            Tool::Spray => {
                self.stroke_bounds = Some(Self::rubber_bounds(cx, cy, cx, cy, 3));
                // TODO: port airbrush/spray routine from C
            }
            _ => {}
        }
    }

    fn on_pen_move(&mut self, cx: i32, cy: i32, px: i32, py: i32, ctx: &mut Ctx<'_>) {
        match self.tool {
            Tool::Pen => {
                // 1-px solid line via original C Bresenham variant.
                let seg_b = Self::rubber_bounds(px, py, cx, cy, 0);
                self.stroke_bounds = Some(match self.stroke_bounds {
                    Some(old) => Self::union_bounds(old, seg_b),
                    None      => seg_b,
                });
                self.draw_line_pen(px, py, cx, cy, ctx);
            }
            Tool::Brush => {
                // No capture — just draw and grow the dirty bounds.
                let m = self.pen_width as i32 + 1;
                let seg_b = Self::rubber_bounds(px, py, cx, cy, m);
                self.stroke_bounds = Some(match self.stroke_bounds {
                    Some(old) => Self::union_bounds(old, seg_b),
                    None      => seg_b,
                });
                self.draw_line(px, py, cx, cy, ctx);
            }
            Tool::Eraser => {
                const ER: i32 = 8;
                let seg_b = Self::rubber_bounds(px, py, cx, cy, ER);
                self.stroke_bounds = Some(match self.stroke_bounds {
                    Some(old) => Self::union_bounds(old, seg_b),
                    None      => seg_b,
                });
                self.erase_line_square(px, py, cx, cy, ER, ctx);
            }
            Tool::Lines => {
                let (ax, ay) = self.anchor.unwrap_or(self.pen_start);
                let m = self.pen_width as i32 + 1;
                let new_b = Self::rubber_bounds(ax, ay, cx, cy, m);
                let old_b = self.rubber_rect;
                // Restore previous rubber-band pixels from ghost (O(dirty rows), no alloc).
                if let Some(old) = old_b {
                    Self::copy_region(&self.ghost, &mut self.pixels, old);
                }
                self.draw_line(ax, ay, cx, cy, ctx);
                let dirty = match old_b { Some(o) => Self::union_bounds(o, new_b), None => new_b };
                Self::invalidate_bounds(ctx, dirty);
                self.rubber_rect = Some(new_b);
            }
            Tool::Frame | Tool::SolidRect | Tool::RoundRect | Tool::RoundFrame => {
                let (ax, ay) = self.anchor.unwrap_or(self.pen_start);
                let m = self.pen_width as i32 + 1;
                let new_b = Self::rubber_bounds(ax, ay, cx, cy, m);
                let old_b = self.rubber_rect;
                if let Some(old) = old_b {
                    Self::copy_region(&self.ghost, &mut self.pixels, old);
                }
                self.draw_shape_tool(ax, ay, cx, cy, ctx);
                let dirty = match old_b { Some(o) => Self::union_bounds(o, new_b), None => new_b };
                Self::invalidate_bounds(ctx, dirty);
                self.rubber_rect = Some(new_b);
            }
            Tool::Selection => {
                if self.sel_dragging {
                    if let Some(ref mut s) = self.selection {
                        let rw = s.x1 - s.x0; // rect width (stays fixed)
                        let rh = s.y1 - s.y0;

                        // Clamp delta so the rect stays entirely on canvas.
                        let dx = (cx - px).clamp(-s.x0, CANVAS_W - 1 - s.x1);
                        let dy = (cy - py).clamp(-s.y0, CANVAS_H - 1 - s.y1);
                        if dx == 0 && dy == 0 { return; }

                        // The two exposed background strips (C: EditMove logic).
                        // Horizontal strip: full rect height, |dx| wide.
                        let hstrip = if dx > 0 {
                            // Moving right — strip exposed on the left.
                            Self::canvas_strip_rect(s.x0, s.y0, dx, rh + 1)
                        } else if dx < 0 {
                            // Moving left — strip exposed on the right.
                            Self::canvas_strip_rect(s.x1 + 1 + dx, s.y0, -dx, rh + 1)
                        } else { None };

                        // Vertical strip: full rect width, |dy| tall.
                        let vstrip = if dy > 0 {
                            // Moving down — strip exposed on the top.
                            Self::canvas_strip_rect(s.x0, s.y0, rw + 1, dy)
                        } else if dy < 0 {
                            // Moving up — strip exposed on the bottom.
                            Self::canvas_strip_rect(s.x0, s.y1 + 1 + dy, rw + 1, -dy)
                        } else { None };

                        // Move the rect.
                        s.x0 += dx; s.x1 += dx;
                        s.y0 += dy; s.y1 += dy;

                        // Invalidate exposed strips + new position.
                        if let Some(r) = hstrip { ctx.invalidate(r); }
                        if let Some(r) = vstrip { ctx.invalidate(r); }
                        // New position of the selection (for the composite draw).
                        if let Some(r) = Self::canvas_strip_rect(s.x0, s.y0, rw + 1, rh + 1) {
                            ctx.invalidate(r);
                        }
                    }
                } else {
                    // Rubber-band a new selection.
                    let (ax, ay) = self.pen_start;
                    self.selection = Some(Selection {
                        x0: ax.min(cx), y0: ay.min(cy),
                        x1: ax.max(cx), y1: ay.max(cy),
                        pixels: None,
                    });
                    ctx.invalidate(Self::canvas_rect());
                }
            }
            _ => {}
        }
    }

    fn on_pen_up(&mut self, cx: i32, cy: i32, ctx: &mut Ctx<'_>) {
        match self.tool {
            Tool::Selection => {
                if self.sel_dragging {
                    self.commit_selection();
                    self.sel_dragging = false;
                    // Imprint full canvas into ghost (selection may have moved anywhere).
                    self.ghost.copy_from_slice(&self.pixels);
                    ctx.invalidate(Self::canvas_rect());
                }
                // New-rect case: selection is already set by on_pen_move, nothing extra needed.
            }
            Tool::Lines | Tool::Frame | Tool::SolidRect | Tool::RoundRect | Tool::RoundFrame => {
                let (ax, ay) = self.anchor.unwrap_or(self.pen_start);
                let m = self.pen_width as i32 + 1;
                let final_b = Self::rubber_bounds(ax, ay, cx, cy, m);

                // Erase last rubber-band preview from screen.
                if let Some(old) = self.rubber_rect.take() {
                    Self::copy_region(&self.ghost, &mut self.pixels, old);
                }

                // Push ghost[final_b] as undo record (pre-draw clean state).
                if let Some(pr) = PixelRect::capture(&self.ghost, final_b.0, final_b.1, final_b.2, final_b.3) {
                    self.push_undo_record(pr);
                }

                // Commit the final draw onto screen.
                if self.tool == Tool::Lines {
                    self.draw_line(ax, ay, cx, cy, ctx);
                } else {
                    self.draw_shape_tool(ax, ay, cx, cy, ctx);
                }

                // Imprint the changed region: screen → ghost.
                Self::copy_region(&self.pixels, &mut self.ghost, final_b);
                Self::invalidate_bounds(ctx, final_b);
            }
            Tool::Pen | Tool::Brush | Tool::Eraser => {
                // Stroke complete — now we know the full dirty bounds.
                // Push ghost[bounds] as undo (pre-stroke clean state), then imprint.
                if let Some(sb) = self.stroke_bounds.take() {
                    if let Some(pr) = PixelRect::capture(&self.ghost, sb.0, sb.1, sb.2, sb.3) {
                        self.push_undo_record(pr);
                    }
                    Self::copy_region(&self.pixels, &mut self.ghost, sb);
                }
            }
            _ => {}
        }
        ctx.invalidate(Self::canvas_rect());
    }

    // -----------------------------------------------------------------------
    // Shape primitives — replace with C ports as needed
    // -----------------------------------------------------------------------

    /// Corner-ellipse diameter (pixels) for RoundRect / RoundFrame tools.
    /// Increase for rounder corners, decrease toward 0 for sharper.
    const ROUND_DIAM: i32 = 14;

    /// Drive `draw_round_rect` for the current shape tool.
    ///
    /// | Tool        | diam   | pen_width             |
    /// |-------------|--------|-----------------------|
    /// | SolidRect   | 0, 0   | forced large → solid  |
    /// | Frame       | 0, 0   | current (border only) |
    /// | RoundRect   | R, R   | forced large → solid  |
    /// | RoundFrame  | R, R   | current (border only) |
    fn draw_shape_tool(&mut self, ax: i32, ay: i32, cx: i32, cy: i32, ctx: &mut Ctx<'_>) {
        let rect = corners_to_rect(ax, ay, cx, cy);
        let (diam_x, diam_y, solid) = match self.tool {
            Tool::SolidRect  => (0,              0,              true),
            Tool::Frame      => (0,              0,              false),
            Tool::RoundRect  => (Self::ROUND_DIAM, Self::ROUND_DIAM, true),
            Tool::RoundFrame => (Self::ROUND_DIAM, Self::ROUND_DIAM, false),
            _ => return,
        };

        let saved_pw = self.pen_width;
        // draw_round_rect interprets self.pen_width as an actual pixel border
        // thickness.  Our UI index is 0-based (0 = thinnest), so add 1 to
        // convert to real pixel count (1, 2, 3, 4).  Solid tools get a huge
        // value so draw_round_rect's solid_fill condition is always satisfied.
        self.pen_width = if solid { 32767 } else { saved_pw + 1 };
        self.draw_round_rect(ctx, &rect, diam_x, diam_y);
        self.pen_width = saved_pw;
    }

    fn draw_rect_solid(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, ctx: &mut Ctx<'_>) {
        let (lx, rx) = (x0.min(x1), x0.max(x1));
        let (ty, by) = (y0.min(y1), y0.max(y1));
        for y in ty..=by {
            for x in lx..=rx {
                let v = pattern_value(self.pattern, x, y);
                self.put_pixel(x, y, v);
            }
        }
        ctx.invalidate(Rectangle::new(
            Point::new(CANVAS_X + lx, CANVAS_Y + ty),
            Size::new((rx - lx + 1) as u32, (by - ty + 1) as u32),
        ));
    }




    // -----------------------------------------------------------------------
    // Event handler
    // -----------------------------------------------------------------------

    pub fn handle_event(&mut self, event: Event, ctx: &mut Ctx<'_>) -> Option<SystemRequest> {
        match event {
            Event::AppStart => ctx.invalidate_all(),
            Event::Tick(ms) => {
                if self.selection.is_some() && ms.saturating_sub(self.ant_last_ms) >= 100 {
                    self.ant_last_ms = ms;
                    self.ant_phase = self.ant_phase.wrapping_sub(2) % 8;
                    ctx.invalidate(Self::canvas_rect());
                }
            }
            Event::Menu => {
                self.menu_open = !self.menu_open;
                ctx.invalidate_all();
            }
            Event::PenDown { x, y } => {
                if self.menu_open {
                    self.menu_open = false;
                    ctx.invalidate_all();
                    return None;
                }
                if (x as i32) < PALETTE_W {
                    self.touch = Self::hit_palette(x, y);
                    ctx.invalidate(palette_rect());
                } else if let Some((cx, cy)) = Self::screen_to_canvas(x, y) {
                    self.touch = TouchTarget::Canvas;
                    self.pen_active = true;
                    self.pen_start = (cx, cy);
                    self.pen_pos = (cx, cy);
                    self.anchor = Some((cx, cy));
                    self.on_pen_down(cx, cy, ctx);
                }
            }
            Event::PenMove { x, y } => {
                if self.touch != TouchTarget::Canvas || !self.pen_active {
                    return None;
                }
                if let Some((cx, cy)) = Self::screen_to_canvas(x, y) {
                    let (px, py) = self.pen_pos;
                    if (cx, cy) != (px, py) {
                        self.on_pen_move(cx, cy, px, py, ctx);
                        self.pen_pos = (cx, cy);
                    }
                }
            }
            Event::PenUp { x, y } => {
                match self.touch {
                    TouchTarget::ToolCell(i) => {
                        if let Some(tool) = PALETTE_CELLS[i] {
                            if tool != self.tool {
                                self.tool = tool;
                                // Commit floating selection and dismiss ants.
                                if self.selection.is_some() {
                                    self.commit_selection();
                                    self.sel_dragging = false;
                                    self.selection = None;
                                    ctx.invalidate(Self::canvas_rect());
                                }
                            }
                            ctx.invalidate(palette_rect());
                        }
                    }
                    TouchTarget::UndoBtn => {
                        self.pop_undo(ctx);
                        ctx.invalidate(palette_rect());
                    }
                    TouchTarget::ClearBtn => {
                        self.clear_canvas(ctx);
                        ctx.invalidate(palette_rect());
                    }
                    TouchTarget::LineWidth(i) => {
                        self.pen_width = i;
                        ctx.invalidate(palette_rect());
                    }
                    TouchTarget::Pattern(i) => {
                        self.pattern = i;
                        ctx.invalidate(palette_rect());
                    }
                    TouchTarget::Canvas => {
                        if self.pen_active {
                            if let Some((cx, cy)) = Self::screen_to_canvas(x, y) {
                                self.on_pen_up(cx, cy, ctx);
                            }
                            self.pen_active = false;
                            self.anchor = None;
                        }
                    }
                    TouchTarget::None => {}
                }
                self.touch = TouchTarget::None;
            }
            _ => {}
        }
        None
    }

    // -----------------------------------------------------------------------
    // Drawing
    // -----------------------------------------------------------------------

    pub fn draw<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D, dirty: Rectangle) {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "Paint");
        self.draw_canvas_area(canvas, dirty);
        self.draw_palette(canvas);
    }

    fn draw_canvas_area<D: DrawTarget<Color = Gray8>>(&self, canvas: &mut D, dirty: Rectangle) {
        // Intersect the dirty rect with the canvas area to get the
        // canvas-local row/col bounds we actually need to render.
        let d_x0 = dirty.top_left.x;
        let d_y0 = dirty.top_left.y;
        let d_x1 = d_x0 + dirty.size.width as i32 - 1;
        let d_y1 = d_y0 + dirty.size.height as i32 - 1;

        // Convert to canvas-local coordinates, clamped to valid range.
        let col0 = ((d_x0 - CANVAS_X).max(0)).min(CANVAS_W - 1) as usize;
        let row0 = ((d_y0 - CANVAS_Y).max(0)).min(CANVAS_H - 1) as usize;
        let col1 = ((d_x1 - CANVAS_X).max(0)).min(CANVAS_W - 1) as usize;
        let row1 = ((d_y1 - CANVAS_Y).max(0)).min(CANVAS_H - 1) as usize;

        // Only render pixels within the dirty intersection.
        // fill_contiguous pushes a whole row slice in one call — far cheaper
        // than per-pixel Pixel::draw(), especially in debug builds.
        let row_w = col1 - col0 + 1;
        for row in row0..=row1 {
            let off = row * CANVAS_W as usize + col0;
            let area = Rectangle::new(
                Point::new(CANVAS_X + col0 as i32, CANVAS_Y + row as i32),
                Size::new(row_w as u32, 1),
            );
            let _ = canvas.fill_contiguous(
                &area,
                self.pixels[off..off + row_w].iter().map(|&v| Gray8::new(v)),
            );
        }

        // Composite floating selection pixels — only within dirty bounds.
        if let Some(ref sel) = self.selection {
            if let Some(ref captured) = sel.pixels {
                let sw = (sel.x1 - sel.x0 + 1) as usize;
                let sh = captured.len() / sw.max(1);
                for dy in 0..sh {
                    let fy = sel.y0 + dy as i32;
                    if fy < 0 || fy >= CANVAS_H { continue; }
                    if fy + CANVAS_Y < d_y0 || fy + CANVAS_Y > d_y1 { continue; }
                    for dx in 0..sw {
                        let fx = sel.x0 + dx as i32;
                        if fx < 0 || fx >= CANVAS_W { continue; }
                        if fx + CANVAS_X < d_x0 || fx + CANVAS_X > d_x1 { continue; }
                        let v = captured[dy * sw + dx];
                        if v == 255 { continue; }  // transparent
                        let _ = Pixel(
                            Point::new(CANVAS_X + fx, CANVAS_Y + fy),
                            Gray8::new(v),
                        ).draw(canvas);
                    }
                }
            }
        }

        // Marching ants — draw_ants only touches the selection border pixels;
        // the clip on canvas already discards anything outside dirty.
        if let Some(ref sel) = self.selection {
            self.draw_ants(canvas, sel, self.ant_phase);
        }

        // Canvas border — 1-pixel stroke, very cheap even full-width.
        let _ = Self::canvas_rect()
            .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
            .draw(canvas);
    }

    /// Draw a marching-ants border around `sel` (canvas-local coordinates).
    ///
    /// Uses the same 45°-diagonal-stripe 8×8 pattern as the original PenRight
    /// `antPtn[]`.  `phase` (0, 2, 4, or 6) selects which row of the tile to
    /// start from, making the stripes appear to scroll diagonally.
    ///
    /// For each border pixel at canvas position (x, y) the pattern bit is:
    ///   bit = (antPtn[(phase + y) & 7] >> (7 - (x & 7))) & 1
    /// When set the pixel is XOR-inverted so ants are visible on any background.
    fn draw_ants<D: DrawTarget<Color = Gray8>>(
        &self,
        canvas: &mut D,
        sel: &Selection,
        phase: usize,
    ) {
        /// Matches the 14-byte `antPtn[]` from antmarch.c — 45° diagonal stripes.
        /// The array is 8 unique bytes plus a 6-byte overlap so that slicing at
        /// any phase 0-6 still yields 8 valid rows.
        const ANT_PTN: [u8; 14] = [
            0xF1, 0xE3, 0xC7, 0x8F, 0x1F, 0x3E, 0x7C, 0xF8,
            0xF1, 0xE3, 0xC7, 0x8F, 0x1F, 0x3E,
        ];

        let x0 = sel.x0.max(0) as i32;
        let y0 = sel.y0.max(0) as i32;
        let x1 = sel.x1.min(CANVAS_W - 1) as i32;
        let y1 = sel.y1.min(CANVAS_H - 1) as i32;
        if x1 < x0 || y1 < y0 { return; }

        // Return the ant pattern bit for canvas-local (x, y).
        let ant_bit = |cx: i32, cy: i32| -> bool {
            let row = ANT_PTN[(phase + cy as usize) & 7];
            (row >> (7 - (cx as usize & 7))) & 1 != 0
        };

        // Emit one border pixel — XOR-invert when the pattern is "on".
        let mut dot = |cx: i32, cy: i32| {
            let v = self.get_pixel(cx, cy);
            let v = if ant_bit(cx, cy) { 255 - v } else { v };
            let _ = Pixel(Point::new(CANVAS_X + cx, CANVAS_Y + cy), Gray8::new(v)).draw(canvas);
        };

        // Top and bottom edges — process whole rows (8 pixels per pattern byte).
        for x in x0..=x1 {
            dot(x, y0);
            if y1 > y0 { dot(x, y1); }
        }
        // Left and right edges (skip corners already drawn above).
        for y in (y0 + 1)..y1 {
            dot(x0, y);
            if x1 > x0 { dot(x1, y); }
        }
    }

    fn draw_palette<D: DrawTarget<Color = Gray8>>(&self, canvas: &mut D) {
        // Palette background
        let _ = palette_rect()
            .into_styled(PrimitiveStyle::with_fill(Gray8::new(220)))
            .draw(canvas);

        let border = PrimitiveStyle::with_stroke(BLACK, 1);
        let sel_border = PrimitiveStyle::with_stroke(BLACK, 1);

        // --- Tool cells ---------------------------------------------------
        for (i, cell) in PALETTE_CELLS.iter().enumerate() {
            let r = Self::tool_cell_rect(i);
            let is_undo  = i == 12 && cell.is_none();
            let is_clear = i == 15 && cell.is_none();
            let is_empty = cell.is_none() && !is_undo && !is_clear;
            if is_empty { continue; }

            let is_selected = cell.map_or(false, |t| t == self.tool);
            let is_pressed = self.touch == TouchTarget::ToolCell(i)
                || (is_undo  && self.touch == TouchTarget::UndoBtn)
                || (is_clear && self.touch == TouchTarget::ClearBtn);
            let highlighted = is_selected || is_pressed;

            // Cell background: black when selected (MacPaint convention).
            let bg_value = if highlighted { 0u8 } else { 240u8 };
            let _ = r.into_styled(PrimitiveStyle::with_fill(Gray8::new(bg_value))).draw(canvas);

            // Sprite sheet or fallback text.
            let action_label: Option<&str> = if is_undo { Some("Undo") }
                                        else if is_clear { Some("Clr") }
                                        else { None };
            match &self.tool_sheet {
                Some(sheet) => {
                    if let Some(label) = action_label {
                        // Action buttons: draw text directly (no sprite).
                        let text_color = if highlighted { WHITE } else { BLACK };
                        let style = MonoTextStyle::new(&FONT_5X8, text_color);
                        let tx = r.top_left.x + 2;
                        let ty = r.top_left.y + (TOOL_CELL_H - 8) / 2;
                        let _ = Text::with_baseline(label, Point::new(tx, ty), style, Baseline::Top)
                            .draw(canvas);
                    } else {
                        sheet.blit_cell(canvas, i, &r, highlighted);
                    }
                }
                None => {
                    let label = action_label.unwrap_or_else(|| {
                        cell.map_or("", |t| t.short_label())
                    });
                    let text_color = if highlighted { WHITE } else { BLACK };
                    let style = MonoTextStyle::new(&FONT_5X8, text_color);
                    let tx = r.top_left.x + 2;
                    let ty = r.top_left.y + (TOOL_CELL_H - 8) / 2;
                    let _ = Text::with_baseline(label, Point::new(tx, ty), style, Baseline::Top)
                        .draw(canvas);
                }
            }

            // Border — thicker for selected.
            let _ = r.into_styled(if is_selected { sel_border } else { border }).draw(canvas);
        }

        // --- Line-width selector ------------------------------------------
        for i in 0..LW_COUNT {
            let r = Self::lw_cell_rect(i);
            let is_sel = self.pen_width == i;
            let bg = if is_sel { 0u8 } else { 255u8 };
            let _ = r.into_styled(PrimitiveStyle::with_fill(Gray8::new(bg))).draw(canvas);
            let _ = r.into_styled(border).draw(canvas);

            // A horizontal stroke of the corresponding thickness.
            let stroke_color = if is_sel { Gray8::WHITE } else { BLACK };
            let thick = (i + 1) as u32;
            let mid_y = r.top_left.y + LW_CELL_H / 2;
            let _ = embedded_graphics::primitives::Line::new(
                Point::new(r.top_left.x + 3, mid_y),
                Point::new(r.top_left.x + PALETTE_W - 5, mid_y),
            )
            .into_styled(PrimitiveStyle::with_stroke(stroke_color, thick))
            .draw(canvas);
        }

        // --- Pattern strip ------------------------------------------------
        for i in 0..8usize {
            let r = Self::pat_cell_rect(i);
            // Render the pattern tile pixel by pixel using pattern_value so
            // gray-shade slots (2 & 3) draw a solid fill instead of dither.
            for py in 0..PAT_CELL_H {
                for px in 0..PAT_CELL_W {
                    let v = pattern_value(i, px, py);
                    let _ = Pixel(
                        Point::new(r.top_left.x + px, r.top_left.y + py),
                        Gray8::new(v),
                    )
                    .draw(canvas);
                }
            }
            // Selection highlight.
            let bstyle = if self.pattern == i { sel_border } else { border };
            let _ = r.into_styled(bstyle).draw(canvas);
        }

        // Right-edge divider
        let _ = Rectangle::new(
            Point::new(PALETTE_W - 1, CANVAS_Y),
            Size::new(1, CANVAS_H as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(BLACK))
        .draw(canvas);
    }

    pub fn a11y_nodes(&self) -> Vec<soul_core::a11y::A11yNode> {
        let mut nodes = Vec::new();
        for (i, cell) in PALETTE_CELLS.iter().enumerate() {
            let label: String = if i == 12 {
                "Undo".into()
            } else if let Some(t) = cell {
                t.name().into()
            } else {
                continue;
            };
            nodes.push(soul_core::a11y::A11yNode {
                bounds: Self::tool_cell_rect(i),
                label,
                role: "button".into(),
            });
        }
        nodes
    }
}

impl App for Paint {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        self.handle_event(event, ctx);
    }

    fn draw<D: DrawTarget<Color = Gray8>>(&mut self, canvas: &mut D, dirty: Rectangle) {
        Paint::draw(self, canvas, dirty);
    }
}

// ---------------------------------------------------------------------------
// Palette geometry helper
// ---------------------------------------------------------------------------

/// Build a Rectangle from two arbitrary corner points (order-independent).
fn corners_to_rect(x0: i32, y0: i32, x1: i32, y1: i32) -> Rectangle {
    let lx = x0.min(x1);
    let ty = y0.min(y1);
    let w  = (x0 - x1).unsigned_abs() + 1;
    let h  = (y0 - y1).unsigned_abs() + 1;
    Rectangle::new(Point::new(lx, ty), Size::new(w, h))
}

fn palette_rect() -> Rectangle {
    Rectangle::new(
        Point::new(0, CANVAS_Y),
        Size::new(PALETTE_W as u32, CANVAS_H as u32),
    )
}

fn invalidate_pixel(ctx: &mut Ctx<'_>, x: i32, y: i32) {
    ctx.invalidate(Rectangle::new(
        Point::new(CANVAS_X + x, CANVAS_Y + y),
        Size::new(1, 1),
    ));
}

// ---------------------------------------------------------------------------
// PGM loader (P5 binary, maxval 255)
// ---------------------------------------------------------------------------

/// Load a P5 binary PGM, normalising all pixel values to the 0–255 range
/// regardless of the file's maxval.  maxval=1 binary images (common in
/// original PadPaint assets) are supported: 0→0 (black), 1→255 (white).
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

    let (w, h) = pgm_read_pair(&mut r)?;
    let maxv = pgm_read_value(&mut r)?;
    if maxv == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "maxval 0 is invalid",
        ));
    }

    let mut raw = vec![0u8; w * h];
    r.read_exact(&mut raw)?;

    // Normalise to 0–255.
    let pixels = if maxv == 255 {
        raw
    } else {
        raw.iter()
            .map(|&v| ((v as u32 * 255) / maxv as u32) as u8)
            .collect()
    };

    Ok((w, h, pixels))
}

fn pgm_read_pair<R: std::io::BufRead>(r: &mut R) -> std::io::Result<(usize, usize)> {
    let mut line = String::new();
    loop {
        line.clear();
        r.read_line(&mut line)?;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        let mut it = t.split_whitespace();
        let a: usize = it.next().unwrap_or("0").parse().unwrap_or(0);
        let b: usize = it.next().unwrap_or("0").parse().unwrap_or(0);
        return Ok((a, b));
    }
}

fn pgm_read_value<R: std::io::BufRead>(r: &mut R) -> std::io::Result<usize> {
    let mut line = String::new();
    loop {
        line.clear();
        r.read_line(&mut line)?;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        return Ok(t.parse().unwrap_or(0));
    }
}

// ---------------------------------------------------------------------------
// DB persistence
// ---------------------------------------------------------------------------

const CAT_CANVAS: u8 = 0;

fn load_db(path: &std::path::Path) -> (soul_db::Database, Vec<u8>) {
    let blank = vec![255u8; CANVAS_PIXELS];
    if let Ok(bytes) = std::fs::read(path) {
        if let Some(db) = soul_db::Database::decode(&bytes) {
            let pixels = db.iter_category(CAT_CANVAS).next()
                .filter(|rec| rec.data.len() == CANVAS_PIXELS)
                .map(|rec| rec.data.clone());
            if let Some(p) = pixels {
                return (db, p);
            }
            return (db, blank);
        }
    }
    (soul_db::Database::new("paint"), blank)
}

fn save_db(db: &soul_db::Database, path: &std::path::Path, pixels: &[u8]) {
    let mut db = db.clone();
    let existing_id = db.iter_category(CAT_CANVAS).next().map(|r| r.id);
    let data = pixels.to_vec();
    if let Some(id) = existing_id {
        db.update(id, data);
    } else {
        db.insert(CAT_CANVAS, data);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, db.encode());
}

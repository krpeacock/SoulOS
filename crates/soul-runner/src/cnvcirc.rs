//! Translation of CNVCIRC.C — circle and rounded-rectangle drawing.
//!
//! The original algorithm is preserved verbatim.  The four PalmOS drawing calls
//! are now expressed through the DrawApi trait, which any window type can implement:
//!
//!   RctSetRectangle     → rct_set_rectangle()
//!   WinSetPenWidth      → DrawApi::win_set_pen_width()
//!   WinPatternRectangle → DrawApi::win_pattern_rectangle()
//!   WinPatternLine      → DrawApi::win_pattern_line()

// ── API types ────────────────────────────────────────────────────────────────

/// Mirrors RECTANGLETYPE (topLeft + extent).
#[derive(Debug, Clone, Copy, Default)]
pub struct RectangleType {
    pub top_left_x: i32,
    pub top_left_y: i32,
    pub extent_x: i32,
    pub extent_y: i32,
}

/// Maps RctSetRectangle.
pub fn rct_set_rectangle(r: &mut RectangleType, x: i32, y: i32, w: i32, h: i32) {
    r.top_left_x = x;
    r.top_left_y = y;
    r.extent_x   = w;
    r.extent_y   = h;
}

// ── Drawing abstraction ───────────────────────────────────────────────────────

/// The four GpadAPI drawing calls that cnvcirc.c relies on.
/// Anything that can receive drawing (a Window, a WinContext) implements this.
pub trait DrawApi {
    /// Maps WinSetPenWidth — sets pen width, returns previous value.
    fn win_set_pen_width(&mut self, width: i32) -> i32;
    /// Maps WinPatternRectangle — filled rectangle with current pattern.
    fn win_pattern_rectangle(&mut self, r: &RectangleType, pattern: i32);
    /// Maps WinPatternLine — line with current pen width.
    fn win_pattern_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32);
}

// ── Public drawing functions ──────────────────────────────────────────────────

/// Maps CnvCircle — draws a circle inscribed in rectangle `r`.
pub fn cnv_circle(api: &mut impl DrawApi, r: &RectangleType) {
    cnv_round_rect(api, r, 32767, 32767);
}

/// Maps CnvRoundRect — draws a rounded rectangle or circle.
///
/// `diam_x` / `diam_y` control the corner ellipse diameters.
/// Pass 32767 / 32767 for a full circle/ellipse.
pub fn cnv_round_rect(api: &mut impl DrawApi, r: &RectangleType, mut diam_x: i32, mut diam_y: i32) {
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

    let mut ry = RectangleType::default();
    let square_edge: bool;
    let solid_fill:  bool;

    let pen_width = api.win_set_pen_width(1);

    if diam_x != 0 {
        diam_x += pen_width << 1;
    }
    if diam_y != 0 {
        diam_y += pen_width << 1;
    }

    if diam_x > r.extent_x { diam_x = r.extent_x; }
    if diam_y > r.extent_y { diam_y = r.extent_y; }
    if diam_x < 2 { diam_x = 0; }
    if diam_y < 2 { diam_y = 0; }

    square_edge = (diam_x | diam_y) == 0;
    solid_fill  = (pen_width > (r.extent_x >> 1)) || (pen_width > (r.extent_y >> 1));

    let extra_x: i32;
    let extra_y = r.extent_y - diam_y;
    a0 = diam_x >> 1;
    b0 = diam_y >> 1;
    let xc_left   = r.top_left_x + a0;
    let xc_right  = xc_left + r.extent_x - diam_x - 1;
    let yc_top    = r.top_left_y + b0;
    let yc_bottom = yc_top + r.extent_y - diam_y - 1;

    if extra_y != 0 {
        if solid_fill {
            extra_x = r.extent_x;
        } else {
            extra_x = pen_width;
        }
        rct_set_rectangle(&mut ry, r.top_left_x, r.top_left_y + b0, extra_x, extra_y);
        api.win_pattern_rectangle(&ry, 0);
        if !solid_fill {
            ry.top_left_x += r.extent_x - extra_x;
            api.win_pattern_rectangle(&ry, 0);
        }
        if square_edge {
            if !solid_fill {
                ry.top_left_x = r.top_left_x + pen_width;
                ry.top_left_y = r.top_left_y;
                ry.extent_x   = r.extent_x - (pen_width << 1);
                ry.extent_y   = pen_width;
                api.win_pattern_rectangle(&ry, 0);
                ry.top_left_y += r.extent_y - pen_width;
                api.win_pattern_rectangle(&ry, 0);
            }
            api.win_set_pen_width(pen_width);
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
            set4_pixels(api, xc_left, xc_right, yc_top, yc_bottom, x1, x, y);
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
        set4_pixels(api, xc_left, xc_right, yc_top, yc_bottom, x1, x, y);
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
        set4_pixels(api, xc_left, xc_right, yc_top, yc_bottom, x1, x, y);
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

    api.win_set_pen_width(pen_width);
}

// ── Internal helper ───────────────────────────────────────────────────────────

/// Maps Set4Pixels — draws symmetric horizontal spans in all four quadrants.
fn set4_pixels(
    api: &mut impl DrawApi,
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
        api.win_pattern_line(x_right_start, y_bottom, x_right_end,  y_bottom);
        api.win_pattern_line(x_left_start,  y_bottom, x_left_end,   y_bottom);
        if y != 0 {
            api.win_pattern_line(x_right_start, y_top, x_right_end, y_top);
            api.win_pattern_line(x_left_start,  y_top, x_left_end,  y_top);
        }
    } else {
        api.win_pattern_line(x_left_start, y_bottom, x_right_end, y_bottom);
        if y != 0 {
            api.win_pattern_line(x_left_start, y_top, x_right_end, y_top);
        }
    }
}

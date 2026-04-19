// Script-based launcher — exercises draw_pixels, load_icon, and system_list_apps.

let app_id   = "com.soulos.launcher2";
let app_name = "Launcher 2";

let apps  = [];   // [{id, name, icon}] from system_list_apps()
let icons = [];   // parallel array of raw pixel blobs from load_icon()

// Layout — derived from system dimension queries
let COLS     = 4;
let H_GAP    = 4;
let TOP_PAD  = 4;
let CELL     = icon_size();                                    // 32
let COL_STEP = CELL + H_GAP;                                  // 36
let ROW_STEP = CELL + 16;                                     // 48
let GRID_X   = (screen_width() - COLS * COL_STEP + H_GAP) / 2; // 30
let GRID_Y   = title_bar_height() + TOP_PAD + 1;             // 20

fn on_event(ev) {
    if ev.type == "AppStart" {
        apps  = system_list_apps();
        icons = [];
        let i = 0;
        while i < apps.len() {
            icons += [load_icon(apps[i].icon)];
            i += 1;
        }
        invalidate_all();
        return;
    }

    if ev.type == "PenUp" {
        let cell = icon_size();
        let i = 0;
        while i < apps.len() {
            let col = i % COLS;
            let row = i / COLS;
            let ix = GRID_X + col * COL_STEP;
            let iy = GRID_Y + row * ROW_STEP;
            if ev.x >= ix && ev.x < ix + cell && ev.y >= iy && ev.y < iy + cell + 12 {
                system_launch_by_id(apps[i].id);
                return;
            }
            i += 1;
        }
    }
}

fn on_draw() {
    clear();
    title_bar("Launcher 2");

    let cell = icon_size();
    let i = 0;
    while i < apps.len() {
        let col = i % COLS;
        let row = i / COLS;
        let x = GRID_X + col * COL_STEP;
        let y = GRID_Y + row * ROW_STEP;

        draw_pixels(x, y, cell, icons[i]);
        label(x, y + cell + 2, apps[i].name);
        i += 1;
    }
}

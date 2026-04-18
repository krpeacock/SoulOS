//! Hosted persistence for the launcher icon [`Database`].
//!
//! The canonical in-memory store is written to `.soulos/launcher_icons.sdb`
//! (override with `SOUL_LAUNCHER_CACHE`). On first run or if the cache is
//! missing or invalid, records are seeded from `assets/sprites/*_icon.pgm`.

use soul_db::Database;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::{seed_launcher_icons, APPS, ICON_CELL};

pub struct LauncherIconStore {
    path: PathBuf,
    pub db: Database,
}

impl LauncherIconStore {
    pub fn load_or_seed() -> Self {
        let path = launcher_cache_path();
        if let Ok(bytes) = fs::read(&path) {
            if let Some(db) = Database::decode(&bytes) {
                if launcher_db_valid(&db) {
                    return Self { path, db };
                }
            }
        }
        let mut db = Database::new("launcher_icons");
        seed_launcher_icons(&mut db);
        let s = Self { path, db };
        if let Err(e) = s.persist() {
            eprintln!(
                "launcher: could not write icon cache {}: {e}",
                s.path.display()
            );
        }
        s
    }

    pub fn persist(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, self.db.encode())
    }
}

fn launcher_cache_path() -> PathBuf {
    std::env::var("SOUL_LAUNCHER_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".soulos/launcher_icons.sdb"))
}

fn launcher_db_valid(db: &Database) -> bool {
    let mut expected = [0u8; 32];
    for (i, b) in b"launcher_icons".iter().enumerate() {
        expected[i] = *b;
    }
    if db.name != expected {
        return false;
    }
    let cell = ICON_CELL as usize;
    let area = cell * cell;
    if db.len() != APPS.len() {
        return false;
    }
    for i in 0..APPS.len() {
        let Some(rec) = db.iter_category(i as u8).next() else {
            return false;
        };
        if rec.data.len() != area {
            return false;
        }
    }
    true
}

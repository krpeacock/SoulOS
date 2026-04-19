//! Hosted persistence for the launcher icon [`Database`].
//!
//! The icon DB is used by the Draw app to store and edit launcher icons.
//! It is seeded from `assets/sprites/*_icon.pgm` files by [`Host::new`]
//! after all apps are loaded (so icon stems can be read from the apps themselves).
//!
//! The cache lives at `.soulos/launcher_icons.sdb`
//! (override with `SOUL_LAUNCHER_CACHE`).

use soul_db::Database;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::ICON_CELL;

pub struct LauncherIconStore {
    path: PathBuf,
    pub db: Database,
}

impl LauncherIconStore {
    /// Load an existing icon cache from disk, or create an empty one.
    /// Seeding happens later in [`Host::new`] via [`is_valid_for`] +
    /// [`crate::seed_launcher_icons`].
    pub fn load_or_empty() -> Self {
        let path = launcher_cache_path();
        if let Ok(bytes) = fs::read(&path) {
            if let Some(db) = Database::decode(&bytes) {
                return Self { path, db };
            }
        }
        Self {
            path,
            db: Database::new("launcher_icons"),
        }
    }

    /// Returns `true` if the DB looks valid for `expected_count` apps
    /// (one icon record per app, each exactly 32×32 pixels).
    pub fn is_valid_for(&self, expected_count: usize) -> bool {
        let mut expected_name = [0u8; 32];
        for (i, b) in b"launcher_icons".iter().enumerate() {
            expected_name[i] = *b;
        }
        if self.db.name != expected_name {
            return false;
        }
        let area = (ICON_CELL * ICON_CELL) as usize;
        if self.db.len() != expected_count {
            return false;
        }
        for i in 0..expected_count {
            let Some(rec) = self.db.iter_category(i as u8).next() else {
                return false;
            };
            if rec.data.len() != area {
                return false;
            }
        }
        true
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

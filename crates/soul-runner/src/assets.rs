//! Asset I/O shim used by every app and the Host.
//!
//! On native targets these are thin wrappers around `std::fs`. On
//! `wasm32` the project's bundled scripts and icons are embedded at
//! compile time via `include_str!` / `include_bytes!`, so the same
//! [`Host`](crate::Host) starts up cleanly without any filesystem
//! access. Database paths (`.soulos/*.sdb`) are non-persistent on
//! wasm — reads return `NotFound`, writes are dropped — so the OS
//! runs from a clean slate per page load. LocalStorage-backed
//! persistence can layer on top later without changing call sites.

use std::io;
use std::path::Path;

pub fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let path = path.as_ref();
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(path)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let key = normalize(path);
        match embedded_text(&key) {
            Some(s) => Ok(s.to_string()),
            None => Err(io::Error::new(io::ErrorKind::NotFound, key)),
        }
    }
}

pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let path = path.as_ref();
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read(path)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let key = normalize(path);
        match embedded_bytes(&key) {
            Some(b) => Ok(b.to_vec()),
            None => Err(io::Error::new(io::ErrorKind::NotFound, key)),
        }
    }
}

pub fn write<P: AsRef<Path>>(path: P, bytes: &[u8]) -> io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::write(path, bytes)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (path, bytes);
        Ok(())
    }
}

pub fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::create_dir_all(path)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(target_arch = "wasm32")]
fn embedded_text(key: &str) -> Option<&'static str> {
    macro_rules! script {
        ($name:literal) => {
            (
                concat!("assets/scripts/", $name, ".rhai"),
                include_str!(concat!("../../../assets/scripts/", $name, ".rhai")),
            )
        };
    }
    const TABLE: &[(&str, &str)] = &[
        script!("notes"),
        script!("address"),
        script!("date"),
        script!("todo"),
        script!("egui_demo"),
        script!("mail"),
        script!("prefs"),
        script!("sync"),
        script!("launcher2"),
    ];
    TABLE.iter().find(|(p, _)| *p == key).map(|(_, s)| *s)
}

#[cfg(target_arch = "wasm32")]
fn embedded_bytes(key: &str) -> Option<&'static [u8]> {
    macro_rules! icon {
        ($stem:literal) => {
            (
                concat!("assets/sprites/", $stem, "_icon.pgm"),
                include_bytes!(concat!("../../../assets/sprites/", $stem, "_icon.pgm"))
                    as &[u8],
            )
        };
    }
    const TABLE: &[(&str, &[u8])] = &[
        icon!("default"),
        icon!("calc"),
        icon!("draw"),
        icon!("paint"),
        icon!("builder"),
        icon!("notes"),
        icon!("address"),
        icon!("date"),
        icon!("todo"),
        icon!("mail"),
        icon!("prefs"),
        icon!("sync"),
        icon!("launcher2"),
        (
            "assets/sprites/paint_tools/paint_tools.pgm",
            include_bytes!("../../../assets/sprites/paint_tools/paint_tools.pgm"),
        ),
    ];
    TABLE.iter().find(|(p, _)| *p == key).map(|(_, b)| *b)
}

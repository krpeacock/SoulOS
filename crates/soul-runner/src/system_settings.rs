//! Persistent system-wide accessibility preferences.
//!
//! One [`soul_db::Database`] (`.soulos/system_settings.sdb`) holds two
//! kinds of records:
//!
//! - **Global** (`category = 0`): the user's defaults. Applied to
//!   [`A11yManager`] on `AppStart`.
//! - **Per-app override** (`category = u8 hash of app_id`): replaces
//!   selected fields whenever the corresponding app becomes active.
//!   Hash collisions are resolved by storing the full `app_id` in
//!   the record payload and matching it on read — colliding apps
//!   simply don't share state.
//!
//! Encoding (per record):
//!
//! ```text
//! [app_id_len: u8] [app_id_bytes: app_id_len]
//! [count: u8] [(tag: u8, len: u8, value bytes...) * count]
//! ```
//!
//! Each setting is length-prefixed so an unknown tag from a future
//! build is skipped cleanly without corrupting subsequent fields.
//!
//! Phase 4 deliberately ships the persistence layer without a UI. A
//! later phase (or scripted Settings app) will provide controls; for
//! now the long-press Power → curtain toggle exercises the write path
//! end-to-end.

use std::path::PathBuf;

use soul_core::a11y::{A11yManager, Verbosity};
use soul_db::Database;
use soul_hal::{Punctuation, SpeechRequest};

use crate::assets;

const DB_NAME: &str = "system_settings";

/// Tag values for individual settings. `u8` so an unknown tag from a
/// future build can still be recognized by its length prefix and
/// skipped without confusing the parser.
const KEY_RATE_WPM: u8 = 1;
const KEY_VERBOSITY: u8 = 2;
const KEY_SCREEN_CURTAIN: u8 = 3;
const KEY_PUNCTUATION: u8 = 4;

const GLOBAL_CATEGORY: u8 = 0;

/// All a11y settings, all optional. `None` means "fall back to the
/// next layer" (per-app → global → built-in defaults).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct A11ySettings {
    pub rate_wpm: Option<u16>,
    pub verbosity: Option<Verbosity>,
    pub screen_curtain: Option<bool>,
    pub punctuation: Option<Punctuation>,
}

impl A11ySettings {
    /// Snapshot the current values from an [`A11yManager`].
    pub fn snapshot(m: &A11yManager) -> Self {
        Self {
            rate_wpm: Some(m.rate_wpm),
            verbosity: Some(m.verbosity),
            screen_curtain: Some(m.screen_curtain),
            punctuation: Some(m.punctuation),
        }
    }

    /// Apply each `Some` field to `m`. Fields left as `None` are not
    /// touched, so callers can layer per-app overrides on top of the
    /// global defaults without clobbering them.
    pub fn apply_to(&self, m: &mut A11yManager) {
        if let Some(r) = self.rate_wpm {
            m.rate_wpm = r;
        }
        if let Some(v) = self.verbosity {
            m.verbosity = v;
        }
        if let Some(c) = self.screen_curtain {
            m.screen_curtain = c;
        }
        if let Some(p) = self.punctuation {
            m.punctuation = p;
        }
    }

    /// Combine `self` (lower priority) with `other` (higher priority);
    /// any `Some` field in `other` wins.
    pub fn merged_with(mut self, other: Self) -> Self {
        if other.rate_wpm.is_some() {
            self.rate_wpm = other.rate_wpm;
        }
        if other.verbosity.is_some() {
            self.verbosity = other.verbosity;
        }
        if other.screen_curtain.is_some() {
            self.screen_curtain = other.screen_curtain;
        }
        if other.punctuation.is_some() {
            self.punctuation = other.punctuation;
        }
        self
    }

    /// Encode `self` plus the `app_id` it applies to. Empty `app_id`
    /// marks a global record.
    pub fn encode(&self, app_id: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + app_id.len() + 4 * 4);
        let id_bytes = app_id.as_bytes();
        let id_len = id_bytes.len().min(255) as u8;
        out.push(id_len);
        out.extend_from_slice(&id_bytes[..id_len as usize]);

        let mut count: u8 = 0;
        let mut body: Vec<u8> = Vec::new();
        if let Some(r) = self.rate_wpm {
            body.push(KEY_RATE_WPM);
            body.push(2);
            body.extend_from_slice(&r.to_be_bytes());
            count += 1;
        }
        if let Some(v) = self.verbosity {
            body.push(KEY_VERBOSITY);
            body.push(1);
            body.push(verbosity_to_u8(v));
            count += 1;
        }
        if let Some(c) = self.screen_curtain {
            body.push(KEY_SCREEN_CURTAIN);
            body.push(1);
            body.push(c as u8);
            count += 1;
        }
        if let Some(p) = self.punctuation {
            body.push(KEY_PUNCTUATION);
            body.push(1);
            body.push(punctuation_to_u8(p));
            count += 1;
        }
        out.push(count);
        out.extend(body);
        out
    }

    /// Inverse of [`A11ySettings::encode`]. Returns `(app_id, settings)`
    /// or `None` if the bytes are malformed beyond recovery. Unknown
    /// tags are silently skipped so old binaries can read records
    /// written by newer ones.
    pub fn decode(bytes: &[u8]) -> Option<(String, Self)> {
        let mut cur = 0usize;
        let id_len = *bytes.get(cur)? as usize;
        cur += 1;
        if cur + id_len > bytes.len() {
            return None;
        }
        let app_id = core::str::from_utf8(&bytes[cur..cur + id_len]).ok()?.to_string();
        cur += id_len;

        let count = *bytes.get(cur)? as usize;
        cur += 1;

        let mut s = Self::default();
        for _ in 0..count {
            let tag = *bytes.get(cur)?;
            cur += 1;
            let len = *bytes.get(cur)? as usize;
            cur += 1;
            if cur + len > bytes.len() {
                return None;
            }
            let value = &bytes[cur..cur + len];
            cur += len;
            match (tag, len) {
                (KEY_RATE_WPM, 2) => {
                    s.rate_wpm = Some(u16::from_be_bytes([value[0], value[1]]));
                }
                (KEY_VERBOSITY, 1) => {
                    s.verbosity = u8_to_verbosity(value[0]);
                }
                (KEY_SCREEN_CURTAIN, 1) => {
                    s.screen_curtain = Some(value[0] != 0);
                }
                (KEY_PUNCTUATION, 1) => {
                    s.punctuation = u8_to_punctuation(value[0]);
                }
                // Unknown tag — skip (already advanced cur by `len`).
                _ => {}
            }
        }
        Some((app_id, s))
    }
}

fn verbosity_to_u8(v: Verbosity) -> u8 {
    match v {
        Verbosity::Low => 0,
        Verbosity::Medium => 1,
        Verbosity::High => 2,
    }
}

fn u8_to_verbosity(b: u8) -> Option<Verbosity> {
    match b {
        0 => Some(Verbosity::Low),
        1 => Some(Verbosity::Medium),
        2 => Some(Verbosity::High),
        _ => None,
    }
}

fn punctuation_to_u8(p: Punctuation) -> u8 {
    match p {
        Punctuation::None => 0,
        Punctuation::Some => 1,
        Punctuation::All => 2,
    }
}

fn u8_to_punctuation(b: u8) -> Option<Punctuation> {
    match b {
        0 => Some(Punctuation::None),
        1 => Some(Punctuation::Some),
        2 => Some(Punctuation::All),
        _ => None,
    }
}

/// FNV-1a → bucket in `1..=15`. soul-db caps category at
/// [`soul_db::MAX_CATEGORIES`] (16); we reserve `0` for global, leaving
/// 15 buckets for per-app overrides. Collisions are tolerated — the
/// record payload carries the full `app_id` for disambiguation.
fn app_category(app_id: &str) -> u8 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in app_id.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    // 1..=15 (15 distinct buckets).
    ((h % 15) as u8) + 1
}

/// Wraps the on-disk a11y preferences database.
pub struct SystemSettings {
    db: Database,
    path: PathBuf,
}

impl SystemSettings {
    /// Open the settings DB at `path`, creating it if missing.
    pub fn open(path: PathBuf) -> Self {
        let db = match assets::read(&path) {
            Ok(bytes) => Database::decode(&bytes).unwrap_or_else(|| Database::new(DB_NAME)),
            Err(_) => Database::new(DB_NAME),
        };
        Self { db, path }
    }

    /// Settings the user picked as defaults. Empty `A11ySettings`
    /// when no record exists.
    pub fn global(&self) -> A11ySettings {
        for record in self.db.iter_category(GLOBAL_CATEGORY) {
            if let Some((id, s)) = A11ySettings::decode(&record.data) {
                if id.is_empty() {
                    return s;
                }
            }
        }
        A11ySettings::default()
    }

    /// Settings effective for `app_id`: global merged with the app's
    /// override (override fields win).
    pub fn for_app(&self, app_id: &str) -> A11ySettings {
        let global = self.global();
        let cat = app_category(app_id);
        for record in self.db.iter_category(cat) {
            if let Some((id, s)) = A11ySettings::decode(&record.data) {
                if id == app_id {
                    return global.merged_with(s);
                }
            }
        }
        global
    }

    /// Write `s` as the global default, replacing any previous global
    /// record. Persists immediately.
    pub fn save_global(&mut self, s: &A11ySettings) {
        let payload = s.encode("");
        self.upsert(GLOBAL_CATEGORY, "", payload);
        self.persist();
    }

    /// Write `s` as the override for `app_id`, replacing any previous
    /// override for that app. Persists immediately.
    pub fn save_app_override(&mut self, app_id: &str, s: &A11ySettings) {
        let payload = s.encode(app_id);
        let cat = app_category(app_id);
        self.upsert(cat, app_id, payload);
        self.persist();
    }

    fn upsert(&mut self, category: u8, app_id: &str, payload: Vec<u8>) {
        // Look for an existing record matching (category, app_id).
        let existing_id = self
            .db
            .iter_category(category)
            .find(|r| {
                A11ySettings::decode(&r.data)
                    .map(|(id, _)| id == app_id)
                    .unwrap_or(false)
            })
            .map(|r| r.id);
        match existing_id {
            Some(id) => {
                self.db.update(id, payload);
            }
            None => {
                self.db.insert(category, payload);
            }
        }
    }

    fn persist(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = assets::create_dir_all(parent);
        }
        let _ = assets::write(&self.path, &self.db.encode());
    }
}

/// Default settings reflecting the built-in defaults of [`A11yManager`].
pub fn defaults() -> A11ySettings {
    A11ySettings {
        rate_wpm: Some(SpeechRequest::DEFAULT_RATE_WPM),
        verbosity: Some(Verbosity::Medium),
        screen_curtain: Some(false),
        punctuation: Some(Punctuation::Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> A11ySettings {
        A11ySettings {
            rate_wpm: Some(240),
            verbosity: Some(Verbosity::High),
            screen_curtain: Some(true),
            punctuation: Some(Punctuation::All),
        }
    }

    #[test]
    fn round_trip_global() {
        let s = full();
        let bytes = s.encode("");
        let (id, decoded) = A11ySettings::decode(&bytes).unwrap();
        assert_eq!(id, "");
        assert_eq!(decoded, s);
    }

    #[test]
    fn round_trip_per_app_carries_id() {
        let s = full();
        let bytes = s.encode("com.soulos.address");
        let (id, decoded) = A11ySettings::decode(&bytes).unwrap();
        assert_eq!(id, "com.soulos.address");
        assert_eq!(decoded, s);
    }

    #[test]
    fn decode_skips_unknown_tags() {
        // Hand-build a payload: empty app_id, count=2, one known
        // KEY_RATE_WPM and one unknown tag with a 3-byte payload that
        // the parser must skip cleanly.
        let bytes: Vec<u8> = vec![
            0,    // app_id_len
            2,    // count
            KEY_RATE_WPM, 2, 0x00, 0xC8, // rate = 200
            99,   // unknown tag
            3,    // len
            1, 2, 3, // unknown payload
        ];
        let (_, s) = A11ySettings::decode(&bytes).unwrap();
        assert_eq!(s.rate_wpm, Some(200));
    }

    #[test]
    fn merge_other_wins() {
        let base = A11ySettings {
            rate_wpm: Some(175),
            verbosity: Some(Verbosity::Medium),
            ..Default::default()
        };
        let over = A11ySettings {
            rate_wpm: Some(320),
            ..Default::default()
        };
        let m = base.merged_with(over);
        assert_eq!(m.rate_wpm, Some(320));
        assert_eq!(m.verbosity, Some(Verbosity::Medium)); // not overridden
    }

    #[test]
    fn apply_to_only_overrides_some_fields() {
        let mut m = A11yManager::new();
        m.rate_wpm = 175;
        m.verbosity = Verbosity::Medium;
        let s = A11ySettings {
            rate_wpm: Some(240),
            ..Default::default()
        };
        s.apply_to(&mut m);
        assert_eq!(m.rate_wpm, 240);
        assert_eq!(m.verbosity, Verbosity::Medium);
    }

    #[test]
    fn category_for_global_is_zero() {
        assert_eq!(GLOBAL_CATEGORY, 0);
    }

    #[test]
    fn app_category_never_collides_with_global() {
        // Empty app_id wouldn't be passed to app_category in
        // production, but exercise the reservation logic anyway.
        for id in ["a", "b", "com.soulos.address", "com.soulos.notes"] {
            assert!(app_category(id) != GLOBAL_CATEGORY);
        }
    }

    fn temp_db_path() -> PathBuf {
        let dir = std::env::temp_dir();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        dir.join(format!("soulos_settings_{nonce}.sdb"))
    }

    #[test]
    fn open_save_reopen_round_trip() {
        let path = temp_db_path();
        // Fresh DB is empty.
        {
            let s = SystemSettings::open(path.clone());
            assert_eq!(s.global(), A11ySettings::default());
        }
        // Write a global record and a per-app override, then reopen.
        {
            let mut s = SystemSettings::open(path.clone());
            s.save_global(&A11ySettings {
                rate_wpm: Some(240),
                verbosity: Some(Verbosity::Low),
                ..Default::default()
            });
            s.save_app_override(
                "com.soulos.address",
                &A11ySettings {
                    rate_wpm: Some(320),
                    ..Default::default()
                },
            );
        }
        {
            let s = SystemSettings::open(path.clone());
            let g = s.global();
            assert_eq!(g.rate_wpm, Some(240));
            assert_eq!(g.verbosity, Some(Verbosity::Low));
            // Per-app override wins for rate, inherits verbosity.
            let addr = s.for_app("com.soulos.address");
            assert_eq!(addr.rate_wpm, Some(320));
            assert_eq!(addr.verbosity, Some(Verbosity::Low));
            // Non-overridden app sees only the global.
            let other = s.for_app("com.soulos.notes");
            assert_eq!(other.rate_wpm, Some(240));
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_app_override_replaces_existing_record() {
        let path = temp_db_path();
        let mut s = SystemSettings::open(path.clone());
        s.save_app_override(
            "x",
            &A11ySettings {
                rate_wpm: Some(200),
                ..Default::default()
            },
        );
        s.save_app_override(
            "x",
            &A11ySettings {
                rate_wpm: Some(280),
                ..Default::default()
            },
        );
        // Reopen to confirm the second write replaced the first.
        let reopened = SystemSettings::open(path.clone());
        assert_eq!(reopened.for_app("x").rate_wpm, Some(280));
        let _ = std::fs::remove_file(&path);
    }
}

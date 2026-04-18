//! # soul-db — on-device record storage for SoulOS
//!
//! SoulOS deliberately has no file system. The primitive is a
//! **record**: a categorized, attribute-tagged byte buffer. Apps
//! open a [`Database`] by name, insert and update records, and
//! iterate them by category. The platform is responsible for
//! durability and sync — apps just mutate records.
//!
//! This is SoulOS's analogue to the PalmOS Database Manager. The
//! distinction between `.pdb` (data) and `.prc` (resource)
//! databases is not preserved here; there's just one type,
//! [`Database`]. For platform persistence, [`Database::encode`] and
//! [`Database::decode`] provide a versioned binary snapshot of active
//! records (hosted runner cache files, sync bundles, etc.).
//!
//! # Why records, not files?
//!
//! A file API is a leaky abstraction: it bundles identity,
//! byte-addressable contents, hierarchical location, and
//! human-readable naming into one type, and apps end up duplicating
//! durability logic (save, load, flush, parse) per file format.
//! Records strip this to the essentials — an id, a category, some
//! bytes — and let the platform handle everything else.
//!
//! # Usage
//!
//! ```
//! use soul_db::{Database, CATEGORY_UNFILED};
//!
//! let mut db = Database::new("notes");
//! let id = db.insert(CATEGORY_UNFILED, b"hello world".to_vec());
//! assert_eq!(db.get(id).unwrap().data, b"hello world");
//!
//! db.update(id, b"hello soulos".to_vec());
//! assert_eq!(db.get(id).unwrap().data, b"hello soulos");
//!
//! db.delete(id);
//! assert!(db.get(id).is_none());
//! ```

#![no_std]
extern crate alloc;

use alloc::vec::Vec;

const PERSIST_MAGIC: &[u8; 8] = b"SOULOSDB";
const PERSIST_VERSION: u32 = 1;
/// Hard cap per record payload when decoding from untrusted bytes (hosted cache files).
const PERSIST_MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

/// Maximum number of categories a single database can define.
/// Matches the PalmOS convention.
pub const MAX_CATEGORIES: usize = 16;

/// The "Unfiled" category, used when no category applies. Always
/// category 0.
pub const CATEGORY_UNFILED: u8 = 0;

/// A single stored record.
///
/// Records are identified by a monotonically-increasing `u32` id
/// that is stable across updates but not recycled after deletion.
/// `data` is opaque bytes — apps layer their own typed schema on
/// top.
#[derive(Debug, Clone)]
pub struct Record {
    /// Stable id within the owning database. Assigned on insert.
    pub id: u32,
    /// Category index (0..[`MAX_CATEGORIES`]).
    pub category: u8,
    /// Record-level flags: secret, dirty (unsynced), deleted.
    pub attrs: RecordAttrs,
    /// Opaque payload.
    pub data: Vec<u8>,
}

/// Per-record metadata flags.
///
/// Mirrors the PalmOS record attributes. `dirty` is set on any
/// mutation and is cleared by a sync; `deleted` records are kept
/// as tombstones until sync, then reaped by the platform.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecordAttrs {
    /// Hidden in the default view (requires authentication to
    /// display). Apps honor this by filtering their iterator
    /// explicitly — the database does not hide them.
    pub secret: bool,
    /// Has pending changes not yet acknowledged by sync.
    pub dirty: bool,
    /// Logically deleted; will be reaped by the next sync cycle.
    pub deleted: bool,
}

/// A name-scoped, in-memory collection of [`Record`]s.
///
/// Apps typically open one database per logical document type
/// (Notes, Addresses, Memos, …). Persistence is the platform's
/// responsibility; this struct is the runtime representation.
#[derive(Debug, Clone)]
pub struct Database {
    /// Database name, up to 32 bytes. Null-padded.
    pub name: [u8; 32],
    records: Vec<Record>,
    next_id: u32,
}

impl Database {
    /// Create a new empty database named `name` (truncated to 32
    /// bytes).
    pub fn new(name: &str) -> Self {
        let mut n = [0u8; 32];
        for (i, b) in name.bytes().take(32).enumerate() {
            n[i] = b;
        }
        Self {
            name: n,
            records: Vec::new(),
            next_id: 1,
        }
    }

    /// Insert a new record with the given category and payload.
    /// Returns its freshly-allocated id. The record is marked
    /// `dirty` so the next sync picks it up.
    pub fn insert(&mut self, category: u8, data: Vec<u8>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.records.push(Record {
            id,
            category,
            attrs: RecordAttrs {
                dirty: true,
                ..Default::default()
            },
            data,
        });
        id
    }

    /// Look up a record by id. Returns `None` for missing or
    /// tombstoned ids.
    pub fn get(&self, id: u32) -> Option<&Record> {
        self.records.iter().find(|r| r.id == id && !r.attrs.deleted)
    }

    /// Mark a record as deleted. Returns `true` if a matching
    /// record existed. The record is kept as a tombstone until
    /// sync.
    pub fn delete(&mut self, id: u32) -> bool {
        if let Some(r) = self.records.iter_mut().find(|r| r.id == id) {
            r.attrs.deleted = true;
            r.attrs.dirty = true;
            true
        } else {
            false
        }
    }

    /// Replace the payload of an existing (non-deleted) record.
    /// Returns `true` on success, `false` if the id is unknown.
    pub fn update(&mut self, id: u32, data: Vec<u8>) -> bool {
        if let Some(r) = self
            .records
            .iter_mut()
            .find(|r| r.id == id && !r.attrs.deleted)
        {
            r.data = data;
            r.attrs.dirty = true;
            true
        } else {
            false
        }
    }

    /// Iterate non-deleted records in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &Record> {
        self.records.iter().filter(|r| !r.attrs.deleted)
    }

    /// Iterate non-deleted records in the given category.
    pub fn iter_category(&self, category: u8) -> impl Iterator<Item = &Record> {
        self.records
            .iter()
            .filter(move |r| !r.attrs.deleted && r.category == category)
    }

    /// Number of non-deleted records.
    pub fn len(&self) -> usize {
        self.records.iter().filter(|r| !r.attrs.deleted).count()
    }

    /// Whether the database has no non-deleted records.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Serialize for platform persistence (e.g. hosted cache file).
    ///
    /// Omits tombstoned records; preserves [`Database::name`] and
    /// [`Database::next_id`], and serializes active [`Record`] fields.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(PERSIST_MAGIC);
        out.extend_from_slice(&PERSIST_VERSION.to_le_bytes());
        out.extend_from_slice(&self.name);
        out.extend_from_slice(&self.next_id.to_le_bytes());
        let active: Vec<&Record> = self.records.iter().filter(|r| !r.attrs.deleted).collect();
        out.extend_from_slice(&(active.len() as u32).to_le_bytes());
        for r in active {
            out.extend_from_slice(&r.id.to_le_bytes());
            out.push(r.category);
            let mut flags: u8 = 0;
            if r.attrs.secret {
                flags |= 1;
            }
            if r.attrs.dirty {
                flags |= 2;
            }
            out.push(flags);
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&(r.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&r.data);
        }
        out
    }

    /// Deserialize from [`Self::encode`]. Returns `None` if the blob is
    /// invalid or the version is unsupported.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 + 4 + 32 + 4 + 4 {
            return None;
        }
        let mut i = 0usize;
        if bytes.get(i..i + 8)? != PERSIST_MAGIC.as_slice() {
            return None;
        }
        i += 8;
        let ver = read_u32_le(bytes, &mut i)?;
        if ver != PERSIST_VERSION {
            return None;
        }
        let mut name = [0u8; 32];
        name.copy_from_slice(bytes.get(i..i + 32)?);
        i += 32;
        let next_id = read_u32_le(bytes, &mut i)?;
        let n = read_u32_le(bytes, &mut i)? as usize;
        let mut records = Vec::new();
        for _ in 0..n {
            let id = read_u32_le(bytes, &mut i)?;
            let category = *bytes.get(i)?;
            i += 1;
            if category as usize >= MAX_CATEGORIES {
                return None;
            }
            let flags = *bytes.get(i)?;
            i += 1;
            let secret = (flags & 1) != 0;
            let dirty = (flags & 2) != 0;
            i += 2;
            let len = read_u32_le(bytes, &mut i)? as usize;
            if len > PERSIST_MAX_RECORD_BYTES {
                return None;
            }
            let end = i.checked_add(len)?;
            let data = bytes.get(i..end)?.to_vec();
            i = end;
            records.push(Record {
                id,
                category,
                attrs: RecordAttrs {
                    secret,
                    dirty,
                    deleted: false,
                },
                data,
            });
        }
        if i != bytes.len() {
            return None;
        }
        Some(Self {
            name,
            records,
            next_id,
        })
    }
}

fn read_u32_le(bytes: &[u8], i: &mut usize) -> Option<u32> {
    let slice = bytes.get(*i..*i + 4)?;
    *i += 4;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn encode_decode_roundtrip() {
        let mut db = Database::new("testdb");
        let id = db.insert(CATEGORY_UNFILED, Vec::from([1u8, 2, 3, 4]));
        let mut hundred = Vec::new();
        hundred.resize(100, 9u8);
        assert!(db.update(id, hundred.clone()));
        let bytes = db.encode();
        let db2 = Database::decode(&bytes).expect("decode");
        assert_eq!(db2.name, db.name);
        assert_eq!(db2.next_id, db.next_id);
        assert_eq!(db2.len(), 1);
        assert_eq!(db2.get(id).unwrap().data, hundred);
    }
}

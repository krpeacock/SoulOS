//! # Inter-app exchange payloads
//!
//! Apps swap data through a single, generic envelope: an
//! [`ExchangePayload`] containing one or more [`Representation`]s. A
//! representation is a flat `(mime, bytes, meta)` tuple — the same
//! shape on the wire as in memory. Most apps only ever produce or
//! consume a single representation; the [`ExchangePayload::primary`]
//! / [`ExchangePayload::find_kind`] accessors hide the multi-rep
//! groundwork until it's actually needed (e.g. a paste source that
//! offers both a styled-text and plain-text view of the same
//! selection).
//!
//! ## What lives where
//!
//! * Currently-supported types (Text, Bitmap) get convenience
//!   constructors, accessors, and a typed [`Kind`] tag.
//! * Anything else uses [`Representation::other`] — the
//!   `(mime, bytes)` survives end-to-end even if nobody on the
//!   receiving side knows what to do with it.
//! * MIME ↔ `Kind` ↔ file-extension translation is *only* the job
//!   of [`ExchangeRegistry`]. The rest of SoulOS never has to know
//!   what a MIME string looks like — it asks the registry.
//!
//! Bitmaps are canonically stored as **PGM (P5)** bytes
//! (`image/x-portable-graymap`). PGM is a real registered format,
//! it carries width/height/maxval inside the bytes, the file
//! extension is `.pgm`, and SoulOS already serves icons in this
//! format — so the on-the-wire bitmap is byte-for-byte the same as
//! a PGM file written to disk.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};

/// One typed view of exchanged content.
///
/// `bytes` are interpreted according to `mime`; `meta` carries
/// optional structured hints (subtype, source app, language, …).
/// The shape is deliberately the same as a wire-level record so
/// that "serialize a Representation" is just `(mime, bytes, meta)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Representation {
    /// MIME type of `bytes`. `"text/plain"`, `"image/x-portable-graymap"`, etc.
    pub mime: String,
    /// Raw payload bytes in whatever encoding `mime` implies.
    pub bytes: Vec<u8>,
    /// Optional structured metadata. Well-known keys:
    /// - `"subtype"`  — the part of `mime` after the slash, with a
    ///   leading `x-` stripped (e.g. `"rhai"` for `text/x-rhai`).
    /// - `"app_id"`   — origin or target app identifier.
    /// - `"kind"`     — application-defined resource kind.
    pub meta: BTreeMap<String, String>,
}

impl Representation {
    /// `text/plain` from a UTF-8 string.
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            mime: "text/plain".to_string(),
            bytes: s.into().into_bytes(),
            meta: BTreeMap::new(),
        }
    }

    /// Text with a non-standard subtype, e.g. `"rhai"` → `text/x-rhai`.
    /// Common standardised subtypes (`"plain"`, `"html"`, `"markdown"`,
    /// `"json"`) are emitted without the `x-` prefix.
    pub fn text_with_subtype(subtype: &str, s: impl Into<String>) -> Self {
        let mime = if matches!(subtype, "plain" | "html" | "markdown" | "json" | "csv") {
            format!("text/{subtype}")
        } else {
            format!("text/x-{subtype}")
        };
        Self {
            mime,
            bytes: s.into().into_bytes(),
            meta: BTreeMap::new(),
        }
    }

    /// PGM (P5) bitmap. `pixels.len()` must equal `width * height`.
    pub fn bitmap(bitmap: &Bitmap) -> Self {
        Self {
            mime: "image/x-portable-graymap".to_string(),
            bytes: bitmap.to_pgm(),
            meta: BTreeMap::new(),
        }
    }

    /// Catch-all for any other MIME type. The bytes pass through
    /// intact even if no handler on the receiving side recognises
    /// the type.
    pub fn other(mime: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            mime: mime.into(),
            bytes,
            meta: BTreeMap::new(),
        }
    }

    /// Add or override a metadata entry; chainable.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.meta.insert(key.into(), value.into());
        self
    }

    /// The MIME subtype, with any leading `x-` stripped.
    /// `text/plain` → `"plain"`, `text/x-rhai` → `"rhai"`.
    pub fn subtype(&self) -> Option<&str> {
        let after_slash = self.mime.split('/').nth(1)?;
        let no_params = after_slash.split(';').next().unwrap_or(after_slash);
        let trimmed = no_params.trim();
        Some(trimmed.strip_prefix("x-").unwrap_or(trimmed))
    }

    /// Borrow this representation as text, when `mime` starts with
    /// `text/` and `bytes` are valid UTF-8.
    pub fn as_text(&self) -> Option<&str> {
        if !self.mime.starts_with("text/") {
            return None;
        }
        core::str::from_utf8(&self.bytes).ok()
    }

    /// Decode this representation as a [`Bitmap`] when its MIME is
    /// `image/x-portable-graymap` and the bytes parse as PGM (P5).
    pub fn as_bitmap(&self) -> Option<Bitmap> {
        if self.mime != "image/x-portable-graymap" {
            return None;
        }
        Bitmap::from_pgm(&self.bytes)
    }
}

/// Multi-representation envelope.
///
/// Most messages carry a single representation; richer senders may
/// supply alternates (e.g. styled text *and* plain text) so the
/// receiver can pick the best one it understands. A receiver that
/// doesn't care about alternates simply uses
/// [`Self::primary`] / [`Self::as_text`] / [`Self::as_bitmap`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExchangePayload {
    pub representations: Vec<Representation>,
}

impl ExchangePayload {
    /// Empty payload (no representations).
    pub const fn empty() -> Self {
        Self {
            representations: Vec::new(),
        }
    }

    /// Wrap a single representation.
    pub fn from_representation(rep: Representation) -> Self {
        Self {
            representations: vec![rep],
        }
    }

    /// Single-rep `text/plain`.
    pub fn from_text(s: impl Into<String>) -> Self {
        Self::from_representation(Representation::text(s))
    }

    /// Single-rep PGM bitmap.
    pub fn from_bitmap(bitmap: &Bitmap) -> Self {
        Self::from_representation(Representation::bitmap(bitmap))
    }

    /// Append another representation; chainable.
    pub fn with_representation(mut self, rep: Representation) -> Self {
        self.representations.push(rep);
        self
    }

    /// First (preferred) representation.
    pub fn primary(&self) -> Option<&Representation> {
        self.representations.first()
    }

    /// First representation whose MIME exactly matches `mime`.
    pub fn find_by_mime(&self, mime: &str) -> Option<&Representation> {
        self.representations.iter().find(|r| r.mime == mime)
    }

    /// First representation classified as `kind` by [`classify_mime`].
    pub fn find_kind(&self, kind: Kind) -> Option<&Representation> {
        self.representations
            .iter()
            .find(|r| classify_mime(&r.mime) == kind)
    }

    /// Convenience: text view of the first text representation.
    pub fn as_text(&self) -> Option<&str> {
        self.find_kind(Kind::Text).and_then(Representation::as_text)
    }

    /// Convenience: decoded bitmap view of the first bitmap representation.
    pub fn as_bitmap(&self) -> Option<Bitmap> {
        self.find_kind(Kind::Bitmap)
            .and_then(Representation::as_bitmap)
    }
}

/// Convert a single text-or-bitmap [`Representation`] into a payload.
impl From<Representation> for ExchangePayload {
    fn from(rep: Representation) -> Self {
        ExchangePayload::from_representation(rep)
    }
}

// -----------------------------------------------------------------------
// Bitmap
// -----------------------------------------------------------------------

/// In-memory grayscale bitmap (Gray8). Convertible to/from PGM bytes.
///
/// `pixels.len()` must equal `width as usize * height as usize`. SoulOS
/// uses Gray8 throughout; the on-the-wire form is PGM (P5) with
/// `maxval = 255`. PalmOS-style "offscreen window" semantics fall out
/// for free: a bitmap is just pixels you can both draw into and ship
/// across an exchange.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bitmap {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
}

impl Bitmap {
    /// New all-white bitmap of the given dimensions.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            pixels: vec![255u8; width as usize * height as usize],
        }
    }

    /// Wrap an already-allocated buffer; returns `None` if the length
    /// does not match `width * height`.
    pub fn from_pixels(width: u16, height: u16, pixels: Vec<u8>) -> Option<Self> {
        if pixels.len() != width as usize * height as usize {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    /// Encode as PGM (P5, maxval 255).
    pub fn to_pgm(&self) -> Vec<u8> {
        let header = format!("P5\n{} {}\n255\n", self.width, self.height);
        let mut out = Vec::with_capacity(header.len() + self.pixels.len());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&self.pixels);
        out
    }

    /// Decode from PGM (P5). Accepts comments (`#`) and arbitrary
    /// whitespace between header tokens, like the spec. Returns
    /// `None` for non-P5 streams or maxval ≠ 255 (we only support
    /// 8-bit grayscale).
    pub fn from_pgm(bytes: &[u8]) -> Option<Self> {
        let mut p = PgmParser::new(bytes);
        let magic = p.next_token()?;
        if magic != b"P5" {
            return None;
        }
        let width: u16 = p.next_token_str()?.parse().ok()?;
        let height: u16 = p.next_token_str()?.parse().ok()?;
        let maxval: u32 = p.next_token_str()?.parse().ok()?;
        if maxval != 255 {
            return None;
        }
        // Exactly one byte of whitespace separates the header and
        // the pixel data per the PGM spec.
        p.skip_one_whitespace()?;
        let len = width as usize * height as usize;
        if p.remaining() < len {
            return None;
        }
        let pixels = p.take(len)?.to_vec();
        Some(Self {
            width,
            height,
            pixels,
        })
    }
}

struct PgmParser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> PgmParser<'a> {
    fn new(src: &'a [u8]) -> Self {
        Self { src, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.src.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos + n > self.src.len() {
            return None;
        }
        let s = &self.src[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    /// Consume one whitespace byte (the PGM header/data separator).
    fn skip_one_whitespace(&mut self) -> Option<()> {
        match self.peek()? {
            b' ' | b'\t' | b'\n' | b'\r' => {
                self.advance();
                Some(())
            }
            _ => None,
        }
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => self.advance(),
                Some(b'#') => {
                    while let Some(c) = self.peek() {
                        self.advance();
                        if c == b'\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Option<&'a [u8]> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if matches!(c, b' ' | b'\t' | b'\n' | b'\r' | b'#') {
                break;
            }
            self.advance();
        }
        if self.pos == start {
            return None;
        }
        Some(&self.src[start..self.pos])
    }

    fn next_token_str(&mut self) -> Option<&'a str> {
        core::str::from_utf8(self.next_token()?).ok()
    }
}

// -----------------------------------------------------------------------
// Kind classification + registry
// -----------------------------------------------------------------------

/// Coarse classification of an exchange representation.
///
/// `Kind` is *not* part of the on-the-wire shape — it's a hint
/// derived from the MIME string for fast dispatch (e.g.
/// `payload.find_kind(Kind::Bitmap)`). Apps that introduce a new
/// MIME type can register it with [`ExchangeRegistry::register`] so
/// the kernel and the registry agree on its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Anything `text/*`.
    Text,
    /// `image/x-portable-graymap` (the canonical SoulOS bitmap).
    Bitmap,
    /// Anything else.
    Other,
}

/// Heuristic MIME → [`Kind`] mapping used when no registry lookup
/// is in scope. Strips MIME parameters (`;charset=…`).
pub fn classify_mime(mime: &str) -> Kind {
    let mime = mime.split(';').next().unwrap_or(mime).trim();
    if mime.eq_ignore_ascii_case("image/x-portable-graymap") {
        Kind::Bitmap
    } else if mime.len() >= 5 && mime[..5].eq_ignore_ascii_case("text/") {
        Kind::Text
    } else {
        Kind::Other
    }
}

/// One row in the [`ExchangeRegistry`]: a MIME type plus its
/// classification and any file extensions it owns.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub mime: String,
    pub kind: Kind,
    pub extensions: Vec<String>,
}

/// MIME ↔ kind ↔ extension table.
///
/// Owned by the host. Apps extend it from
/// [`crate::App::register_exchange_types`] when they introduce new
/// payload types. Outside the registry no SoulOS code should
/// hard-code MIME strings or file extensions.
#[derive(Debug, Default, Clone)]
pub struct ExchangeRegistry {
    entries: Vec<RegistryEntry>,
}

impl ExchangeRegistry {
    /// Empty registry.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Pre-populated with the types every SoulOS install supports:
    /// `text/plain`, `text/x-rhai`, and `image/x-portable-graymap`.
    /// Apps add more from `register_exchange_types`.
    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        r.register("text/plain", Kind::Text, &["txt"]);
        r.register("text/x-rhai", Kind::Text, &["rhai"]);
        r.register("image/x-portable-graymap", Kind::Bitmap, &["pgm"]);
        r
    }

    /// Insert or update a registry entry. If `mime` is already
    /// registered the old entry is replaced.
    pub fn register(&mut self, mime: impl Into<String>, kind: Kind, extensions: &[&str]) {
        let mime = mime.into();
        let extensions = extensions.iter().map(|s| s.to_string()).collect();
        let new = RegistryEntry {
            mime: mime.clone(),
            kind,
            extensions,
        };
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|e| e.mime.eq_ignore_ascii_case(&mime))
        {
            *slot = new;
        } else {
            self.entries.push(new);
        }
    }

    /// Convenience: register a `text/x-{subtype}` (or canonical text
    /// MIME) with the given file extensions.
    pub fn register_text_subtype(&mut self, subtype: &str, extensions: &[&str]) {
        let mime = if matches!(subtype, "plain" | "html" | "markdown" | "json" | "csv") {
            format!("text/{subtype}")
        } else {
            format!("text/x-{subtype}")
        };
        self.register(mime, Kind::Text, extensions);
    }

    /// Look up by MIME (case-insensitive; parameters are stripped).
    pub fn entry_for_mime(&self, mime: &str) -> Option<&RegistryEntry> {
        let head = mime.split(';').next().unwrap_or(mime).trim();
        self.entries
            .iter()
            .find(|e| e.mime.eq_ignore_ascii_case(head))
    }

    /// Look up by file extension (case-insensitive; leading `.` allowed).
    pub fn entry_for_extension(&self, ext: &str) -> Option<&RegistryEntry> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        self.entries.iter().find(|e| {
            e.extensions
                .iter()
                .any(|x| x.eq_ignore_ascii_case(&ext))
        })
    }

    /// MIME → [`Kind`]. Falls back to [`classify_mime`] when not
    /// registered, so unknown text/* still returns [`Kind::Text`].
    pub fn kind_for_mime(&self, mime: &str) -> Kind {
        self.entry_for_mime(mime)
            .map(|e| e.kind)
            .unwrap_or_else(|| classify_mime(mime))
    }

    /// Extension → MIME.
    pub fn mime_for_extension(&self, ext: &str) -> Option<&str> {
        self.entry_for_extension(ext).map(|e| e.mime.as_str())
    }

    /// MIME → preferred file extension (the first one registered).
    pub fn primary_extension_for_mime(&self, mime: &str) -> Option<&str> {
        self.entry_for_mime(mime)
            .and_then(|e| e.extensions.first().map(|s| s.as_str()))
    }

    /// Iterate all registered entries.
    pub fn entries(&self) -> &[RegistryEntry] {
        &self.entries
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trip() {
        let r = Representation::text("hello");
        assert_eq!(r.mime, "text/plain");
        assert_eq!(r.as_text(), Some("hello"));
        assert_eq!(r.subtype(), Some("plain"));
        assert!(r.as_bitmap().is_none());
    }

    #[test]
    fn text_subtype_x_prefix() {
        let r = Representation::text_with_subtype("rhai", "let x = 1;");
        assert_eq!(r.mime, "text/x-rhai");
        assert_eq!(r.subtype(), Some("rhai"));
        assert_eq!(r.as_text(), Some("let x = 1;"));
    }

    #[test]
    fn text_subtype_canonical() {
        let r = Representation::text_with_subtype("html", "<b>hi</b>");
        assert_eq!(r.mime, "text/html");
        assert_eq!(r.subtype(), Some("html"));
    }

    #[test]
    fn bitmap_round_trip() {
        let bm = Bitmap::from_pixels(3, 2, vec![10, 20, 30, 40, 50, 60]).unwrap();
        let r = Representation::bitmap(&bm);
        assert_eq!(r.mime, "image/x-portable-graymap");
        let decoded = r.as_bitmap().unwrap();
        assert_eq!(decoded.width, 3);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.pixels, vec![10, 20, 30, 40, 50, 60]);
    }

    #[test]
    fn pgm_skips_comments_and_whitespace() {
        let pgm = b"P5\n# a comment\n2 1\n255\n\xAA\xBB";
        let bm = Bitmap::from_pgm(pgm).unwrap();
        assert_eq!(bm.width, 2);
        assert_eq!(bm.height, 1);
        assert_eq!(bm.pixels, vec![0xAA, 0xBB]);
    }

    #[test]
    fn pgm_rejects_unsupported_maxval() {
        let pgm = b"P5\n2 1\n65535\n\x00\xff\x00\xff";
        assert!(Bitmap::from_pgm(pgm).is_none());
    }

    #[test]
    fn other_passthrough() {
        let r = Representation::other("application/x-rust", b"fn main() {}".to_vec());
        assert_eq!(r.mime, "application/x-rust");
        assert!(r.as_text().is_none());
        assert!(r.as_bitmap().is_none());
        assert_eq!(r.bytes, b"fn main() {}");
    }

    #[test]
    fn payload_picks_first_text_or_bitmap() {
        let bm = Bitmap::from_pixels(1, 1, vec![42]).unwrap();
        let p = ExchangePayload::default()
            .with_representation(Representation::other(
                "application/x-foo",
                b"bytes".to_vec(),
            ))
            .with_representation(Representation::text("hello"))
            .with_representation(Representation::bitmap(&bm));
        assert_eq!(p.as_text(), Some("hello"));
        assert_eq!(p.as_bitmap().unwrap().pixels, vec![42]);
    }

    #[test]
    fn classify_strips_parameters_and_is_case_insensitive() {
        assert_eq!(classify_mime("Text/Plain; charset=utf-8"), Kind::Text);
        assert_eq!(classify_mime("IMAGE/X-Portable-Graymap"), Kind::Bitmap);
        assert_eq!(classify_mime("application/octet-stream"), Kind::Other);
    }

    #[test]
    fn registry_round_trips_mime_and_extensions() {
        let r = ExchangeRegistry::with_builtins();
        assert_eq!(r.kind_for_mime("text/plain"), Kind::Text);
        assert_eq!(r.kind_for_mime("text/x-rhai"), Kind::Text);
        assert_eq!(r.kind_for_mime("image/x-portable-graymap"), Kind::Bitmap);
        assert_eq!(r.primary_extension_for_mime("text/x-rhai"), Some("rhai"));
        assert_eq!(r.mime_for_extension("pgm"), Some("image/x-portable-graymap"));
        assert_eq!(r.mime_for_extension(".rhai"), Some("text/x-rhai"));
        // Case-insensitive
        assert_eq!(r.mime_for_extension("PGM"), Some("image/x-portable-graymap"));
        assert_eq!(r.kind_for_mime("Text/Plain;charset=utf-8"), Kind::Text);
    }

    #[test]
    fn registry_unknown_text_subtype_falls_back() {
        let r = ExchangeRegistry::new();
        // Unregistered: falls back to heuristic.
        assert_eq!(r.kind_for_mime("text/x-foo"), Kind::Text);
        assert_eq!(r.kind_for_mime("application/x-foo"), Kind::Other);
        assert!(r.entry_for_mime("text/x-foo").is_none());
    }

    #[test]
    fn registry_register_replaces_existing_entry() {
        let mut r = ExchangeRegistry::with_builtins();
        r.register_text_subtype("rhai", &["rhai", "soul"]);
        let entry = r.entry_for_mime("text/x-rhai").unwrap();
        assert_eq!(entry.extensions, vec!["rhai".to_string(), "soul".to_string()]);
        assert_eq!(r.mime_for_extension("soul"), Some("text/x-rhai"));
    }

    #[test]
    fn payload_find_kind_finds_other() {
        let r = Representation::other("application/json", b"{}".to_vec());
        let p = ExchangePayload::from_representation(r);
        assert!(p.find_kind(Kind::Other).is_some());
        assert!(p.find_kind(Kind::Text).is_none());
    }
}

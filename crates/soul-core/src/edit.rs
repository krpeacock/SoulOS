//! Standard cut / copy / paste / select-all contract for widgets.
//!
//! [`EditTarget`] is the single abstraction the system edit menu and
//! the Snarf clipboard speak to.  A widget — or an app — that
//! implements it gains Cut / Copy / Paste / Select-All for free, with
//! no new app code:
//!
//! - The menu calls [`EditTarget::copy_selection`] or
//!   [`EditTarget::cut_selection`], takes the returned
//!   [`ExchangePayload`], and ships it to the system clipboard
//!   (Snarf).
//! - Paste is the reverse: the menu pulls the held payload from the
//!   clipboard and hands it to the focused widget's
//!   [`EditTarget::paste`].
//!
//! Widgets only override what they actually support — every method
//! has a no-op default. A read-only widget implements just
//! [`EditTarget::has_selection`], [`EditTarget::copy_selection`], and
//! [`EditTarget::select_all`]; a bitmap-only widget ignores text
//! payloads via [`EditTarget::accepts_paste`].
//!
//! The trait deliberately does **not** invoke any clipboard service
//! itself.  It returns the payload and the dirty rectangle, and the
//! caller (the shell edit menu, or an app's keyboard-shortcut handler)
//! decides what to do with them.

use crate::ExchangePayload;
use embedded_graphics::primitives::Rectangle;

/// Result of an edit operation against an [`EditTarget`].
///
/// Wraps three independent pieces of bookkeeping the caller may need:
/// the dirty rectangle to invalidate, a flag that the underlying
/// buffer changed (so the host can persist), and any payload the
/// operation produced (so the host can ship it to the clipboard).
/// Pure selection moves leave all three at their default of `None` /
/// `false` / `None`.
#[derive(Default, Debug, Clone)]
#[must_use = "edit operations may produce dirty regions and clipboard payloads that must be acted on"]
pub struct EditOutput {
    /// Bounding rectangle of pixels that need repainting, if any.
    pub dirty: Option<Rectangle>,
    /// `true` if the underlying buffer changed (cut, paste).
    /// Pure cursor or selection moves leave this `false`.
    pub text_changed: bool,
    /// Payload produced by the operation (e.g. the cut content).
    /// `cut_selection` populates this; the menu glue forwards it to
    /// the system clipboard.
    pub clipboard: Option<ExchangePayload>,
}

/// Widgets (and apps) that participate in the system edit menu.
///
/// Every method has a no-op default so implementers cover only what
/// they actually support.  See the [module docs](self) for the
/// dispatch model.
pub trait EditTarget {
    /// `true` when the target has a non-empty selection.
    /// Used to enable / disable the Cut and Copy menu items.
    fn has_selection(&self) -> bool {
        false
    }

    /// Return the selected content as an [`ExchangePayload`], or
    /// `None` if there is no selection or the target cannot serialise
    /// its selection.  Pure read; never mutates.
    fn copy_selection(&self) -> Option<ExchangePayload> {
        None
    }

    /// Cut: take the selected content and remove it from the target.
    /// Returns the lifted payload in [`EditOutput::clipboard`] plus
    /// the dirty rectangle for invalidation.  Read-only targets may
    /// implement `copy_selection` only and leave this as the default
    /// no-op.
    fn cut_selection(&mut self) -> EditOutput {
        EditOutput::default()
    }

    /// Replace the current selection (or insert at the cursor) with
    /// `payload`.  Targets that don't understand the payload kind
    /// should leave this as the default no-op and report so via
    /// [`accepts_paste`].
    fn paste(&mut self, _payload: &ExchangePayload) -> EditOutput {
        EditOutput::default()
    }

    /// `true` when the target can consume `payload` via [`paste`].
    /// Used to enable / disable the Paste menu item.  The default
    /// rejects everything.
    fn accepts_paste(&self, _payload: &ExchangePayload) -> bool {
        false
    }

    /// Extend selection to cover the entire target content.
    fn select_all(&mut self) -> EditOutput {
        EditOutput::default()
    }
}

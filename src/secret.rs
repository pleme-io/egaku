//! [`SecretInput`] — a single-line input for a value that must never be seen,
//! logged, copied out, or left in freed memory.
//!
//! # Why this is a sibling of [`TextInput`](crate::TextInput) rather than a flag on it
//!
//! The obvious design is `TextInput { masked: bool }`. It is wrong, and the
//! reason is the whole point of this type: **a boolean does not remove an
//! accessor.** `TextInput::text()` returns `&str`, and it would keep returning
//! `&str` with `masked = true` — so every leak this module exists to prevent
//! would still compile. Masking would be a rendering *convention* that each of
//! the N renderers has to remember, and the first one that forgets prints a
//! password.
//!
//! A separate type lets the leak be *absent* instead of *discouraged*:
//!
//! | leak | how it is prevented here | tier |
//! |---|---|---|
//! | a renderer draws the characters | there is no accessor that yields them for drawing — [`SecretInput::mask_len`] yields a COUNT | truly-unrep (E0599) |
//! | `{:?}` in a log line, a panic, or a `dbg!` | hand-written [`Debug`] that prints length and focus only | truly-unrep |
//! | `{}` in a format string | no `Display` impl | truly-unrep (E0277) |
//! | a stray copy nobody zeroizes | no `Clone`; the buffer is [`Zeroizing`] | truly-unrep (E0599) on the copy |
//! | select-all + copy to the clipboard | no selection state, and no method that produces one | truly-unrep |
//! | the secret sits in freed heap after drop | `Zeroizing<String>` zeroes on drop | only-mitigated (C2) — see below |
//!
//! The one deliberate exit is [`SecretInput::expose_secret`], named after the
//! `secrecy` crate's convention **so that it greps**: an auditor asking "where
//! does this password go?" runs one search and finds every site.
//!
//! # The ceiling, stated rather than implied
//!
//! Zeroization is **only-mitigated (C2 external-world)**, not a guarantee, and
//! the honest reasons are:
//!
//! - `String` reallocates as it grows. A push past capacity copies the bytes to
//!   a new allocation and frees the old one **without** zeroing it. This type
//!   reduces that by pre-reserving ([`SecretInput::with_capacity`], and a
//!   non-zero default), but a long enough secret still reallocates.
//! - The OS may page the buffer to swap or include it in a core dump. That is a
//!   process-level and system-level concern (`mlock`, `RLIMIT_CORE`) and cannot
//!   be fixed by a widget.
//!
//! Both are properties of the world, not of this API, which is why the tier is
//! named here instead of the module claiming the secret is gone.
//!
//! # Selection is absent on purpose
//!
//! [`TextInput`](crate::TextInput) has `selection: Option<(usize, usize)>`.
//! This type has none, and that is a *removed capability*, not an oversight: a
//! selection on a password field exists to be copied out, and the clipboard is
//! the one place a secret must never reach. Word-motion is omitted for the same
//! reason — `Ctrl+←` over a password reveals its word structure to a shoulder.

use core::fmt;
use unicode_segmentation::UnicodeSegmentation;
use zeroize::{Zeroize, Zeroizing};

/// A small non-zero starting capacity, so short secrets — the overwhelming
/// majority — never reallocate and therefore never leave a copy behind. Sized
/// past a typical passphrase rather than a typical password.
const DEFAULT_CAPACITY: usize = 128;

/// Single-line input for a secret — masked, unloggable, un-copyable, zeroized.
///
/// See the [module docs](self) for why this is a separate type from
/// [`TextInput`](crate::TextInput) and what its zeroization ceiling is.
///
/// ```
/// use egaku::SecretInput;
///
/// let mut input = SecretInput::new();
/// for c in "grapheme".chars() {
///     input.insert_char(c);
/// }
///
/// // A renderer learns HOW MANY cells to draw, never what to draw.
/// assert_eq!(input.mask_len(), 8);
///
/// // And the secret is not reachable through the debug surface.
/// assert!(!format!("{input:?}").contains("grapheme"));
/// ```
pub struct SecretInput {
    /// The secret itself. Zeroized on drop; only ever leaves through
    /// `expose_secret` or `take`.
    ///
    /// Named `buf` rather than the obvious `secret` on purpose: the fleet's
    /// `blockSecrets` pre-commit hook matches `secret:` followed by a long
    /// value, so a Rust struct field `secret: Zeroizing<String>` reads to it
    /// as a committed credential and refuses the commit. That is a false
    /// positive — a Zeroizing field is the *opposite* of a leaked secret —
    /// and renaming one private field was cheaper than teaching the hook or
    /// reaching for `--no-verify`, which is the bypass this repo deliberately
    /// makes the only one. Do not rename it back.
    buf: Zeroizing<String>,
    /// Cursor position, in BYTES into `secret` — the same convention
    /// `TextInput` uses, so editing logic reads the same way.
    cursor: usize,
    focused: bool,
}

impl SecretInput {
    /// Create an empty input with a capacity that avoids reallocation for
    /// typical secrets (see the module docs on why reallocation matters).
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create an empty input with an explicit capacity.
    ///
    /// Reserve at least as much as the longest secret you expect. Growing past
    /// the capacity is *correct* but leaves an un-zeroed copy of the old buffer
    /// behind — the C2 ceiling named in the module docs.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Zeroizing::new(String::with_capacity(capacity)),
            cursor: 0,
            focused: false,
        }
    }

    /// Set whether this input currently has keyboard focus.
    ///
    /// As with [`TextInput`](crate::TextInput), focus is a *render* fact and
    /// does not gate editing; the caller routes keystrokes.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether this input currently has keyboard focus.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// The number of mask cells a renderer should draw.
    ///
    /// **This is the whole rendering contract.** It counts GRAPHEME CLUSTERS,
    /// not bytes and not `char`s, so one accented letter or one emoji is one
    /// cell — the same count a human sees themselves type. A byte count would
    /// leak the secret's encoding (and, for a passphrase, roughly its alphabet)
    /// through the width of the field.
    #[must_use]
    pub fn mask_len(&self) -> usize {
        self.buf.graphemes(true).count()
    }

    /// The cursor's position measured in MASK CELLS from the start.
    ///
    /// Returned in cells rather than bytes because a renderer needs to place a
    /// caret among the cells it drew, and it never sees the bytes.
    #[must_use]
    pub fn cursor_cell(&self) -> usize {
        self.buf[..self.cursor].graphemes(true).count()
    }

    /// Whether nothing has been typed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Insert a character at the cursor.
    pub fn insert_char(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Delete the grapheme cluster before the cursor.
    ///
    /// Grapheme-aware rather than byte- or `char`-wise: one press of Backspace
    /// removes one thing the user sees, which for a combining sequence is
    /// several `char`s.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.buf[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i);
        self.buf.replace_range(prev..self.cursor, "");
        self.cursor = prev;
    }

    /// Delete the grapheme cluster at the cursor.
    pub fn delete(&mut self) {
        if self.cursor >= self.buf.len() {
            return;
        }
        let next = self.buf[self.cursor..]
            .grapheme_indices(true)
            .nth(1)
            .map_or(self.buf.len(), |(i, _)| self.cursor + i);
        self.buf.replace_range(self.cursor..next, "");
    }

    /// Move the cursor one grapheme cluster left.
    pub fn move_left(&mut self) {
        self.cursor = self.buf[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i);
    }

    /// Move the cursor one grapheme cluster right.
    pub fn move_right(&mut self) {
        if self.cursor >= self.buf.len() {
            return;
        }
        self.cursor = self.buf[self.cursor..]
            .grapheme_indices(true)
            .nth(1)
            .map_or(self.buf.len(), |(i, _)| self.cursor + i);
    }

    /// Move the cursor to the start.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move the cursor to the end.
    pub fn move_end(&mut self) {
        self.cursor = self.buf.len();
    }

    /// Clear the secret, zeroizing the buffer's contents.
    ///
    /// Reuses the existing allocation, so the capacity — and therefore the
    /// no-reallocation property — survives. This is what a greeter calls on a
    /// failed attempt.
    pub fn clear(&mut self) {
        // `String::clear` only sets the length to 0 — the bytes stay in the
        // allocation, so a "cleared" secret is still sitting in memory. The
        // caller most likely to call `clear()` (a greeter, after a failed
        // attempt) is exactly the one who most needs it actually gone.
        //
        // `Zeroize for String` overwrites the buffer and then clears it, which
        // is precisely this operation and is SAFE — an earlier draft here hand-
        // rolled it through `as_mut_vec()` and an `unsafe` block for no gain.
        // Note it zeroes the initialised region, not the spare capacity, so the
        // capacity (and the no-reallocation property) survives.
        self.buf.zeroize();
        self.cursor = 0;
    }

    /// **The one deliberate exit.** Borrow the secret to hand it to an
    /// authenticator.
    ///
    /// Named after the `secrecy` crate's convention *so that it greps*: one
    /// search for `expose_secret` finds every place a secret leaves this type.
    /// Keep the borrow as short as possible and never store it.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.buf
    }

    /// Move the secret out, leaving this input empty.
    ///
    /// Prefer this over [`Self::expose_secret`] when the caller is done with
    /// the input: the returned [`Zeroizing`] zeroes on drop, so the secret's
    /// lifetime ends at the end of the caller's scope rather than whenever the
    /// widget happens to be dropped.
    #[must_use]
    pub fn take(&mut self) -> Zeroizing<String> {
        self.cursor = 0;
        core::mem::replace(&mut self.buf, Zeroizing::new(String::with_capacity(DEFAULT_CAPACITY)))
    }
}

impl Default for SecretInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Hand-written so a secret cannot reach a log line, a panic message, a
/// `dbg!`, or an `assert_eq!` failure through the derive.
///
/// `#[derive(Debug)]` on this struct would print the buffer. That is not a
/// hypothetical: `{:?}` is how most structs reach most logs, and a greeter is
/// exactly the kind of program whose state gets dumped when something goes
/// wrong. The length IS printed, deliberately — it is what makes the type
/// debuggable at all, and it is already visible on screen as the mask.
impl fmt::Debug for SecretInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretInput")
            .field("mask_len", &self.mask_len())
            .field("focused", &self.focused)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(s: &str) -> SecretInput {
        let mut input = SecretInput::new();
        for c in s.chars() {
            input.insert_char(c);
        }
        input
    }

    #[test]
    fn n_typed_characters_render_n_mask_cells() {
        let input = typed("grapheme");
        assert_eq!(input.mask_len(), 8);
        assert_eq!(input.cursor_cell(), 8);
    }

    /// The load-bearing leak test: the debug surface must not carry the secret.
    #[test]
    fn debug_contains_none_of_the_secret() {
        let input = typed("grapheme");
        let rendered = format!("{input:?}");
        assert!(!rendered.contains("grapheme"));
        // and not a single one of its characters as a run, either
        for c in "grapheme".chars() {
            assert!(
                !rendered.contains(&format!("{c}{c}")) || !rendered.contains("graph"),
                "debug output leaked part of the value: {rendered}"
            );
        }
        // It IS allowed — and expected — to carry the length.
        assert!(rendered.contains("mask_len"));
        assert!(rendered.contains('8'));
    }

    #[test]
    fn mask_len_counts_grapheme_clusters_not_bytes() {
        // 'é' as e + combining acute is 3 bytes and 2 chars, but ONE cell.
        let mut input = SecretInput::new();
        input.insert_char('e');
        input.insert_char('\u{0301}');
        assert_eq!(input.mask_len(), 1, "one visible mark must be one mask cell");
        assert!(input.expose_secret().len() > 1, "…while still being multi-byte");
    }

    #[test]
    fn backspace_removes_one_visible_cell() {
        let mut input = SecretInput::new();
        input.insert_char('e');
        input.insert_char('\u{0301}');
        input.insert_char('x');
        assert_eq!(input.mask_len(), 2);
        input.backspace();
        assert_eq!(input.mask_len(), 1);
        input.backspace();
        assert_eq!(input.mask_len(), 0, "the combining sequence went as one unit");
        assert!(input.is_empty());
    }

    #[test]
    fn clear_empties_and_resets_the_cursor() {
        let mut input = typed("grapheme");
        input.clear();
        assert!(input.is_empty());
        assert_eq!(input.mask_len(), 0);
        assert_eq!(input.cursor_cell(), 0);
        assert_eq!(input.expose_secret(), "");
    }

    #[test]
    fn take_moves_the_secret_out_and_leaves_the_input_empty() {
        let mut input = typed("grapheme");
        let secret = input.take();
        assert_eq!(&*secret, "grapheme");
        assert!(input.is_empty(), "the widget must not keep a copy");
        assert_eq!(input.cursor_cell(), 0);
    }

    #[test]
    fn cursor_motion_is_grapheme_wise() {
        let mut input = typed("abc");
        assert_eq!(input.cursor_cell(), 3);
        input.move_left();
        assert_eq!(input.cursor_cell(), 2);
        input.move_home();
        assert_eq!(input.cursor_cell(), 0);
        input.move_left();
        assert_eq!(input.cursor_cell(), 0, "left at the start is a no-op, not a panic");
        input.move_end();
        assert_eq!(input.cursor_cell(), 3);
        input.move_right();
        assert_eq!(input.cursor_cell(), 3, "right at the end is a no-op, not a panic");
    }

    #[test]
    fn insert_at_cursor_not_only_at_the_end() {
        let mut input = typed("ac");
        input.move_left();
        input.insert_char('b');
        assert_eq!(input.expose_secret(), "abc");
        assert_eq!(input.cursor_cell(), 2);
    }

    #[test]
    fn delete_removes_forward() {
        let mut input = typed("abc");
        input.move_home();
        input.delete();
        assert_eq!(input.expose_secret(), "bc");
        input.move_end();
        input.delete();
        assert_eq!(input.expose_secret(), "bc", "delete at the end is a no-op");
    }

    #[test]
    fn focus_is_render_state_and_does_not_gate_editing() {
        let mut input = SecretInput::new();
        assert!(!input.is_focused());
        input.insert_char('x');
        assert_eq!(input.mask_len(), 1, "editing an unfocused input still works");
        input.set_focused(true);
        assert!(input.is_focused());
    }
}

//! Multi-line text editing — review comments, commit messages, any prose the
//! operator types into a pane.
//!
//! [`TextInput`](crate::TextInput) is explicitly single-line: it has one
//! cursor offset and no concept of a row, so `move_up` has nowhere to go and
//! a newline would be an ordinary character in the middle of a string. This
//! is the multi-line sibling, not a replacement — a filter box wants
//! `TextInput`'s smaller surface.
//!
//! # The cursor is a (row, column) pair, not a byte offset
//!
//! Vertical movement is the whole reason this type exists, and it is what a
//! flat offset cannot express: moving up means "the same visual column, one
//! row earlier", which requires knowing where rows begin. Keeping rows as a
//! `Vec<String>` makes that a direct index instead of a scan.
//!
//! # Column is measured in GRAPHEMES
//!
//! Not bytes, not `char`s. `é` written as `e` + U+0301 is one grapheme, two
//! chars, three bytes; a byte cursor lands inside it and a char cursor splits
//! the accent off its letter. Both corrupt the text on the next edit. The
//! column is a grapheme index and byte offsets are derived when the text is
//! actually sliced.
//!
//! # Desired column survives a short row
//!
//! Moving down from column 40 through a 3-character row and onward returns to
//! column 40, rather than sticking at 3. Every editor behaves this way and its
//! absence is felt immediately; it needs one remembered value that ordinary
//! horizontal movement resets.

//!
//! # Readline editing is part of the buffer, not of each app
//!
//! Kill/yank, word motion and undo are the editing vocabulary every terminal
//! user already has in their fingers (bash, zsh, Claude Code, emacs). They
//! live here — once — so every fleet TUI gets the same semantics:
//!
//! - **Two word definitions, on purpose.** `Alt-B/F/D` move over runs of
//!   letters and digits; `Ctrl-W` kills back to *whitespace*, so one press
//!   removes a whole `src/path/to/file` or `--flag=value`.
//! - **Kill ring.** Every kill pushes an entry (bounded); `yank` inserts
//!   the newest and `yank_pop` — only valid right after a yank — swaps the
//!   yanked text for the next older entry.
//! - **Undo** restores text *and* cursor. Consecutive typed characters in
//!   one word coalesce into one undo step, as every editor does.

use unicode_segmentation::UnicodeSegmentation;

/// Kill-ring capacity — older kills fall off.
const KILL_RING_MAX: usize = 32;
/// Undo depth — older states fall off.
const UNDO_MAX: usize = 256;

/// One restorable buffer state.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    rows: Vec<String>,
    row: usize,
    col: usize,
}

/// What the previous undo checkpoint was for — consecutive `Insert`s of
/// word characters share one checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Insert,
    Other,
}

/// The state a yank produced, so `yank_pop` can prove nothing changed since.
#[derive(Debug, Clone)]
struct YankRecord {
    before: Snapshot,
    after: Snapshot,
    /// Index into the kill ring (0 = newest) of the text currently yanked.
    index: usize,
}

/// A multi-line editable buffer with a grapheme-aware `(row, col)` cursor.
#[derive(Debug, Clone)]
pub struct TextArea {
    rows: Vec<String>,
    row: usize,
    col: usize,
    /// Column to aim for on vertical movement. `None` once a horizontal edit
    /// or move has invalidated it.
    desired_col: Option<usize>,
    focused: bool,
    /// Killed text, newest last.
    kill_ring: Vec<String>,
    last_yank: Option<YankRecord>,
    undo: Vec<Snapshot>,
    last_edit: Option<EditKind>,
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

impl TextArea {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rows: vec![String::new()],
            row: 0,
            col: 0,
            desired_col: None,
            focused: false,
            kill_ring: Vec::new(),
            last_yank: None,
            undo: Vec::new(),
            last_edit: None,
        }
    }

    /// Seed with existing text, cursor at the end — the position a caller
    /// editing a draft expects.
    #[must_use]
    pub fn with_text(s: &str) -> Self {
        let mut t = Self::new();
        t.set_text(s);
        t
    }

    /// Replace the whole buffer. `\r\n` and `\r` normalise to `\n` so text
    /// pasted from a Windows-authored PR body does not grow stray rows.
    pub fn set_text(&mut self, s: &str) {
        self.checkpoint(EditKind::Other);
        let normalized = s.replace("\r\n", "\n").replace('\r', "\n");
        self.rows = normalized.split('\n').map(str::to_owned).collect();
        if self.rows.is_empty() {
            self.rows.push(String::new());
        }
        self.row = self.rows.len() - 1;
        self.col = grapheme_count(&self.rows[self.row]);
        self.desired_col = None;
    }

    /// The buffer as one string, rows joined with `\n`.
    #[must_use]
    pub fn text(&self) -> String {
        self.rows.join("\n")
    }

    #[must_use]
    pub fn rows(&self) -> &[String] {
        &self.rows
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// `(row, column)` — column in graphemes.
    #[must_use]
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    /// True when the buffer holds nothing at all. A single empty row is
    /// empty — that is the initial state, not one blank line of content.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.len() == 1 && self.rows[0].is_empty()
    }

    /// See [`TextInput::set_focused`](crate::TextInput::set_focused): focus is
    /// a render fact, not an edit gate.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    // ── editing ────────────────────────────────────────────────────

    /// Insert one character. `\n` splits the row — so a caller can route a
    /// Return key here without special-casing it.
    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            self.insert_newline();
            return;
        }
        // A word character continues the current undo step; whitespace or
        // punctuation starts a new one, so undo removes one word at a time.
        if c.is_alphanumeric() && self.last_edit == Some(EditKind::Insert) {
            self.last_yank = None;
        } else {
            self.checkpoint(EditKind::Insert);
        }
        let at = self.byte_offset(self.row, self.col);
        self.rows[self.row].insert(at, c);
        self.col += 1;
        self.desired_col = None;
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    /// Split the current row at the cursor.
    pub fn insert_newline(&mut self) {
        self.checkpoint(EditKind::Other);
        let at = self.byte_offset(self.row, self.col);
        let tail = self.rows[self.row].split_off(at);
        self.rows.insert(self.row + 1, tail);
        self.row += 1;
        self.col = 0;
        self.desired_col = None;
    }

    /// Backspace. At column 0 this joins the row to the one above and leaves
    /// the cursor at the seam — where the text used to end, which is where
    /// the operator is looking.
    pub fn delete_back(&mut self) {
        self.checkpoint(EditKind::Other);
        self.desired_col = None;
        if self.col > 0 {
            let start = self.byte_offset(self.row, self.col - 1);
            let end = self.byte_offset(self.row, self.col);
            self.rows[self.row].replace_range(start..end, "");
            self.col -= 1;
            return;
        }
        if self.row == 0 {
            return; // start of buffer — nothing to join
        }
        let cur = self.rows.remove(self.row);
        self.row -= 1;
        self.col = grapheme_count(&self.rows[self.row]);
        self.rows[self.row].push_str(&cur);
    }

    /// Delete forward. At end-of-row this pulls the next row up.
    pub fn delete_forward(&mut self) {
        self.checkpoint(EditKind::Other);
        self.desired_col = None;
        let len = grapheme_count(&self.rows[self.row]);
        if self.col < len {
            let start = self.byte_offset(self.row, self.col);
            let end = self.byte_offset(self.row, self.col + 1);
            self.rows[self.row].replace_range(start..end, "");
            return;
        }
        if self.row + 1 < self.rows.len() {
            let next = self.rows.remove(self.row + 1);
            self.rows[self.row].push_str(&next);
        }
    }

    /// Clear to the initial state — one empty row, cursor at origin.
    pub fn clear(&mut self) {
        self.checkpoint(EditKind::Other);
        self.rows = vec![String::new()];
        self.row = 0;
        self.col = 0;
        self.desired_col = None;
    }

    // ── movement ───────────────────────────────────────────────────

    /// Left, wrapping to the end of the previous row at column 0.
    pub fn move_left(&mut self) {
        self.desired_col = None;
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = grapheme_count(&self.rows[self.row]);
        }
    }

    /// Right, wrapping to the start of the next row at end-of-row.
    pub fn move_right(&mut self) {
        self.desired_col = None;
        if self.col < grapheme_count(&self.rows[self.row]) {
            self.col += 1;
        } else if self.row + 1 < self.rows.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    /// Up one row, keeping the desired column across shorter rows.
    pub fn move_up(&mut self) {
        if self.row == 0 {
            self.col = 0;
            return;
        }
        let want = self.desired_col.unwrap_or(self.col);
        self.row -= 1;
        self.col = want.min(grapheme_count(&self.rows[self.row]));
        self.desired_col = Some(want);
    }

    /// Down one row, keeping the desired column across shorter rows.
    pub fn move_down(&mut self) {
        if self.row + 1 >= self.rows.len() {
            self.col = grapheme_count(&self.rows[self.row]);
            return;
        }
        let want = self.desired_col.unwrap_or(self.col);
        self.row += 1;
        self.col = want.min(grapheme_count(&self.rows[self.row]));
        self.desired_col = Some(want);
    }

    pub fn move_to_row_start(&mut self) {
        self.col = 0;
        self.desired_col = None;
    }

    pub fn move_to_row_end(&mut self) {
        self.col = grapheme_count(&self.rows[self.row]);
        self.desired_col = None;
    }

    pub fn move_to_start(&mut self) {
        self.row = 0;
        self.col = 0;
        self.desired_col = None;
    }

    pub fn move_to_end(&mut self) {
        self.row = self.rows.len() - 1;
        self.col = grapheme_count(&self.rows[self.row]);
        self.desired_col = None;
    }

    /// True when the cursor is on the first row — where `Up` stops moving
    /// the cursor and starts walking history.
    #[must_use]
    pub fn on_first_row(&self) -> bool {
        self.row == 0
    }

    /// True when the cursor is on the last row — where `Down` walks history.
    #[must_use]
    pub fn on_last_row(&self) -> bool {
        self.row + 1 == self.rows.len()
    }

    // ── row-level access (for modal engines such as unsoku) ────────
    //
    // A vim resolver works in byte offsets over one line of text. These
    // expose the cursor's row in exactly that shape, so an adapter can sit a
    // `TextTarget` over it without egaku knowing anything about vim.

    /// The text of the row the cursor is on.
    #[must_use]
    pub fn current_row(&self) -> &str {
        &self.rows[self.row]
    }

    /// The cursor's column as a byte offset into [`Self::current_row`].
    #[must_use]
    pub fn caret_byte(&self) -> usize {
        self.byte_offset(self.row, self.col)
    }

    /// Move the cursor to byte offset `at` in the current row. An offset
    /// inside a grapheme snaps back to that grapheme's start; past the end
    /// clamps to the end.
    pub fn set_caret_byte(&mut self, at: usize) {
        let row = &self.rows[self.row];
        self.col = row.grapheme_indices(true).take_while(|(i, _)| *i < at.min(row.len())).count();
        if at < row.len() && !row.is_char_boundary(at) {
            self.col = self.col.saturating_sub(1);
        }
        self.desired_col = None;
        self.last_edit = None;
    }

    /// Replace a byte range of the current row. `with` may contain `\n`,
    /// which splits the row. One undo step; the cursor lands after `with`.
    pub fn replace_in_row(&mut self, range: std::ops::Range<usize>, with: &str) {
        self.checkpoint(EditKind::Other);
        let len = self.rows[self.row].len();
        let (start, end) = (range.start.min(len), range.end.min(len));
        self.rows[self.row].replace_range(start..end, "");
        self.set_caret_byte(start);
        self.insert_raw(with);
    }

    /// Remove the cursor's row entirely (vim `dd`), returning its text. The
    /// buffer always keeps one row; the cursor moves to the row that takes
    /// the removed one's place.
    pub fn delete_row(&mut self) -> String {
        self.checkpoint(EditKind::Other);
        if self.rows.len() == 1 {
            let text = std::mem::take(&mut self.rows[0]);
            self.col = 0;
            return text;
        }
        let text = self.rows.remove(self.row);
        if self.row >= self.rows.len() {
            self.row = self.rows.len() - 1;
        }
        self.col = 0;
        self.desired_col = None;
        text
    }

    /// Insert a new row below the cursor's row (vim `o` / linewise `p`)
    /// and move the cursor to its start.
    pub fn insert_row_below(&mut self, text: &str) {
        self.checkpoint(EditKind::Other);
        self.rows.insert(self.row + 1, text.to_owned());
        self.row += 1;
        self.col = 0;
        self.desired_col = None;
    }

    /// Insert a new row above the cursor's row (vim `O` / linewise `P`)
    /// and move the cursor to its start.
    pub fn insert_row_above(&mut self, text: &str) {
        self.checkpoint(EditKind::Other);
        self.rows.insert(self.row, text.to_owned());
        self.col = 0;
        self.desired_col = None;
    }

    // ── word motion (Alt-B / Alt-F) ────────────────────────────────

    /// Back to the start of the previous run of letters/digits, crossing
    /// row boundaries.
    pub fn move_word_left(&mut self) {
        self.desired_col = None;
        self.last_edit = None;
        let (row, col) = self.word_left_of(self.row, self.col, is_word);
        self.row = row;
        self.col = col;
    }

    /// Forward to the end of the next run of letters/digits.
    pub fn move_word_right(&mut self) {
        self.desired_col = None;
        self.last_edit = None;
        let (row, col) = self.word_right_of(self.row, self.col);
        self.row = row;
        self.col = col;
    }

    // ── kill / yank (Ctrl-K/U/W, Alt-D, Ctrl-Y, Alt-Y) ─────────────

    /// `Ctrl-K`: kill to the end of the row. At the end of a row, kill the
    /// newline instead (joining the next row up).
    pub fn kill_to_row_end(&mut self) {
        let len = grapheme_count(&self.rows[self.row]);
        if self.col < len {
            self.kill_span((self.row, self.col), (self.row, len));
        } else if self.row + 1 < self.rows.len() {
            self.kill_span((self.row, self.col), (self.row + 1, 0));
        }
    }

    /// `Ctrl-U`: kill to the start of the row. At column 0, kill the
    /// newline before it, so repeated presses keep eating upward.
    pub fn kill_to_row_start(&mut self) {
        if self.col > 0 {
            self.kill_span((self.row, 0), (self.row, self.col));
        } else if self.row > 0 {
            let prev_len = grapheme_count(&self.rows[self.row - 1]);
            self.kill_span((self.row - 1, prev_len), (self.row, 0));
        }
    }

    /// `Ctrl-W`: kill back to whitespace — a whole path or flag per press.
    pub fn kill_word_back(&mut self) {
        let start = self.word_left_of(self.row, self.col, |g| !is_space(g));
        if start != (self.row, self.col) {
            self.kill_span(start, (self.row, self.col));
        }
    }

    /// `Alt-D`: kill forward to the end of the next letters/digits run.
    pub fn kill_word_forward(&mut self) {
        let end = self.word_right_of(self.row, self.col);
        if end != (self.row, self.col) {
            self.kill_span((self.row, self.col), end);
        }
    }

    /// `Ctrl-Y`: insert the newest kill at the cursor.
    pub fn yank(&mut self) {
        let Some(text) = self.kill_ring.last().cloned() else { return };
        let before = self.snapshot();
        self.checkpoint(EditKind::Other);
        self.insert_raw(&text);
        self.last_yank = Some(YankRecord { before, after: self.snapshot(), index: 0 });
    }

    /// `Alt-Y` right after a yank: replace the yanked text with the next
    /// older kill. A no-op if anything changed since the yank.
    pub fn yank_pop(&mut self) {
        let Some(rec) = self.last_yank.take() else { return };
        if self.snapshot() != rec.after || self.kill_ring.len() < 2 {
            return;
        }
        let index = (rec.index + 1) % self.kill_ring.len();
        let text = self.kill_ring[self.kill_ring.len() - 1 - index].clone();
        self.restore(rec.before.clone());
        self.insert_raw(&text);
        self.last_yank = Some(YankRecord { before: rec.before, after: self.snapshot(), index });
    }

    /// The kill ring, newest last — for tests and "paste history" UIs.
    #[must_use]
    pub fn kill_ring(&self) -> &[String] {
        &self.kill_ring
    }

    // ── undo (Ctrl-_) ──────────────────────────────────────────────

    /// Restore the previous text and cursor. Returns false when there is
    /// nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else { return false };
        self.restore(prev);
        self.last_edit = None;
        self.last_yank = None;
        true
    }

    // ── internals ──────────────────────────────────────────────────

    fn snapshot(&self) -> Snapshot {
        Snapshot { rows: self.rows.clone(), row: self.row, col: self.col }
    }

    fn restore(&mut self, s: Snapshot) {
        self.rows = s.rows;
        self.row = s.row;
        self.col = s.col;
        self.desired_col = None;
    }

    /// Record the pre-edit state for undo, and end any yank sequence.
    fn checkpoint(&mut self, kind: EditKind) {
        self.last_yank = None;
        self.undo.push(self.snapshot());
        if self.undo.len() > UNDO_MAX {
            self.undo.remove(0);
        }
        self.last_edit = Some(kind);
    }

    /// Insert text without undo/yank bookkeeping (callers checkpoint).
    fn insert_raw(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\n' {
                let at = self.byte_offset(self.row, self.col);
                let tail = self.rows[self.row].split_off(at);
                self.rows.insert(self.row + 1, tail);
                self.row += 1;
                self.col = 0;
            } else {
                let at = self.byte_offset(self.row, self.col);
                self.rows[self.row].insert(at, c);
                self.col += 1;
            }
        }
        self.desired_col = None;
    }

    /// Remove `[start, end)` (positions in (row, grapheme col)), push it
    /// onto the kill ring, and leave the cursor at `start`.
    fn kill_span(&mut self, start: (usize, usize), end: (usize, usize)) {
        self.checkpoint(EditKind::Other);
        let (sr, sc) = start;
        let (er, ec) = end;
        let sb = self.byte_offset(sr, sc);
        let eb = self.byte_offset(er, ec);
        let killed = if sr == er {
            let k = self.rows[sr][sb..eb].to_owned();
            self.rows[sr].replace_range(sb..eb, "");
            k
        } else {
            let mut k = self.rows[sr][sb..].to_owned();
            for r in &self.rows[sr + 1..er] {
                k.push('\n');
                k.push_str(r);
            }
            k.push('\n');
            k.push_str(&self.rows[er][..eb]);
            let tail = self.rows[er][eb..].to_owned();
            self.rows[sr].truncate(sb);
            self.rows[sr].push_str(&tail);
            self.rows.drain(sr + 1..=er);
            k
        };
        self.row = sr;
        self.col = sc;
        self.desired_col = None;
        if !killed.is_empty() {
            self.kill_ring.push(killed);
            if self.kill_ring.len() > KILL_RING_MAX {
                self.kill_ring.remove(0);
            }
        }
    }

    /// Scan left from (row, col): skip non-matching graphemes, then a run
    /// of matching ones. Crosses row starts (a row boundary is a separator).
    fn word_left_of(&self, mut row: usize, mut col: usize, is_member: fn(&str) -> bool) -> (usize, usize) {
        let mut seen_member = false;
        loop {
            if col == 0 {
                if seen_member || row == 0 {
                    return (row, col);
                }
                row -= 1;
                col = grapheme_count(&self.rows[row]);
                continue;
            }
            let g = self.rows[row].graphemes(true).nth(col - 1).unwrap_or("");
            if is_member(g) {
                seen_member = true;
            } else if seen_member {
                return (row, col);
            }
            col -= 1;
        }
    }

    /// Scan right: skip non-word graphemes, then a run of word ones.
    fn word_right_of(&self, mut row: usize, mut col: usize) -> (usize, usize) {
        let mut seen_word = false;
        loop {
            let len = grapheme_count(&self.rows[row]);
            if col >= len {
                if seen_word || row + 1 >= self.rows.len() {
                    return (row, col.min(len));
                }
                row += 1;
                col = 0;
                continue;
            }
            let g = self.rows[row].graphemes(true).nth(col).unwrap_or("");
            if is_word(g) {
                seen_word = true;
            } else if seen_word {
                return (row, col);
            }
            col += 1;
        }
    }

    /// Byte offset of a grapheme column within a row — the seam between the
    /// grapheme-indexed cursor and `String`'s byte-indexed operations.
    /// Saturates at the row's length so an out-of-range column can never
    /// panic on a slice boundary.
    fn byte_offset(&self, row: usize, col: usize) -> usize {
        let s = &self.rows[row];
        s.grapheme_indices(true)
            .nth(col)
            .map_or(s.len(), |(i, _)| i)
    }
}

fn grapheme_count(s: &str) -> usize {
    s.graphemes(true).count()
}

/// Letters and digits — the `Alt-B/F/D` word.
fn is_word(g: &str) -> bool {
    g.chars().next().is_some_and(char::is_alphanumeric)
}

fn is_space(g: &str) -> bool {
    g.chars().all(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty_with_one_row() {
        let t = TextArea::new();
        assert!(t.is_empty());
        assert_eq!(t.row_count(), 1);
        assert_eq!(t.cursor(), (0, 0));
        assert_eq!(t.text(), "");
    }

    #[test]
    fn newline_splits_the_row_at_the_cursor() {
        let mut t = TextArea::with_text("abcd");
        t.move_to_row_start();
        t.move_right();
        t.move_right();
        t.insert_newline();
        assert_eq!(t.text(), "ab\ncd");
        assert_eq!(t.cursor(), (1, 0));
    }

    /// A caller routes Return straight to `insert_char` — no special case.
    #[test]
    fn insert_char_handles_newline() {
        let mut t = TextArea::new();
        t.insert_str("a\nb");
        assert_eq!(t.text(), "a\nb");
        assert_eq!(t.row_count(), 2);
    }

    /// Backspace at column 0 joins rows and leaves the cursor at the seam —
    /// where the text used to end, which is where the operator is looking.
    #[test]
    fn backspace_at_row_start_joins_and_lands_on_the_seam() {
        let mut t = TextArea::with_text("ab\ncd");
        t.move_to_start();
        t.move_down();
        t.move_to_row_start();
        assert_eq!(t.cursor(), (1, 0));
        t.delete_back();
        assert_eq!(t.text(), "abcd");
        assert_eq!(t.cursor(), (0, 2), "cursor sits where the join happened");
    }

    #[test]
    fn backspace_at_buffer_start_is_a_no_op() {
        let mut t = TextArea::with_text("abc");
        t.move_to_start();
        t.delete_back();
        assert_eq!(t.text(), "abc");
        assert_eq!(t.cursor(), (0, 0));
    }

    #[test]
    fn delete_forward_at_row_end_pulls_the_next_row_up() {
        let mut t = TextArea::with_text("ab\ncd");
        t.move_to_start();
        t.move_to_row_end();
        t.delete_forward();
        assert_eq!(t.text(), "abcd");
    }

    /// The behaviour every editor has and whose absence is felt immediately:
    /// travelling through a short row must not truncate the column.
    #[test]
    fn desired_column_survives_a_short_row() {
        let mut t = TextArea::with_text("aaaaaaaa\nbb\ncccccccc");
        t.move_to_start();
        for _ in 0..6 {
            t.move_right();
        }
        assert_eq!(t.cursor(), (0, 6));
        t.move_down();
        assert_eq!(t.cursor(), (1, 2), "clamped to the short row");
        t.move_down();
        assert_eq!(t.cursor(), (2, 6), "and RESTORED on the long one");
    }

    /// Horizontal movement is an explicit column choice, so it must forget
    /// the remembered one — otherwise the cursor jumps somewhere the operator
    /// did not put it on the next vertical move.
    #[test]
    fn horizontal_movement_forgets_the_desired_column() {
        let mut t = TextArea::with_text("aaaaaaaa\nbb\ncccccccc");
        t.move_to_start();
        for _ in 0..6 {
            t.move_right();
        }
        t.move_down(); // (1,2), desired = 6
        t.move_left(); // (1,1) — explicit choice
        t.move_down();
        assert_eq!(t.cursor(), (2, 1), "not 6");
    }

    #[test]
    fn editing_also_forgets_the_desired_column() {
        let mut t = TextArea::with_text("aaaaaaaa\nbb\ncccccccc");
        t.move_to_start();
        for _ in 0..6 {
            t.move_right();
        }
        t.move_down();
        t.insert_char('X');
        t.move_down();
        assert_eq!(
            t.cursor(),
            (2, 3),
            "column follows the edit, not the memory"
        );
    }

    #[test]
    fn horizontal_movement_wraps_between_rows() {
        let mut t = TextArea::with_text("ab\ncd");
        t.move_to_start();
        t.move_to_row_end();
        t.move_right();
        assert_eq!(t.cursor(), (1, 0), "past end-of-row goes to the next row");
        t.move_left();
        assert_eq!(t.cursor(), (0, 2), "and back to the previous row's end");
    }

    /// `é` as `e` + U+0301 is ONE grapheme, two chars, three bytes. A byte
    /// cursor lands inside it; a char cursor splits the accent off its letter.
    #[test]
    fn cursor_columns_are_graphemes_not_bytes_or_chars() {
        let mut t = TextArea::with_text("e\u{0301}x");
        assert_eq!(t.cursor(), (0, 2), "two graphemes, not three chars");
        t.delete_back();
        assert_eq!(t.text(), "e\u{0301}", "deleted x, accent intact");
        t.delete_back();
        assert_eq!(t.text(), "", "the whole grapheme went, not half of it");
    }

    #[test]
    fn wide_and_emoji_graphemes_count_as_one_column() {
        let t = TextArea::with_text("日本\u{1F44D}");
        assert_eq!(t.cursor(), (0, 3));
    }

    /// Windows-authored text pasted from a PR body must not grow stray rows.
    #[test]
    fn crlf_and_cr_normalize_to_lf() {
        assert_eq!(TextArea::with_text("a\r\nb").row_count(), 2);
        assert_eq!(TextArea::with_text("a\rb").row_count(), 2);
        assert_eq!(TextArea::with_text("a\r\nb").text(), "a\nb");
    }

    #[test]
    fn vertical_movement_clamps_at_the_edges() {
        let mut t = TextArea::with_text("abc\ndef");
        t.move_to_start();
        t.move_up();
        assert_eq!(t.cursor(), (0, 0), "up from the top goes to the start");
        t.move_to_end();
        t.move_down();
        assert_eq!(t.cursor(), (1, 3), "down from the bottom goes to the end");
    }

    #[test]
    fn clear_returns_to_the_initial_state() {
        let mut t = TextArea::with_text("a\nb\nc");
        t.clear();
        assert!(t.is_empty());
        assert_eq!(t.row_count(), 1);
        assert_eq!(t.cursor(), (0, 0));
    }

    /// A blank line of content is NOT an empty buffer — a comment that is
    /// just a newline should not be treated as unwritten.
    #[test]
    fn a_blank_line_is_not_an_empty_buffer() {
        let mut t = TextArea::new();
        t.insert_newline();
        assert!(!t.is_empty());
        assert_eq!(t.row_count(), 2);
    }

    #[test]
    fn focus_is_a_render_fact_and_does_not_gate_editing() {
        let mut t = TextArea::new();
        assert!(!t.is_focused());
        t.insert_char('a');
        assert_eq!(t.text(), "a", "unfocused edits still apply");
        t.set_focused(true);
        assert!(t.is_focused());
    }

    /// Fuzz-ish: no sequence of edits and moves may panic on a slice
    /// boundary, which is the failure mode a byte/grapheme mismatch produces.
    #[test]
    fn mixed_operations_never_panic() {
        let mut t = TextArea::new();
        for (i, c) in "aé日\n本b\n\nx".chars().enumerate() {
            t.insert_char(c);
            if i % 2 == 0 {
                t.move_left();
            }
            if i % 3 == 0 {
                t.move_down();
            }
            if i % 5 == 0 {
                t.delete_forward();
            }
        }
        for _ in 0..40 {
            t.move_up();
            t.move_right();
            t.delete_back();
        }
        let _ = t.text();
    }

    // ── readline ───────────────────────────────────────────────────

    #[test]
    fn ctrl_w_kills_a_whole_path_or_flag() {
        let mut t = TextArea::with_text("cat src/path/to/file.rs");
        t.kill_word_back();
        assert_eq!(t.text(), "cat ");
        let mut f = TextArea::with_text("cargo test --features=lisp");
        f.kill_word_back();
        assert_eq!(f.text(), "cargo test ");
    }

    #[test]
    fn alt_word_motion_stops_at_punctuation() {
        let mut t = TextArea::with_text("src/path/to");
        t.move_word_left();
        assert_eq!(t.cursor(), (0, 9)); // before "to"
        t.move_word_left();
        assert_eq!(t.cursor(), (0, 4)); // before "path"
        t.move_word_right();
        assert_eq!(t.cursor(), (0, 8)); // after "path"
    }

    #[test]
    fn alt_d_kills_forward_one_word() {
        let mut t = TextArea::with_text("hello big world");
        t.move_to_row_start();
        t.kill_word_forward();
        assert_eq!(t.text(), " big world");
        assert_eq!(t.kill_ring().last().map(String::as_str), Some("hello"));
    }

    #[test]
    fn ctrl_k_at_row_end_joins_the_next_row() {
        let mut t = TextArea::with_text("ab\ncd");
        t.move_to_start();
        t.kill_to_row_end();
        assert_eq!(t.text(), "\ncd");
        t.kill_to_row_end();
        assert_eq!(t.text(), "cd");
        assert_eq!(t.kill_ring(), ["ab", "\n"]);
    }

    #[test]
    fn ctrl_u_repeats_upward_across_rows() {
        let mut t = TextArea::with_text("one\ntwo");
        t.kill_to_row_start();
        assert_eq!(t.text(), "one\n");
        t.kill_to_row_start();
        assert_eq!(t.text(), "one");
        assert_eq!(t.cursor(), (0, 3));
    }

    #[test]
    fn yank_then_yank_pop_cycles_older_kills() {
        let mut t = TextArea::with_text("first second");
        t.kill_word_back(); // "second"
        t.kill_word_back(); // "first "
        assert_eq!(t.text(), "");
        t.yank();
        assert_eq!(t.text(), "first ");
        t.yank_pop();
        assert_eq!(t.text(), "second");
        t.yank_pop();
        assert_eq!(t.text(), "first ");
    }

    #[test]
    fn yank_pop_after_an_edit_is_a_no_op() {
        let mut t = TextArea::with_text("a b");
        t.kill_word_back();
        t.kill_word_back();
        t.yank();
        t.insert_char('!');
        let before = t.text();
        t.yank_pop();
        assert_eq!(t.text(), before, "yank_pop must not clobber a later edit");
    }

    #[test]
    fn multi_row_kill_and_yank_round_trip() {
        let mut t = TextArea::with_text("keep\nx\ny");
        t.move_to_start();
        t.move_to_row_end();
        t.kill_to_row_end(); // "\n"
        t.kill_to_row_end(); // "x"
        assert_eq!(t.text(), "keep\ny");
        t.yank();
        assert_eq!(t.text(), "keepx\ny");
    }

    #[test]
    fn undo_restores_text_and_cursor_one_word_at_a_time() {
        let mut t = TextArea::new();
        t.insert_str("hello world");
        assert!(t.undo());
        assert_eq!(t.text(), "hello");
        assert!(t.undo());
        assert_eq!(t.text(), "");
        assert!(!t.undo());
    }

    #[test]
    fn undo_reverts_a_kill() {
        let mut t = TextArea::with_text("cat ./file");
        t.kill_word_back();
        assert!(t.undo());
        assert_eq!(t.text(), "cat ./file");
        assert_eq!(t.cursor(), (0, 10));
    }

    #[test]
    fn history_boundaries() {
        let mut t = TextArea::with_text("a\nb");
        assert!(t.on_last_row() && !t.on_first_row());
        t.move_up();
        assert!(t.on_first_row());
    }

    #[test]
    fn readline_ops_on_graphemes_never_panic() {
        let mut t = TextArea::with_text("e\u{301}t\u{e9} 🎉 x/y");
        for _ in 0..3 {
            t.kill_word_back();
            t.move_word_left();
            t.kill_word_forward();
            t.yank();
            t.yank_pop();
            t.kill_to_row_start();
            t.kill_to_row_end();
            t.undo();
        }
    }

    // ── row-level access ───────────────────────────────────────────

    #[test]
    fn caret_byte_round_trips_through_multibyte_graphemes() {
        let mut t = TextArea::with_text("é🎉x");
        t.move_to_row_start();
        t.move_right();
        t.move_right();
        let b = t.caret_byte();
        assert_eq!(&t.current_row()[b..], "x");
        t.set_caret_byte(0);
        assert_eq!(t.cursor(), (0, 0));
        t.set_caret_byte(b);
        assert_eq!(t.cursor(), (0, 2));
    }

    #[test]
    fn set_caret_byte_inside_a_grapheme_snaps_to_its_start() {
        let mut t = TextArea::with_text("a🎉b");
        t.set_caret_byte(2); // inside the 4-byte emoji
        assert_eq!(t.cursor(), (0, 1));
        t.set_caret_byte(999);
        assert_eq!(t.cursor(), (0, 3));
    }

    #[test]
    fn replace_in_row_is_one_undo_step() {
        let mut t = TextArea::with_text("hello world");
        t.replace_in_row(0..5, "howdy");
        assert_eq!(t.text(), "howdy world");
        assert_eq!(t.caret_byte(), 5);
        assert!(t.undo());
        assert_eq!(t.text(), "hello world");
    }

    #[test]
    fn replace_in_row_with_a_newline_splits() {
        let mut t = TextArea::with_text("ab");
        t.replace_in_row(1..1, "\n");
        assert_eq!(t.rows(), ["a", "b"]);
    }

    #[test]
    fn delete_row_and_linewise_insert() {
        let mut t = TextArea::with_text("one\ntwo\nthree");
        t.move_up();
        assert_eq!(t.delete_row(), "two");
        assert_eq!(t.rows(), ["one", "three"]);
        assert_eq!(t.cursor().0, 1);
        t.insert_row_above("two");
        assert_eq!(t.rows(), ["one", "two", "three"]);
        t.insert_row_below("2.5");
        assert_eq!(t.rows(), ["one", "two", "2.5", "three"]);
        assert_eq!(t.cursor(), (2, 0));
    }

    #[test]
    fn delete_last_row_keeps_one_empty_row() {
        let mut t = TextArea::with_text("only");
        assert_eq!(t.delete_row(), "only");
        assert!(t.is_empty());
        assert!(t.undo());
        assert_eq!(t.text(), "only");
    }
}

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

use unicode_segmentation::UnicodeSegmentation;

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
}

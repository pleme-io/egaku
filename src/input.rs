use unicode_segmentation::UnicodeSegmentation;

/// Single-line text input with cursor, selection, and grapheme-aware editing.
#[derive(Debug, Clone)]
pub struct TextInput {
    text: String,
    cursor: usize,
    selection: Option<(usize, usize)>,
}

impl TextInput {
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            selection: None,
        }
    }

    #[must_use]
    pub fn with_text(s: &str) -> Self {
        let len = s.len();
        Self {
            text: s.to_string(),
            cursor: len,
            selection: None,
        }
    }

    /// Returns the full text content.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the current cursor byte position.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns true if the text is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Insert a character at the current cursor position.
    /// If there is a selection, it is deleted first.
    pub fn insert_char(&mut self, c: char) {
        self.delete_selection();
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Delete the grapheme before the cursor (backspace).
    pub fn delete_back(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor == 0 {
            return;
        }
        // Find the previous grapheme boundary
        let before = &self.text[..self.cursor];
        if let Some(g) = before.grapheme_indices(true).next_back() {
            let prev_pos = g.0;
            self.text.drain(prev_pos..self.cursor);
            self.cursor = prev_pos;
        }
    }

    /// Delete the grapheme after the cursor (delete key).
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor >= self.text.len() {
            return;
        }
        let after = &self.text[self.cursor..];
        if let Some(g) = after.grapheme_indices(true).next() {
            let grapheme_len = g.1.len();
            self.text.drain(self.cursor..self.cursor + grapheme_len);
        }
    }

    /// Move cursor one grapheme to the left.
    pub fn move_left(&mut self) {
        self.selection = None;
        if self.cursor == 0 {
            return;
        }
        let before = &self.text[..self.cursor];
        if let Some(g) = before.grapheme_indices(true).next_back() {
            self.cursor = g.0;
        }
    }

    /// Move cursor one grapheme to the right.
    pub fn move_right(&mut self) {
        self.selection = None;
        if self.cursor >= self.text.len() {
            return;
        }
        let after = &self.text[self.cursor..];
        if let Some((_, grapheme)) = after.grapheme_indices(true).next() {
            self.cursor += grapheme.len();
        }
    }

    /// Move cursor to the start of text.
    pub fn move_to_start(&mut self) {
        self.selection = None;
        self.cursor = 0;
    }

    /// Move cursor to the end of text.
    pub fn move_to_end(&mut self) {
        self.selection = None;
        self.cursor = self.text.len();
    }

    /// Select all text.
    pub fn select_all(&mut self) {
        if self.text.is_empty() {
            return;
        }
        self.selection = Some((0, self.text.len()));
        self.cursor = self.text.len();
    }

    /// Returns the currently selected text, if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<&str> {
        let (start, end) = self.selection?;
        Some(&self.text[start..end])
    }

    /// Delete the current selection. Returns true if a selection was deleted.
    pub fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection.take() else {
            return false;
        };
        self.text.drain(start..end);
        self.cursor = start;
        true
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let input = TextInput::new();
        assert!(input.is_empty());
        assert_eq!(input.cursor(), 0);
        assert_eq!(input.text(), "");
    }

    #[test]
    fn with_text_sets_cursor_at_end() {
        let input = TextInput::with_text("hello");
        assert_eq!(input.text(), "hello");
        assert_eq!(input.cursor(), 5);
        assert!(!input.is_empty());
    }

    #[test]
    fn insert_char_at_end() {
        let mut input = TextInput::new();
        input.insert_char('a');
        input.insert_char('b');
        assert_eq!(input.text(), "ab");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn insert_char_at_middle() {
        let mut input = TextInput::with_text("ac");
        input.move_left(); // cursor before 'c'
        input.insert_char('b');
        assert_eq!(input.text(), "abc");
    }

    #[test]
    fn insert_multibyte_char() {
        let mut input = TextInput::new();
        input.insert_char('\u{00e9}'); // e with acute
        assert_eq!(input.text(), "\u{00e9}");
        assert_eq!(input.cursor(), 2); // 2-byte UTF-8
    }

    #[test]
    fn delete_back_basic() {
        let mut input = TextInput::with_text("abc");
        input.delete_back();
        assert_eq!(input.text(), "ab");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn delete_back_at_start_does_nothing() {
        let mut input = TextInput::with_text("abc");
        input.move_to_start();
        input.delete_back();
        assert_eq!(input.text(), "abc");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn delete_back_empty() {
        let mut input = TextInput::new();
        input.delete_back();
        assert_eq!(input.text(), "");
    }

    #[test]
    fn delete_back_multibyte() {
        let mut input = TextInput::with_text("he\u{0301}llo"); // e + combining accent
        // cursor at end
        input.delete_back(); // remove 'o'
        assert_eq!(input.text(), "he\u{0301}ll");
    }

    #[test]
    fn delete_forward_basic() {
        let mut input = TextInput::with_text("abc");
        input.move_to_start();
        input.delete_forward();
        assert_eq!(input.text(), "bc");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn delete_forward_at_end_does_nothing() {
        let mut input = TextInput::with_text("abc");
        input.delete_forward();
        assert_eq!(input.text(), "abc");
    }

    #[test]
    fn move_left_and_right() {
        let mut input = TextInput::with_text("abc");
        assert_eq!(input.cursor(), 3);
        input.move_left();
        assert_eq!(input.cursor(), 2);
        input.move_left();
        assert_eq!(input.cursor(), 1);
        input.move_right();
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn move_left_at_start() {
        let mut input = TextInput::with_text("a");
        input.move_to_start();
        input.move_left();
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn move_right_at_end() {
        let mut input = TextInput::with_text("a");
        input.move_right();
        assert_eq!(input.cursor(), 1);
    }

    #[test]
    fn move_to_start_and_end() {
        let mut input = TextInput::with_text("hello");
        input.move_to_start();
        assert_eq!(input.cursor(), 0);
        input.move_to_end();
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn select_all_and_selected_text() {
        let mut input = TextInput::with_text("hello");
        input.select_all();
        assert_eq!(input.selected_text(), Some("hello"));
    }

    #[test]
    fn select_all_empty() {
        let mut input = TextInput::new();
        input.select_all();
        assert_eq!(input.selected_text(), None);
    }

    #[test]
    fn delete_selection() {
        let mut input = TextInput::with_text("hello world");
        input.select_all();
        input.delete_selection();
        assert!(input.is_empty());
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn insert_replaces_selection() {
        let mut input = TextInput::with_text("hello");
        input.select_all();
        input.insert_char('X');
        assert_eq!(input.text(), "X");
    }

    #[test]
    fn grapheme_aware_movement_emoji() {
        // Family emoji is a single grapheme cluster but multiple code points
        let mut input = TextInput::with_text("a\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}b");
        input.move_to_end();
        input.move_left(); // move before 'b'
        input.move_left(); // move before the family emoji
        input.move_right(); // move past the family emoji
        // We should be right before 'b'
        input.delete_forward();
        assert!(input.text().ends_with('\u{1F467}'));
    }

    #[test]
    fn delete_back_with_selection_deletes_selection() {
        let mut input = TextInput::with_text("hello");
        input.select_all();
        input.delete_back();
        assert!(input.is_empty());
    }
}

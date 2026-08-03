//! Read-only scrolled, wrapped, styled document — a PR body, a review
//! thread, a log tail.
//!
//! The three pieces existed and did not meet. [`ScrollView`](crate::ScrollView)
//! tracks an offset and knows nothing about content; egaku-term's
//! `draw::wrap_text` / `draw::paragraph` wrap but do not scroll and carry no
//! styling. So every consumer that wanted "a scrollable paragraph" had to
//! join them by hand, and each one re-derived the same arithmetic.
//!
//! # Wrapping is a function of width, so it is recomputed on resize, not stored
//!
//! A wrapped line count depends on the viewport width. Caching wrapped rows
//! and forgetting to invalidate on resize is how a document renders with the
//! old wrap and a scrollbar that disagrees with it. [`Self::set_width`]
//! rewraps and re-clamps in one step; there is no way to change the width
//! without it.
//!
//! # Styling is per-span and carried, not interpreted
//!
//! A [`Span`] pairs text with a caller-defined `style: u8` — an index into
//! whatever palette the renderer owns. egaku does not know what "3" means,
//! which is exactly what keeps this type renderer-free and lets the same
//! document draw to a TTY and to a GPU pane. Wrapping splits spans at the
//! wrap point and preserves each half's style.

use unicode_segmentation::UnicodeSegmentation;

/// A run of text sharing one caller-defined style index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    text: String,
    style: u8,
}

impl Span {
    #[must_use]
    pub fn new(text: impl Into<String>, style: u8) -> Self {
        Self { text: text.into(), style }
    }
    /// Style 0 — the caller's default.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, 0)
    }
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Index into the renderer's palette. egaku never interprets it.
    #[must_use]
    pub fn style(&self) -> u8 {
        self.style
    }
    #[must_use]
    pub fn width(&self) -> usize {
        self.text.graphemes(true).count()
    }
}

/// One visual row after wrapping — what a renderer draws on a single line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WrappedLine {
    spans: Vec<Span>,
    /// Index of the logical (pre-wrap) line this came from, so a caller can
    /// map a visual row back to a source line — needed for "jump to line N"
    /// and for anchoring a comment to the right place.
    source_line: usize,
}

impl WrappedLine {
    #[must_use]
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }
    #[must_use]
    pub fn source_line(&self) -> usize {
        self.source_line
    }
    /// Plain text of this visual row, styles discarded.
    #[must_use]
    pub fn text(&self) -> String {
        self.spans.iter().map(Span::text).collect()
    }
    #[must_use]
    pub fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }
}

/// A scrollable, wrapped, styled read-only document.
#[derive(Debug, Clone, Default)]
pub struct TextView {
    /// Logical lines, each a span list. Wrapping never mutates this.
    lines: Vec<Vec<Span>>,
    wrapped: Vec<WrappedLine>,
    width: usize,
    height: usize,
    offset: usize,
}

impl TextView {
    /// Empty view. `set_width`/`set_height` before rendering — a zero width
    /// wraps to nothing, which is correct and visibly empty rather than a
    /// panic.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from plain text, one style for everything.
    #[must_use]
    pub fn from_text(s: &str, width: usize, height: usize) -> Self {
        let mut v = Self::new();
        v.width = width;
        v.height = height;
        v.set_lines(
            s.replace("\r\n", "\n")
                .replace('\r', "\n")
                .split('\n')
                .map(|l| vec![Span::plain(l)])
                .collect(),
        );
        v
    }

    /// Replace the document. Rewraps and clamps.
    pub fn set_lines(&mut self, lines: Vec<Vec<Span>>) {
        self.lines = lines;
        self.rewrap();
    }

    pub fn push_line(&mut self, spans: Vec<Span>) {
        self.lines.push(spans);
        self.rewrap();
    }

    /// Set the wrap width. Rewraps and re-clamps in one step — there is
    /// deliberately no way to change width without rewrapping, because a
    /// stale wrap plus a fresh scrollbar is the classic resize artefact.
    pub fn set_width(&mut self, width: usize) {
        if self.width != width {
            self.width = width;
            self.rewrap();
        }
    }

    /// Set the viewport height in rows. Re-clamps, since a taller viewport
    /// can leave the offset past the new maximum.
    pub fn set_height(&mut self, height: usize) {
        self.height = height;
        self.clamp();
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Every wrapped row in the document.
    #[must_use]
    pub fn wrapped(&self) -> &[WrappedLine] {
        &self.wrapped
    }

    /// Total wrapped rows — the scrollbar's denominator.
    #[must_use]
    pub fn total_rows(&self) -> usize {
        self.wrapped.len()
    }

    /// First visible wrapped row.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The rows a renderer should draw right now.
    #[must_use]
    pub fn visible(&self) -> &[WrappedLine] {
        let end = self.offset.saturating_add(self.height).min(self.wrapped.len());
        &self.wrapped[self.offset.min(self.wrapped.len())..end]
    }

    /// Largest valid offset. Zero when the document fits — a document
    /// shorter than its viewport must not scroll at all.
    #[must_use]
    pub fn max_offset(&self) -> usize {
        self.wrapped.len().saturating_sub(self.height)
    }

    pub fn scroll_to(&mut self, offset: usize) {
        self.offset = offset;
        self.clamp();
    }

    pub fn scroll_by(&mut self, delta: isize) {
        let next = if delta >= 0 {
            self.offset.saturating_add(delta.unsigned_abs())
        } else {
            self.offset.saturating_sub(delta.unsigned_abs())
        };
        self.scroll_to(next);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_by(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll_by(-1);
    }

    /// Page down by one viewport, less nothing — a full-page jump with no
    /// overlap loses the reader's place, so callers usually want this minus
    /// a line; that is their choice, not a hidden one here.
    pub fn page_down(&mut self) {
        let h = self.height.max(1);
        self.scroll_by(h as isize);
    }

    pub fn page_up(&mut self) {
        let h = self.height.max(1);
        self.scroll_by(-(h as isize));
    }

    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.max_offset();
    }

    #[must_use]
    pub fn is_at_top(&self) -> bool {
        self.offset == 0
    }

    #[must_use]
    pub fn is_at_bottom(&self) -> bool {
        self.offset >= self.max_offset()
    }

    /// Fraction scrolled, `0.0..=1.0`. Zero when the document fits, so a
    /// scrollbar reads "nothing to scroll" rather than dividing by zero.
    #[must_use]
    pub fn scroll_fraction(&self) -> f32 {
        let max = self.max_offset();
        if max == 0 {
            0.0
        } else {
            self.offset as f32 / max as f32
        }
    }

    /// Scroll so that a logical (pre-wrap) line is visible. Used for
    /// "jump to line N" and for anchoring a comment.
    pub fn scroll_to_source_line(&mut self, line: usize) {
        if let Some(i) = self.wrapped.iter().position(|w| w.source_line == line) {
            self.scroll_to(i);
        }
    }

    fn clamp(&mut self) {
        self.offset = self.offset.min(self.max_offset());
    }

    fn rewrap(&mut self) {
        self.wrapped = Vec::new();
        for (idx, spans) in self.lines.iter().enumerate() {
            wrap_spans(spans, self.width, idx, &mut self.wrapped);
        }
        self.clamp();
    }
}

/// Wrap one logical line's spans into visual rows at `width` graphemes.
///
/// Width 0 means "unknown viewport" — emit the line unwrapped rather than
/// looping forever or emitting nothing. A renderer that has not been told its
/// size yet should show something.
fn wrap_spans(spans: &[Span], width: usize, source_line: usize, out: &mut Vec<WrappedLine>) {
    if width == 0 {
        out.push(WrappedLine { spans: spans.to_vec(), source_line });
        return;
    }

    let mut current: Vec<Span> = Vec::new();
    let mut used = 0usize;

    for span in spans {
        // Walk graphemes so a wrap never lands inside one — the same
        // correctness reason TextArea indexes by grapheme.
        let mut chunk = String::new();
        for g in span.text.graphemes(true) {
            if used == width {
                if !chunk.is_empty() {
                    current.push(Span::new(std::mem::take(&mut chunk), span.style));
                }
                out.push(WrappedLine { spans: std::mem::take(&mut current), source_line });
                used = 0;
            }
            chunk.push_str(g);
            used += 1;
        }
        if !chunk.is_empty() {
            current.push(Span::new(chunk, span.style));
        }
    }

    // An empty logical line still occupies one visual row — dropping it
    // would silently close the paragraph gaps in a PR body.
    if !current.is_empty() || out.last().map(|w| w.source_line) != Some(source_line) {
        out.push(WrappedLine { spans: current, source_line });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_the_viewport_width() {
        let v = TextView::from_text("abcdefghij", 4, 10);
        assert_eq!(v.total_rows(), 3);
        assert_eq!(v.wrapped()[0].text(), "abcd");
        assert_eq!(v.wrapped()[2].text(), "ij");
    }

    /// Caching the wrap and forgetting to invalidate on resize is how a
    /// document renders with the old wrap under a fresh scrollbar.
    #[test]
    fn resize_rewraps_rather_than_serving_a_stale_layout() {
        let mut v = TextView::from_text("abcdefghij", 4, 10);
        assert_eq!(v.total_rows(), 3);
        v.set_width(10);
        assert_eq!(v.total_rows(), 1);
        v.set_width(2);
        assert_eq!(v.total_rows(), 5);
    }

    /// An empty logical line occupies a visual row — otherwise the paragraph
    /// gaps in a PR body silently close up.
    #[test]
    fn blank_lines_keep_their_row() {
        let v = TextView::from_text("a\n\nb", 10, 10);
        assert_eq!(v.total_rows(), 3);
        assert_eq!(v.wrapped()[1].text(), "");
    }

    /// Both cases in one: a line that wraps to several rows, and one that
    /// fits in a single row. Every visual row must name the logical line it
    /// came from, or "jump to line N" and comment anchoring land in the
    /// wrong place.
    #[test]
    fn every_wrapped_row_maps_back_to_its_source_line() {
        //  line 0: "abcdefghij" at width 4 → abcd / efgh / ij   (3 rows)
        //  line 1: "hi"         at width 4 → hi                 (1 row)
        let v = TextView::from_text("abcdefghij\nhi", 4, 10);
        let srcs: Vec<usize> = v.wrapped().iter().map(WrappedLine::source_line).collect();
        assert_eq!(srcs, vec![0, 0, 0, 1]);

        //  and a two-row second line maps both of its rows back to line 1
        let v2 = TextView::from_text("abcdefghij\nshort", 4, 10);
        let srcs2: Vec<usize> = v2.wrapped().iter().map(WrappedLine::source_line).collect();
        assert_eq!(srcs2, vec![0, 0, 0, 1, 1], "\"short\" is 5 wide — it wraps too");
    }

    /// Wrapping splits a span at the wrap point and BOTH halves keep the
    /// style — losing it on the continuation is the classic wrap bug.
    #[test]
    fn wrapping_preserves_style_across_the_split() {
        let mut v = TextView::new();
        v.set_width(3);
        v.set_height(10);
        v.set_lines(vec![vec![Span::new("abcdef", 7)]]);
        assert_eq!(v.total_rows(), 2);
        assert_eq!(v.wrapped()[0].spans()[0].style(), 7);
        assert_eq!(v.wrapped()[1].spans()[0].style(), 7);
        assert_eq!(v.wrapped()[1].spans()[0].text(), "def");
    }

    #[test]
    fn multiple_spans_on_one_row_keep_their_own_styles() {
        let mut v = TextView::new();
        v.set_width(10);
        v.set_height(10);
        v.set_lines(vec![vec![Span::new("ab", 1), Span::new("cd", 2)]]);
        assert_eq!(v.total_rows(), 1);
        let s = v.wrapped()[0].spans();
        assert_eq!((s[0].text(), s[0].style()), ("ab", 1));
        assert_eq!((s[1].text(), s[1].style()), ("cd", 2));
    }

    /// A wrap must never land inside a grapheme.
    #[test]
    fn wrapping_counts_graphemes_not_bytes() {
        // Three graphemes, each multi-byte.
        let v = TextView::from_text("e\u{0301}e\u{0301}e\u{0301}", 2, 10);
        assert_eq!(v.total_rows(), 2);
        assert_eq!(v.wrapped()[0].width(), 2);
        assert_eq!(v.wrapped()[1].width(), 1);
    }

    /// A document shorter than its viewport must not scroll at all.
    #[test]
    fn a_short_document_does_not_scroll() {
        let mut v = TextView::from_text("a\nb", 10, 10);
        assert_eq!(v.max_offset(), 0);
        v.scroll_down();
        assert_eq!(v.offset(), 0);
        assert!(v.is_at_top() && v.is_at_bottom());
        assert_eq!(v.scroll_fraction(), 0.0, "no division by zero");
    }

    #[test]
    fn scrolling_clamps_at_both_ends() {
        let mut v = TextView::from_text("1\n2\n3\n4\n5\n6", 10, 2);
        assert_eq!(v.max_offset(), 4);
        for _ in 0..50 {
            v.scroll_down();
        }
        assert_eq!(v.offset(), 4);
        assert!(v.is_at_bottom());
        for _ in 0..50 {
            v.scroll_up();
        }
        assert_eq!(v.offset(), 0);
    }

    #[test]
    fn visible_returns_exactly_the_viewport() {
        let mut v = TextView::from_text("1\n2\n3\n4\n5", 10, 2);
        assert_eq!(v.visible().len(), 2);
        assert_eq!(v.visible()[0].text(), "1");
        v.scroll_to(3);
        assert_eq!(v.visible()[0].text(), "4");
        assert_eq!(v.visible().len(), 2);
    }

    /// Growing the viewport can leave the offset past the new maximum.
    #[test]
    fn growing_the_viewport_reclamps_the_offset() {
        let mut v = TextView::from_text("1\n2\n3\n4\n5\n6", 10, 2);
        v.scroll_to_bottom();
        assert_eq!(v.offset(), 4);
        v.set_height(6);
        assert_eq!(v.offset(), 0, "whole document fits — nothing to scroll");
    }

    #[test]
    fn paging_moves_by_a_viewport() {
        let mut v = TextView::from_text("1\n2\n3\n4\n5\n6\n7\n8", 10, 3);
        v.page_down();
        assert_eq!(v.offset(), 3);
        v.page_up();
        assert_eq!(v.offset(), 0);
    }

    #[test]
    fn scroll_to_source_line_finds_the_first_visual_row() {
        let mut v = TextView::from_text("aaaaaa\nbbbbbb\ncccccc", 3, 2);
        // each logical line wraps to 2 rows
        v.scroll_to_source_line(2);
        assert_eq!(v.visible()[0].text(), "ccc");
    }

    /// Width 0 means "not sized yet" — show something rather than nothing,
    /// and above all do not loop.
    #[test]
    fn zero_width_emits_unwrapped_rows_and_terminates() {
        let v = TextView::from_text("abcdef\nghi", 0, 5);
        assert_eq!(v.total_rows(), 2);
        assert_eq!(v.wrapped()[0].text(), "abcdef");
    }

    #[test]
    fn empty_document_is_safe() {
        let v = TextView::new();
        assert_eq!(v.total_rows(), 0);
        assert_eq!(v.visible().len(), 0);
        assert_eq!(v.max_offset(), 0);
        assert_eq!(v.scroll_fraction(), 0.0);
    }

    #[test]
    fn crlf_normalizes_like_textarea() {
        assert_eq!(TextView::from_text("a\r\nb", 10, 10).total_rows(), 2);
    }

    #[test]
    fn scroll_fraction_spans_zero_to_one() {
        let mut v = TextView::from_text("1\n2\n3\n4\n5", 10, 1);
        assert_eq!(v.scroll_fraction(), 0.0);
        v.scroll_to_bottom();
        assert!((v.scroll_fraction() - 1.0).abs() < f32::EPSILON);
    }
}

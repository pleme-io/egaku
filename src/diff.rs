//! Unified-diff viewer state — hunks, collapse, cursor, unified ⟷ side-by-side.
//!
//! Pure logic, no renderer: the same value drives a TTY pane through
//! egaku-term and a GPU pane through garasu. That is the whole reason this
//! lands in egaku rather than in the app that wanted it first (P2 — *"new
//! widget logic lands in egaku, never in an app, never in ratatui"*).
//!
//! # What this owns, and what it does not
//!
//! It owns **structure and navigation**: parsing a unified diff into files and
//! hunks, flattening those into the row sequence a renderer walks, moving a
//! cursor through it, and collapsing a hunk or a whole file.
//!
//! It does **not** own colour. A renderer maps [`LineKind`] to a palette
//! entry, which is what lets the same view honour `NO_COLOR` — a red/green
//! diff that cannot degrade is unusable for a red/green-colourblind operator,
//! and colour carried inside the model would make that a rewrite rather than
//! a palette swap.
//!
//! # Line numbers are computed at parse time, once
//!
//! Old/new line numbers are assigned while walking a hunk, not derived at
//! render time from a row index. A renderer that recomputed them would have to
//! know that a removed line advances only the old counter and an added line
//! only the new one — the exact off-by-one every hand-rolled diff view ships.

/// What a single diff row *is*. A renderer maps this to a colour; the model
/// never carries one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Unchanged, present on both sides.
    Context,
    /// Present only in the new file.
    Added,
    /// Present only in the old file.
    Removed,
    /// `\ No newline at end of file` — metadata, belongs to neither side.
    NoNewline,
}

/// One row inside a hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    kind: LineKind,
    old_lineno: Option<u32>,
    new_lineno: Option<u32>,
    text: String,
}

impl DiffLine {
    #[must_use]
    pub fn kind(&self) -> LineKind {
        self.kind
    }
    /// Line number on the OLD side, or `None` for an added line.
    #[must_use]
    pub fn old_lineno(&self) -> Option<u32> {
        self.old_lineno
    }
    /// Line number on the NEW side, or `None` for a removed line.
    #[must_use]
    pub fn new_lineno(&self) -> Option<u32> {
        self.new_lineno
    }
    /// The line's content, with the leading `+`/`-`/space marker stripped.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// One `@@ … @@` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    header: String,
    old_start: u32,
    new_start: u32,
    lines: Vec<DiffLine>,
    collapsed: bool,
}

impl Hunk {
    /// The raw `@@ -a,b +c,d @@ trailing` header line.
    #[must_use]
    pub fn header(&self) -> &str {
        &self.header
    }
    #[must_use]
    pub fn old_start(&self) -> u32 {
        self.old_start
    }
    #[must_use]
    pub fn new_start(&self) -> u32 {
        self.new_start
    }
    #[must_use]
    pub fn lines(&self) -> &[DiffLine] {
        &self.lines
    }
    #[must_use]
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }
    /// `(added, removed)` for this hunk.
    #[must_use]
    pub fn stats(&self) -> (usize, usize) {
        let a = self.lines.iter().filter(|l| l.kind == LineKind::Added).count();
        let r = self.lines.iter().filter(|l| l.kind == LineKind::Removed).count();
        (a, r)
    }
}

/// All hunks for one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    path: String,
    old_path: Option<String>,
    hunks: Vec<Hunk>,
    collapsed: bool,
    binary: bool,
}

impl FileDiff {
    /// Path in the NEW tree. For a delete, the old path — a file always has
    /// a name to show.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Set only when the path differs across the rename, so a renderer can
    /// show `old → new` without diffing the two strings itself.
    #[must_use]
    pub fn old_path(&self) -> Option<&str> {
        self.old_path.as_deref()
    }
    #[must_use]
    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }
    #[must_use]
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }
    /// A binary file has no hunks and must not render as "no changes".
    #[must_use]
    pub fn is_binary(&self) -> bool {
        self.binary
    }
    /// `(added, removed)` across every hunk.
    #[must_use]
    pub fn stats(&self) -> (usize, usize) {
        self.hunks.iter().fold((0, 0), |(a, r), h| {
            let (ha, hr) = h.stats();
            (a + ha, r + hr)
        })
    }
}

/// How the caller wants the diff laid out. The model is identical either way —
/// only [`Row::SideBySide`] pairing differs — so toggling is not a reparse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    /// One column, `+`/`-` interleaved.
    #[default]
    Unified,
    /// Two columns, removed on the left and added on the right, blank-padded
    /// where one side has no counterpart.
    SideBySide,
}

/// One renderable row. This is the flattened sequence a renderer walks; the
/// cursor indexes into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row<'a> {
    /// A file's header row. Always emitted, even when collapsed — collapsing
    /// a file must not make it vanish.
    File(&'a FileDiff),
    /// A hunk's `@@` header. Always emitted when its file is expanded.
    HunkHeader { file: &'a FileDiff, hunk: &'a Hunk },
    /// A content line, unified layout.
    Line { file: &'a FileDiff, hunk: &'a Hunk, line: &'a DiffLine },
    /// A content line, side-by-side. Either side may be absent.
    SideBySide {
        file: &'a FileDiff,
        hunk: &'a Hunk,
        left: Option<&'a DiffLine>,
        right: Option<&'a DiffLine>,
    },
}

/// A parsed unified diff plus a cursor over its visible rows.
#[derive(Debug, Clone, Default)]
pub struct DiffView {
    files: Vec<FileDiff>,
    cursor: usize,
    mode: DiffMode,
}

impl DiffView {
    /// Parse a unified diff (`git diff` / `gh pr diff` output).
    ///
    /// Lenient by construction: anything it does not recognise is skipped
    /// rather than rejected. A review UI that refuses to show a diff because
    /// one preamble line was unexpected is worse than one that shows the
    /// hunks it understood — and `git` emits several optional headers
    /// (`index`, `similarity index`, `old mode`) that carry no rows.
    #[must_use]
    pub fn parse(diff: &str) -> Self {
        let mut files: Vec<FileDiff> = Vec::new();
        let mut old_ln: u32 = 0;
        let mut new_ln: u32 = 0;

        for raw in diff.lines() {
            // `diff --git a/x b/y` opens a file. Prefer the +++ header for the
            // real path (it survives quoting better), so record a placeholder.
            if let Some(rest) = raw.strip_prefix("diff --git ") {
                files.push(FileDiff {
                    path: git_header_path(rest),
                    old_path: None,
                    hunks: Vec::new(),
                    collapsed: false,
                    binary: false,
                });
                continue;
            }
            if raw.starts_with("Binary files ") || raw.starts_with("GIT binary patch") {
                if let Some(f) = files.last_mut() {
                    f.binary = true;
                }
                continue;
            }
            if let Some(p) = raw.strip_prefix("--- ") {
                // Plain `diff -u` output has no `diff --git` line, so `---`
                // is the FIRST thing that opens a file. Skipping when none is
                // open silently discarded old_path for every non-git diff —
                // renames and deletes both lost their name.
                if files.is_empty() {
                    files.push(FileDiff {
                        path: String::new(),
                        old_path: None,
                        hunks: Vec::new(),
                        collapsed: false,
                        binary: false,
                    });
                }
                if let Some(f) = files.last_mut() {
                    let p = strip_ab_prefix(p);
                    if p != "/dev/null" {
                        f.old_path = Some(p.to_owned());
                    }
                }
                continue;
            }
            if let Some(p) = raw.strip_prefix("+++ ") {
                // A bare `--- / +++` pair with no `diff --git` (plain
                // `diff -u` output) still has to open a file.
                let p = strip_ab_prefix(p);
                if files.is_empty() {
                    files.push(FileDiff {
                        path: String::new(),
                        old_path: None,
                        hunks: Vec::new(),
                        collapsed: false,
                        binary: false,
                    });
                }
                if let Some(f) = files.last_mut() {
                    if p != "/dev/null" {
                        f.path = p.to_owned();
                    } else if let Some(old) = f.old_path.clone() {
                        // A delete: /dev/null on the new side. Name it by the
                        // path it HAD, never leave it blank.
                        f.path = old;
                    }
                    // Only a real rename keeps old_path — otherwise every
                    // file would render an `old → new` arrow to itself.
                    if f.old_path.as_deref() == Some(f.path.as_str()) {
                        f.old_path = None;
                    }
                }
                continue;
            }
            if raw.starts_with("@@") {
                let (os, ns) = parse_hunk_header(raw);
                old_ln = os;
                new_ln = ns;
                if let Some(f) = files.last_mut() {
                    f.hunks.push(Hunk {
                        header: raw.to_owned(),
                        old_start: os,
                        new_start: ns,
                        lines: Vec::new(),
                        collapsed: false,
                    });
                }
                continue;
            }

            // Content lines only count once a hunk is open — a `+++` in a
            // commit message preamble must not become an added line.
            let Some(hunk) = files.last_mut().and_then(|f| f.hunks.last_mut()) else {
                continue;
            };
            let (kind, text) = match raw.as_bytes().first() {
                Some(b'+') => (LineKind::Added, &raw[1..]),
                Some(b'-') => (LineKind::Removed, &raw[1..]),
                Some(b'\\') => (LineKind::NoNewline, raw),
                Some(b' ') => (LineKind::Context, &raw[1..]),
                // A totally empty line inside a hunk is a context line whose
                // single space git elided. Treating it as unknown would drop
                // real content.
                None => (LineKind::Context, raw),
                _ => continue,
            };
            let (o, n) = match kind {
                LineKind::Added => {
                    let n = new_ln;
                    new_ln += 1;
                    (None, Some(n))
                }
                LineKind::Removed => {
                    let o = old_ln;
                    old_ln += 1;
                    (Some(o), None)
                }
                LineKind::Context => {
                    let (o, n) = (old_ln, new_ln);
                    old_ln += 1;
                    new_ln += 1;
                    (Some(o), Some(n))
                }
                LineKind::NoNewline => (None, None),
            };
            hunk.lines.push(DiffLine {
                kind,
                old_lineno: o,
                new_lineno: n,
                text: text.to_owned(),
            });
        }

        Self { files, cursor: 0, mode: DiffMode::Unified }
    }

    #[must_use]
    pub fn files(&self) -> &[FileDiff] {
        &self.files
    }

    #[must_use]
    pub fn mode(&self) -> DiffMode {
        self.mode
    }

    /// Switching layout preserves the cursor's *row index*, which is stable
    /// because side-by-side only ever merges paired add/remove rows — it
    /// never reorders files or hunks.
    pub fn set_mode(&mut self, mode: DiffMode) {
        self.mode = mode;
        self.clamp();
    }

    pub fn toggle_mode(&mut self) {
        self.set_mode(match self.mode {
            DiffMode::Unified => DiffMode::SideBySide,
            DiffMode::SideBySide => DiffMode::Unified,
        });
    }

    /// `(added, removed)` across the whole diff.
    #[must_use]
    pub fn stats(&self) -> (usize, usize) {
        self.files.iter().fold((0, 0), |(a, r), f| {
            let (fa, fr) = f.stats();
            (a + fa, r + fr)
        })
    }

    /// The flattened row sequence, honouring collapse and mode. This is what
    /// a renderer walks and what [`Self::cursor`] indexes.
    #[must_use]
    pub fn rows(&self) -> Vec<Row<'_>> {
        let mut out = Vec::new();
        for f in &self.files {
            out.push(Row::File(f));
            if f.collapsed {
                continue;
            }
            for h in &f.hunks {
                out.push(Row::HunkHeader { file: f, hunk: h });
                if h.collapsed {
                    continue;
                }
                match self.mode {
                    DiffMode::Unified => {
                        for line in &h.lines {
                            out.push(Row::Line { file: f, hunk: h, line });
                        }
                    }
                    DiffMode::SideBySide => pair_rows(f, h, &mut out),
                }
            }
        }
        out
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows().len()
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, i: usize) {
        self.cursor = i;
        self.clamp();
    }

    pub fn move_down(&mut self) {
        self.cursor = self.cursor.saturating_add(1);
        self.clamp();
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Jump to the next `@@` header. Returns false when there is none below —
    /// callers use that to decide whether to advance to the next file.
    pub fn next_hunk(&mut self) -> bool {
        self.jump(|r| matches!(r, Row::HunkHeader { .. }), true)
    }

    pub fn prev_hunk(&mut self) -> bool {
        self.jump(|r| matches!(r, Row::HunkHeader { .. }), false)
    }

    pub fn next_file(&mut self) -> bool {
        self.jump(|r| matches!(r, Row::File(_)), true)
    }

    pub fn prev_file(&mut self) -> bool {
        self.jump(|r| matches!(r, Row::File(_)), false)
    }

    fn jump(&mut self, pred: impl Fn(&Row<'_>) -> bool, forward: bool) -> bool {
        let rows = self.rows();
        let found = if forward {
            rows.iter().enumerate().skip(self.cursor + 1).find(|(_, r)| pred(r)).map(|(i, _)| i)
        } else {
            rows.iter().enumerate().take(self.cursor).filter(|(_, r)| pred(r)).next_back().map(|(i, _)| i)
        };
        match found {
            Some(i) => {
                self.cursor = i;
                true
            }
            None => false,
        }
    }

    /// Collapse/expand whatever the cursor is on: a file row toggles the
    /// file, any row inside a hunk toggles that hunk.
    ///
    /// The cursor is re-clamped afterwards because collapsing removes rows
    /// beneath it — leaving it past the end is how a diff view starts
    /// rendering blank after a keystroke.
    pub fn toggle_collapse(&mut self) {
        let Some((fi, hi)) = self.locate(self.cursor) else {
            return;
        };
        match hi {
            None => {
                let f = &mut self.files[fi];
                f.collapsed = !f.collapsed;
            }
            Some(hi) => {
                let h = &mut self.files[fi].hunks[hi];
                h.collapsed = !h.collapsed;
            }
        }
        self.clamp();
    }

    pub fn collapse_all_files(&mut self) {
        for f in &mut self.files {
            f.collapsed = true;
        }
        self.clamp();
    }

    pub fn expand_all(&mut self) {
        for f in &mut self.files {
            f.collapsed = false;
            for h in &mut f.hunks {
                h.collapsed = false;
            }
        }
        self.clamp();
    }

    /// `(file index, hunk index)` for a row — `None` hunk means the row IS
    /// the file header.
    #[must_use]
    fn locate(&self, row: usize) -> Option<(usize, Option<usize>)> {
        let rows = self.rows();
        let r = rows.get(row)?;
        let (target_file, target_hunk) = match r {
            Row::File(f) => (f.path.as_str(), None),
            Row::HunkHeader { file, hunk } => (file.path.as_str(), Some(hunk.header.as_str())),
            Row::Line { file, hunk, .. } => (file.path.as_str(), Some(hunk.header.as_str())),
            Row::SideBySide { file, hunk, .. } => {
                (file.path.as_str(), Some(hunk.header.as_str()))
            }
        };
        let fi = self.files.iter().position(|f| f.path == target_file)?;
        let hi = target_hunk
            .and_then(|hh| self.files[fi].hunks.iter().position(|h| h.header == hh));
        Some((fi, hi))
    }

    fn clamp(&mut self) {
        let n = self.rows().len();
        if n == 0 {
            self.cursor = 0;
        } else if self.cursor >= n {
            self.cursor = n - 1;
        }
    }
}

/// Pair removed/added runs into side-by-side rows.
///
/// The pairing is per-RUN, not per-line: a hunk that removes 3 and adds 1
/// yields 3 rows, the first paired and the last two left-only. Zipping the
/// whole hunk instead would align an unrelated removal with an unrelated
/// addition and read as a modification that never happened.
fn pair_rows<'a>(f: &'a FileDiff, h: &'a Hunk, out: &mut Vec<Row<'a>>) {
    let mut i = 0;
    while i < h.lines.len() {
        match h.lines[i].kind {
            LineKind::Context | LineKind::NoNewline => {
                out.push(Row::SideBySide {
                    file: f,
                    hunk: h,
                    left: Some(&h.lines[i]),
                    right: Some(&h.lines[i]),
                });
                i += 1;
            }
            LineKind::Removed | LineKind::Added => {
                let start = i;
                while i < h.lines.len() && h.lines[i].kind == LineKind::Removed {
                    i += 1;
                }
                let removed = &h.lines[start..i];
                let add_start = i;
                while i < h.lines.len() && h.lines[i].kind == LineKind::Added {
                    i += 1;
                }
                let added = &h.lines[add_start..i];
                for k in 0..removed.len().max(added.len()) {
                    out.push(Row::SideBySide {
                        file: f,
                        hunk: h,
                        left: removed.get(k),
                        right: added.get(k),
                    });
                }
            }
        }
    }
}

/// `-1,4 +1,6 @@ fn foo()` → `(1, 1)`. Malformed headers yield `(0, 0)`
/// rather than refusing the hunk — see [`DiffView::parse`] on leniency.
fn parse_hunk_header(h: &str) -> (u32, u32) {
    let mut old = 0;
    let mut new = 0;
    for tok in h.split_whitespace() {
        if let Some(t) = tok.strip_prefix('-') {
            old = leading_u32(t);
        } else if let Some(t) = tok.strip_prefix('+') {
            new = leading_u32(t);
        }
    }
    (old, new)
}

fn leading_u32(s: &str) -> u32 {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(0)
}

/// `a/src/x.rs` → `src/x.rs`. Only strips the conventional one-letter dir,
/// so a real top-level `a/` directory survives when git did not add a prefix.
fn strip_ab_prefix(p: &str) -> &str {
    let p = p.split('\t').next().unwrap_or(p).trim_end();
    p.strip_prefix("a/").or_else(|| p.strip_prefix("b/")).unwrap_or(p)
}

/// `a/x b/x` → `x`. Uses the SECOND half (the new path).
fn git_header_path(rest: &str) -> String {
    let mut parts = rest.split_whitespace();
    let _old = parts.next();
    parts.next().map_or_else(String::new, |b| strip_ab_prefix(b).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -10,6 +10,7 @@ fn existing()
 context one
-removed one
+added one
+added two
 context two
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,2 +1,2 @@
-old title
+new title
 body
";

    #[test]
    fn parses_files_hunks_and_stats() {
        let d = DiffView::parse(DIFF);
        assert_eq!(d.files().len(), 2);
        assert_eq!(d.files()[0].path(), "src/lib.rs");
        assert_eq!(d.files()[1].path(), "README.md");
        assert_eq!(d.files()[0].hunks().len(), 1);
        // 3 added / 2 removed across both files.
        assert_eq!(d.stats(), (3, 2));
    }

    /// The off-by-one every hand-rolled diff view ships: a removed line
    /// advances ONLY the old counter, an added line ONLY the new one.
    #[test]
    fn line_numbers_track_each_side_independently() {
        let d = DiffView::parse(DIFF);
        let lines = d.files()[0].hunks()[0].lines();
        // @@ -10,6 +10,7 @@
        assert_eq!((lines[0].old_lineno(), lines[0].new_lineno()), (Some(10), Some(10)));
        // removed: old advances, new does not
        assert_eq!((lines[1].old_lineno(), lines[1].new_lineno()), (Some(11), None));
        // added ×2: new advances, old does not
        assert_eq!((lines[2].old_lineno(), lines[2].new_lineno()), (None, Some(11)));
        assert_eq!((lines[3].old_lineno(), lines[3].new_lineno()), (None, Some(12)));
        // context resumes on both, each having advanced by its own count
        assert_eq!((lines[4].old_lineno(), lines[4].new_lineno()), (Some(12), Some(13)));
    }

    #[test]
    fn markers_are_stripped_from_text() {
        let d = DiffView::parse(DIFF);
        let lines = d.files()[0].hunks()[0].lines();
        assert_eq!(lines[0].text(), "context one");
        assert_eq!(lines[1].text(), "removed one");
        assert_eq!(lines[2].text(), "added one");
    }

    #[test]
    fn rows_include_file_and_hunk_headers() {
        let d = DiffView::parse(DIFF);
        let rows = d.rows();
        assert!(matches!(rows[0], Row::File(_)));
        assert!(matches!(rows[1], Row::HunkHeader { .. }));
        assert!(matches!(rows[2], Row::Line { .. }));
        // 2 file rows + 2 hunk headers + 5 content lines + 3 content lines
        assert_eq!(rows.len(), 2 + 2 + 5 + 3);
    }

    /// Collapsing a file must not make it disappear — its header row stays.
    #[test]
    fn collapsing_a_file_keeps_its_header_row() {
        let mut d = DiffView::parse(DIFF);
        d.set_cursor(0);
        d.toggle_collapse();
        let rows = d.rows();
        assert!(matches!(rows[0], Row::File(_)));
        // file 1 collapsed → only its header; file 2 intact (header+hunk+3)
        assert_eq!(rows.len(), 1 + 1 + 1 + 3);
    }

    /// Collapsing removes rows beneath the cursor; leaving it past the end is
    /// how a diff view starts rendering blank after a keystroke.
    #[test]
    fn collapse_reclamps_a_cursor_that_would_dangle() {
        let mut d = DiffView::parse(DIFF);
        let last = d.row_count() - 1;
        d.set_cursor(last);
        d.collapse_all_files();
        assert!(d.cursor() < d.row_count(), "cursor {} vs {}", d.cursor(), d.row_count());
    }

    #[test]
    fn hunk_navigation_moves_between_headers_and_reports_the_end() {
        let mut d = DiffView::parse(DIFF);
        assert!(d.next_hunk());
        assert!(matches!(d.rows()[d.cursor()], Row::HunkHeader { .. }));
        assert!(d.next_hunk()); // second file's hunk
        assert!(!d.next_hunk(), "no hunk below the last one");
        assert!(d.prev_hunk());
    }

    /// Per-RUN pairing: 3 removed + 1 added is 3 rows, not 4 and not 1.
    /// Zipping the whole hunk would align unrelated lines and read as a
    /// modification that never happened.
    #[test]
    fn side_by_side_pairs_runs_not_whole_hunks() {
        let d = DiffView::parse(
            "\
--- a/x
+++ b/x
@@ -1,4 +1,2 @@
-a
-b
-c
+z
",
        );
        let mut d = d;
        d.set_mode(DiffMode::SideBySide);
        let rows: Vec<_> = d
            .rows()
            .into_iter()
            .filter(|r| matches!(r, Row::SideBySide { .. }))
            .collect();
        assert_eq!(rows.len(), 3, "max(removed, added) = 3");
        match &rows[0] {
            Row::SideBySide { left, right, .. } => {
                assert_eq!(left.unwrap().text(), "a");
                assert_eq!(right.unwrap().text(), "z");
            }
            _ => panic!(),
        }
        match &rows[2] {
            Row::SideBySide { left, right, .. } => {
                assert_eq!(left.unwrap().text(), "c");
                assert!(right.is_none(), "no counterpart to pad against");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn mode_toggle_is_not_a_reparse() {
        let mut d = DiffView::parse(DIFF);
        let before = d.files().to_vec();
        d.toggle_mode();
        assert_eq!(d.mode(), DiffMode::SideBySide);
        assert_eq!(d.files(), before.as_slice(), "structure is layout-independent");
        d.toggle_mode();
        assert_eq!(d.mode(), DiffMode::Unified);
    }

    /// A rename shows `old → new`; a plain edit must NOT render an arrow to
    /// itself.
    #[test]
    fn old_path_is_set_only_for_a_real_rename() {
        let renamed = DiffView::parse("--- a/old.rs\n+++ b/new.rs\n@@ -1 +1 @@\n-x\n+y\n");
        assert_eq!(renamed.files()[0].path(), "new.rs");
        assert_eq!(renamed.files()[0].old_path(), Some("old.rs"));

        let edited = DiffView::parse("--- a/same.rs\n+++ b/same.rs\n@@ -1 +1 @@\n-x\n+y\n");
        assert_eq!(edited.files()[0].old_path(), None);
    }

    /// A delete has `/dev/null` on the new side — it must still be named.
    #[test]
    fn a_deleted_file_is_named_by_the_path_it_had() {
        let d = DiffView::parse("--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n");
        assert_eq!(d.files()[0].path(), "gone.rs");
    }

    /// A binary file has no hunks and must not read as "no changes".
    #[test]
    fn binary_files_are_flagged_not_silently_empty() {
        let d = DiffView::parse(
            "diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n",
        );
        assert!(d.files()[0].is_binary());
        assert!(d.files()[0].hunks().is_empty());
    }

    /// Preamble text outside any hunk must not become diff content — a commit
    /// message mentioning `+1` is not an added line.
    #[test]
    fn preamble_outside_a_hunk_is_not_content() {
        let d = DiffView::parse("commit abc\n+not a real addition\ndiff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n+real\n");
        assert_eq!(d.files().len(), 1);
        assert_eq!(d.stats(), (1, 0));
    }

    #[test]
    fn empty_and_garbage_input_are_safe() {
        assert_eq!(DiffView::parse("").row_count(), 0);
        assert_eq!(DiffView::parse("").cursor(), 0);
        let g = DiffView::parse("this is not a diff at all\nnor is this\n");
        assert_eq!(g.files().len(), 0);
        assert_eq!(g.stats(), (0, 0));
    }

    /// A malformed `@@` must not refuse the hunk — leniency is the documented
    /// contract, and a review UI that blanks on one bad header is worse than
    /// one showing rows numbered from 0.
    #[test]
    fn malformed_hunk_header_still_yields_rows() {
        let d = DiffView::parse("--- a/x\n+++ b/x\n@@ garbage @@\n+added\n");
        assert_eq!(d.files()[0].hunks().len(), 1);
        assert_eq!(d.stats(), (1, 0));
    }

    #[test]
    fn no_newline_marker_belongs_to_neither_side() {
        let d = DiffView::parse("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n\\ No newline at end of file\n");
        let lines = d.files()[0].hunks()[0].lines();
        let last = lines.last().unwrap();
        assert_eq!(last.kind(), LineKind::NoNewline);
        assert_eq!((last.old_lineno(), last.new_lineno()), (None, None));
    }

    #[test]
    fn cursor_never_leaves_the_row_range() {
        let mut d = DiffView::parse(DIFF);
        for _ in 0..1000 {
            d.move_down();
        }
        assert_eq!(d.cursor(), d.row_count() - 1);
        for _ in 0..1000 {
            d.move_up();
        }
        assert_eq!(d.cursor(), 0);
    }
}

//! [`Navigator`] — one vim navigation grammar for every list, menu and pane.
//!
//! Every fleet surface that shows rows (a browser, a help pane, a picker, a
//! dialog, a scrollback) needs the same keys to mean the same thing: `j`/`k`
//! move, `gg`/`G` jump, `5j` repeats, `Ctrl-d`/`Ctrl-u` move half a page, `/`
//! searches with `n`/`N`, `Enter`/`l` open, `h` goes back, `q` closes, `Esc`
//! backs out one layer at a time, `y` yanks. Written per surface, each copy
//! drifts; here it is one typed state machine that any surface drives with
//! its own rows.
//!
//! # Three layers
//!
//! - [`NavKeymap`] — chord rows (`"g g"`, `"ctrl+d"`, `"G"`) → [`NavAction`],
//!   as data. Two default sets: [`NavKeymap::menu`] (bare letters bind) and
//!   [`NavKeymap::filter`] (for a picker whose typed letters filter: only
//!   modified and named keys bind). Rows can be rebound or unbound.
//! - [`NavResolver`] — the chord + count state machine over a keymap. A
//!   surface that owns its own cursor (a `FuzzyPicker`) uses this alone and
//!   translates [`Feed::Command`] into its own events.
//! - [`Navigator`] — resolver + cursor + viewport + search. Drive it with
//!   [`Navigator::handle`] and the surface's row texts; act on the returned
//!   [`NavOutcome`].
//!
//! # Keys are strokes or chords
//!
//! [`NavKey::Char`] carries a printable character with its case, because vim's
//! alphabet is case-sensitive (`g` vs `G`, `n` vs `N`) and `awase` folds `?`
//! into `/` and `G` into `g`. [`NavKey::Chord`] carries everything else as an
//! `awase::Hotkey`. A surface converts its terminal event into one of the two:
//! an unmodified character is a `Char`, anything else a `Chord`.
//!
//! # Precedence
//!
//! A surface's own keys come first (an approval dialog's `1`/`2`/`3` and
//! `y`/`n`), then the navigator. Keys the navigator does not bind come back as
//! [`NavOutcome::Unhandled`] so the surface can type them, close, or pass
//! them on.

use std::fmt::Write as _;

use awase::Hotkey;

/// One keystroke as navigation reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavKey {
    /// An unmodified printable character, case preserved.
    Char(char),
    /// A named or modified key (`escape`, `ctrl+d`, `down`).
    Chord(Hotkey),
}

impl NavKey {
    /// Parse one key of a binding row. A single printable character other
    /// than space is a [`NavKey::Char`]; anything else goes through
    /// `awase::Hotkey::parse`.
    ///
    /// # Errors
    /// Names the key when awase cannot parse it.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next())
            && !c.is_whitespace()
            && !c.is_control()
        {
            return Ok(Self::Char(c));
        }
        Hotkey::parse(s).map(Self::Chord).map_err(|e| format!("key {s:?}: {e}"))
    }

    /// Shorthand for a chord key, panicking on a spelling awase rejects.
    /// For tests and constant tables; runtime input should use [`Self::parse`].
    #[must_use]
    pub fn chord(s: &str) -> Self {
        Self::Chord(Hotkey::parse(s).unwrap_or_else(|e| panic!("chord {s:?}: {e}")))
    }

    /// How a footer or help pane shows this key.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Char(c) => c.to_string(),
            Self::Chord(h) => {
                let d = h.display();
                match d.as_str() {
                    "escape" => "esc".to_owned(),
                    "return" => "enter".to_owned(),
                    _ => d,
                }
            }
        }
    }

    fn digit(self) -> Option<usize> {
        match self {
            Self::Char(c) => c.to_digit(10).map(|d| d as usize),
            Self::Chord(_) => None,
        }
    }
}

/// Everything a navigation key can mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavAction {
    /// One row down (`j`).
    Down,
    /// One row up (`k`).
    Up,
    /// First row (`gg`); with a count, that row.
    Top,
    /// Last row (`G`); with a count, that row.
    Bottom,
    /// Half a viewport down (`Ctrl-d`).
    HalfPageDown,
    /// Half a viewport up (`Ctrl-u`).
    HalfPageUp,
    /// A viewport down (`Ctrl-f`).
    PageDown,
    /// A viewport up (`Ctrl-b`).
    PageUp,
    /// First visible row (`H`).
    ViewTop,
    /// Middle visible row (`M`).
    ViewMiddle,
    /// Last visible row (`L`).
    ViewBottom,
    /// Start a search (`/`).
    Search,
    /// Next match (`n`).
    SearchNext,
    /// Previous match (`N`).
    SearchPrev,
    /// Open / accept the selected row (`Enter`, `l`).
    Open,
    /// Go up one level (`h`, `Backspace`).
    Back,
    /// Back out one layer: search, filter, count, then the surface (`Esc`).
    Dismiss,
    /// Close the surface from anywhere (`q`).
    Close,
    /// Copy the selected row's text (`y`).
    Yank,
}

impl NavAction {
    pub const ALL: [Self; 19] = [
        Self::Down,
        Self::Up,
        Self::Top,
        Self::Bottom,
        Self::HalfPageDown,
        Self::HalfPageUp,
        Self::PageDown,
        Self::PageUp,
        Self::ViewTop,
        Self::ViewMiddle,
        Self::ViewBottom,
        Self::Search,
        Self::SearchNext,
        Self::SearchPrev,
        Self::Open,
        Self::Back,
        Self::Dismiss,
        Self::Close,
        Self::Yank,
    ];

    /// The stable id a user keymap binds to (`nav:down`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Down => "nav:down",
            Self::Up => "nav:up",
            Self::Top => "nav:top",
            Self::Bottom => "nav:bottom",
            Self::HalfPageDown => "nav:halfPageDown",
            Self::HalfPageUp => "nav:halfPageUp",
            Self::PageDown => "nav:pageDown",
            Self::PageUp => "nav:pageUp",
            Self::ViewTop => "nav:viewTop",
            Self::ViewMiddle => "nav:viewMiddle",
            Self::ViewBottom => "nav:viewBottom",
            Self::Search => "nav:search",
            Self::SearchNext => "nav:searchNext",
            Self::SearchPrev => "nav:searchPrev",
            Self::Open => "nav:open",
            Self::Back => "nav:back",
            Self::Dismiss => "nav:dismiss",
            Self::Close => "nav:close",
            Self::Yank => "nav:yank",
        }
    }

    /// The action whose id is `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }

    /// The word a footer shows for this action.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Down | Self::Up => "move",
            Self::Top | Self::Bottom => "ends",
            Self::HalfPageDown | Self::HalfPageUp => "half page",
            Self::PageDown | Self::PageUp => "page",
            Self::ViewTop | Self::ViewMiddle | Self::ViewBottom => "screen",
            Self::Search => "search",
            Self::SearchNext | Self::SearchPrev => "next/prev",
            Self::Open => "open",
            Self::Back => "back",
            Self::Dismiss => "back out",
            Self::Close => "close",
            Self::Yank => "yank",
        }
    }

    /// The other half of a paired motion, shown together in a footer.
    const fn partner(self) -> Option<Self> {
        match self {
            Self::Down => Some(Self::Up),
            Self::Up => Some(Self::Down),
            Self::Top => Some(Self::Bottom),
            Self::Bottom => Some(Self::Top),
            Self::HalfPageDown => Some(Self::HalfPageUp),
            Self::HalfPageUp => Some(Self::HalfPageDown),
            Self::PageDown => Some(Self::PageUp),
            Self::PageUp => Some(Self::PageDown),
            Self::SearchNext => Some(Self::SearchPrev),
            Self::SearchPrev => Some(Self::SearchNext),
            _ => None,
        }
    }
}

/// The default chords for a menu: a list whose letters are free to bind.
pub const MENU_BINDINGS: &[(&str, NavAction)] = &[
    ("j", NavAction::Down),
    ("down", NavAction::Down),
    ("ctrl+n", NavAction::Down),
    ("ctrl+j", NavAction::Down),
    ("tab", NavAction::Down),
    ("k", NavAction::Up),
    ("up", NavAction::Up),
    ("ctrl+p", NavAction::Up),
    ("ctrl+k", NavAction::Up),
    ("shift+tab", NavAction::Up),
    ("g g", NavAction::Top),
    ("home", NavAction::Top),
    ("G", NavAction::Bottom),
    ("end", NavAction::Bottom),
    ("ctrl+d", NavAction::HalfPageDown),
    ("ctrl+u", NavAction::HalfPageUp),
    ("ctrl+f", NavAction::PageDown),
    ("pagedown", NavAction::PageDown),
    ("ctrl+b", NavAction::PageUp),
    ("pageup", NavAction::PageUp),
    ("H", NavAction::ViewTop),
    ("M", NavAction::ViewMiddle),
    ("L", NavAction::ViewBottom),
    ("/", NavAction::Search),
    ("n", NavAction::SearchNext),
    ("N", NavAction::SearchPrev),
    ("enter", NavAction::Open),
    ("l", NavAction::Open),
    ("right", NavAction::Open),
    ("h", NavAction::Back),
    ("backspace", NavAction::Back),
    ("left", NavAction::Back),
    ("escape", NavAction::Dismiss),
    ("q", NavAction::Close),
    ("ctrl+c", NavAction::Close),
    ("y", NavAction::Yank),
];

/// The default chords for a filtering picker: typed letters go to the
/// query, so only named and modified keys move.
pub const FILTER_BINDINGS: &[(&str, NavAction)] = &[
    ("down", NavAction::Down),
    ("ctrl+n", NavAction::Down),
    ("ctrl+j", NavAction::Down),
    ("tab", NavAction::Down),
    ("up", NavAction::Up),
    ("ctrl+p", NavAction::Up),
    ("ctrl+k", NavAction::Up),
    ("shift+tab", NavAction::Up),
    ("home", NavAction::Top),
    ("end", NavAction::Bottom),
    ("ctrl+d", NavAction::HalfPageDown),
    ("ctrl+u", NavAction::HalfPageUp),
    ("pagedown", NavAction::PageDown),
    ("pageup", NavAction::PageUp),
    ("enter", NavAction::Open),
    ("escape", NavAction::Dismiss),
    ("ctrl+c", NavAction::Dismiss),
];

/// Which default set a keymap starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavProfile {
    /// Bare letters bind; digits are counts.
    Menu,
    /// Letters and digits are typed; only named and modified keys bind.
    Filter,
}

/// Chord rows → actions, as data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavKeymap {
    bindings: Vec<(Vec<NavKey>, NavAction)>,
    counts: bool,
}

impl NavKeymap {
    /// Parse `(chord, action)` rows. Keys of a sequence are separated by
    /// whitespace (`"g g"`). `counts` makes digits a repeat count.
    ///
    /// # Errors
    /// Names the first row that does not parse.
    pub fn from_rows(rows: &[(&str, NavAction)], counts: bool) -> Result<Self, String> {
        let mut km = Self { bindings: Vec::with_capacity(rows.len()), counts };
        for (chord, action) in rows {
            km.bind(chord, *action)?;
        }
        Ok(km)
    }

    /// The default set for `profile`.
    #[must_use]
    pub fn for_profile(profile: NavProfile) -> Self {
        match profile {
            NavProfile::Menu => Self::from_rows(MENU_BINDINGS, true),
            NavProfile::Filter => Self::from_rows(FILTER_BINDINGS, false),
        }
        .expect("default navigation rows parse (tested)")
    }

    /// [`MENU_BINDINGS`] with counts.
    #[must_use]
    pub fn menu() -> Self {
        Self::for_profile(NavProfile::Menu)
    }

    /// [`FILTER_BINDINGS`] without counts.
    #[must_use]
    pub fn filter() -> Self {
        Self::for_profile(NavProfile::Filter)
    }

    fn parse_seq(chord: &str) -> Result<Vec<NavKey>, String> {
        let keys = chord.split_whitespace().map(NavKey::parse).collect::<Result<Vec<_>, _>>()?;
        if keys.is_empty() {
            return Err("empty chord".to_owned());
        }
        Ok(keys)
    }

    /// Bind `chord` to `action`, replacing whatever it meant before.
    ///
    /// # Errors
    /// When the chord does not parse.
    pub fn bind(&mut self, chord: &str, action: NavAction) -> Result<(), String> {
        let keys = Self::parse_seq(chord)?;
        self.bindings.retain(|(k, _)| *k != keys);
        self.bindings.push((keys, action));
        Ok(())
    }

    /// Remove `chord`. Returns whether it was bound.
    pub fn unbind(&mut self, chord: &str) -> bool {
        let Ok(keys) = Self::parse_seq(chord) else { return false };
        let before = self.bindings.len();
        self.bindings.retain(|(k, _)| *k != keys);
        before != self.bindings.len()
    }

    /// Bind by action id (`nav:down`), for user configuration.
    ///
    /// # Errors
    /// When the action id or the chord is unknown.
    pub fn bind_named(&mut self, chord: &str, action: &str) -> Result<(), String> {
        let a = NavAction::from_name(action).ok_or_else(|| format!("unknown navigation action {action:?}"))?;
        self.bind(chord, a)
    }

    /// Whether digits count.
    #[must_use]
    pub fn counts(&self) -> bool {
        self.counts
    }

    /// Every chord bound to `action`, rendered in binding order.
    #[must_use]
    pub fn chords_for(&self, action: NavAction) -> Vec<String> {
        self.bindings
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(ks, _)| ks.iter().copied().map(NavKey::label).collect::<String>())
            .collect()
    }

    /// Every `(chord, action)` row, for a help pane.
    #[must_use]
    pub fn rows(&self) -> Vec<(String, NavAction)> {
        self.bindings.iter().map(|(ks, a)| (ks.iter().copied().map(NavKey::label).collect(), *a)).collect()
    }

    /// A one-line key hint for `actions`, from the bindings: the first chord
    /// of each action, paired motions joined (`j/k move · gg/G ends`).
    /// Actions with no chord are left out.
    #[must_use]
    pub fn footer(&self, actions: &[NavAction]) -> String {
        let first = |a: NavAction| self.chords_for(a).into_iter().next();
        let mut parts: Vec<String> = Vec::new();
        let mut done: Vec<NavAction> = Vec::new();
        for &a in actions {
            if done.contains(&a) {
                continue;
            }
            done.push(a);
            let mut keys = vec![];
            if let Some(k) = first(a) {
                keys.push(k);
            }
            if let Some(p) = a.partner().filter(|p| actions.contains(p)) {
                done.push(p);
                if let Some(k) = first(p) {
                    keys.push(k);
                }
            }
            if !keys.is_empty() {
                parts.push(format!("{} {}", keys.join("/"), a.label()));
            }
        }
        parts.join(" · ")
    }

    fn exact(&self, keys: &[NavKey]) -> Option<NavAction> {
        self.bindings.iter().find(|(k, _)| k.as_slice() == keys).map(|(_, a)| *a)
    }

    fn is_prefix(&self, keys: &[NavKey]) -> bool {
        self.bindings.iter().any(|(k, _)| k.len() > keys.len() && k.starts_with(keys))
    }
}

impl Default for NavKeymap {
    fn default() -> Self {
        Self::menu()
    }
}

/// What one key did to a [`NavResolver`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feed {
    /// A complete command; `count` is the typed repeat, if any.
    Command { action: NavAction, count: Option<usize> },
    /// Part of a sequence or a count; wait for the next key.
    Pending,
    /// Not a navigation key. Any half-typed sequence or count is dropped.
    Unbound,
}

/// Chords + counts over a [`NavKeymap`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavResolver {
    keymap: NavKeymap,
    pending: Vec<NavKey>,
    count: Option<usize>,
}

impl NavResolver {
    #[must_use]
    pub fn new(keymap: NavKeymap) -> Self {
        Self { keymap, pending: Vec::new(), count: None }
    }

    #[must_use]
    pub fn keymap(&self) -> &NavKeymap {
        &self.keymap
    }

    pub fn keymap_mut(&mut self) -> &mut NavKeymap {
        &mut self.keymap
    }

    /// Whether a sequence or a count is half typed.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty() || self.count.is_some()
    }

    /// The half-typed count and sequence, for a footer (`5`, `g`, `3g`).
    #[must_use]
    pub fn pending_label(&self) -> String {
        let mut s = self.count.map(|c| c.to_string()).unwrap_or_default();
        s.extend(self.pending.iter().copied().map(NavKey::label));
        s
    }

    /// Drop a half-typed sequence and count.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.count = None;
    }

    /// Resolve one key.
    pub fn feed(&mut self, key: NavKey) -> Feed {
        if self.keymap.counts
            && self.pending.is_empty()
            && let Some(d) = key.digit()
            && (d != 0 || self.count.is_some())
            && self.keymap.exact(&[key]).is_none()
        {
            self.count = Some(self.count.unwrap_or(0).saturating_mul(10).saturating_add(d));
            return Feed::Pending;
        }
        let mut seq = std::mem::take(&mut self.pending);
        seq.push(key);
        if let Some(action) = self.keymap.exact(&seq) {
            return Feed::Command { action, count: self.count.take() };
        }
        if self.keymap.is_prefix(&seq) {
            self.pending = seq;
            return Feed::Pending;
        }
        // A broken sequence: forget it and read the key on its own.
        if seq.len() > 1 {
            return self.feed(key);
        }
        self.count = None;
        Feed::Unbound
    }
}

/// How the navigator's cursor relates to the rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKind {
    /// A selected row inside a scrolling viewport (menus, lists).
    Cursor,
    /// No selection: the motions scroll the viewport (help, scrollback).
    /// The "cursor" is the top visible row.
    Pager,
}

/// What `/` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchStyle {
    /// Jump to matches, keep every row (vim `/`).
    Jump,
    /// Keep only matching rows; the surface filters with [`Navigator::filter`].
    Filter,
}

/// What a key did, for the surface to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavOutcome {
    /// The cursor or viewport moved.
    Moved,
    /// A navigation key that changed nothing (already at the edge).
    Unchanged,
    /// Half a sequence or a count.
    Pending,
    /// The search prompt changed (typed, erased, committed or cancelled).
    Search,
    /// The filter query changed; recompute the rows. The cursor is at 0.
    FilterChanged,
    /// Open this row.
    Open(usize),
    /// Go up one level.
    Back,
    /// Nothing left to back out of inside the navigator: the surface goes
    /// back a level, or closes at the top.
    Dismiss,
    /// Close the surface.
    Close,
    /// Copy this row's text.
    Yank(usize),
    /// Not a navigation key.
    Unhandled,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct SearchState {
    /// Being typed: the query and where the cursor was when `/` was pressed.
    typing: Option<(String, usize)>,
    /// The last committed query (the `n`/`N` target, or the live filter).
    committed: Option<String>,
}

/// One navigable surface: a resolver, a cursor, a viewport and a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Navigator {
    resolver: NavResolver,
    kind: NavKind,
    style: SearchStyle,
    cursor: usize,
    offset: usize,
    height: usize,
    search: SearchState,
}

impl Navigator {
    /// A navigator over `keymap`.
    #[must_use]
    pub fn new(kind: NavKind, style: SearchStyle, keymap: NavKeymap) -> Self {
        Self {
            resolver: NavResolver::new(keymap),
            kind,
            style,
            cursor: 0,
            offset: 0,
            height: 10,
            search: SearchState::default(),
        }
    }

    /// A menu: selected row, jump search, the menu keys.
    #[must_use]
    pub fn menu() -> Self {
        Self::new(NavKind::Cursor, SearchStyle::Jump, NavKeymap::menu())
    }

    /// A pager: scrolling text, jump search, the menu keys.
    #[must_use]
    pub fn pager() -> Self {
        Self::new(NavKind::Pager, SearchStyle::Jump, NavKeymap::menu())
    }

    #[must_use]
    pub fn kind(&self) -> NavKind {
        self.kind
    }

    #[must_use]
    pub fn resolver(&self) -> &NavResolver {
        &self.resolver
    }

    #[must_use]
    pub fn keymap(&self) -> &NavKeymap {
        self.resolver.keymap()
    }

    pub fn keymap_mut(&mut self) -> &mut NavKeymap {
        self.resolver.keymap_mut()
    }

    /// The selected row (a menu) or the top visible row (a pager).
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// First visible row.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Rows the viewport shows.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Set the viewport height (the renderer knows it; call before `handle`).
    pub fn set_viewport(&mut self, height: usize) {
        self.height = height.max(1);
    }

    /// Put the cursor on `row` (clamped to `len`), keeping it visible.
    pub fn set_cursor(&mut self, row: usize, len: usize) {
        self.cursor = row;
        self.clamp(len);
    }

    /// Whether the viewport shows the last row (a pager's "live tail").
    #[must_use]
    pub fn at_end(&self, len: usize) -> bool {
        match self.kind {
            NavKind::Pager => self.offset >= self.max_offset(len),
            NavKind::Cursor => self.cursor + 1 >= len,
        }
    }

    /// Scroll a pager to the end.
    pub fn to_end(&mut self, len: usize) {
        match self.kind {
            NavKind::Pager => self.cursor = self.max_offset(len),
            NavKind::Cursor => self.cursor = len.saturating_sub(1),
        }
        self.clamp(len);
    }

    /// The search being typed (without the `/`), if any.
    #[must_use]
    pub fn search_prompt(&self) -> Option<&str> {
        self.search.typing.as_ref().map(|(q, _)| q.as_str())
    }

    /// The live search: being typed, else the last committed.
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.search_prompt().or(self.search.committed.as_deref())
    }

    /// For [`SearchStyle::Filter`]: the text rows must contain, or `""`.
    #[must_use]
    pub fn filter(&self) -> &str {
        match self.style {
            SearchStyle::Filter => self.query().unwrap_or(""),
            SearchStyle::Jump => "",
        }
    }

    /// Whether `text` matches the live query (smartcase: case matters only
    /// when the query has an uppercase letter).
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        self.query().is_none_or(|q| smartcase_contains(text, q))
    }

    /// Clear search and filter.
    pub fn clear_search(&mut self) {
        self.search = SearchState::default();
    }

    /// The key hint, rendered from the keymap, with the typed prompt or the
    /// pending count/sequence in front.
    #[must_use]
    pub fn footer(&self, actions: &[NavAction]) -> String {
        if let Some(q) = self.search_prompt() {
            return format!("/{q}\u{2588}  enter keeps · esc cancels");
        }
        let mut s = String::new();
        if let Some(q) = self.search.committed.as_deref() {
            let _ = write!(s, "/{q} · ");
        }
        let pending = self.resolver.pending_label();
        if !pending.is_empty() {
            let _ = write!(s, "{pending}… · ");
        }
        s.push_str(&self.keymap().footer(actions));
        s
    }

    fn max_offset(&self, len: usize) -> usize {
        len.saturating_sub(self.height)
    }

    fn clamp(&mut self, len: usize) {
        match self.kind {
            NavKind::Cursor => {
                self.cursor = self.cursor.min(len.saturating_sub(1));
                if self.cursor < self.offset {
                    self.offset = self.cursor;
                } else if self.cursor >= self.offset + self.height {
                    self.offset = self.cursor + 1 - self.height;
                }
                self.offset = self.offset.min(self.max_offset(len));
            }
            NavKind::Pager => {
                self.cursor = self.cursor.min(self.max_offset(len));
                self.offset = self.cursor;
            }
        }
    }

    /// One key over `rows` (the surface's rows as text: labels for a menu,
    /// lines for a pager). In [`SearchStyle::Filter`], pass the rows the
    /// filter kept.
    pub fn handle<S: AsRef<str>>(&mut self, key: NavKey, rows: &[S]) -> NavOutcome {
        if self.search.typing.is_some() {
            return self.search_key(key, rows);
        }
        match self.resolver.feed(key) {
            Feed::Pending => NavOutcome::Pending,
            Feed::Unbound => NavOutcome::Unhandled,
            Feed::Command { action, count } => self.apply(action, count, rows),
        }
    }

    /// Carry out `action` directly (a surface's own button, or a test).
    pub fn apply<S: AsRef<str>>(&mut self, action: NavAction, count: Option<usize>, rows: &[S]) -> NavOutcome {
        let len = rows.len();
        let n = count.unwrap_or(1).max(1);
        let half = (self.height / 2).max(1);
        let before = (self.cursor, self.offset);
        match action {
            NavAction::Down => self.cursor = self.cursor.saturating_add(n),
            NavAction::Up => self.cursor = self.cursor.saturating_sub(n),
            NavAction::HalfPageDown => self.cursor = self.cursor.saturating_add(half.saturating_mul(n)),
            NavAction::HalfPageUp => self.cursor = self.cursor.saturating_sub(half.saturating_mul(n)),
            NavAction::PageDown => self.cursor = self.cursor.saturating_add(self.height.saturating_mul(n)),
            NavAction::PageUp => self.cursor = self.cursor.saturating_sub(self.height.saturating_mul(n)),
            NavAction::Top => self.cursor = count.map_or(0, |c| c.saturating_sub(1)),
            NavAction::Bottom => self.cursor = count.map_or(usize::MAX, |c| c.saturating_sub(1)),
            NavAction::ViewTop | NavAction::ViewMiddle | NavAction::ViewBottom => {
                if self.kind == NavKind::Pager {
                    return NavOutcome::Unchanged;
                }
                let shown = len.saturating_sub(self.offset).min(self.height).max(1);
                self.cursor = self.offset
                    + match action {
                        NavAction::ViewTop => 0,
                        NavAction::ViewMiddle => (shown - 1) / 2,
                        _ => shown - 1,
                    };
            }
            NavAction::Search => {
                self.search.typing = Some((String::new(), self.cursor));
                return NavOutcome::Search;
            }
            NavAction::SearchNext | NavAction::SearchPrev => {
                let forward = action == NavAction::SearchNext;
                if self.style == SearchStyle::Filter {
                    return self.apply(if forward { NavAction::Down } else { NavAction::Up }, count, rows);
                }
                let Some(q) = self.search.committed.clone() else { return NavOutcome::Unchanged };
                for _ in 0..n {
                    if let Some(i) = find(rows, &q, self.cursor, forward, false) {
                        self.cursor = i;
                    }
                }
            }
            NavAction::Open => return if len == 0 { NavOutcome::Unchanged } else { NavOutcome::Open(self.cursor.min(len - 1)) },
            NavAction::Yank => return if len == 0 { NavOutcome::Unchanged } else { NavOutcome::Yank(self.cursor.min(len - 1)) },
            NavAction::Back => return NavOutcome::Back,
            NavAction::Close => return NavOutcome::Close,
            NavAction::Dismiss => {
                if self.search.committed.take().is_some() {
                    return if self.style == SearchStyle::Filter {
                        self.cursor = 0;
                        self.offset = 0;
                        NavOutcome::FilterChanged
                    } else {
                        NavOutcome::Search
                    };
                }
                return NavOutcome::Dismiss;
            }
        }
        self.clamp(len);
        if (self.cursor, self.offset) == before { NavOutcome::Unchanged } else { NavOutcome::Moved }
    }

    fn search_key<S: AsRef<str>>(&mut self, key: NavKey, rows: &[S]) -> NavOutcome {
        let Some((mut q, origin)) = self.search.typing.take() else { return NavOutcome::Unhandled };
        let name = match key {
            NavKey::Chord(h) if h.modifiers.is_empty() => h.key.to_string(),
            _ => String::new(),
        };
        match (key, name.as_str()) {
            (_, "escape") => {
                if self.style == SearchStyle::Jump {
                    self.cursor = origin;
                    self.clamp(rows.len());
                    return NavOutcome::Search;
                }
                self.search.committed = None;
                self.cursor = 0;
                self.offset = 0;
                return NavOutcome::FilterChanged;
            }
            (_, "return" | "enter") => {
                self.search.committed = (!q.is_empty()).then_some(q);
                return NavOutcome::Search;
            }
            (_, "backspace") => {
                if q.pop().is_none() {
                    self.cursor = origin;
                    self.clamp(rows.len());
                    return NavOutcome::Search;
                }
            }
            (NavKey::Char(c), _) => q.push(c),
            (_, "space") => q.push(' '),
            _ => {
                self.search.typing = Some((q, origin));
                return NavOutcome::Unchanged;
            }
        }
        let outcome = match self.style {
            SearchStyle::Jump => {
                self.cursor = if q.is_empty() { origin } else { find(rows, &q, origin, true, true).unwrap_or(origin) };
                self.clamp(rows.len());
                NavOutcome::Search
            }
            SearchStyle::Filter => {
                self.cursor = 0;
                self.offset = 0;
                NavOutcome::FilterChanged
            }
        };
        self.search.typing = Some((q, origin));
        outcome
    }
}

impl Default for Navigator {
    fn default() -> Self {
        Self::menu()
    }
}

fn smartcase_contains(text: &str, q: &str) -> bool {
    if q.chars().any(char::is_uppercase) {
        text.contains(q)
    } else {
        text.to_lowercase().contains(&q.to_lowercase())
    }
}

/// The next row matching `q` from `from`, wrapping. `inclusive` tests
/// `from` itself first (incremental search starts where you were).
fn find<S: AsRef<str>>(rows: &[S], q: &str, from: usize, forward: bool, inclusive: bool) -> Option<usize> {
    let len = rows.len();
    if len == 0 || q.is_empty() {
        return None;
    }
    let from = from.min(len - 1);
    let start = usize::from(!inclusive);
    (start..len + start)
        .map(|step| if forward { (from + step) % len } else { (from + len - step % len) % len })
        .find(|&i| smartcase_contains(rows[i].as_ref(), q))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(s: &str) -> NavKey {
        NavKey::parse(s).unwrap()
    }

    fn rows(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("row {i}")).collect()
    }

    fn press(nav: &mut Navigator, keys: &str, rows: &[String]) -> NavOutcome {
        let mut last = NavOutcome::Unhandled;
        for key in keys.split_whitespace() {
            last = nav.handle(k(key), rows);
        }
        last
    }

    #[test]
    fn default_rows_parse_and_every_action_is_reachable_in_a_menu() {
        let menu = NavKeymap::menu();
        for a in NavAction::ALL {
            assert!(!menu.chords_for(a).is_empty(), "{} has no menu chord", a.name());
        }
        let _ = NavKeymap::filter();
    }

    #[test]
    fn action_names_round_trip_and_are_unique() {
        for a in NavAction::ALL {
            assert_eq!(NavAction::from_name(a.name()), Some(a));
        }
        let mut names: Vec<_> = NavAction::ALL.iter().map(|a| a.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), NavAction::ALL.len());
    }

    /// The chord table: (keys, starting cursor, expected cursor, expected
    /// outcome), over 50 rows in a 10-row viewport.
    #[test]
    fn menu_chord_table() {
        use NavOutcome as O;
        let r = rows(50);
        let table: &[(&str, usize, usize, NavOutcome)] = &[
            ("j", 0, 1, O::Moved),
            ("down", 0, 1, O::Moved),
            ("ctrl+n", 0, 1, O::Moved),
            ("ctrl+j", 0, 1, O::Moved),
            ("tab", 0, 1, O::Moved),
            ("k", 5, 4, O::Moved),
            ("up", 5, 4, O::Moved),
            ("ctrl+p", 5, 4, O::Moved),
            ("shift+tab", 5, 4, O::Moved),
            ("k", 0, 0, O::Unchanged),
            ("5 j", 0, 5, O::Moved),
            ("1 2 j", 0, 12, O::Moved),
            ("3 k", 10, 7, O::Moved),
            ("g g", 30, 0, O::Moved),
            ("home", 30, 0, O::Moved),
            ("G", 0, 49, O::Moved),
            ("end", 0, 49, O::Moved),
            ("7 G", 0, 6, O::Moved),
            ("7 g g", 30, 6, O::Moved),
            ("ctrl+d", 0, 5, O::Moved),
            ("ctrl+u", 20, 15, O::Moved),
            ("ctrl+f", 0, 10, O::Moved),
            ("pagedown", 0, 10, O::Moved),
            ("ctrl+b", 25, 15, O::Moved),
            ("pageup", 25, 15, O::Moved),
            ("9 9 j", 0, 49, O::Moved),
            ("j", 49, 49, O::Unchanged),
            ("enter", 3, 3, O::Open(3)),
            ("l", 3, 3, O::Open(3)),
            ("right", 3, 3, O::Open(3)),
            ("h", 3, 3, O::Back),
            ("backspace", 3, 3, O::Back),
            ("left", 3, 3, O::Back),
            ("escape", 3, 3, O::Dismiss),
            ("q", 3, 3, O::Close),
            ("ctrl+c", 3, 3, O::Close),
            ("y", 3, 3, O::Yank(3)),
            ("g", 3, 3, O::Pending),
            ("5", 3, 3, O::Pending),
            ("x", 3, 3, O::Unhandled),
        ];
        for &(keys, start, want, outcome) in table {
            let mut nav = Navigator::menu();
            nav.set_viewport(10);
            nav.set_cursor(start, r.len());
            let got = press(&mut nav, keys, &r);
            assert_eq!((nav.cursor(), got), (want, outcome), "{keys:?} from {start}");
        }
    }

    #[test]
    fn screen_relative_motions_use_the_viewport() {
        let r = rows(50);
        let mut nav = Navigator::menu();
        nav.set_viewport(10);
        nav.set_cursor(25, r.len());
        let top = nav.offset();
        press(&mut nav, "H", &r);
        assert_eq!(nav.cursor(), top);
        press(&mut nav, "L", &r);
        assert_eq!(nav.cursor(), top + 9);
        press(&mut nav, "M", &r);
        assert_eq!(nav.cursor(), top + 4);
    }

    #[test]
    fn the_cursor_stays_inside_the_viewport() {
        let r = rows(50);
        let mut nav = Navigator::menu();
        nav.set_viewport(10);
        for _ in 0..30 {
            nav.handle(k("j"), &r);
            assert!(nav.cursor() >= nav.offset() && nav.cursor() < nav.offset() + 10);
        }
        press(&mut nav, "g g", &r);
        assert_eq!(nav.offset(), 0);
    }

    #[test]
    fn a_broken_sequence_reads_the_key_alone() {
        let r = rows(10);
        let mut nav = Navigator::menu();
        assert_eq!(press(&mut nav, "g j", &r), NavOutcome::Moved);
        assert_eq!(nav.cursor(), 1);
        assert!(!nav.resolver().is_pending());
    }

    #[test]
    fn an_unbound_key_drops_a_count() {
        let r = rows(10);
        let mut nav = Navigator::menu();
        press(&mut nav, "5 x", &r);
        assert_eq!(press(&mut nav, "j", &r), NavOutcome::Moved);
        assert_eq!(nav.cursor(), 1, "the 5 was forgotten");
    }

    #[test]
    fn jump_search_is_incremental_and_n_cycles_with_wrap() {
        let r: Vec<String> = ["alpha", "beta", "gamma", "beta two", "delta"].map(String::from).to_vec();
        let mut nav = Navigator::menu();
        assert_eq!(press(&mut nav, "/ b", &r), NavOutcome::Search);
        assert_eq!(nav.cursor(), 1, "jumps while typing");
        assert_eq!(nav.search_prompt(), Some("b"));
        press(&mut nav, "e t a enter", &r);
        assert_eq!(nav.search_prompt(), None);
        assert_eq!(nav.query(), Some("beta"));
        press(&mut nav, "n", &r);
        assert_eq!(nav.cursor(), 3);
        press(&mut nav, "n", &r);
        assert_eq!(nav.cursor(), 1, "wraps");
        press(&mut nav, "N", &r);
        assert_eq!(nav.cursor(), 3, "wraps backwards");
        press(&mut nav, "2 n", &r);
        assert_eq!(nav.cursor(), 3, "a count repeats");
    }

    #[test]
    fn escape_backs_out_one_layer_at_a_time() {
        let r: Vec<String> = ["alpha", "beta"].map(String::from).to_vec();
        let mut nav = Navigator::menu();
        press(&mut nav, "/ b", &r);
        assert_eq!(nav.cursor(), 1);
        press(&mut nav, "escape", &r);
        assert_eq!((nav.cursor(), nav.query()), (0, None), "cancelled search restores the cursor");
        press(&mut nav, "/ b enter", &r);
        assert_eq!(press(&mut nav, "escape", &r), NavOutcome::Search, "first esc clears the search");
        assert_eq!(nav.query(), None);
        assert_eq!(press(&mut nav, "escape", &r), NavOutcome::Dismiss, "then the surface");
    }

    #[test]
    fn smartcase() {
        let r: Vec<String> = ["Alpha", "alpha"].map(String::from).to_vec();
        let mut nav = Navigator::menu();
        press(&mut nav, "/ A", &r);
        assert_eq!(nav.cursor(), 0);
        press(&mut nav, "escape", &r);
        nav.set_cursor(1, 2);
        press(&mut nav, "/ a", &r);
        assert_eq!(nav.cursor(), 1, "lowercase matches either case from where it starts");
    }

    #[test]
    fn filter_style_reports_query_changes_and_n_moves() {
        let mut nav = Navigator::new(NavKind::Cursor, SearchStyle::Filter, NavKeymap::menu());
        let all = rows(20);
        nav.set_cursor(5, all.len());
        assert_eq!(press(&mut nav, "/ 1", &all), NavOutcome::FilterChanged);
        assert_eq!((nav.filter(), nav.cursor()), ("1", 0));
        press(&mut nav, "enter", &all);
        assert_eq!(nav.filter(), "1", "committed filter stays");
        let kept: Vec<String> = all.iter().filter(|r| nav.matches(r)).cloned().collect();
        press(&mut nav, "n", &kept);
        assert_eq!(nav.cursor(), 1);
        assert_eq!(press(&mut nav, "escape", &kept), NavOutcome::FilterChanged);
        assert_eq!(nav.filter(), "");
    }

    #[test]
    fn pager_scrolls_the_viewport() {
        let r = rows(100);
        let mut nav = Navigator::pager();
        nav.set_viewport(20);
        press(&mut nav, "j", &r);
        assert_eq!(nav.offset(), 1);
        press(&mut nav, "G", &r);
        assert_eq!(nav.offset(), 80, "the last page, not the last line");
        assert!(nav.at_end(r.len()));
        press(&mut nav, "ctrl+u", &r);
        assert_eq!(nav.offset(), 70);
        assert!(!nav.at_end(r.len()));
        press(&mut nav, "g g", &r);
        assert_eq!(nav.offset(), 0);
        press(&mut nav, "/ 5 0 enter", &r);
        assert_eq!(nav.offset(), 50);
        assert_eq!(press(&mut nav, "H", &r), NavOutcome::Unchanged);
    }

    #[test]
    fn filter_profile_leaves_letters_and_digits_to_the_query() {
        let mut res = NavResolver::new(NavKeymap::filter());
        for c in ['j', 'k', 'q', 'G', '5', '/', 'y'] {
            assert_eq!(res.feed(NavKey::Char(c)), Feed::Unbound, "{c}");
        }
        for (key, action) in [
            ("ctrl+j", NavAction::Down),
            ("ctrl+k", NavAction::Up),
            ("ctrl+n", NavAction::Down),
            ("ctrl+p", NavAction::Up),
            ("tab", NavAction::Down),
            ("shift+tab", NavAction::Up),
            ("down", NavAction::Down),
            ("up", NavAction::Up),
            ("enter", NavAction::Open),
            ("escape", NavAction::Dismiss),
        ] {
            assert_eq!(res.feed(k(key)), Feed::Command { action, count: None }, "{key}");
        }
    }

    #[test]
    fn bindings_are_data() {
        let mut km = NavKeymap::menu();
        km.bind("ctrl+e", NavAction::Down).unwrap();
        assert!(km.unbind("j"));
        km.bind_named("x", "nav:close").unwrap();
        assert!(km.bind_named("x", "nav:nope").is_err());
        assert!(km.bind("ctrl+nosuchkey", NavAction::Down).is_err());
        let mut nav = Navigator::new(NavKind::Cursor, SearchStyle::Jump, km);
        let r = rows(5);
        assert_eq!(press(&mut nav, "j", &r), NavOutcome::Unhandled);
        assert_eq!(press(&mut nav, "ctrl+e", &r), NavOutcome::Moved);
        assert_eq!(press(&mut nav, "x", &r), NavOutcome::Close);
    }

    #[test]
    fn footer_is_rendered_from_the_bindings() {
        let km = NavKeymap::menu();
        let f = km.footer(&[
            NavAction::Down,
            NavAction::Up,
            NavAction::Top,
            NavAction::Bottom,
            NavAction::Search,
            NavAction::Open,
            NavAction::Close,
        ]);
        assert_eq!(f, "j/k move · gg/G ends · / search · enter open · q close");
        let mut km = km;
        km.unbind("j");
        assert!(km.footer(&[NavAction::Down, NavAction::Up]).starts_with("down/k"), "follows rebinding");
        assert_eq!(NavKeymap::filter().footer(&[NavAction::Down, NavAction::Up]), "down/up move");
    }

    #[test]
    fn footer_shows_the_prompt_and_pending_keys() {
        let r = rows(5);
        let mut nav = Navigator::menu();
        press(&mut nav, "5", &r);
        assert!(nav.footer(&[NavAction::Down]).starts_with("5… · "));
        press(&mut nav, "escape / r o", &r);
        assert!(nav.footer(&[NavAction::Down]).starts_with("/ro\u{2588}"));
    }

    #[test]
    fn empty_rows_never_panic() {
        let r: Vec<String> = vec![];
        let mut nav = Navigator::menu();
        for key in ["j", "k", "G", "g", "g", "ctrl+d", "H", "M", "L", "n", "enter", "y", "/", "a", "enter", "n"] {
            nav.handle(k(key), &r);
        }
        assert_eq!(nav.cursor(), 0);
        let mut pager = Navigator::pager();
        for key in ["j", "G", "ctrl+f", "k"] {
            pager.handle(k(key), &r);
        }
        assert_eq!(pager.offset(), 0);
    }
}

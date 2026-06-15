//! `FuzzyPicker<T>` — the fleet-shared modal fuzzy-picker primitive.
//!
//! A modal overlay that fuzzy-filters a caller-supplied list of items,
//! ranks them by `(match quality, frecency)`, lets the operator navigate
//! with up/down (the consumer maps Ctrl-N/Ctrl-P or the arrows → the
//! [`PickerEvent::NavDown`] / [`PickerEvent::NavUp`] events), and
//! commits the highlighted item with Enter (→ [`PickerEvent::Accept`])
//! or cancels with Esc (→ [`PickerEvent::Cancel`]).
//!
//! This is the GENERIC version of mado's proven Ctrl-S session picker
//! (`mado::session_picker::SessionPickerState` driven by the
//! `mado::ux::modes::Overlay` FSM): a session/tab switcher, a command
//! palette (`:command-mode`), and a plain fuzzy chooser are all *one*
//! shape — a fuzzy-filter input + a ranked item list + nav +
//! accept/cancel — so the shape lives here, once, and every pleme-io GPU
//! app (mado, namimado, …) reuses it.
//!
//! ## Composition, not a fork
//!
//! The picker is built ON egaku's existing primitives rather than
//! re-implementing them:
//! * [`crate::TextInput`] owns the query line — cursor, grapheme-aware
//!   insert/backspace, the whole edit surface. The picker does not
//!   re-derive text editing.
//! * The fuzzy scorer ([`fuzzy_score`]) is the same subsequence-fuzzy
//!   ranking mado's praca uses (contiguity + word-boundary bonuses,
//!   light length penalty) — copied here (egaku is a leaf widget crate
//!   and cannot depend on praca) so a frecency-ranked picker and a plain
//!   fuzzy picker share one ranking.
//! * The wrapping up/down nav mirrors mado's `SessionPickerState`.
//!
//! ## Generic over the item key
//!
//! The caller owns what `T` is — a `SessionId`, a command, a tab id, a
//! URL. The picker is data-source-agnostic: it filters and ranks
//! [`PickerItem`]s by their `label` and optional frecency `score`, and
//! on accept hands back the chosen `key: T`. Nothing in the picker
//! knows what `T` means.
//!
//! ## Typed modal FSM — `(state, event) -> (state, effects)`
//!
//! [`PickerState`] is a two-arm sum (`Closed` / `Open`), NOT a bool
//! flag. [`PickerState::on_event`] is the pure, total transition: it
//! matches on the state enum with **no wildcard arm** — a new state
//! would be a compile error until every transition is decided (the same
//! forcing function mado's `Overlay::on_event` carries). It returns a
//! [`PickerStep`] = `(next state, effects)`; the consumer drives events
//! (from key input) and acts on the effects ([`PickerEffect::Accepted`]
//! → do the switch/run; [`PickerEffect::Cancelled`] → close).
//!
//! ## Render-backend-agnostic
//!
//! The picker is pure state + a [`FuzzyPicker::view`] accessor that
//! returns a [`PickerView`] (the query string, the visible
//! filtered+ranked rows, and the selected index). A GPU consumer
//! (mado/namimado via garasu/egaku) draws that overlay; the
//! terminal-rendered path (egaku-term) draws the same data as text.
//! There is no `wgpu` in here — exactly like [`crate::ScrollKinetics`]
//! is pure physics, this is pure state.

use crate::input::TextInput;

/// One row the picker can display + commit. Plain data — the caller
/// owns `key`; the picker only reads `label` (for fuzzy filtering +
/// display) and the optional frecency `score`.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerItem<T> {
    /// The value handed back on accept ([`PickerEffect::Accepted`]).
    pub key: T,
    /// The display + fuzzy-match text (e.g. `"🌊 tide  mado"`).
    pub label: String,
    /// Optional frecency / priority weight. Higher sorts first. Used as
    /// a tiebreak when fuzzy match quality is equal, and as the sole
    /// ordering when the query is empty (everything matches → rank by
    /// frecency). `None` is treated as `0.0`.
    pub score: Option<f64>,
}

impl<T> PickerItem<T> {
    /// A picker row with no frecency weight (a plain fuzzy chooser).
    #[must_use]
    pub fn new(key: T, label: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
            score: None,
        }
    }

    /// A picker row carrying a frecency / priority weight.
    #[must_use]
    pub fn with_score(key: T, label: impl Into<String>, score: f64) -> Self {
        Self {
            key,
            label: label.into(),
            score: Some(score),
        }
    }
}

/// The modal state of the picker overlay. A two-arm sum, never a bool
/// flag: `Closed` is "the keyboard belongs to the app"; `Open` is "the
/// picker captures input". The `Open` payload carries no item data —
/// the items live in [`FuzzyPicker`]; this is the routing state only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerState {
    /// The overlay is closed; events other than `Open` are inert.
    Closed,
    /// The overlay is open and capturing input.
    Open,
}

impl PickerState {
    /// Every state variant, in declaration order. The total-match
    /// forcing function in [`Self::on_event`] is what actually guards
    /// completeness; `ALL` exists so a registry/coverage test can
    /// iterate the states mechanically (mirrors mado's
    /// `Overlay::ALL`).
    pub const ALL: [PickerState; 2] = [PickerState::Closed, PickerState::Open];
}

/// Events the consumer feeds the FSM (typically from decoded key
/// input). The picker has no `Key(KeyCode)` arm — the consumer does the
/// key→event mapping (Ctrl-N/Ctrl-P/arrows → Nav, Enter → Accept,
/// Esc → Cancel, a printable → `Type`), keeping the picker
/// key-binding-agnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerEvent {
    /// Open the overlay (the consumer has already populated the items
    /// via [`FuzzyPicker::set_items`] / construction).
    Open,
    /// A printable character typed into the query.
    Type(char),
    /// Backspace one grapheme off the query.
    Backspace,
    /// Move the highlight up one row (wraps to the bottom at the top).
    NavUp,
    /// Move the highlight down one row (wraps to the top at the bottom).
    NavDown,
    /// Commit the highlighted row.
    Accept,
    /// Dismiss the overlay without committing.
    Cancel,
}

/// Typed side effects the consumer executes after a transition. The
/// `Accepted(T)` arm carries the chosen key so the consumer (which owns
/// what `T` means) can act on it — switch a session, run a command,
/// open a tab.
#[derive(Debug, Clone, PartialEq)]
pub enum PickerEffect<T> {
    /// The overlay opened; the renderer should start drawing.
    Opened,
    /// The query changed; `visible` is the new count of filtered+ranked
    /// rows (so a consumer can resize/relayout without re-querying the
    /// view). The full rows are read via [`FuzzyPicker::view`].
    Filtered {
        /// Number of rows matching the current query.
        visible: usize,
    },
    /// The highlight moved to `index` (an index into the filtered view).
    Moved {
        /// New selected index within the filtered rows.
        index: usize,
    },
    /// The highlighted row was committed; `key` is its [`PickerItem::key`].
    Accepted {
        /// The committed item's caller-owned key.
        key: T,
    },
    /// The overlay was dismissed without committing.
    Cancelled,
}

/// One transition's output: the next state + the effects to run.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerStep<T> {
    /// The state after the transition.
    pub state: PickerState,
    /// Effects the consumer executes (in order).
    pub effects: Vec<PickerEffect<T>>,
}

/// The generic modal fuzzy picker. Holds the full item set, the live
/// query (a [`TextInput`]), the current filtered+ranked view, and the
/// highlight. Drive it through [`Self::on_event`]; render it through
/// [`Self::view`].
///
/// `T` is the caller's key type. `T: Clone` is required so [`Accept`]
/// can hand the chosen key back in a [`PickerEffect::Accepted`] without
/// consuming the item (the picker keeps its list for a possible reopen).
///
/// [`Accept`]: PickerEvent::Accept
#[derive(Debug, Clone)]
pub struct FuzzyPicker<T> {
    state: PickerState,
    query: TextInput,
    /// The full, caller-supplied item set (unfiltered).
    items: Vec<PickerItem<T>>,
    /// Indices into `items`, filtered to the current query and ranked
    /// `(match quality desc, frecency desc, original order)`. This is
    /// the picker's view onto `items`.
    filtered: Vec<usize>,
    /// Highlight position within `filtered`.
    selected: usize,
}

/// The render-facing snapshot: everything a GPU or terminal renderer
/// needs to draw the overlay, and nothing it doesn't. Borrows from the
/// picker — no allocation, no clone of `T`.
#[derive(Debug)]
pub struct PickerView<'a, T> {
    /// The current query text (draw it in the filter input box).
    pub query: &'a str,
    /// The cursor byte-offset within `query` (for a caret).
    pub cursor: usize,
    /// The filtered + ranked rows, in display order.
    pub rows: Vec<&'a PickerItem<T>>,
    /// The highlighted index within `rows` (`0` when `rows` is empty).
    pub selected: usize,
}

impl<T: Clone> FuzzyPicker<T> {
    /// A closed picker over `items`. The view is pre-filtered for the
    /// empty query (everything, frecency-ranked) so a consumer that
    /// opens it immediately renders a populated list.
    #[must_use]
    pub fn new(items: Vec<PickerItem<T>>) -> Self {
        let mut p = Self {
            state: PickerState::Closed,
            query: TextInput::new(),
            items,
            filtered: Vec::new(),
            selected: 0,
        };
        p.recompute();
        p
    }

    /// An empty closed picker (items supplied later via
    /// [`Self::set_items`]).
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// The current FSM state.
    #[must_use]
    pub fn state(&self) -> PickerState {
        self.state
    }

    /// Whether the overlay is open (gates rendering + input capture).
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state == PickerState::Open
    }

    /// The current query text.
    #[must_use]
    pub fn query(&self) -> &str {
        self.query.text()
    }

    /// The number of rows matching the current query.
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.filtered.len()
    }

    /// The highlighted item's key, if any row is visible.
    #[must_use]
    pub fn selected_key(&self) -> Option<&T> {
        self.filtered
            .get(self.selected)
            .map(|&i| &self.items[i].key)
    }

    /// The highlighted item, if any row is visible.
    #[must_use]
    pub fn selected_item(&self) -> Option<&PickerItem<T>> {
        self.filtered.get(self.selected).map(|&i| &self.items[i])
    }

    /// Replace the full item set (e.g. the consumer re-listed from a
    /// live data source). Re-filters against the current query and
    /// resets the highlight to the top — mirrors
    /// `SessionPickerState::set_results`.
    pub fn set_items(&mut self, items: Vec<PickerItem<T>>) {
        self.items = items;
        self.selected = 0;
        self.recompute();
    }

    /// The render-facing snapshot of the current state.
    #[must_use]
    pub fn view(&self) -> PickerView<'_, T> {
        PickerView {
            query: self.query.text(),
            cursor: self.query.cursor(),
            rows: self.filtered.iter().map(|&i| &self.items[i]).collect(),
            selected: self.selected,
        }
    }

    /// Drive one FSM transition and apply it to this picker, returning
    /// the typed effects with their live payloads. The pure
    /// [`PickerState::on_event`] decides the next *routing state* +
    /// whether an event acts (the total-match table); this engine half
    /// then carries out the data mutation the event implies (query
    /// edit + re-filter, highlight move) and synthesizes the payloaded
    /// effects ([`PickerEffect::Filtered`] / `Moved` / `Accepted`) that
    /// depend on the live item list — the same "engine applies the
    /// step" split mado's `InputEngine::apply_overlay_step` uses.
    pub fn on_event(&mut self, event: PickerEvent) -> Vec<PickerEffect<T>> {
        let was_open = self.state == PickerState::Open;
        let step: PickerStep<T> = self.state.on_event(&event);
        self.state = step.state;
        match event {
            PickerEvent::Open => {
                // Reset the query to empty + reseed the empty-query view.
                self.query.select_all();
                self.query.delete_selection();
                self.selected = 0;
                self.recompute();
                vec![PickerEffect::Opened]
            }
            PickerEvent::Type(c) if was_open => {
                self.query.insert_char(c);
                self.selected = 0;
                self.recompute();
                vec![PickerEffect::Filtered {
                    visible: self.filtered.len(),
                }]
            }
            PickerEvent::Backspace if was_open => {
                self.query.delete_back();
                self.selected = 0;
                self.recompute();
                vec![PickerEffect::Filtered {
                    visible: self.filtered.len(),
                }]
            }
            PickerEvent::NavUp if was_open => {
                self.move_up();
                vec![PickerEffect::Moved {
                    index: self.selected,
                }]
            }
            PickerEvent::NavDown if was_open => {
                self.move_down();
                vec![PickerEffect::Moved {
                    index: self.selected,
                }]
            }
            PickerEvent::Accept if was_open => {
                // Snapshot the chosen key; the FSM has already moved us
                // to Closed. No row highlighted → a clean close with no
                // Accepted effect (an empty Vec, never a panic).
                match self.selected_key().cloned() {
                    Some(key) => vec![PickerEffect::Accepted { key }],
                    None => vec![],
                }
            }
            // Cancel, or any event on a Closed picker: defer to the pure
            // table's effects (Cancelled, or nothing).
            _ => step.effects,
        }
    }

    /// Move the highlight down one row (wraps to the top).
    fn move_down(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    /// Move the highlight up one row (wraps to the bottom).
    fn move_up(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Recompute the filtered+ranked index set against the current
    /// query. Empty query → every item, frecency-ranked. Non-empty →
    /// fuzzy-filtered, ranked `(match quality desc, frecency desc,
    /// original order)`.
    fn recompute(&mut self) {
        let q = self.query.text();
        let mut scored: Vec<(usize, i32)> = if q.is_empty() {
            // Everything matches; rank by frecency alone.
            (0..self.items.len()).map(|i| (i, 0)).collect()
        } else {
            self.items
                .iter()
                .enumerate()
                .filter_map(|(i, it)| fuzzy_score(q, &it.label).map(|s| (i, s)))
                .collect()
        };
        // Sort by match quality (desc), then frecency (desc), then
        // original order (stable index) for determinism.
        scored.sort_by(|&(ia, sa), &(ib, sb)| {
            sb.cmp(&sa)
                .then_with(|| {
                    let fa = self.items[ia].score.unwrap_or(0.0);
                    let fb = self.items[ib].score.unwrap_or(0.0);
                    fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| ia.cmp(&ib))
        });
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }
}

impl PickerState {
    /// The pure, total transition: `(state, event) -> (state, effects)`.
    /// No I/O, no locks, no data mutation — only the next routing state
    /// + the routing-only effects (`Opened` / `Cancelled`). The outer
    /// match carries **no wildcard arm** on the state enum: a new
    /// [`PickerState`] is a compile error until every transition is
    /// decided (the forcing function from mado's `Overlay::on_event`).
    ///
    /// The payloaded effects (`Filtered` / `Moved` / `Accepted`) depend
    /// on the live item list, which this generic-free table has no
    /// access to — so they are synthesized by the engine half
    /// ([`FuzzyPicker::on_event`]). Keeping one total-match table here
    /// (routing) and one engine half there (data) is exactly mado's
    /// `Overlay::on_event` / `InputEngine::apply_overlay_step` split.
    #[must_use]
    pub fn on_event<T>(self, event: &PickerEvent) -> PickerStep<T> {
        match self {
            // ── Closed: the keyboard belongs to the app. Only `Open`
            // does anything; every other event is inert by construction
            // (the consuming arms simply do not exist here — the
            // Esc-eating law from mado, made structural). ──
            PickerState::Closed => match event {
                PickerEvent::Open => PickerStep {
                    state: PickerState::Open,
                    effects: vec![PickerEffect::Opened],
                },
                PickerEvent::Type(_)
                | PickerEvent::Backspace
                | PickerEvent::NavUp
                | PickerEvent::NavDown
                | PickerEvent::Accept
                | PickerEvent::Cancel => PickerStep {
                    state: PickerState::Closed,
                    effects: vec![],
                },
            },
            // ── Open: the picker captures input. ──
            PickerState::Open => match event {
                // Re-open is idempotent (reseeds via the engine half).
                PickerEvent::Open => PickerStep {
                    state: PickerState::Open,
                    effects: vec![PickerEffect::Opened],
                },
                // The payloaded Filtered / Moved / Accepted effects are
                // synthesized by the engine half (it owns the live item
                // list + post-filter counts); the routing table only
                // decides the next state.
                PickerEvent::Type(_) | PickerEvent::Backspace => PickerStep {
                    state: PickerState::Open,
                    effects: vec![],
                },
                PickerEvent::NavUp | PickerEvent::NavDown => PickerStep {
                    state: PickerState::Open,
                    effects: vec![],
                },
                PickerEvent::Accept => PickerStep {
                    state: PickerState::Closed,
                    effects: vec![],
                },
                PickerEvent::Cancel => PickerStep {
                    state: PickerState::Closed,
                    effects: vec![PickerEffect::Cancelled],
                },
            },
        }
    }
}

/// Case-insensitive subsequence fuzzy scorer — the same ranking mado's
/// praca uses, copied here because egaku is a leaf widget crate and
/// cannot depend on praca.
///
/// Returns `Some(score)` if every char of `needle` appears in
/// `haystack` in order, else `None`. Higher is better. Rewards:
/// * contiguous runs of matched chars (`+run`, growing within a run),
/// * a match at the haystack start (`+8`),
/// * matches right after a separator (`/`, `-`, `_`, `.`, space) — word
///   boundaries (`+6`),
///
/// and lightly penalizes a longer haystack so a tight match on a short
/// label outranks the same subsequence buried in a long one. An empty
/// needle scores `0` against any haystack (matches everything) — the
/// picker routes the empty-query case to frecency-only before reaching
/// here.
#[must_use]
pub fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    let needle = needle.to_lowercase();
    let haystack_lc = haystack.to_lowercase();
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() {
        return Some(0);
    }
    let hay: Vec<char> = haystack_lc.chars().collect();

    let is_sep = |c: char| matches!(c, '/' | '-' | '_' | '.' | ' ');

    let mut ni = 0usize;
    let mut score = 0i32;
    let mut run = 0i32;
    for (hi, &hc) in hay.iter().enumerate() {
        if ni < n.len() && hc == n[ni] {
            run += 1;
            score += run; // contiguity reward grows within a run
            if hi == 0 {
                score += 8; // start-of-string bonus
            } else if is_sep(hay[hi - 1]) {
                score += 6; // word-boundary bonus
            }
            ni += 1;
        } else {
            run = 0;
        }
    }
    if ni == n.len() {
        // light length penalty: tighter haystack wins ties.
        Some(score - (i32::try_from(hay.len()).unwrap_or(i32::MAX) / 16))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Generic-T proof: a String command-palette picker AND a numeric
    // id picker exercise the SAME code, proving data-source-agnosticism.

    fn commands() -> Vec<PickerItem<String>> {
        vec![
            PickerItem::new("git.commit".to_string(), "git commit"),
            PickerItem::new("git.push".to_string(), "git push"),
            PickerItem::new("deploy".to_string(), "deploy"),
            PickerItem::new("docker.build".to_string(), "docker build"),
        ]
    }

    fn ids() -> Vec<PickerItem<u64>> {
        vec![
            PickerItem::with_score(10, "alpha", 1.0),
            PickerItem::with_score(20, "beta", 5.0),
            PickerItem::with_score(30, "gamma", 2.0),
        ]
    }

    // ── FSM totality: every (state, event) pair is decided. ──

    fn all_events() -> Vec<PickerEvent> {
        vec![
            PickerEvent::Open,
            PickerEvent::Type('x'),
            PickerEvent::Backspace,
            PickerEvent::NavUp,
            PickerEvent::NavDown,
            PickerEvent::Accept,
            PickerEvent::Cancel,
        ]
    }

    #[test]
    fn fsm_is_total_every_state_every_event() {
        // No (state, event) pair panics or is missing — on_event returns
        // a step for all 2 × 7 = 14 combinations.
        for state in PickerState::ALL {
            for ev in all_events() {
                let step: PickerStep<()> = state.on_event(&ev);
                // The next state is always a valid variant.
                assert!(PickerState::ALL.contains(&step.state));
            }
        }
    }

    #[test]
    fn closed_only_open_acts() {
        for ev in all_events() {
            let step: PickerStep<()> = PickerState::Closed.on_event(&ev);
            match ev {
                PickerEvent::Open => {
                    assert_eq!(step.state, PickerState::Open);
                    assert_eq!(step.effects, vec![PickerEffect::Opened]);
                }
                _ => {
                    assert_eq!(step.state, PickerState::Closed);
                    assert!(step.effects.is_empty(), "closed picker ignores {ev:?}");
                }
            }
        }
    }

    // ── Open / type / filter ── (String generic) ──

    #[test]
    fn open_shows_all_then_type_filters() {
        let mut p = FuzzyPicker::new(commands());
        assert!(!p.is_open());
        let fx = p.on_event(PickerEvent::Open);
        assert!(p.is_open());
        assert_eq!(fx, vec![PickerEffect::Opened]);
        // Empty query → all four.
        assert_eq!(p.visible_count(), 4);

        // Type "git" → only the two git commands remain, fewer visible.
        p.on_event(PickerEvent::Type('g'));
        p.on_event(PickerEvent::Type('i'));
        let fx = p.on_event(PickerEvent::Type('t'));
        assert_eq!(p.query(), "git");
        assert_eq!(p.visible_count(), 2);
        // The Type effect reports the post-filter visible count.
        assert_eq!(fx, vec![PickerEffect::Filtered { visible: 2 }]);
        let rows = p.view().rows;
        assert!(rows.iter().all(|r| r.label.starts_with("git")));
    }

    #[test]
    fn backspace_widens_filter() {
        let mut p = FuzzyPicker::new(commands());
        p.on_event(PickerEvent::Open);
        for c in "git".chars() {
            p.on_event(PickerEvent::Type(c));
        }
        assert_eq!(p.visible_count(), 2);
        let fx = p.on_event(PickerEvent::Backspace);
        assert_eq!(p.query(), "gi");
        // "gi" still only matches the git pair here.
        assert_eq!(fx, vec![PickerEffect::Filtered { visible: 2 }]);
        // Backspace to empty → all four.
        p.on_event(PickerEvent::Backspace);
        p.on_event(PickerEvent::Backspace);
        assert_eq!(p.query(), "");
        assert_eq!(p.visible_count(), 4);
    }

    #[test]
    fn empty_query_shows_all_frecency_ranked() {
        // ids() carry frecency: beta(5) > gamma(2) > alpha(1).
        let mut p = FuzzyPicker::new(ids());
        p.on_event(PickerEvent::Open);
        assert_eq!(p.query(), "");
        assert_eq!(p.visible_count(), 3);
        let rows = p.view().rows;
        // Frecency-ranked: beta, gamma, alpha.
        assert_eq!(rows[0].key, 20);
        assert_eq!(rows[1].key, 30);
        assert_eq!(rows[2].key, 10);
    }

    // ── Frecency boosts ranking on a tie ──

    #[test]
    fn frecency_breaks_fuzzy_ties() {
        // Two labels that score identically for the query "a"; the one
        // with higher frecency must rank first.
        let items = vec![
            PickerItem::with_score(1u64, "a-low", 1.0),
            PickerItem::with_score(2u64, "a-high", 9.0),
        ];
        let mut p = FuzzyPicker::new(items);
        p.on_event(PickerEvent::Open);
        p.on_event(PickerEvent::Type('a'));
        let rows = p.view().rows;
        // Both match "a" at the start equally → frecency decides.
        assert_eq!(rows[0].key, 2, "higher frecency ranks first on a tie");
        assert_eq!(rows[1].key, 1);
    }

    // ── Nav wraps + clamps ──

    #[test]
    fn nav_down_up_wraps() {
        let mut p = FuzzyPicker::new(ids());
        p.on_event(PickerEvent::Open);
        // 3 rows; selected starts at 0.
        assert_eq!(p.view().selected, 0);
        p.on_event(PickerEvent::NavDown);
        assert_eq!(p.view().selected, 1);
        p.on_event(PickerEvent::NavDown);
        assert_eq!(p.view().selected, 2);
        // Wrap down from the bottom → top.
        p.on_event(PickerEvent::NavDown);
        assert_eq!(p.view().selected, 0);
        // Wrap up from the top → bottom.
        p.on_event(PickerEvent::NavUp);
        assert_eq!(p.view().selected, 2);
    }

    #[test]
    fn nav_on_empty_results_is_noop() {
        let mut p = FuzzyPicker::new(commands());
        p.on_event(PickerEvent::Open);
        // A query matching nothing.
        for c in "zzzz".chars() {
            p.on_event(PickerEvent::Type(c));
        }
        assert_eq!(p.visible_count(), 0);
        p.on_event(PickerEvent::NavDown);
        p.on_event(PickerEvent::NavUp);
        assert_eq!(p.view().selected, 0);
        assert!(p.selected_key().is_none());
    }

    #[test]
    fn typing_resets_highlight_to_top() {
        let mut p = FuzzyPicker::new(ids());
        p.on_event(PickerEvent::Open);
        p.on_event(PickerEvent::NavDown);
        assert_eq!(p.view().selected, 1);
        // Any edit resets the highlight to the top, like
        // SessionPickerState::set_results.
        p.on_event(PickerEvent::Type('a'));
        assert_eq!(p.view().selected, 0);
    }

    // ── Accept / Cancel ──

    #[test]
    fn accept_emits_selected_key_and_closes() {
        let mut p = FuzzyPicker::new(commands());
        p.on_event(PickerEvent::Open);
        // Filter to "deploy", which becomes the only / top row.
        for c in "deploy".chars() {
            p.on_event(PickerEvent::Type(c));
        }
        assert_eq!(p.selected_key().map(String::as_str), Some("deploy"));
        let fx = p.on_event(PickerEvent::Accept);
        assert!(!p.is_open(), "accept closes the overlay");
        assert_eq!(
            fx,
            vec![PickerEffect::Accepted {
                key: "deploy".to_string()
            }]
        );
    }

    #[test]
    fn accept_with_no_rows_closes_without_accepted() {
        let mut p = FuzzyPicker::new(commands());
        p.on_event(PickerEvent::Open);
        for c in "zzzz".chars() {
            p.on_event(PickerEvent::Type(c));
        }
        assert_eq!(p.visible_count(), 0);
        let fx = p.on_event(PickerEvent::Accept);
        assert!(!p.is_open());
        // Nothing to accept → no Accepted effect, just a clean close.
        assert!(
            !fx.iter()
                .any(|e| matches!(e, PickerEffect::Accepted { .. })),
            "no Accepted when no row is highlighted"
        );
    }

    #[test]
    fn reopen_clears_the_query_and_reseeds() {
        let mut p = FuzzyPicker::new(commands());
        p.on_event(PickerEvent::Open);
        for c in "git".chars() {
            p.on_event(PickerEvent::Type(c));
        }
        assert_eq!(p.query(), "git");
        assert_eq!(p.visible_count(), 2);
        // Re-open (e.g. Ctrl-S pressed again while open) resets the
        // query to empty + reseeds the full frecency view.
        let fx = p.on_event(PickerEvent::Open);
        assert_eq!(fx, vec![PickerEffect::Opened]);
        assert!(p.is_open());
        assert_eq!(p.query(), "");
        assert_eq!(p.visible_count(), 4);
    }

    #[test]
    fn cancel_emits_cancelled_and_closes() {
        let mut p = FuzzyPicker::new(ids());
        p.on_event(PickerEvent::Open);
        let fx = p.on_event(PickerEvent::Cancel);
        assert!(!p.is_open());
        assert_eq!(fx, vec![PickerEffect::Cancelled]);
    }

    #[test]
    fn numeric_id_accept_round_trips_the_key() {
        // Generic-T proof on a numeric key: accept hands back the u64.
        let mut p = FuzzyPicker::new(ids());
        p.on_event(PickerEvent::Open);
        // Empty query, frecency-ranked → top is beta (key 20).
        let fx = p.on_event(PickerEvent::Accept);
        assert_eq!(fx, vec![PickerEffect::Accepted { key: 20u64 }]);
    }

    // ── set_items re-filters + resets ──

    #[test]
    fn set_items_refilters_and_resets() {
        let mut p = FuzzyPicker::new(commands());
        p.on_event(PickerEvent::Open);
        p.on_event(PickerEvent::NavDown);
        p.set_items(vec![PickerItem::new("x".to_string(), "xenon")]);
        assert_eq!(p.visible_count(), 1);
        assert_eq!(p.view().selected, 0);
        assert_eq!(p.selected_key().map(String::as_str), Some("x"));
        assert_eq!(p.selected_item().map(|i| i.label.as_str()), Some("xenon"));
    }

    // ── view() is the render-agnostic surface ──

    #[test]
    fn view_exposes_query_cursor_rows_selected() {
        let mut p = FuzzyPicker::new(ids());
        p.on_event(PickerEvent::Open);
        p.on_event(PickerEvent::Type('b'));
        let v = p.view();
        assert_eq!(v.query, "b");
        assert_eq!(v.cursor, 1);
        assert_eq!(v.selected, 0);
        assert!(v.rows.iter().any(|r| r.label == "beta"));
    }

    // ── fuzzy_score behaviors (mirrors praca's contract) ──

    #[test]
    fn fuzzy_subsequence_and_miss() {
        assert!(fuzzy_score("dpl", "deploy").is_some());
        assert!(fuzzy_score("xyz", "deploy").is_none());
    }

    #[test]
    fn fuzzy_empty_needle_matches_all() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn fuzzy_is_case_insensitive() {
        assert!(fuzzy_score("GIT", "git commit").is_some());
    }

    #[test]
    fn fuzzy_start_bonus_outranks_mid() {
        let start = fuzzy_score("g", "git").unwrap();
        let mid = fuzzy_score("g", "a-g").unwrap();
        // Start-of-string bonus (+8) beats the word-boundary bonus (+6).
        assert!(start > mid, "start {start} should beat boundary {mid}");
    }
}

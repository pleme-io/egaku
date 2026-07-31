//! [`TableView`] — the generic columnar table state machine.
//!
//! # Provenance: this is a lift, not a new design
//!
//! banken hand-rolled a `PodTable` (492 LOC) against its own Kubernetes row
//! type, carrying an open `pending-banken: promote-tableview-to-egaku` token
//! that named exactly this module as the destination. About two thirds of that
//! model was already generic — columns, the identity-field join, selection,
//! sort, the unresolved-field report — and only the pod-specific *bindings*
//! (which columns, which resource kind, how to read a `(defk8sview)` form)
//! were app knowledge. This module is that generic two thirds, parameterized
//! over [`TableRow`] instead of over one concrete row struct.
//!
//! The lift is behaviour-preserving on purpose: every method here does what
//! `PodTable`'s did, so an app collapsing onto it is an adapter, not a
//! rewrite. Three things did change, each deliberate and each noted at its
//! method:
//!
//! 1. The sort-column validation, which `PodTable` performed only on its
//!    spec-reading constructor, is now on the *only* constructor — so no
//!    `TableView` can exist with a sort that silently degenerates.
//! 2. Sort direction cycling is expressed once, as [`SortOrder::toggled`].
//! 3. [`Selectable`] is implemented from birth. `PodTable`'s four selection
//!    methods matched egaku's [`ListView`] signature *verbatim*, written
//!    independently in a different repo against a different row type. That
//!    coincidence is the evidence the trait describes something real.
//!
//! # What a consumer supplies
//!
//! Exactly one impl of [`TableRow`] — an identity string and a
//! field-name-to-cell lookup — plus the [`Column`] list and the initial
//! [`SortKey`]. Everything else (widths, ordering, cursor, refresh-preserving
//! selection) is here.
//!
//! # Scrolling
//!
//! `TableView` deliberately carries **no viewport state**: no offset, no
//! visible-row count. It is the full ordered row set plus a cursor. A renderer
//! that has fewer rows of screen than the table has rows derives its own
//! window from [`TableView::selected_index`] and its own height — see
//! `egaku_term::draw::table`, which does exactly that. This mirrors what the
//! source model did (`PodTable` had no scroll surface at all) and keeps the
//! one piece of state that genuinely depends on the *display* out of the
//! display-independent model.

use crate::selectable::Selectable;

/// The reserved column field that projects a row's identity
/// ([`TableRow::identity`]) rather than one of its cells.
///
/// One name, one place. Column layouts are usually authored as data (a config
/// file, a Lisp form, a spec), and the identity column is the one field that
/// is not a cell lookup — so the string that joins "the authored `name`
/// column" to "call `identity()`" must exist exactly once. Repeating the
/// literal in the value projection and again in the sort comparator is how an
/// identity column silently stops resolving in one of the two.
pub const IDENTITY_FIELD: &str = "name";

/// A row a [`TableView`] can display: something with a stable identity and
/// named cells.
///
/// # Why identity is separate from the cells
///
/// Identity is not decoration — it is what makes a *refresh* non-destructive.
/// [`TableView::set_rows`] re-finds the cursor by identity, so a poll that
/// returns the same logical rows in a different order does not jump the
/// operator's selection. A row type that folded its identity into the cell map
/// would leave the model no way to ask "is this the same thing I had before?".
///
/// # Contract
///
/// - `identity` is stable for the lifetime of the logical row and unique
///   within one row set. Duplicate identities are not rejected (that would
///   make a legitimately-ambiguous read unusable) but selection-preservation
///   across a refresh then lands on the first match.
/// - `cell` returns `None` for a field this row does not carry, never a
///   placeholder. The distinction is load-bearing:
///   [`TableView::unresolved_fields`] reports columns that *no* row populates,
///   and it can only do that if "absent" and "present but empty" are different
///   answers.
pub trait TableRow {
    /// This row's stable identity — what the identity column renders and what
    /// a refresh re-finds the cursor by.
    fn identity(&self) -> &str;

    /// This row's value for `field`, or `None` if the row does not carry it.
    fn cell(&self, field: &str) -> Option<&str>;
}

/// One resolved column: a header to draw and the [`TableRow`] field it
/// projects.
///
/// **The `field` IS the row's cell key.** In the model this was lifted from,
/// the authored column said `:field phase` while every reader emitted a cell
/// keyed `"STATUS"` — two vocabularies for one thing, so the authored field
/// was decorative and a typo in it was invisible. Keeping header and field
/// distinct *and* making the field the real join key is what lets
/// [`TableView::unresolved_fields`] be meaningful.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// The header text drawn at the top of the column.
    pub header: String,
    /// The [`TableRow::cell`] key this column reads. [`IDENTITY_FIELD`] reads
    /// [`TableRow::identity`] instead.
    pub field: String,
}

impl Column {
    /// Construct a column from a header + field key.
    #[must_use]
    pub fn new(header: impl Into<String>, field: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            field: field.into(),
        }
    }

    /// Whether this column projects [`TableRow::identity`] rather than a cell.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.field == IDENTITY_FIELD
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Ascending — lexicographic on the projected cell value.
    Asc,
    /// Descending — the reverse.
    Desc,
}

impl SortOrder {
    /// The other direction. The cycle is expressed here once so a caller never
    /// hand-writes the `match`.
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

/// The active sort: a column **header** plus a direction.
///
/// Note it names the *header*, not the field. The header is the
/// operator-facing name — it is what a user clicks, types, or reads in a
/// status line — while the field is the reader's join key and may be an
/// internal spelling. [`TableView`] resolves header to field through the
/// declared columns; that resolution is the whole reason the constructor can
/// reject a sort naming a column that does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    /// The header of the column to sort by.
    pub column: String,
    /// The direction.
    pub order: SortOrder,
}

impl SortKey {
    /// Construct a sort key from a column header + direction.
    #[must_use]
    pub fn new(column: impl Into<String>, order: SortOrder) -> Self {
        Self {
            column: column.into(),
            order,
        }
    }
}

/// The one way a [`TableView`] can refuse to exist or refuse a mutation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    /// A sort key named a column header the table does not declare.
    #[error("sort column `{0}` is not one of this table's declared columns")]
    UnknownSortColumn(String),
}

/// A columnar table over rows of `R`: the ordered rows, the column layout, the
/// cursor, and the active sort.
///
/// Pure state — no rendering, no IO, no viewport (see the module docs). The
/// cursor is clamped to the row set on every mutation, so an out-of-range
/// selected index does not exist after construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableView<R> {
    columns: Vec<Column>,
    rows: Vec<R>,
    selected: usize,
    sort: SortKey,
    focused: bool,
}

impl<R: TableRow> TableView<R> {
    /// A table over `rows`, laid out by `columns`, initially sorted by `sort`.
    ///
    /// The rows are sorted immediately, so a freshly-built table is already in
    /// display order.
    ///
    /// # Errors
    ///
    /// [`TableError::UnknownSortColumn`] when `sort.column` names no declared
    /// column.
    ///
    /// **This check is the reason the constructor is fallible, and it is worth
    /// the ergonomic cost.** An unresolvable sort header falls through to a
    /// field no row carries, so every row projects `""`, every comparison ties,
    /// and the table comes out in whatever order the data source happened to
    /// return — a *silently arbitrary* order that looks sorted. A refusal at
    /// construction is strictly better than a plausible lie at render. Tier:
    /// parse-time-rejected (a `Result` at the boundary), not
    /// truly-unrepresentable — `SortKey` is a plain struct and can still be
    /// *written* naming a nonexistent column; it just cannot be *installed*.
    pub fn new(columns: Vec<Column>, rows: Vec<R>, sort: SortKey) -> Result<Self, TableError> {
        Self::require_declared(&columns, &sort.column)?;
        let mut t = Self {
            columns,
            rows,
            selected: 0,
            sort,
            focused: false,
        };
        t.apply_sort();
        Ok(t)
    }

    /// The column fields no observed row carries — a declared column that will
    /// always render empty.
    ///
    /// Reported as data rather than as silence, and deliberately **not** an
    /// error: a legitimately-absent cell exists (a reader may not populate
    /// every field for every row), so the honest surface is a report the caller
    /// can display or assert on, not a refusal that would make the widget
    /// unusable against a partially-populated read.
    ///
    /// This stays generic on purpose. A declared column naming a field the row
    /// type never populates is a live hazard for *any* [`TableRow`] impl —
    /// authored column lists and row producers drift apart in every domain, not
    /// just the one this was lifted from.
    ///
    /// The identity column is exempt: it reads [`TableRow::identity`], which
    /// every row has by construction.
    #[must_use]
    pub fn unresolved_fields(&self) -> Vec<&str> {
        self.columns
            .iter()
            .filter(|c| !c.is_identity())
            .filter(|c| !self.rows.iter().any(|r| r.cell(&c.field).is_some()))
            .map(|c| c.field.as_str())
            .collect()
    }

    /// The declared columns.
    #[must_use]
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// The current rows, already in sorted display order.
    #[must_use]
    pub fn rows(&self) -> &[R] {
        &self.rows
    }

    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the table has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The selected row index (always in range, or `0` when empty).
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The active sort key.
    #[must_use]
    pub fn sort(&self) -> &SortKey {
        &self.sort
    }

    /// The selected row, or `None` when the table is empty.
    #[must_use]
    pub fn selected_row(&self) -> Option<&R> {
        self.rows.get(self.selected)
    }

    /// Set whether this table currently has keyboard focus.
    ///
    /// See [`ListView::set_focused`](crate::ListView::set_focused) for why
    /// focus is widget-owned state and how it relates to
    /// [`FocusManager`](crate::FocusManager).
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether this table currently has keyboard focus.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Replace the rows (a watch/poll refresh), preserving the cursor by row
    /// **identity** when the same row is still present. Re-applies the active
    /// sort.
    ///
    /// Preserving by identity rather than by index is what makes a live-updating
    /// table usable: a background refresh that reorders rows would otherwise
    /// move the operator's cursor onto a different row between the moment they
    /// read it and the moment they act on it.
    pub fn set_rows(&mut self, rows: Vec<R>) {
        let selected_identity = self.selected_row().map(|r| r.identity().to_string());
        self.rows = rows;
        self.apply_sort();
        self.selected = match selected_identity {
            Some(id) => self
                .rows
                .iter()
                .position(|r| r.identity() == id)
                .unwrap_or(0),
            None => 0,
        };
        self.clamp_selection();
    }

    /// Move the cursor down one row (saturating at the last row).
    pub fn select_next(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.rows.len() - 1);
    }

    /// Move the cursor up one row (saturating at the first row).
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Cycle the sort direction on the current column and re-sort, holding the
    /// cursor on the same row.
    pub fn toggle_sort_direction(&mut self) {
        self.sort.order = self.sort.order.toggled();
        self.resort_holding_cursor();
    }

    /// Sort by a different column, keeping the current direction, holding the
    /// cursor on the same row.
    ///
    /// This is the column picker the source model explicitly deferred to this
    /// promotion ("a real column-picker lands with the egaku `TableView`
    /// promotion; for M0 the sort column is fixed and only the direction
    /// cycles"). It is here because it is the same header→field resolution the
    /// constructor already performs, and because leaving it out would push
    /// every consumer to rebuild the table just to change its sort — which
    /// would drop the cursor.
    ///
    /// # Errors
    ///
    /// [`TableError::UnknownSortColumn`] when `header` names no declared
    /// column. Same reasoning as [`TableView::new`]: refusing is better than
    /// sorting by nothing.
    pub fn sort_by(&mut self, header: &str) -> Result<(), TableError> {
        Self::require_declared(&self.columns, header)?;
        self.sort.column = header.to_string();
        self.resort_holding_cursor();
        Ok(())
    }

    /// The projected value of `column` for `row`: the identity column reads
    /// [`TableRow::identity`], every other column reads
    /// [`TableRow::cell`]. A missing cell projects `""` — never a panic.
    /// [`TableView::unresolved_fields`] is what surfaces a column that is
    /// *always* empty.
    #[must_use]
    pub fn cell_value<'a>(&self, row: &'a R, column: &Column) -> &'a str {
        Self::project(row, &column.field)
    }

    /// Reject a sort header that no declared column carries.
    fn require_declared(columns: &[Column], header: &str) -> Result<(), TableError> {
        if columns.iter().any(|c| c.header == header) {
            Ok(())
        } else {
            Err(TableError::UnknownSortColumn(header.to_string()))
        }
    }

    /// The value one row projects for a *field* (not a header).
    fn project<'a>(row: &'a R, field: &str) -> &'a str {
        if field == IDENTITY_FIELD {
            row.identity()
        } else {
            row.cell(field).unwrap_or("")
        }
    }

    /// The field the active sort projects, resolved through the declared
    /// columns — [`SortKey::column`] is a HEADER, [`TableRow::cell`] is keyed
    /// by FIELD, and resolving is what joins the two.
    ///
    /// The fallback (use the header as a field) is unreachable while every
    /// mutation of `sort.column` goes through [`Self::require_declared`]; it is
    /// here so this helper is total rather than panicking.
    fn sort_field(&self) -> String {
        self.columns
            .iter()
            .find(|c| c.header == self.sort.column)
            .map_or_else(|| self.sort.column.clone(), |c| c.field.clone())
    }

    fn clamp_selection(&mut self) {
        if self.rows.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.rows.len() {
            self.selected = self.rows.len() - 1;
        }
    }

    /// Re-sort and re-find the cursor by identity — the shared tail of every
    /// sort mutation, so "changing the sort moves the cursor" cannot be true
    /// of one entry point and false of another.
    fn resort_holding_cursor(&mut self) {
        let selected_identity = self.selected_row().map(|r| r.identity().to_string());
        self.apply_sort();
        if let Some(id) = selected_identity {
            self.selected = self
                .rows
                .iter()
                .position(|r| r.identity() == id)
                .unwrap_or(self.selected);
        }
        self.clamp_selection();
    }

    fn apply_sort(&mut self) {
        let field = self.sort_field();
        let order = self.sort.order;
        self.rows.sort_by(|a, b| {
            let base = Self::project(a, &field).cmp(Self::project(b, &field));
            match order {
                SortOrder::Asc => base,
                SortOrder::Desc => base.reverse(),
            }
        });
    }
}

/// `TableView` implements [`Selectable`] from birth.
///
/// Not a retrofit: the model this was lifted from had `selected_index` /
/// `len` / `select_next` / `select_prev` with these exact shapes before the
/// trait existed, in a different repo, against a different row type, written
/// by someone who had never seen [`ListView`]'s signature. The trait is the
/// name for what two independent implementations already agreed on.
impl<R: TableRow> Selectable for TableView<R> {
    fn selected_index(&self) -> usize {
        TableView::selected_index(self)
    }

    fn len(&self) -> usize {
        TableView::len(self)
    }

    fn select_next(&mut self) {
        TableView::select_next(self);
    }

    fn select_prev(&mut self) {
        TableView::select_prev(self);
    }

    fn is_empty(&self) -> bool {
        TableView::is_empty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal [`TableRow`]: an identity plus an association list of cells.
    /// Deliberately shaped like the real consumer's row (an ordered `Vec` of
    /// pairs, not a map) so the lift is exercised against the access pattern
    /// it was lifted from.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestRow {
        name: String,
        cells: Vec<(String, String)>,
    }

    impl TableRow for TestRow {
        fn identity(&self) -> &str {
            &self.name
        }

        fn cell(&self, field: &str) -> Option<&str> {
            self.cells
                .iter()
                .find(|(k, _)| k == field)
                .map(|(_, v)| v.as_str())
        }
    }

    fn row(name: &str, status: &str) -> TestRow {
        TestRow {
            name: name.into(),
            cells: vec![
                ("ready".into(), "1/1".into()),
                ("phase".into(), status.into()),
            ],
        }
    }

    fn columns() -> Vec<Column> {
        vec![
            Column::new("NAME", IDENTITY_FIELD),
            Column::new("READY", "ready"),
            Column::new("STATUS", "phase"),
        ]
    }

    fn table(rows: Vec<TestRow>) -> TableView<TestRow> {
        TableView::new(columns(), rows, SortKey::new("STATUS", SortOrder::Desc))
            .expect("STATUS is a declared column")
    }

    // ---- construction ------------------------------------------------------

    #[test]
    fn empty_table_has_zero_selection_and_no_selected_row() {
        let t = table(vec![]);
        assert_eq!(t.selected_index(), 0);
        assert!(t.selected_row().is_none());
        assert!(t.is_empty());
    }

    #[test]
    fn a_sort_naming_an_undeclared_column_is_refused() {
        let err = TableView::new(
            columns(),
            vec![row("a", "Running")],
            SortKey::new("NOPE", SortOrder::Asc),
        )
        .expect_err("an undeclared sort column must not construct");
        assert_eq!(err, TableError::UnknownSortColumn("NOPE".into()));
    }

    #[test]
    fn the_refusal_names_the_offending_column_in_its_message() {
        let err = TableView::new(
            columns(),
            vec![row("a", "Running")],
            SortKey::new("PHASE", SortOrder::Asc),
        )
        .expect_err("undeclared");
        assert!(
            err.to_string().contains("PHASE"),
            "the error must name the column the author typed, got {err}"
        );
    }

    // ---- sorting -----------------------------------------------------------

    #[test]
    fn rows_are_sorted_at_construction_by_the_declared_sort() {
        let t = table(vec![
            row("a", "CrashLoopBackOff"),
            row("b", "Running"),
            row("c", "Pending"),
        ]);
        let statuses: Vec<&str> = t
            .rows()
            .iter()
            .map(|r| t.cell_value(r, &Column::new("STATUS", "phase")))
            .collect();
        assert_eq!(statuses, vec!["Running", "Pending", "CrashLoopBackOff"]);
    }

    #[test]
    fn toggle_sort_direction_reverses_order() {
        let mut t = table(vec![
            row("a", "Running"),
            row("b", "Pending"),
            row("c", "CrashLoopBackOff"),
        ]);
        let before: Vec<String> = t.rows().iter().map(|r| r.name.clone()).collect();
        t.toggle_sort_direction();
        let after: Vec<String> = t.rows().iter().map(|r| r.name.clone()).collect();
        assert_eq!(
            after,
            before.iter().rev().cloned().collect::<Vec<_>>(),
            "a direction cycle is exactly a reversal on distinct keys"
        );
        assert_eq!(t.sort().order, SortOrder::Asc);
    }

    #[test]
    fn sorting_by_the_identity_column_orders_by_identity() {
        let mut t = table(vec![row("c", "x"), row("a", "x"), row("b", "x")]);
        t.sort_by("NAME").expect("NAME is declared");
        // Still Desc from the fixture.
        let names: Vec<&str> = t.rows().iter().map(TableRow::identity).collect();
        assert_eq!(names, vec!["c", "b", "a"]);
    }

    #[test]
    fn sort_by_an_undeclared_column_is_refused_and_changes_nothing() {
        let mut t = table(vec![row("a", "Running"), row("b", "Pending")]);
        let before = t.clone();
        let err = t.sort_by("MISSING").expect_err("undeclared");
        assert_eq!(err, TableError::UnknownSortColumn("MISSING".into()));
        assert_eq!(t, before, "a refused sort leaves the table untouched");
    }

    #[test]
    fn sort_resolves_the_header_to_the_field_not_the_header_itself() {
        // The sort names header "STATUS"; rows are keyed by field "phase".
        // If the resolution were dropped, every row would project "" and the
        // order would be whatever the input order happened to be.
        let t = table(vec![
            row("first-in", "Aaa"),
            row("second-in", "Zzz"),
            row("third-in", "Mmm"),
        ]);
        let names: Vec<&str> = t.rows().iter().map(TableRow::identity).collect();
        assert_eq!(
            names,
            vec!["second-in", "third-in", "first-in"],
            "rows ordered by the phase cell, not by input order"
        );
    }

    // ---- selection ---------------------------------------------------------

    #[test]
    fn selection_saturates_at_both_ends() {
        let mut t = table(vec![row("a", "Running"), row("b", "Running")]);
        t.select_prev();
        assert_eq!(t.selected_index(), 0);
        t.select_next();
        t.select_next();
        t.select_next();
        assert_eq!(t.selected_index(), 1, "cannot exceed the last row");
    }

    #[test]
    fn set_rows_preserves_selection_by_identity() {
        let mut t = table(vec![row("a", "Running"), row("b", "Running")]);
        t.select_next();
        let selected = t.selected_row().unwrap().name.clone();
        t.set_rows(vec![row("b", "Running"), row("a", "Running")]);
        assert_eq!(
            t.selected_row().unwrap().name,
            selected,
            "the cursor follows the row by identity across a refresh"
        );
    }

    #[test]
    fn set_rows_that_drops_the_selected_row_falls_back_to_the_first() {
        let mut t = table(vec![row("a", "Running"), row("b", "Running")]);
        t.select_next();
        t.set_rows(vec![row("c", "Running")]);
        assert_eq!(t.selected_index(), 0);
        assert_eq!(t.selected_row().unwrap().name, "c");
    }

    #[test]
    fn set_rows_to_empty_clamps_the_cursor() {
        let mut t = table(vec![row("a", "Running"), row("b", "Running")]);
        t.select_next();
        t.set_rows(vec![]);
        assert_eq!(t.selected_index(), 0);
        assert!(t.selected_row().is_none());
    }

    #[test]
    fn toggling_the_sort_holds_the_cursor_on_the_same_row() {
        let mut t = table(vec![row("a", "Aaa"), row("b", "Bbb"), row("c", "Ccc")]);
        t.select_next();
        let held = t.selected_row().unwrap().name.clone();
        t.toggle_sort_direction();
        assert_eq!(t.selected_row().unwrap().name, held);
    }

    // ---- cell projection ---------------------------------------------------

    #[test]
    fn cell_value_reads_identity_and_cells_and_missing_is_empty() {
        let t = table(vec![row("catch-0", "Running")]);
        let r = &t.rows()[0];
        assert_eq!(
            t.cell_value(r, &Column::new("NAME", IDENTITY_FIELD)),
            "catch-0"
        );
        assert_eq!(t.cell_value(r, &Column::new("STATUS", "phase")), "Running");
        assert_eq!(
            t.cell_value(r, &Column::new("MISSING", "missing")),
            "",
            "a missing cell projects empty, never panics"
        );
    }

    // ---- unresolved fields -------------------------------------------------

    #[test]
    fn unresolved_fields_reports_a_column_no_row_populates() {
        let cols = vec![
            Column::new("NAME", IDENTITY_FIELD),
            Column::new("STATUS", "phase"),
            Column::new("AGE", "age"),
        ];
        let t = TableView::new(
            cols,
            vec![row("a", "Running")],
            SortKey::new("STATUS", SortOrder::Asc),
        )
        .unwrap();
        assert_eq!(t.unresolved_fields(), vec!["age"]);
    }

    #[test]
    fn unresolved_fields_never_reports_the_identity_column() {
        // Identity is satisfied by TableRow::identity, not by a cell — so it
        // must never be reported even though no row carries a "name" cell.
        let t = table(vec![row("a", "Running")]);
        assert!(t.unresolved_fields().is_empty());
    }

    #[test]
    fn a_present_but_empty_cell_is_resolved_not_missing() {
        // `cell()` returning Some("") means the row DOES carry the field. This
        // is the distinction the TableRow contract asks implementors to keep.
        let r = TestRow {
            name: "a".into(),
            cells: vec![("phase".into(), String::new())],
        };
        let t = TableView::new(
            vec![Column::new("STATUS", "phase")],
            vec![r],
            SortKey::new("STATUS", SortOrder::Asc),
        )
        .unwrap();
        assert!(
            t.unresolved_fields().is_empty(),
            "present-but-empty is resolved; only absent is unresolved"
        );
    }

    // ---- Selectable --------------------------------------------------------

    #[test]
    fn table_view_satisfies_selectable() {
        fn advance<S: Selectable>(s: &mut S) -> usize {
            s.select_next();
            s.selected_index()
        }
        let mut t = table(vec![row("a", "x"), row("b", "x"), row("c", "x")]);
        assert_eq!(advance(&mut t), 1);
        assert_eq!(Selectable::len(&t), 3);
        assert!(!Selectable::is_empty(&t));
    }

    #[test]
    fn selectable_and_inherent_selection_are_the_same_cursor() {
        let mut t = table(vec![row("a", "x"), row("b", "x")]);
        Selectable::select_next(&mut t);
        assert_eq!(TableView::selected_index(&t), 1);
        assert_eq!(Selectable::selected_index(&t), 1);
    }

    // ---- focus -------------------------------------------------------------

    #[test]
    fn starts_unfocused_and_focus_is_settable() {
        let mut t = table(vec![row("a", "x")]);
        assert!(!t.is_focused());
        t.set_focused(true);
        assert!(t.is_focused());
    }
}

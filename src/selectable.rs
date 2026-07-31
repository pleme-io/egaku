//! [`Selectable`] — the one selection vocabulary every index-selecting widget
//! speaks.
//!
//! # Why this exists
//!
//! Until this module landed, egaku had **zero traits**: eleven independent
//! structs, no shared vocabulary, so nothing generic could be written over
//! "a widget with a cursor". Every consumer that wanted `j`/`k` to drive
//! *whichever* pane is focused had to write a `match` over the concrete
//! widget types, and every new widget widened that `match` in every consumer.
//!
//! `Selectable` is the smallest honest generalization of what was already
//! there: [`ListView`](crate::ListView) already had this exact signature —
//! `selected_index` / `len` / `select_next` / `select_prev` — so the trait is
//! *discovered*, not invented. [`TableView`](crate::TableView) implements it
//! from birth for the same reason: banken's hand-rolled `PodTable`, written
//! independently, arrived at the same four methods verbatim. Two independent
//! derivations of one signature is the evidence that the abstraction is real.
//!
//! # The roster, and the two deliberate exclusions
//!
//! | Widget | `Selectable` | Note |
//! |---|---|---|
//! | [`ListView`](crate::ListView) | yes | literal signature match — the impl is pure forwarding |
//! | [`TabBar`](crate::TabBar) | yes | one alias: it spells the cursor `active_index` |
//! | [`TableView`](crate::TableView) | yes | implemented at birth |
//! | [`FuzzyPicker`](crate::FuzzyPicker) | **no — by design** | see below |
//! | [`TextInput`](crate::TextInput) | **no — category error** | see below |
//!
//! **[`FuzzyPicker`](crate::FuzzyPicker) is excluded deliberately.** It routes
//! *all* mutation through `on_event(PickerEvent) -> Vec<PickerEffect<T>>`: the
//! caller hands it an event and gets back the effects to apply, so every state
//! transition is observable and testable and there is exactly one way in.
//! Bolting `select_next` / `select_prev` onto it would re-open the direct
//! mutation bypass that design closed — a caller could move the cursor without
//! the picker emitting the effects its consumers rely on. The picker is not a
//! near-miss for this trait; it is a *different* contract that happens to have
//! a cursor. If a future consumer wants uniform cursor movement across a
//! picker and a list, the answer is a `PickerEvent`, not a `Selectable` impl.
//!
//! **[`TextInput`](crate::TextInput) is excluded because its cursor is a
//! CHARACTER index, not an ITEM index.** `selected_index() == 3` on a list
//! means "the fourth item"; on a text input it would mean "byte offset 3",
//! which is not even a character count. `len()` would answer in bytes while
//! every other implementor answers in items. Forcing it in would make the
//! trait's one invariant — `selected_index() < len()` indexes a selectable
//! thing — silently false for one implementor.

/// A widget with an item cursor: a selected index into a bounded set, moved
/// one step at a time.
///
/// # Invariant
///
/// `selected_index() < len()` whenever `len() > 0`. An implementor clamps on
/// every mutation rather than letting an out-of-range index exist; when
/// `len() == 0`, `selected_index()` reads `0` and indexes nothing.
///
/// # Movement is saturating or wrapping — the trait does not say which
///
/// [`ListView`](crate::ListView) saturates at both ends; [`TabBar`](crate::TabBar)
/// wraps. Both are correct for their widget, and a caller driving a focused
/// pane generically does not need to know. What the trait *does* promise is
/// that neither call can panic and neither can leave the cursor out of range.
pub trait Selectable {
    /// The currently selected index (`0` when empty).
    fn selected_index(&self) -> usize;

    /// The number of selectable items.
    fn len(&self) -> usize;

    /// Move the cursor one item forward.
    fn select_next(&mut self);

    /// Move the cursor one item backward.
    fn select_prev(&mut self);

    /// Whether there is nothing to select.
    ///
    /// Provided rather than required: it is `len() == 0` for every implementor
    /// egaku ships. (It is also what keeps `clippy::len_without_is_empty` quiet
    /// — a `len` with no `is_empty` beside it is a pedantic-lint failure, and
    /// every egaku widget already offers the inherent pair.)
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::Selectable;
    use crate::{ListView, TabBar};

    /// The point of the trait: one function drives any implementor.
    fn advance_twice<S: Selectable>(s: &mut S) -> usize {
        s.select_next();
        s.select_next();
        s.selected_index()
    }

    #[test]
    fn list_view_is_selectable_and_saturates() {
        let mut lv = ListView::new(vec!["a".into(), "b".into()], 5);
        assert_eq!(advance_twice(&mut lv), 1, "ListView saturates at the end");
        assert_eq!(Selectable::len(&lv), 2);
        assert!(!Selectable::is_empty(&lv));
    }

    #[test]
    fn tab_bar_is_selectable_and_wraps() {
        let mut tb = TabBar::new(vec!["a".into(), "b".into()]);
        assert_eq!(advance_twice(&mut tb), 0, "TabBar wraps around");
        assert_eq!(Selectable::len(&tb), 2);
    }

    #[test]
    fn tab_bar_selected_index_aliases_active_index() {
        let mut tb = TabBar::new(vec!["a".into(), "b".into(), "c".into()]);
        tb.select(2);
        assert_eq!(Selectable::selected_index(&tb), tb.active_index());
    }

    #[test]
    fn empty_implementors_report_empty_and_index_zero() {
        let lv = ListView::new(vec![], 5);
        let tb = TabBar::new(vec![]);
        assert!(Selectable::is_empty(&lv));
        assert!(Selectable::is_empty(&tb));
        assert_eq!(Selectable::selected_index(&lv), 0);
        assert_eq!(Selectable::selected_index(&tb), 0);
    }

    #[test]
    fn moving_an_empty_implementor_cannot_panic_or_go_out_of_range() {
        let mut lv = ListView::new(vec![], 5);
        let mut tb = TabBar::new(vec![]);
        for w in [&mut lv as &mut dyn Selectable, &mut tb] {
            w.select_next();
            w.select_prev();
            assert_eq!(w.selected_index(), 0);
        }
    }

    #[test]
    fn selected_index_stays_in_range_under_arbitrary_movement() {
        // The trait's one invariant, exercised through the trait only.
        let mut lv = ListView::new(vec!["a".into(), "b".into(), "c".into()], 2);
        let mut tb = TabBar::new(vec!["a".into(), "b".into(), "c".into()]);
        for w in [&mut lv as &mut dyn Selectable, &mut tb] {
            for step in 0..12 {
                if step % 3 == 0 {
                    w.select_prev();
                } else {
                    w.select_next();
                }
                assert!(
                    w.selected_index() < w.len(),
                    "cursor left the item range at step {step}"
                );
            }
        }
    }
}

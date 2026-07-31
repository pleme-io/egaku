use crate::selectable::Selectable;

/// Tab bar with wrap-around keyboard navigation.
#[derive(Debug, Clone)]
pub struct TabBar {
    tabs: Vec<String>,
    active: usize,
    focused: bool,
}

impl TabBar {
    #[must_use]
    pub fn new(tabs: Vec<String>) -> Self {
        Self {
            tabs,
            active: 0,
            focused: false,
        }
    }

    /// Select the next tab, wrapping around to the first.
    pub fn select_next(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    /// Select the previous tab, wrapping around to the last.
    pub fn select_prev(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
        }
    }

    /// Select a tab by index. No-op if out of bounds.
    pub fn select(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    /// Returns a reference to the active tab label.
    #[must_use]
    pub fn active_tab(&self) -> &str {
        &self.tabs[self.active]
    }

    /// Returns the active tab index.
    #[must_use]
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Add a tab at the end.
    pub fn add_tab(&mut self, name: String) {
        self.tabs.push(name);
    }

    /// Remove a tab by index. Adjusts active index if needed.
    /// Returns the removed tab label, or `None` if index is out of bounds.
    pub fn remove_tab(&mut self, index: usize) -> Option<String> {
        if index >= self.tabs.len() {
            return None;
        }
        let removed = self.tabs.remove(index);
        if !self.tabs.is_empty() && self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        Some(removed)
    }

    /// Returns all tab labels.
    #[must_use]
    pub fn tabs(&self) -> &[String] {
        &self.tabs
    }

    /// Returns the number of tabs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Returns true if there are no tabs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Set whether this tab bar currently has keyboard focus.
    ///
    /// See [`ListView::set_focused`](crate::ListView::set_focused) for why
    /// focus is widget-owned state and how it relates to
    /// [`FocusManager`](crate::FocusManager).
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether this tab bar currently has keyboard focus.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }
}

/// `TabBar` needs exactly one alias to satisfy [`Selectable`]: it spells its
/// cursor `active_index` because a *tab* is active rather than selected. The
/// name stays (it is the right word for a tab bar and it is public API); the
/// trait method forwards to it.
///
/// Note the movement semantics differ from [`ListView`](crate::ListView):
/// `TabBar` **wraps**, a list **saturates**. Both satisfy the trait — see the
/// `Selectable` docs on why movement direction is not part of the contract.
impl Selectable for TabBar {
    fn selected_index(&self) -> usize {
        TabBar::active_index(self)
    }

    fn len(&self) -> usize {
        TabBar::len(self)
    }

    fn select_next(&mut self) {
        TabBar::select_next(self);
    }

    fn select_prev(&mut self) {
        TabBar::select_prev(self);
    }

    fn is_empty(&self) -> bool {
        TabBar::is_empty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tabs() -> Vec<String> {
        vec!["Alpha".into(), "Beta".into(), "Gamma".into()]
    }

    #[test]
    fn new_starts_at_first() {
        let tb = TabBar::new(sample_tabs());
        assert_eq!(tb.active_index(), 0);
        assert_eq!(tb.active_tab(), "Alpha");
    }

    #[test]
    fn select_next_wraps() {
        let mut tb = TabBar::new(sample_tabs());
        tb.select_next(); // Beta
        tb.select_next(); // Gamma
        tb.select_next(); // wraps to Alpha
        assert_eq!(tb.active_tab(), "Alpha");
    }

    #[test]
    fn select_prev_wraps() {
        let mut tb = TabBar::new(sample_tabs());
        tb.select_prev(); // wraps to Gamma
        assert_eq!(tb.active_tab(), "Gamma");
    }

    #[test]
    fn select_by_index() {
        let mut tb = TabBar::new(sample_tabs());
        tb.select(2);
        assert_eq!(tb.active_tab(), "Gamma");
    }

    #[test]
    fn select_out_of_bounds_noop() {
        let mut tb = TabBar::new(sample_tabs());
        tb.select(99);
        assert_eq!(tb.active_index(), 0);
    }

    #[test]
    fn add_tab() {
        let mut tb = TabBar::new(sample_tabs());
        tb.add_tab("Delta".into());
        assert_eq!(tb.len(), 4);
        assert_eq!(tb.tabs()[3], "Delta");
    }

    #[test]
    fn remove_tab_adjusts_active() {
        let mut tb = TabBar::new(sample_tabs());
        tb.select(2); // Gamma
        let removed = tb.remove_tab(2);
        assert_eq!(removed, Some("Gamma".into()));
        assert_eq!(tb.active_index(), 1); // clamped
        assert_eq!(tb.active_tab(), "Beta");
    }

    #[test]
    fn remove_tab_out_of_bounds() {
        let mut tb = TabBar::new(sample_tabs());
        assert_eq!(tb.remove_tab(99), None);
    }

    #[test]
    fn remove_first_tab() {
        let mut tb = TabBar::new(sample_tabs());
        tb.remove_tab(0);
        assert_eq!(tb.active_tab(), "Beta");
        assert_eq!(tb.len(), 2);
    }

    #[test]
    fn starts_unfocused_and_focus_is_settable() {
        let mut tb = TabBar::new(sample_tabs());
        assert!(!tb.is_focused());
        tb.set_focused(true);
        assert!(tb.is_focused());
    }
}

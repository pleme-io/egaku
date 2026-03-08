/// Tab bar with wrap-around keyboard navigation.
#[derive(Debug, Clone)]
pub struct TabBar {
    tabs: Vec<String>,
    active: usize,
}

impl TabBar {
    #[must_use]
    pub fn new(tabs: Vec<String>) -> Self {
        Self { tabs, active: 0 }
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
}

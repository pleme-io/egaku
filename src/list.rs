/// Scrollable item list with keyboard selection.
#[derive(Debug, Clone)]
pub struct ListView {
    items: Vec<String>,
    selected: usize,
    offset: usize,
    visible_count: usize,
}

impl ListView {
    #[must_use]
    pub fn new(items: Vec<String>, visible_count: usize) -> Self {
        Self {
            items,
            selected: 0,
            offset: 0,
            visible_count,
        }
    }

    /// Move selection down by one, scrolling if needed.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
            self.ensure_visible();
        }
    }

    /// Move selection up by one, scrolling if needed.
    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }

    /// Jump to the first item.
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.ensure_visible();
    }

    /// Jump to the last item.
    pub fn select_last(&mut self) {
        if !self.items.is_empty() {
            self.selected = self.items.len() - 1;
            self.ensure_visible();
        }
    }

    /// Returns the currently selected item, if any.
    #[must_use]
    pub fn selected_item(&self) -> Option<&str> {
        self.items.get(self.selected).map(String::as_str)
    }

    /// Returns the slice of items currently visible in the viewport.
    #[must_use]
    pub fn visible_items(&self) -> &[String] {
        let end = (self.offset + self.visible_count).min(self.items.len());
        &self.items[self.offset..end]
    }

    /// Replace all items and reset selection to first.
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.selected = 0;
        self.offset = 0;
    }

    /// Returns the current selected index.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Returns the current scroll offset.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the total number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn ensure_visible(&mut self) {
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + self.visible_count {
            self.offset = self.selected + 1 - self.visible_count;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("item-{i}")).collect()
    }

    #[test]
    fn new_starts_at_first() {
        let lv = ListView::new(sample_items(5), 3);
        assert_eq!(lv.selected_index(), 0);
        assert_eq!(lv.selected_item(), Some("item-0"));
    }

    #[test]
    fn select_next() {
        let mut lv = ListView::new(sample_items(5), 3);
        lv.select_next();
        assert_eq!(lv.selected_index(), 1);
        lv.select_next();
        assert_eq!(lv.selected_index(), 2);
    }

    #[test]
    fn select_next_stops_at_end() {
        let mut lv = ListView::new(sample_items(3), 3);
        lv.select_next();
        lv.select_next();
        lv.select_next(); // already at last
        assert_eq!(lv.selected_index(), 2);
    }

    #[test]
    fn select_prev() {
        let mut lv = ListView::new(sample_items(5), 3);
        lv.select_next();
        lv.select_next();
        lv.select_prev();
        assert_eq!(lv.selected_index(), 1);
    }

    #[test]
    fn select_prev_stops_at_start() {
        let mut lv = ListView::new(sample_items(5), 3);
        lv.select_prev();
        assert_eq!(lv.selected_index(), 0);
    }

    #[test]
    fn select_first_and_last() {
        let mut lv = ListView::new(sample_items(10), 3);
        lv.select_last();
        assert_eq!(lv.selected_index(), 9);
        assert_eq!(lv.selected_item(), Some("item-9"));
        lv.select_first();
        assert_eq!(lv.selected_index(), 0);
    }

    #[test]
    fn visible_items_initial() {
        let lv = ListView::new(sample_items(10), 3);
        let vis = lv.visible_items();
        assert_eq!(vis.len(), 3);
        assert_eq!(vis[0], "item-0");
        assert_eq!(vis[2], "item-2");
    }

    #[test]
    fn visible_items_scrolled() {
        let mut lv = ListView::new(sample_items(10), 3);
        // Move selection past visible window
        for _ in 0..4 {
            lv.select_next();
        }
        // selected=4, offset should have scrolled
        let vis = lv.visible_items();
        assert_eq!(vis.len(), 3);
        assert!(vis.contains(&"item-4".to_string()));
    }

    #[test]
    fn set_items_resets() {
        let mut lv = ListView::new(sample_items(10), 3);
        lv.select_next();
        lv.select_next();
        lv.set_items(sample_items(5));
        assert_eq!(lv.selected_index(), 0);
        assert_eq!(lv.offset(), 0);
        assert_eq!(lv.len(), 5);
    }

    #[test]
    fn empty_list() {
        let lv = ListView::new(vec![], 3);
        assert!(lv.is_empty());
        assert_eq!(lv.selected_item(), None);
        assert!(lv.visible_items().is_empty());
    }

    #[test]
    fn select_next_on_empty() {
        let mut lv = ListView::new(vec![], 3);
        lv.select_next(); // should not panic
        assert_eq!(lv.selected_index(), 0);
    }

    #[test]
    fn fewer_items_than_visible_count() {
        let lv = ListView::new(sample_items(2), 5);
        assert_eq!(lv.visible_items().len(), 2);
    }
}

/// Tab-order focus traversal across widgets.
#[derive(Debug, Clone)]
pub struct FocusManager {
    widgets: Vec<String>,
    focused: usize,
}

impl FocusManager {
    #[must_use]
    pub fn new(widgets: Vec<String>) -> Self {
        Self { widgets, focused: 0 }
    }

    /// Advance focus to the next widget, wrapping around.
    pub fn focus_next(&mut self) {
        if !self.widgets.is_empty() {
            self.focused = (self.focused + 1) % self.widgets.len();
        }
    }

    /// Move focus to the previous widget, wrapping around.
    pub fn focus_prev(&mut self) {
        if !self.widgets.is_empty() {
            self.focused = (self.focused + self.widgets.len() - 1) % self.widgets.len();
        }
    }

    /// Returns the currently focused widget name.
    #[must_use]
    pub fn focused_widget(&self) -> &str {
        &self.widgets[self.focused]
    }

    /// Set focus to a widget by name. Returns true if found.
    pub fn set_focus(&mut self, name: &str) -> bool {
        if let Some(pos) = self.widgets.iter().position(|w| w == name) {
            self.focused = pos;
            true
        } else {
            false
        }
    }

    /// Register a new widget at the end of the focus chain.
    pub fn register(&mut self, name: String) {
        self.widgets.push(name);
    }

    /// Unregister a widget by name. Adjusts focused index if needed.
    pub fn unregister(&mut self, name: &str) {
        if let Some(pos) = self.widgets.iter().position(|w| w == name) {
            self.widgets.remove(pos);
            if !self.widgets.is_empty() && self.focused >= self.widgets.len() {
                self.focused = self.widgets.len() - 1;
            }
        }
    }

    /// Returns the number of registered widgets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    /// Returns true if no widgets are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_widgets() -> Vec<String> {
        vec!["input".into(), "list".into(), "button".into()]
    }

    #[test]
    fn new_focuses_first() {
        let fm = FocusManager::new(sample_widgets());
        assert_eq!(fm.focused_widget(), "input");
    }

    #[test]
    fn focus_next_wraps() {
        let mut fm = FocusManager::new(sample_widgets());
        fm.focus_next(); // list
        fm.focus_next(); // button
        fm.focus_next(); // wraps to input
        assert_eq!(fm.focused_widget(), "input");
    }

    #[test]
    fn focus_prev_wraps() {
        let mut fm = FocusManager::new(sample_widgets());
        fm.focus_prev(); // wraps to button
        assert_eq!(fm.focused_widget(), "button");
    }

    #[test]
    fn set_focus_found() {
        let mut fm = FocusManager::new(sample_widgets());
        assert!(fm.set_focus("button"));
        assert_eq!(fm.focused_widget(), "button");
    }

    #[test]
    fn set_focus_not_found() {
        let mut fm = FocusManager::new(sample_widgets());
        assert!(!fm.set_focus("nonexistent"));
        assert_eq!(fm.focused_widget(), "input"); // unchanged
    }

    #[test]
    fn register_widget() {
        let mut fm = FocusManager::new(sample_widgets());
        fm.register("slider".into());
        assert_eq!(fm.len(), 4);
        fm.set_focus("slider");
        assert_eq!(fm.focused_widget(), "slider");
    }

    #[test]
    fn unregister_focused_widget() {
        let mut fm = FocusManager::new(sample_widgets());
        fm.set_focus("button"); // index 2
        fm.unregister("button");
        assert_eq!(fm.len(), 2);
        // focused should clamp to len-1 = 1
        assert_eq!(fm.focused_widget(), "list");
    }

    #[test]
    fn unregister_non_focused() {
        let mut fm = FocusManager::new(sample_widgets());
        fm.unregister("list");
        assert_eq!(fm.len(), 2);
        assert_eq!(fm.focused_widget(), "input");
    }

    #[test]
    fn unregister_nonexistent() {
        let mut fm = FocusManager::new(sample_widgets());
        fm.unregister("nope");
        assert_eq!(fm.len(), 3);
    }
}

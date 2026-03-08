/// Modal dialog state.
#[derive(Debug, Clone)]
pub struct Modal {
    visible: bool,
    title: String,
}

impl Modal {
    #[must_use]
    pub fn new(title: &str) -> Self {
        Self {
            visible: false,
            title: title.to_string(),
        }
    }

    /// Show the modal.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the modal.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Returns whether the modal is currently visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns the modal title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_hidden() {
        let m = Modal::new("Confirm");
        assert!(!m.is_visible());
        assert_eq!(m.title(), "Confirm");
    }

    #[test]
    fn show_and_hide() {
        let mut m = Modal::new("Dialog");
        m.show();
        assert!(m.is_visible());
        m.hide();
        assert!(!m.is_visible());
    }

    #[test]
    fn show_idempotent() {
        let mut m = Modal::new("Test");
        m.show();
        m.show();
        assert!(m.is_visible());
    }

    #[test]
    fn hide_idempotent() {
        let mut m = Modal::new("Test");
        m.hide();
        assert!(!m.is_visible());
    }
}

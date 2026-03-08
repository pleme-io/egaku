use std::collections::HashMap;

/// A key combination (key name + modifier flags).
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct KeyCombo {
    pub key: String,
    pub modifiers: Vec<String>,
}

impl KeyCombo {
    #[must_use]
    pub fn new(key: &str, modifiers: Vec<String>) -> Self {
        let mut mods = modifiers;
        mods.sort();
        Self { key: key.to_string(), modifiers: mods }
    }

    /// Create a `KeyCombo` with no modifiers.
    #[must_use]
    pub fn key(key: &str) -> Self {
        Self { key: key.to_string(), modifiers: Vec::new() }
    }
}

/// Configurable keybinding system mapping key combinations to actions.
#[derive(Debug, Clone)]
pub struct KeyMap<A> {
    bindings: HashMap<KeyCombo, A>,
}

impl<A> KeyMap<A> {
    #[must_use]
    pub fn new() -> Self {
        Self { bindings: HashMap::new() }
    }

    /// Bind a key combination to an action.
    pub fn bind(&mut self, combo: KeyCombo, action: A) {
        self.bindings.insert(combo, action);
    }

    /// Look up the action for a key combination.
    #[must_use]
    pub fn lookup(&self, combo: &KeyCombo) -> Option<&A> {
        self.bindings.get(combo)
    }

    /// Remove a binding.
    pub fn unbind(&mut self, combo: &KeyCombo) {
        self.bindings.remove(combo);
    }

    /// Returns the number of bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Returns true if there are no bindings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl<A> Default for KeyMap<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    enum Action {
        Quit,
        Save,
        Copy,
        Paste,
    }

    #[test]
    fn bind_and_lookup() {
        let mut km = KeyMap::new();
        let combo = KeyCombo::key("q");
        km.bind(combo.clone(), Action::Quit);
        assert_eq!(km.lookup(&combo), Some(&Action::Quit));
    }

    #[test]
    fn lookup_missing() {
        let km: KeyMap<Action> = KeyMap::new();
        assert_eq!(km.lookup(&KeyCombo::key("x")), None);
    }

    #[test]
    fn bind_with_modifiers() {
        let mut km = KeyMap::new();
        let combo = KeyCombo::new("s", vec!["ctrl".into()]);
        km.bind(combo.clone(), Action::Save);
        assert_eq!(km.lookup(&combo), Some(&Action::Save));
    }

    #[test]
    fn modifier_order_normalized() {
        // Modifiers are sorted, so order of input doesn't matter
        let a = KeyCombo::new("c", vec!["shift".into(), "ctrl".into()]);
        let b = KeyCombo::new("c", vec!["ctrl".into(), "shift".into()]);
        assert_eq!(a, b);

        let mut km = KeyMap::new();
        km.bind(a, Action::Copy);
        assert_eq!(km.lookup(&b), Some(&Action::Copy));
    }

    #[test]
    fn unbind() {
        let mut km = KeyMap::new();
        let combo = KeyCombo::key("q");
        km.bind(combo.clone(), Action::Quit);
        assert_eq!(km.len(), 1);
        km.unbind(&combo);
        assert_eq!(km.lookup(&combo), None);
        assert!(km.is_empty());
    }

    #[test]
    fn overwrite_binding() {
        let mut km = KeyMap::new();
        let combo = KeyCombo::key("v");
        km.bind(combo.clone(), Action::Copy);
        km.bind(combo.clone(), Action::Paste);
        assert_eq!(km.lookup(&combo), Some(&Action::Paste));
        assert_eq!(km.len(), 1);
    }
}

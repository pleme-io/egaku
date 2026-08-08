use std::collections::HashMap;

/// A key combination (key name + modifier names), **canonicalised through
/// `awase`** so two spellings of one chord are one value.
///
/// # Why this canonicalises
///
/// `KeyCombo` is a `HashMap` key. It used to store whatever string it was
/// handed, which made every spelling difference a silent lookup miss — bind
/// one way, press the key, nothing happens, no error anywhere. Three parts of
/// the fleet found that independently and worked around it three ways:
///
/// - `ishou_tokens::fleet_keybinds` carries a written warning that its atlas
///   spells the key `"escape"` while `egaku-term` delivers `"esc"`, so a chord
///   taken from the atlas "silently never matches" — and says the translation
///   "belongs in one shared bridge, not per app".
/// - `egaku-term` has a test named
///   `the_spacebar_dispatches_on_the_typed_path_and_is_dead_on_the_string_path`,
///   pinning that a real spacebar press delivers the name `" "` so a `"space"`
///   binding is unreachable.
/// - Case (`"Ctrl"` vs `"ctrl"`) and duplicate modifiers (`["ctrl","ctrl"]`)
///   each produced a second, unmatchable value for the same chord.
///
/// `awase` is that shared bridge: it already owns the fleet's chord
/// vocabulary, resolves `esc`↔`escape` and `enter`↔`return` to one key, and
/// carries modifiers as a bitflag set where order and duplication cannot be
/// expressed. Canonicalising here fixes every consumer at once instead of
/// asking each to translate first.
///
/// # What this does not fix
///
/// A name `awase` does not know is kept verbatim (lowercased and trimmed)
/// rather than rejected, so nothing that works today stops working. `backtab`
/// is the one such name `egaku-term` emits — measured, not assumed. Those
/// keys keep the old exact-match behaviour.
///
/// **The destination is a `KeyMap` keyed on `awase::Hotkey` directly**, where
/// a misspelled key is a compile error rather than a canonicalised string.
/// `egaku-term` already built that typed path (`event::to_hotkey`). This is
/// the step that makes the existing string path correct meanwhile; it is
/// only-mitigated, not unrepresentable.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct KeyCombo {
    pub key: String,
    pub modifiers: Vec<String>,
}

impl KeyCombo {
    #[must_use]
    pub fn new(key: &str, modifiers: Vec<String>) -> Self {
        Self::canonical(key, &modifiers)
    }

    /// Create a `KeyCombo` with no modifiers.
    #[must_use]
    pub fn key(key: &str) -> Self {
        Self::canonical(key, &[])
    }

    /// Resolve `key` + `modifiers` to one canonical spelling.
    fn canonical(key: &str, modifiers: &[String]) -> Self {
        // A key that is entirely whitespace is the spacebar. It has to be
        // named before parsing, because a parser trims its input and would
        // see an empty string — which is exactly how a real spacebar press
        // came to be delivered as `" "` and match no binding anyone could
        // write.
        let raw = if !key.is_empty() && key.trim().is_empty() {
            "space"
        } else {
            key.trim()
        };

        // Ask awase to resolve the whole chord. Modifiers go in with it so
        // their names are canonicalised by the same authority as the key —
        // two passes would be two vocabularies again.
        let mut chord = String::new();
        for m in modifiers {
            chord.push_str(m.trim());
            chord.push('+');
        }
        chord.push_str(raw);

        match awase::Hotkey::parse_atlas_chord(&chord) {
            Ok(h) => Self {
                key: h.key.to_string().to_ascii_lowercase(),
                modifiers: modifier_names(h.modifiers),
            },
            // Unknown to awase — keep the old behaviour rather than dropping
            // the binding. Still lowercased, sorted and de-duplicated, so the
            // case and duplicate classes are closed even here.
            Err(_) => {
                let mut mods: Vec<String> =
                    modifiers.iter().map(|m| m.trim().to_ascii_lowercase()).collect();
                mods.sort();
                mods.dedup();
                Self { key: raw.to_ascii_lowercase(), modifiers: mods }
            }
        }
    }
}

/// `awase::Modifiers` → the sorted lowercase names `KeyCombo` stores.
///
/// Emitted in a fixed order from a bitflag set, so ordering and duplication
/// are gone by construction rather than by a `sort()` a caller might skip.
fn modifier_names(m: awase::Modifiers) -> Vec<String> {
    // `cmd` and `super` are one bit in awase; `egaku-term` delivers the name
    // `super`, so that is the spelling stored.
    [
        (awase::Modifiers::CTRL, "ctrl"),
        (awase::Modifiers::ALT, "alt"),
        (awase::Modifiers::SHIFT, "shift"),
        (awase::Modifiers::CMD, "super"),
        (awase::Modifiers::FN, "fn"),
        (awase::Modifiers::CAPS_LOCK, "caps_lock"),
    ]
    .into_iter()
    .filter(|(bit, _)| m.contains(*bit))
    .map(|(_, name)| name.to_string())
    .collect()
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

    /// The three silent misses this type used to have, each pinned.
    ///
    /// Every one of them produced `None` from a lookup with no error
    /// anywhere: you bind a key, press it, and nothing happens.
    mod one_chord_is_one_value {
        use super::*;

        #[test]
        fn case_does_not_split_a_chord() {
            // What a human writes in a binding table vs. what egaku-term's
            // crossterm adapter delivers.
            let mut km = KeyMap::new();
            km.bind(KeyCombo::new("s", vec!["Ctrl".into()]), Action::Save);
            assert_eq!(
                km.lookup(&KeyCombo::new("s", vec!["ctrl".into()])),
                Some(&Action::Save),
                "`Ctrl` and `ctrl` are one chord",
            );
        }

        #[test]
        fn a_repeated_modifier_does_not_split_a_chord() {
            let mut km = KeyMap::new();
            km.bind(
                KeyCombo::new("c", vec!["ctrl".into(), "ctrl".into()]),
                Action::Copy,
            );
            assert_eq!(
                km.lookup(&KeyCombo::new("c", vec!["ctrl".into()])),
                Some(&Action::Copy),
                "holding ctrl twice is holding ctrl",
            );
        }

        #[test]
        fn the_fleets_two_spellings_are_one_key() {
            // The trap `ishou_tokens::fleet_keybinds` documents: its atlas
            // says `escape`/`return`, egaku-term delivers `esc`/`enter`.
            assert_eq!(KeyCombo::key("escape"), KeyCombo::key("esc"));
            assert_eq!(KeyCombo::key("return"), KeyCombo::key("enter"));

            let mut km = KeyMap::new();
            km.bind(KeyCombo::key("escape"), Action::Quit);
            assert_eq!(
                km.lookup(&KeyCombo::key("esc")),
                Some(&Action::Quit),
                "a chord taken from the fleet atlas must reach a real press",
            );
        }

        #[test]
        fn the_spacebar_is_bindable() {
            // egaku-term pinned this as broken:
            // `the_spacebar_dispatches_on_the_typed_path_and_is_dead_on_the_string_path`
            // — a real press delivered the name `" "`, so the `space`
            // binding anyone would actually write was unreachable.
            let mut km = KeyMap::new();
            km.bind(KeyCombo::key("space"), Action::Paste);
            assert_eq!(
                km.lookup(&KeyCombo::key(" ")),
                Some(&Action::Paste),
                "a literal space is the spacebar",
            );
        }

        #[test]
        fn a_key_awase_does_not_know_still_works() {
            // The stated limit. `backtab` is outside awase's vocabulary and
            // must keep the old exact-match behaviour rather than being
            // dropped — a canonicalisation that silently lost a binding
            // would be the same defect wearing a different hat.
            let mut km = KeyMap::new();
            km.bind(KeyCombo::key("backtab"), Action::Quit);
            assert_eq!(km.lookup(&KeyCombo::key("backtab")), Some(&Action::Quit));
            // …and case is still closed on that path.
            assert_eq!(km.lookup(&KeyCombo::key("BackTab")), Some(&Action::Quit));
        }

        #[test]
        fn canonicalising_is_idempotent() {
            // Round-tripping a stored combo back through the constructor
            // must not move it, or a value read out of a map and rebuilt
            // would stop matching itself.
            for (k, mods) in [
                ("escape", vec![]),
                (" ", vec![]),
                ("s", vec!["Ctrl".to_string()]),
                ("backtab", vec!["shift".to_string()]),
            ] {
                let once = KeyCombo::new(k, mods.clone());
                let twice = KeyCombo::new(&once.key, once.modifiers.clone());
                assert_eq!(once, twice, "not idempotent for {k:?} + {mods:?}");
            }
        }
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

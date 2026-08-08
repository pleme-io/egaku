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
/// rather than rejected, so nothing that works today stops working. Measured
/// rather than assumed: the names `egaku-term` emits that `awase` cannot
/// parse are the ten shifted digits (`! @ # $ % ^ & * ( )`) and `f21`–`f35`.
/// All of them bind and look up correctly here and stay distinct from one
/// another — they simply take the verbatim path, where case and duplicate
/// modifiers are still normalised. `keys_outside_awases_vocabulary_still_round_trip`
/// pins that.
///
/// `backtab` WAS on that list and is no longer: it resolves to `shift+tab`,
/// because that is what the key is and what `egaku-term`'s typed path already
/// says it is.
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
        let trimmed = key.trim();
        let raw = if !key.is_empty() && trimmed.is_empty() {
            "space"
        } else if trimmed.eq_ignore_ascii_case("backtab") {
            // BackTab IS shift+tab. `egaku-term`'s TYPED path already says so
            // (`KeyCode::Tab | KeyCode::BackTab => AKey::Tab`, with the
            // modifier added separately, because crossterm reports the
            // composite as its own code and "does not always set SHIFT").
            // Its STRING path never got that treatment, so one physical press
            // produced `{backtab, []}` or `{backtab, ["shift"]}` depending on
            // the terminal — two unmatchable values for one keypress, which
            // is the class this whole function exists to close.
            "shift+tab"
        } else {
            trimmed
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
        fn one_shift_tab_press_is_one_value_however_the_terminal_reports_it() {
            // egaku-term's string path emits the name "backtab" and adds
            // "shift" only when crossterm sets it — which its own comment
            // says it does NOT always do. So the same physical press arrived
            // as two different values, neither reachable from the other.
            let bare = KeyCombo::key("backtab");
            let shifted = KeyCombo::new("backtab", vec!["shift".into()]);
            assert_eq!(bare, shifted, "both spellings are one keypress");

            // …and it is the same chord as an explicitly written shift+tab,
            // which is what BackTab MEANS and what the typed path resolves.
            assert_eq!(bare, KeyCombo::new("tab", vec!["shift".into()]));

            let mut km = KeyMap::new();
            km.bind(KeyCombo::key("backtab"), Action::Quit);
            assert_eq!(km.lookup(&shifted), Some(&Action::Quit));
            assert_eq!(
                km.lookup(&KeyCombo::new("tab", vec!["shift".into()])),
                Some(&Action::Quit),
            );
            // A plain Tab must NOT collide with it.
            assert_eq!(km.lookup(&KeyCombo::key("tab")), None);
        }

        #[test]
        fn keys_outside_awases_vocabulary_still_round_trip() {
            // The stated limit, measured rather than asserted. A recon pass
            // claimed 27 names were broken; those were measured against
            // `awase::Key::from_name`, which this path does not call. Through
            // `KeyCombo` they are kept verbatim and bind/look up correctly —
            // and, critically, stay DISTINCT from one another.
            let exotic = ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "f21", "f35"];
            let mut km = KeyMap::new();
            for (i, n) in exotic.iter().enumerate() {
                km.bind(KeyCombo::key(n), i);
            }
            assert_eq!(km.len(), exotic.len(), "no two collapsed into one");
            for (i, n) in exotic.iter().enumerate() {
                assert_eq!(km.lookup(&KeyCombo::key(n)), Some(&i), "{n} did not round-trip");
            }
        }

        #[test]
        fn a_key_awase_does_not_know_still_works() {
            // The stated limit. `backtab` is outside awase's vocabulary and
            // must keep the old exact-match behaviour rather than being
            // dropped — a canonicalisation that silently lost a binding
            // would be the same defect wearing a different hat.
            // `backtab` is no longer an example — it resolves to shift+tab
            // now. Use a name awase genuinely does not carry.
            let mut km = KeyMap::new();
            km.bind(KeyCombo::key("kp_begin"), Action::Quit);
            assert_eq!(km.lookup(&KeyCombo::key("kp_begin")), Some(&Action::Quit));
            // …and case is still closed on that path.
            assert_eq!(km.lookup(&KeyCombo::key("KP_Begin")), Some(&Action::Quit));
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

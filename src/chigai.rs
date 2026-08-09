//! `chigai` (違い) — compute the difference between two sequences.
//!
//! The fleet had **no diff computation at all**: measured 2026-08-03, zero
//! declarations of `similar`, `imara-diff`, `diffy` or `dissimilar` across
//! every `Cargo.toml` in the org. [`DiffView`](crate::DiffView) could *parse*
//! a unified diff someone else produced; nothing could produce one.
//!
//! # Why this is here and not a crate of its own
//!
//! It has exactly ONE consumer — the review UI, via [`DiffView::between`].
//! `magma-drift`, breathe's shadow pass and sui's reconcile loop do all
//! compute differences, but they compute **structured-state** differences
//! (resource attributes, band values) which is a different algorithm with
//! different semantics. Counting them as consumers of *this* primitive would
//! be the round-up the fleet's own naming rule warns about, so it is not
//! claimed.
//!
//! **Lift trigger, stated so it is not forgotten:** the moment a second
//! genuine consumer of *text-sequence* diff appears, this module becomes the
//! `chigai` crate. Until then, extracting it would be over-abstraction —
//! *"the test is whether the third use demonstrably reuses the same shape."*
//!
//! # The name
//!
//! `chigai` (違い) is simply *difference* — Law 2, the mnemonic law: the gloss
//! must let a reader guess the job, and "difference" does. Craft/Making 匠
//! (the Japanese-led substrate-tool family) is the home; its listed untaken
//! words (`kanna` plane, `nomi` chisel) are craft-authentic but teach nothing
//! about comparing, and transparency outranks family convenience.
//!
//! Three candidates died on the collision check, recorded so they are not
//! re-proposed: `sabun` (差分) is **taken on crates.io by a semantic-binary-diff
//! crate** — doubly wrong, since a reader searching would find the other one.
//! `kurabe` (比べ) collides with `kura`, `kurage` AND `kurayami`. `hikaku`
//! (比較) collides with `hikari` and the shipped `hikyaku`. Near-homophones
//! are not pedantry here: these names get typed at a shell prompt with
//! tab-completion.
//!
//! # The algorithm, and its honest ceiling
//!
//! Myers' O(ND) shortest-edit-script over a common prefix/suffix-trimmed
//! window. Myers is *optimal* — it finds a minimal edit script — which is the
//! property that makes a diff readable: a minimal script does not report a
//! line as changed when it merely moved.
//!
//! **The ceiling is real and bounded on purpose.** Myers is O(N·D) in time
//! and its classic formulation is O(N) space *per D-step*; a pathological
//! pair (two large files sharing almost nothing) costs O(N²). Rather than
//! degrade unpredictably on a 50k-line generated file, [`DiffOptions::max_cost`]
//! caps the search and the computation **falls back to a whole-block replace**
//! — a correct, non-minimal script. It never silently returns a wrong diff and
//! never hangs; [`Diff::is_minimal`] reports which happened, so a caller can
//! say "diff too large to minimise" instead of implying optimality it does not
//! have.

/// One edit operation over the input sequences.
///
/// Indices are into the ORIGINAL slices, so a caller can always recover the
/// text without carrying copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// `old[old_index]` and `new[new_index]` are equal.
    Equal { old_index: usize, new_index: usize },
    /// `old[old_index]` is absent from the new sequence.
    Delete { old_index: usize },
    /// `new[new_index]` is absent from the old sequence.
    Insert { new_index: usize },
}

impl Change {
    #[must_use]
    pub fn is_equal(&self) -> bool {
        matches!(self, Change::Equal { .. })
    }
}

/// Tuning for [`diff`]. Defaults are chosen for source files, not for
/// minified blobs.
#[derive(Debug, Clone, Copy)]
pub struct DiffOptions {
    /// Maximum edit distance to explore before giving up on minimality.
    ///
    /// Bounds the worst case. `None` means "always minimise", which is the
    /// right choice for small inputs and a liability for large unrelated
    /// ones.
    pub max_cost: Option<usize>,
}

impl Default for DiffOptions {
    /// 4096 edits. Comfortably above any human-authored change; low enough
    /// that two unrelated 50k-line files fall back in milliseconds instead of
    /// spending seconds proving they differ everywhere.
    fn default() -> Self {
        Self {
            max_cost: Some(4096),
        }
    }
}

/// The result of comparing two sequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    changes: Vec<Change>,
    minimal: bool,
}

impl Diff {
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// Whether the script is a *minimal* one.
    ///
    /// False when [`DiffOptions::max_cost`] was hit and the computation fell
    /// back to a block replace. A UI should say "too large to minimise"
    /// rather than present a non-minimal script as if it were optimal.
    #[must_use]
    pub fn is_minimal(&self) -> bool {
        self.minimal
    }

    /// True when the sequences are identical.
    #[must_use]
    pub fn is_empty_change(&self) -> bool {
        self.changes.iter().all(Change::is_equal)
    }

    /// `(inserted, deleted)`.
    #[must_use]
    pub fn stats(&self) -> (usize, usize) {
        let i = self
            .changes
            .iter()
            .filter(|c| matches!(c, Change::Insert { .. }))
            .count();
        let d = self
            .changes
            .iter()
            .filter(|c| matches!(c, Change::Delete { .. }))
            .count();
        (i, d)
    }
}

/// Compare two sequences of anything comparable.
///
/// Generic over `T: PartialEq` rather than hard-wired to `&str`: the same
/// engine diffs lines, words, or typed tokens, and a caller that wants
/// whitespace-insensitive comparison passes a pre-normalised slice instead of
/// asking for a flag.
#[must_use]
pub fn diff<T: PartialEq>(old: &[T], new: &[T], opts: DiffOptions) -> Diff {
    // Trim the common prefix and suffix first. This is not merely an
    // optimisation: it is what keeps the O(ND) search proportional to the
    // CHANGE rather than to the file, which is why a one-line edit in a
    // 10k-line file is instant.
    let mut pre = 0;
    while pre < old.len() && pre < new.len() && old[pre] == new[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < old.len() - pre
        && suf < new.len() - pre
        && old[old.len() - 1 - suf] == new[new.len() - 1 - suf]
    {
        suf += 1;
    }

    let mut changes: Vec<Change> = Vec::new();
    for i in 0..pre {
        changes.push(Change::Equal {
            old_index: i,
            new_index: i,
        });
    }

    let om = &old[pre..old.len() - suf];
    let nm = &new[pre..new.len() - suf];
    let (mid, minimal) = myers(om, nm, pre, opts.max_cost);
    changes.extend(mid);

    for k in 0..suf {
        changes.push(Change::Equal {
            old_index: old.len() - suf + k,
            new_index: new.len() - suf + k,
        });
    }
    Diff { changes, minimal }
}

/// Convenience: diff two texts by line. Returns the changes plus the split
/// line slices, so a caller can index straight into them.
#[must_use]
pub fn diff_lines<'a>(old: &'a str, new: &'a str) -> (Diff, Vec<&'a str>, Vec<&'a str>) {
    let o: Vec<&str> = old.lines().collect();
    let n: Vec<&str> = new.lines().collect();
    let d = diff(&o, &n, DiffOptions::default());
    (d, o, n)
}

/// Myers O(ND) over the trimmed middle. `offset` maps indices back onto the
/// untrimmed sequences.
///
/// Returns `(changes, minimal)`. When the cost bound is exceeded it emits a
/// whole-block replace — correct, not minimal — and says so.
fn myers<T: PartialEq>(
    old: &[T],
    new: &[T],
    offset: usize,
    max_cost: Option<usize>,
) -> (Vec<Change>, bool) {
    let n = old.len();
    let m = new.len();

    // Degenerate ends: nothing to search for.
    if n == 0 && m == 0 {
        return (Vec::new(), true);
    }
    if n == 0 {
        return (
            (0..m)
                .map(|j| Change::Insert {
                    new_index: offset + j,
                })
                .collect(),
            true,
        );
    }
    if m == 0 {
        return (
            (0..n)
                .map(|i| Change::Delete {
                    old_index: offset + i,
                })
                .collect(),
            true,
        );
    }

    let max = max_cost.unwrap_or(n + m).min(n + m);
    let vsize = 2 * max + 1;
    let center = max as isize;
    let mut v = vec![0usize; vsize];
    // One frontier snapshot per D — the trace Myers backtracks through.
    let mut trace: Vec<Vec<usize>> = Vec::with_capacity(max + 1);

    for d in 0..=max {
        trace.push(v.clone());
        let dd = d as isize;
        let mut k = -dd;
        while k <= dd {
            let idx = (center + k) as usize;
            // Choose the move that reaches furthest right.
            let mut x = if k == -dd || (k != dd && v[idx - 1] < v[idx + 1]) {
                v[idx + 1] // down = insertion
            } else {
                v[idx - 1] + 1 // right = deletion
            };
            let mut y = (x as isize - k) as usize;
            // Slide along the diagonal through equal elements.
            while x < n && y < m && old[x] == new[y] {
                x += 1;
                y += 1;
            }
            v[idx] = x;
            if x >= n && y >= m {
                return (backtrack(old, new, &trace, d, center, offset), true);
            }
            k += 2;
        }
    }

    // Cost bound hit. A whole-block replace is correct and obviously
    // non-minimal — which `is_minimal() == false` tells the caller.
    let mut out: Vec<Change> = (0..n)
        .map(|i| Change::Delete {
            old_index: offset + i,
        })
        .collect();
    out.extend((0..m).map(|j| Change::Insert {
        new_index: offset + j,
    }));
    (out, false)
}

/// Walk the recorded frontiers backwards to recover the edit script.
fn backtrack<T: PartialEq>(
    old: &[T],
    new: &[T],
    trace: &[Vec<usize>],
    d_final: usize,
    center: isize,
    offset: usize,
) -> Vec<Change> {
    let mut out: Vec<Change> = Vec::new();
    let mut x = old.len();
    let mut y = new.len();

    for d in (0..=d_final).rev() {
        let v = &trace[d];
        let k = x as isize - y as isize;
        let idx = (center + k) as usize;
        let dd = d as isize;

        let prev_k = if k == -dd || (k != dd && v[idx - 1] < v[idx + 1]) {
            k + 1
        } else {
            k - 1
        };
        let prev_idx = (center + prev_k) as usize;
        let prev_x = v[prev_idx];
        let prev_y = (prev_x as isize - prev_k) as usize;

        // Diagonal run first — these are the equal elements.
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            out.push(Change::Equal {
                old_index: offset + x,
                new_index: offset + y,
            });
        }
        if d > 0 {
            if x == prev_x {
                y -= 1;
                out.push(Change::Insert {
                    new_index: offset + y,
                });
            } else {
                x -= 1;
                out.push(Change::Delete {
                    old_index: offset + x,
                });
            }
        }
    }
    out.reverse();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<&str> {
        s.lines().collect()
    }

    /// Applying the script to `old` must reproduce `new`, exactly. This is
    /// the property that matters — a diff that looks plausible but does not
    /// reconstruct is worthless, and it is the one check that catches an
    /// off-by-one in the backtrack.
    fn assert_reconstructs(old: &str, new: &str) {
        let (d, o, n) = diff_lines(old, new);
        let mut got: Vec<&str> = Vec::new();
        for c in d.changes() {
            match *c {
                Change::Equal { new_index, .. } | Change::Insert { new_index } => {
                    got.push(n[new_index]);
                }
                Change::Delete { .. } => {}
            }
        }
        assert_eq!(
            got, n,
            "script did not reconstruct new from old ({old:?} → {new:?})"
        );

        // …and the delete/equal side must reconstruct `old`.
        let mut back: Vec<&str> = Vec::new();
        for c in d.changes() {
            match *c {
                Change::Equal { old_index, .. } | Change::Delete { old_index } => {
                    back.push(o[old_index]);
                }
                Change::Insert { .. } => {}
            }
        }
        assert_eq!(back, o, "script did not reconstruct old");
    }

    #[test]
    fn identical_sequences_are_all_equal() {
        let (d, _, _) = diff_lines("a\nb\nc", "a\nb\nc");
        assert!(d.is_empty_change());
        assert_eq!(d.stats(), (0, 0));
        assert!(d.is_minimal());
    }

    #[test]
    fn pure_insertion_and_pure_deletion() {
        let (ins, _, _) = diff_lines("a\nc", "a\nb\nc");
        assert_eq!(ins.stats(), (1, 0));
        let (del, _, _) = diff_lines("a\nb\nc", "a\nc");
        assert_eq!(del.stats(), (0, 1));
    }

    #[test]
    fn a_replacement_is_one_delete_and_one_insert() {
        let (d, _, _) = diff_lines("a\nX\nc", "a\nY\nc");
        assert_eq!(d.stats(), (1, 1));
    }

    /// Myers is optimal: a minimal script must not report a moved line as
    /// changed. Inserting one line into a 5-line file is exactly 1 edit.
    #[test]
    fn the_script_is_minimal_not_merely_correct() {
        let (d, _, _) = diff_lines("a\nb\nc\nd\ne", "a\nb\nX\nc\nd\ne");
        assert_eq!(d.stats(), (1, 0), "one insertion, not a block replace");
        assert!(d.is_minimal());
    }

    #[test]
    fn reconstruction_holds_across_shapes() {
        assert_reconstructs("", "");
        assert_reconstructs("a", "");
        assert_reconstructs("", "a");
        assert_reconstructs("a\nb\nc", "a\nb\nc");
        assert_reconstructs("a\nb\nc", "c\nb\na");
        assert_reconstructs("a\nb\nc\nd", "b\nd\ne");
        assert_reconstructs("the\nquick\nbrown\nfox", "the\nlazy\nbrown\ndog\n!");
        assert_reconstructs("x\nx\nx\nx", "x\nx");
        assert_reconstructs("1\n2\n3\n4\n5\n6\n7\n8\n9", "1\n9\n2\n8\n3\n7");
    }

    /// The prefix/suffix trim is what keeps a one-line edit in a large file
    /// proportional to the CHANGE, not the file.
    #[test]
    fn common_prefix_and_suffix_are_trimmed() {
        let old: String = (0..500).map(|i| format!("line{i}\n")).collect();
        let mut new_lines: Vec<String> = (0..500).map(|i| format!("line{i}")).collect();
        new_lines[250] = "CHANGED".into();
        let new = new_lines.join("\n");
        let (d, _, _) = diff_lines(&old, &new);
        assert_eq!(d.stats(), (1, 1), "one line changed, not 500");
        assert!(d.is_minimal());
    }

    /// The bounded ceiling, exercised: two large unrelated inputs must fall
    /// back rather than hang — and must SAY they fell back.
    #[test]
    fn exceeding_the_cost_bound_falls_back_and_admits_it() {
        let a: Vec<usize> = (0..400).collect();
        let b: Vec<usize> = (10_000..10_400).collect();
        let d = diff(&a, &b, DiffOptions { max_cost: Some(8) });
        assert!(!d.is_minimal(), "a bounded run must not claim minimality");
        assert_eq!(d.stats(), (400, 400), "correct, just not minimal");
    }

    /// Falling back must still reconstruct — non-minimal is allowed, wrong
    /// is not.
    #[test]
    fn the_fallback_script_is_still_correct() {
        let a: Vec<usize> = (0..50).collect();
        let b: Vec<usize> = (900..960).collect();
        let d = diff(&a, &b, DiffOptions { max_cost: Some(2) });
        let mut rebuilt: Vec<usize> = Vec::new();
        for c in d.changes() {
            match *c {
                Change::Equal { new_index, .. } | Change::Insert { new_index } => {
                    rebuilt.push(b[new_index]);
                }
                Change::Delete { .. } => {}
            }
        }
        assert_eq!(rebuilt, b);
    }

    /// Unbounded is available for callers who genuinely want optimality at
    /// any cost.
    #[test]
    fn max_cost_none_always_minimises() {
        let a: Vec<usize> = (0..60).collect();
        let b: Vec<usize> = (30..90).collect();
        let d = diff(&a, &b, DiffOptions { max_cost: None });
        assert!(d.is_minimal());
        assert_eq!(d.stats(), (30, 30));
    }

    /// Generic over T, not hard-wired to text — the property that lets the
    /// same engine diff words or typed tokens later.
    #[test]
    fn diffs_any_comparable_element() {
        #[derive(PartialEq, Debug)]
        struct Tok(u8);
        let a = [Tok(1), Tok(2), Tok(3)];
        let b = [Tok(1), Tok(9), Tok(3)];
        let d = diff(&a, &b, DiffOptions::default());
        assert_eq!(d.stats(), (1, 1));
    }

    #[test]
    fn indices_point_into_the_original_slices() {
        let (d, o, n) = diff_lines("keep\nold", "keep\nnew");
        for c in d.changes() {
            match *c {
                Change::Equal {
                    old_index,
                    new_index,
                } => {
                    assert_eq!(o[old_index], n[new_index]);
                }
                Change::Delete { old_index } => assert_eq!(o[old_index], "old"),
                Change::Insert { new_index } => assert_eq!(n[new_index], "new"),
            }
        }
    }

    #[test]
    fn empty_inputs_are_safe() {
        let e: [u8; 0] = [];
        assert!(diff(&e, &e, DiffOptions::default()).is_empty_change());
        assert_eq!(diff(&e, &[1u8, 2], DiffOptions::default()).stats(), (2, 0));
        assert_eq!(diff(&[1u8, 2], &e, DiffOptions::default()).stats(), (0, 2));
    }
}

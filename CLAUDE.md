# Egaku (描く) — GPU Widget Toolkit

> **★★★ CSE / Knowable Construction.** This repo operates under **Constructive Substrate Engineering** — canonical specification at [`pleme-io/theory/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md`](https://github.com/pleme-io/theory/blob/main/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md). The Compounding Directive (operational rules: solve once, load-bearing fixes only, idiom-first, models stay current, direction beats velocity) is in the org-level pleme-io/CLAUDE.md ★★★ section. Read both before non-trivial changes.


## Build & Test

```bash
cargo build
cargo test --lib
```

## Architecture

Reusable UI widget library for all pleme-io graphical applications. Sits between garasu (GPU rendering) and application-level UI code.

### Modules

| Module | Type | Purpose |
|--------|------|---------|
| `selectable.rs` | `Selectable` **(trait)** | The one shared vocabulary: `selected_index` / `len` / `select_next` / `select_prev` |
| `input.rs` | `TextInput` | Text editing with cursor, selection, backspace, delete |
| `scroll.rs` | `ScrollView`, `ScrollKinetics` | Virtualized scroll + fleet-shared momentum/inertia physics (unit-agnostic: lines/pixels/ticks) |
| `list.rs` | `ListView` | Scrollable item list with keyboard selection |
| `table.rs` | `TableView<R>`, `TableRow` **(trait)**, `Column`, `SortKey` | Columnar table: typed columns, identity-preserving refresh, header→field sort resolution |
| `tabs.rs` | `TabBar` | Tab container with wrap-around navigation |
| `split.rs` | `SplitPane` | Resizable H/V splits with min/max ratio |
| `modal.rs` | `Modal` | Centered overlay with visibility toggle |
| `focus.rs` | `FocusManager` | Tab-order focus traversal by widget ID |
| `keymap.rs` | `KeyMap` | Key combo → action string lookup |
| `layout.rs` | `Rect` | Layout primitives (contains, inset, split) |
| `theme.rs` | `Theme` | Colors, spacing, font config (serde, Nord defaults) |

### The two traits, and who is deliberately outside them

egaku had **zero traits** until 2026-07-31 — independent structs only, so
nothing generic could be written over "a widget with a cursor". There are now
exactly two, both discovered rather than invented:

- **`Selectable`** — implemented by `ListView`, `TabBar`, `TableView`.
  **Not** implemented by `FuzzyPicker` (all mutation routes through
  `on_event(PickerEvent) -> Vec<PickerEffect<T>>` by design; direct
  `select_next` would re-open the bypass that closed) or by `TextInput`
  (its cursor is a *character* index, not an item index — `len()` would
  answer in bytes while every other implementor answers in items). Both
  exclusions are argued in `selectable.rs`; do not "complete" the roster.
- **`TableRow`** — what a consumer supplies to get a `TableView`: an
  identity plus a `field -> Option<&str>` cell lookup. `None` means the row
  does not carry the field, and that is load-bearing: it is what makes
  `TableView::unresolved_fields()` able to tell *absent* from *present but
  empty*.

The **`Draw`** trait is NOT here — it lives in `egaku-term`, because a
drawer needs a `Buffer` and a `Palette` and egaku depends on no renderer.
That direction is one-way and load-bearing (see below).

### `TableView` provenance

Lifted 2026-07-31 from banken's hand-rolled `PodTable`, which carried an
open `pending-banken: promote-tableview-to-egaku` token naming this crate
as the destination. The generic ~two-thirds moved here parameterized over
`TableRow`; the pod-specific bindings (which columns, `ResourceKind`, the
`(defk8sview)` reader) stay in banken. Behaviour-preserving, with three
deliberate deltas documented in `table.rs`'s module header — chiefly that
the sort-column check now guards the *only* constructor, so a `TableView`
whose sort silently degenerates into "arbitrary order that looks sorted"
cannot be built.

### Layer Position

```
Application UI code
       ↓
    egaku (widgets, layout, focus, keybindings)
       ↓
    garasu (GPU context, text rendering, shaders)
       ↓
    wgpu + winit + glyphon
```

### Consumers

Used by: mado, hibiki, kagi, kekkai, fumi, nami

## Design Decisions

- **Dependency direction is ONE-WAY and load-bearing**: egaku depends on
  serde / tracing / thiserror / unicode-* and *nothing else* — no crossterm,
  no `egaku-term`, no GPU crate. `egaku-term` → `egaku`, never back. This is
  what lets one widget value drive a GPU pane and a TTY pane. Anything that
  needs a `Buffer`, a `Palette`, a `Color` or a surface belongs downstream;
  if a change here would pull a renderer in, the change is in the wrong crate.
- **Focus is widget-owned state, `FocusManager` is the authority**: each of
  `ListView` / `TextInput` / `TabBar` / `TableView` owns a `focused: bool`
  (`set_focused` / `is_focused`), mirroring the precedent `Modal` set by
  owning its own `show`/`hide`/`is_visible`. `FocusManager` still answers
  "who has focus" across a screen, by widget *name*; the per-widget flag is
  the projection of that answer, so drive it from
  `FocusManager::focused_widget()` rather than setting both independently.
  It exists because a uniform renderer (`egaku_term::Draw`) has one argument
  for the widget and none for an out-of-band flag.
- **Pure state, no rendering**: widgets are state machines; consumers call garasu to render them
- **No async**: all operations are synchronous; widgets don't own event loops
- **Serde on Theme**: themes can be loaded from config files via shikumi
- **Unicode-aware**: text input uses unicode-segmentation for correct cursor movement

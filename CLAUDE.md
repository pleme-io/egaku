# Egaku (描く) — GPU Widget Toolkit

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
| `input.rs` | `TextInput` | Text editing with cursor, selection, backspace, delete |
| `scroll.rs` | `ScrollView` | Virtualized scroll with visible range calculation |
| `list.rs` | `ListView` | Scrollable item list with keyboard selection |
| `tabs.rs` | `TabBar` | Tab container with wrap-around navigation |
| `split.rs` | `SplitPane` | Resizable H/V splits with min/max ratio |
| `modal.rs` | `Modal` | Centered overlay with visibility toggle |
| `focus.rs` | `FocusManager` | Tab-order focus traversal by widget ID |
| `keymap.rs` | `KeyMap` | Key combo → action string lookup |
| `layout.rs` | `Rect` | Layout primitives (contains, inset, split) |
| `theme.rs` | `Theme` | Colors, spacing, font config (serde, Nord defaults) |

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

- **Pure state, no rendering**: widgets are state machines; consumers call garasu to render them
- **No async**: all operations are synchronous; widgets don't own event loops
- **Serde on Theme**: themes can be loaded from config files via shikumi
- **Unicode-aware**: text input uses unicode-segmentation for correct cursor movement

# Egaku (描く)

GPU widget toolkit for pleme-io applications. Built on top of garasu, provides reusable UI primitives so every app shares the same interaction patterns.

## Widgets

| Widget | Purpose |
|--------|---------|
| `TextInput` | Single/multi-line text input with cursor, selection, clipboard |
| `ScrollView` | Virtualized scrollable container (handles thousands of items) |
| `ListView` | Scrollable item list with keyboard selection |
| `TabBar` | Tab container with keyboard navigation |
| `SplitPane` | Resizable horizontal/vertical splits |
| `Modal` | Overlay dialog with focus trap |
| `FocusManager` | Tab-order focus traversal |
| `KeyMap` | Configurable keybinding system |
| `Theme` | Color, spacing, typography theming (Nord defaults) |

## Usage

```toml
[dependencies]
egaku = { git = "https://github.com/pleme-io/egaku" }
```

```rust
use egaku::{TextInput, ListView, TabBar, Theme};

let mut input = TextInput::new();
input.insert_str("hello");

let theme = Theme::default(); // Nord palette
```

## Build

```bash
cargo build
cargo test --lib
```

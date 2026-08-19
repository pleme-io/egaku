//! Egaku (描く) — pure-logic widget toolkit for pleme-io applications.
//!
//! Provides reusable UI state machines that operate on abstract geometry
//! without requiring a GPU or windowing system:
//! - `TextInput`: single-line text input with cursor, selection, grapheme-aware editing
//! - `ScrollView`: scroll offset tracking with clamping and fraction
//! - `ScrollKinetics`: fleet-shared momentum/inertia scroll physics
//!   (unit-agnostic: lines, pixels, or scroll-ticks) + `ScrollKineticsConfig`
//! - `ListView`: scrollable item list with keyboard selection
//! - `TableView`: generic columnar table — typed columns, identity-preserving
//!   refresh, header→field sort resolution (rows implement `TableRow`)
//! - `TabBar`: tab container with wrap-around keyboard navigation
//! - `SplitPane`: resizable horizontal/vertical splits
//! - `Modal`: overlay dialog visibility state
//! - `FuzzyPicker`: generic modal fuzzy-picker (typed FSM) — session/tab
//!   switcher, command palette, fuzzy chooser; render-backend-agnostic
//! - `FocusManager`: tab-order focus traversal across widgets
//! - `KeyMap`: configurable keybinding system (generic over action type)
//! - `Rect` / `Padding`: layout geometry primitives
//! - `Theme`: color, spacing, and font configuration (serde, Nord defaults)
//!
//! ## The one trait
//!
//! Everything above is an independent struct except for one shared
//! vocabulary: [`Selectable`] — `selected_index` / `len` / `select_next` /
//! `select_prev` — implemented by [`ListView`], [`TabBar`] and [`TableView`],
//! and *deliberately not* by [`FuzzyPicker`] or [`TextInput`]. Read
//! [`selectable`] for why those two are excluded rather than forced.
//!
//! Rendering lives outside this crate — `garasu` for GPU, `egaku-term` for
//! terminals — and egaku depends on neither. That direction is one-way and
//! load-bearing: egaku's dependency set is serde / tracing / thiserror /
//! unicode-*, with no renderer and no backend, which is what lets one widget
//! state machine drive a GPU pane and a TTY pane from the same value.

pub mod chigai;
pub mod diff;
pub mod focus;
pub mod input;
pub mod keymap;
pub mod layout;
pub mod list;
pub mod modal;
pub mod picker;
pub mod scroll;
pub mod secret;
pub mod selectable;
pub mod split;
pub mod table;
pub mod tabs;
pub mod textarea;
pub mod textview;
pub mod theme;

pub use chigai::{Change, Diff, DiffOptions};
pub use diff::{DiffLine, DiffMode, DiffView, FileDiff, Hunk, LineKind, Row};
pub use focus::FocusManager;
pub use input::TextInput;
pub use keymap::{KeyCombo, KeyMap};
pub use layout::{Padding, Rect};
pub use list::ListView;
pub use modal::Modal;
pub use picker::{
    FuzzyPicker, PickerEffect, PickerEvent, PickerItem, PickerState, PickerStep, PickerView,
    QueryGuard,
};
pub use secret::SecretInput;
pub use scroll::{ScrollKinetics, ScrollKineticsConfig, ScrollView};
pub use selectable::Selectable;
pub use split::{Orientation, SplitPane};
pub use table::{Column, IDENTITY_FIELD, SortKey, SortOrder, TableError, TableRow, TableView};
pub use tabs::TabBar;
pub use textarea::TextArea;
pub use textview::{Span, TextView, WrappedLine};
pub use theme::Theme;

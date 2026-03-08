//! Egaku (描く) — pure-logic widget toolkit for pleme-io applications.
//!
//! Provides reusable UI state machines that operate on abstract geometry
//! without requiring a GPU or windowing system:
//! - `TextInput`: single-line text input with cursor, selection, grapheme-aware editing
//! - `ScrollView`: scroll offset tracking with clamping and fraction
//! - `ListView`: scrollable item list with keyboard selection
//! - `TabBar`: tab container with wrap-around keyboard navigation
//! - `SplitPane`: resizable horizontal/vertical splits
//! - `Modal`: overlay dialog visibility state
//! - `FocusManager`: tab-order focus traversal across widgets
//! - `KeyMap`: configurable keybinding system (generic over action type)
//! - `Rect` / `Padding`: layout geometry primitives
//! - `Theme`: color, spacing, and font configuration (serde, Nord defaults)

pub mod focus;
pub mod input;
pub mod keymap;
pub mod layout;
pub mod list;
pub mod modal;
pub mod scroll;
pub mod split;
pub mod tabs;
pub mod theme;

pub use focus::FocusManager;
pub use input::TextInput;
pub use keymap::{KeyCombo, KeyMap};
pub use layout::{Padding, Rect};
pub use list::ListView;
pub use modal::Modal;
pub use scroll::ScrollView;
pub use split::{Orientation, SplitPane};
pub use tabs::TabBar;
pub use theme::Theme;

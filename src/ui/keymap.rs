//! Explore keymap (PRD FR-KB-5), active while the treemap has focus. Esc stays
//! the window-wide Back and ⌘Y the window-wide Quick Look (root.rs).

use gpui::{App, KeyBinding, actions};

actions!(
    treemap,
    [
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        StageSelected,
        OpenSelected,
        GoUp,
    ]
);

pub const CONTEXT: &str = "Treemap";

/// `/` is left unbound: Phase 13 opens the palette with it.
pub fn bind_keys(cx: &mut App) {
    let ctx = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("h", SelectLeft, ctx),
        KeyBinding::new("left", SelectLeft, ctx),
        KeyBinding::new("l", SelectRight, ctx),
        KeyBinding::new("right", SelectRight, ctx),
        KeyBinding::new("k", SelectUp, ctx),
        KeyBinding::new("up", SelectUp, ctx),
        KeyBinding::new("j", SelectDown, ctx),
        KeyBinding::new("down", SelectDown, ctx),
        KeyBinding::new("space", StageSelected, ctx),
        KeyBinding::new("enter", OpenSelected, ctx),
        KeyBinding::new("backspace", GoUp, ctx),
        KeyBinding::new("cmd-up", GoUp, ctx),
    ]);
}

//! The canvas's middle-button pan and wheel zoom on macOS (RB-090-15; the Windows twin is
//! `../board_pan.rs`, fed by the UI thread's mouse hook). Not on macOS yet: the canvas pans and
//! zooms through its commands and shortcuts only.

/// The canvas's state changed: nothing to follow without the hook.
pub fn refresh() {}

//! Custom event type for Ruffle on Android

use ruffle_core::events::KeyDescriptor;

use crate::PlayerRunnable;

/// User-defined events.
pub enum RuffleEvent {
    /// Indicates that a task is ready to be polled.
    TaskPoll(PlayerRunnable),
    VirtualKeyEvent {
        down: bool,
        key_descriptor: KeyDescriptor,
    },
    RunContextMenuCallback(usize),
    ClearContextMenu,
    RequestContextMenu,
    /// The root movie was loaded and is about to start playing.
    RootMovieLoaded,
    /// Show (`true`) or hide (`false`) the soft keyboard.
    SoftKeyboard(bool),
    /// The user asked for the soft keyboard (toolbar button), e.g. to play with it.
    ShowSoftKeyboard,
    /// The soft keyboard was closed (e.g. by the user, with Back).
    SoftKeyboardHidden,
}

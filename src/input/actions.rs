//! Normalized actions produced by gamepad controls.

/// Actions that the B-Emu interface can consume independently of a controller model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadAction {
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    Confirm,
    Back,
    ToggleFavorite,
    MarkPlayed,
}

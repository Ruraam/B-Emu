//! Native gamepad input support.

pub mod actions;
pub mod gamepad;

pub use actions::GamepadAction;
pub use gamepad::{GamepadError, GamepadEvent, GamepadInfo, GamepadInput};

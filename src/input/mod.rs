//! Native gamepad input support.

pub mod actions;
pub mod gamepad;
pub mod navigation;
pub mod profiles;

pub use actions::GamepadAction;
pub use gamepad::{GamepadError, GamepadEvent, GamepadInfo, GamepadInput};

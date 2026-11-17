//! Linux gamepad discovery and event normalization through GilRs.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};

use super::{
    actions::GamepadAction,
    profiles::{ControllerProfile, format_guid, match_profile},
};

/// Error returned when the gamepad subsystem cannot be initialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamepadError {
    Initialization(String),
}

impl std::fmt::Display for GamepadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initialization(message) => {
                write!(f, "could not initialize the gamepad subsystem: {message}")
            }
        }
    }
}

impl std::error::Error for GamepadError {}

/// Information about a currently connected controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamepadInfo {
    pub id: usize,
    pub name: String,
}

/// Events exposed to the rest of B-Emu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamepadEvent {
    Connected(GamepadInfo),
    Disconnected { id: usize },
    Action { id: usize, action: GamepadAction },
}

/// Non-blocking gamepad event source.
///
/// Call poll_events regularly from the dedicated input worker. GilRs handles
/// Linux evdev discovery and hotplugging; this type translates its normalized
/// events into B-Emu actions.
pub struct GamepadInput {
    gilrs: Gilrs,
    axis_states: HashMap<GamepadId, AxisState>,
    profiles: Arc<RwLock<Vec<ControllerProfile>>>,
}

impl GamepadInput {
    pub fn new() -> Result<Self, GamepadError> {
        Self::new_with_profiles(Arc::new(RwLock::new(Vec::new())))
    }

    pub fn new_with_profiles(
        profiles: Arc<RwLock<Vec<ControllerProfile>>>,
    ) -> Result<Self, GamepadError> {
        let gilrs =
            Gilrs::new().map_err(|error| GamepadError::Initialization(format!("{error:?}")))?;

        Ok(Self {
            gilrs,
            axis_states: HashMap::new(),
            profiles,
        })
    }

    fn remap(&self, id: GamepadId, action: GamepadAction) -> GamepadAction {
        let gamepad = self.gilrs.gamepad(id);
        let name = gamepad.name().to_owned();
        let guid = format_guid(gamepad.uuid());
        let profiles = self
            .profiles
            .read()
            .unwrap_or_else(|error| error.into_inner());
        match_profile(&profiles, &name, &guid)
            .map(|profile| profile.apply(action))
            .unwrap_or(action)
    }

    /// Returns the controllers already connected when the subsystem was created.
    pub fn connected_gamepads(&self) -> Vec<GamepadInfo> {
        self.gilrs
            .gamepads()
            .map(|(id, gamepad)| GamepadInfo {
                id: id.into(),
                name: gamepad.name().to_owned(),
            })
            .collect()
    }

    /// Drains currently queued events without waiting for future input.
    pub fn poll_events(&mut self) -> Vec<GamepadEvent> {
        let mut translated = Vec::new();

        while let Some(event) = self.gilrs.next_event() {
            let id = event.id;
            let numeric_id = usize::from(id);

            match event.event {
                EventType::Connected => {
                    let name = self.gilrs.gamepad(id).name().to_owned();
                    self.axis_states.insert(id, AxisState::default());
                    translated.push(GamepadEvent::Connected(GamepadInfo {
                        id: numeric_id,
                        name,
                    }));
                }
                EventType::Disconnected => {
                    self.axis_states.remove(&id);
                    translated.push(GamepadEvent::Disconnected { id: numeric_id });
                }
                EventType::ButtonPressed(button, _) => {
                    if let Some(action) = button_action(button) {
                        translated.push(GamepadEvent::Action {
                            id: numeric_id,
                            action: self.remap(id, action),
                        });
                    }
                }
                EventType::AxisChanged(axis, value, _) => {
                    if let Some(action) =
                        self.axis_states.entry(id).or_default().update(axis, value)
                    {
                        translated.push(GamepadEvent::Action {
                            id: numeric_id,
                            action: self.remap(id, action),
                        });
                    }
                }
                _ => {}
            }
        }

        translated
    }
}

fn button_action(button: Button) -> Option<GamepadAction> {
    match button {
        Button::DPadUp => Some(GamepadAction::NavigateUp),
        Button::DPadDown => Some(GamepadAction::NavigateDown),
        Button::DPadLeft => Some(GamepadAction::NavigateLeft),
        Button::DPadRight => Some(GamepadAction::NavigateRight),
        Button::South => Some(GamepadAction::Confirm),
        Button::East => Some(GamepadAction::Back),
        Button::West => Some(GamepadAction::ToggleFavorite),
        Button::North => Some(GamepadAction::MarkPlayed),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum AxisDirection {
    #[default]
    Neutral,
    Negative,
    Positive,
}

#[derive(Debug, Default)]
struct AxisGate {
    direction: AxisDirection,
}

impl AxisGate {
    fn update(&mut self, value: f32) -> Option<AxisDirection> {
        if !value.is_finite() {
            return None;
        }

        let next = match self.direction {
            AxisDirection::Neutral if value <= -0.65 => AxisDirection::Negative,
            AxisDirection::Neutral if value >= 0.65 => AxisDirection::Positive,
            AxisDirection::Neutral => AxisDirection::Neutral,
            AxisDirection::Negative if value >= 0.65 => AxisDirection::Positive,
            AxisDirection::Negative if value > -0.35 => AxisDirection::Neutral,
            AxisDirection::Negative => AxisDirection::Negative,
            AxisDirection::Positive if value <= -0.65 => AxisDirection::Negative,
            AxisDirection::Positive if value < 0.35 => AxisDirection::Neutral,
            AxisDirection::Positive => AxisDirection::Positive,
        };

        if next == self.direction {
            return None;
        }

        self.direction = next;
        (next != AxisDirection::Neutral).then_some(next)
    }
}

#[derive(Debug, Default)]
struct AxisState {
    left_x: AxisGate,
    left_y: AxisGate,
    dpad_x: AxisGate,
    dpad_y: AxisGate,
}

impl AxisState {
    fn update(&mut self, axis: Axis, value: f32) -> Option<GamepadAction> {
        let (gate, negative, positive) = match axis {
            Axis::LeftStickX => (
                &mut self.left_x,
                GamepadAction::NavigateLeft,
                GamepadAction::NavigateRight,
            ),
            // GilRs normalizes stick Y so positive values point up.
            Axis::LeftStickY => (
                &mut self.left_y,
                GamepadAction::NavigateDown,
                GamepadAction::NavigateUp,
            ),
            Axis::DPadX => (
                &mut self.dpad_x,
                GamepadAction::NavigateLeft,
                GamepadAction::NavigateRight,
            ),
            // GilRs normalizes D-pad Y so positive values point up as well.
            Axis::DPadY => (
                &mut self.dpad_y,
                GamepadAction::NavigateDown,
                GamepadAction::NavigateUp,
            ),
            _ => return None,
        };

        match gate.update(value)? {
            AxisDirection::Negative => Some(negative),
            AxisDirection::Positive => Some(positive),
            AxisDirection::Neutral => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AxisDirection, AxisGate, AxisState, button_action};
    use crate::input::GamepadAction;
    use gilrs::{Axis, Button};

    #[test]
    fn xbox_style_face_buttons_map_to_confirm_and_back() {
        assert_eq!(button_action(Button::South), Some(GamepadAction::Confirm));
        assert_eq!(button_action(Button::East), Some(GamepadAction::Back));
    }

    #[test]
    fn dpad_buttons_map_to_navigation() {
        assert_eq!(
            button_action(Button::DPadUp),
            Some(GamepadAction::NavigateUp)
        );
        assert_eq!(
            button_action(Button::DPadDown),
            Some(GamepadAction::NavigateDown)
        );
        assert_eq!(
            button_action(Button::DPadLeft),
            Some(GamepadAction::NavigateLeft)
        );
        assert_eq!(
            button_action(Button::DPadRight),
            Some(GamepadAction::NavigateRight)
        );
    }

    #[test]
    fn face_buttons_map_to_library_actions() {
        assert_eq!(
            button_action(Button::West),
            Some(GamepadAction::ToggleFavorite)
        );
        assert_eq!(
            button_action(Button::North),
            Some(GamepadAction::MarkPlayed)
        );
    }

    #[test]
    fn axis_gate_uses_threshold_and_hysteresis() {
        let mut gate = AxisGate::default();

        assert_eq!(gate.update(-0.5), None);
        assert_eq!(gate.update(-0.7), Some(AxisDirection::Negative));
        assert_eq!(gate.update(-0.5), None);
        assert_eq!(gate.update(-0.2), None);
        assert_eq!(gate.update(0.7), Some(AxisDirection::Positive));
        assert_eq!(gate.update(0.5), None);
        assert_eq!(gate.update(0.2), None);
    }

    #[test]
    fn non_finite_axis_values_are_ignored() {
        let mut gate = AxisGate::default();
        assert_eq!(gate.update(f32::NAN), None);
        assert_eq!(gate.update(f32::INFINITY), None);
        assert_eq!(gate.update(-0.7), Some(AxisDirection::Negative));
        assert_eq!(gate.update(f32::NAN), None);
    }

    #[test]
    fn left_stick_axes_map_to_navigation() {
        let mut state = AxisState::default();

        assert_eq!(
            state.update(Axis::LeftStickY, 0.8),
            Some(GamepadAction::NavigateUp)
        );
        assert_eq!(
            state.update(Axis::LeftStickX, 0.8),
            Some(GamepadAction::NavigateRight)
        );
    }

    #[test]
    fn dpad_axes_map_to_navigation() {
        let mut state = AxisState::default();

        assert_eq!(
            state.update(Axis::DPadY, -0.8),
            Some(GamepadAction::NavigateDown)
        );
        assert_eq!(
            state.update(Axis::DPadX, -0.8),
            Some(GamepadAction::NavigateLeft)
        );
    }

    #[test]
    fn axis_neutral_transition_does_not_emit_an_action() {
        let mut state = AxisState::default();

        assert_eq!(
            state.update(Axis::LeftStickY, 0.8),
            Some(GamepadAction::NavigateUp)
        );
        assert_eq!(state.update(Axis::LeftStickY, 0.1), None);
    }
}

//! Linux gamepad discovery and event normalization through GilRs.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};

use super::actions::GamepadAction;

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
    repeater: RepeatScheduler,
}

impl GamepadInput {
    pub fn new() -> Result<Self, GamepadError> {
        let gilrs =
            Gilrs::new().map_err(|error| GamepadError::Initialization(format!("{error:?}")))?;

        Ok(Self {
            gilrs,
            axis_states: HashMap::new(),
            repeater: RepeatScheduler::default(),
        })
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
                    self.repeater.release_gamepad(numeric_id);
                    translated.push(GamepadEvent::Disconnected { id: numeric_id });
                }
                EventType::ButtonPressed(button, _) => {
                    if let Some(action) = button_action(button) {
                        translated.push(GamepadEvent::Action {
                            id: numeric_id,
                            action,
                        });
                        if is_repeatable_direction(action) {
                            self.repeater.press(numeric_id, action, Instant::now());
                        }
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(action) = button_action(button).filter(|a| is_repeatable_direction(*a)) {
                        self.repeater.release(numeric_id, action);
                    }
                }
                EventType::AxisChanged(axis, value, _) => {
                    let transitions = self.axis_states.entry(id).or_default().update(axis, value);
                    for (action, pressed) in transitions {
                        if pressed {
                            translated.push(GamepadEvent::Action { id: numeric_id, action });
                            self.repeater.press(numeric_id, action, Instant::now());
                        } else {
                            self.stop_repeat(id, action);
                        }
                    }
                }
                _ => {}
            }
        }

        translated.extend(
            self.repeater
                .due(Instant::now())
                .into_iter()
                .map(|(id, action)| GamepadEvent::Action { id, action }),
        );

        translated
    }
}

#[derive(Debug, Default)]
struct RepeatScheduler {
    next_repeat: HashMap<(usize, GamepadAction), Instant>,
}

impl RepeatScheduler {
    fn press(&mut self, id: usize, action: GamepadAction, now: Instant) {
        self.next_repeat
            .insert((id, action), now + REPEAT_DELAY);
    }

    fn release(&mut self, id: usize, action: GamepadAction) {
        self.next_repeat.remove(&(id, action));
    }

    fn release_gamepad(&mut self, id: usize) {
        self.next_repeat.retain(|(gamepad_id, _), _| *gamepad_id != id);
    }

    fn due(&mut self, now: Instant) -> Vec<(usize, GamepadAction)> {
        let mut actions = Vec::new();
        for (&key, next_at) in &mut self.next_repeat {
            if now >= *next_at {
                actions.push(key);
                *next_at = now + REPEAT_INTERVAL;
            }
        }
        actions
    }
}

const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_INTERVAL: Duration = Duration::from_millis(120);

fn is_repeatable_direction(action: GamepadAction) -> bool {
    matches!(
        action,
        GamepadAction::NavigateUp
            | GamepadAction::NavigateDown
            | GamepadAction::NavigateLeft
            | GamepadAction::NavigateRight
    )
}

fn button_action(button: Button) -> Option<GamepadAction> {
    match button {
        Button::DPadUp => Some(GamepadAction::NavigateUp),
        Button::DPadDown => Some(GamepadAction::NavigateDown),
        Button::DPadLeft => Some(GamepadAction::NavigateLeft),
        Button::DPadRight => Some(GamepadAction::NavigateRight),
        Button::South => Some(GamepadAction::Confirm),
        Button::East => Some(GamepadAction::Back),
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
    fn update(&mut self, axis: Axis, value: f32) -> Vec<(GamepadAction, bool)> {
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
            _ => return Vec::new(),
        };

        let previous = gate.direction;
        gate.update(value);
        let current = gate.direction;
        if previous == current {
            return Vec::new();
        }

        let action_for = |direction| match direction {
            AxisDirection::Negative => Some(negative),
            AxisDirection::Positive => Some(positive),
            AxisDirection::Neutral => None,
        };
        let mut transitions = Vec::with_capacity(2);
        if let Some(action) = action_for(previous) {
            transitions.push((action, false));
        }
        if let Some(action) = action_for(current) {
            transitions.push((action, true));
        }
        transitions
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
    fn only_navigation_directions_repeat() {
        assert!(is_repeatable_direction(GamepadAction::NavigateUp));
        assert!(is_repeatable_direction(GamepadAction::NavigateDown));
        assert!(is_repeatable_direction(GamepadAction::NavigateLeft));
        assert!(is_repeatable_direction(GamepadAction::NavigateRight));
        assert!(!is_repeatable_direction(GamepadAction::Confirm));
        assert!(!is_repeatable_direction(GamepadAction::Back));
        assert_eq!(REPEAT_DELAY, Duration::from_millis(400));
        assert_eq!(REPEAT_INTERVAL, Duration::from_millis(120));
    }

    #[test]
    fn held_direction_repeats_after_delay_then_at_interval_until_released() {
        use super::RepeatScheduler;
        use std::time::{Duration, Instant};

        let start = Instant::now();
        let mut repeater = RepeatScheduler::default();
        repeater.press(0, GamepadAction::NavigateUp, start);

        assert!(repeater.due(start + Duration::from_millis(399)).is_empty());
        assert_eq!(
            repeater.due(start + Duration::from_millis(400)),
            vec![(0, GamepadAction::NavigateUp)]
        );
        assert!(repeater.due(start + Duration::from_millis(519)).is_empty());
        assert_eq!(
            repeater.due(start + Duration::from_millis(520)),
            vec![(0, GamepadAction::NavigateUp)]
        );

        repeater.release(0, GamepadAction::NavigateUp);
        assert!(repeater.due(start + Duration::from_secs(5)).is_empty());
    }

    #[test]
    fn disconnect_clears_all_repeat_states_for_that_controller() {
        use super::RepeatScheduler;
        use std::time::{Duration, Instant};

        let start = Instant::now();
        let mut repeater = RepeatScheduler::default();
        repeater.press(0, GamepadAction::NavigateUp, start);
        repeater.press(1, GamepadAction::NavigateDown, start);
        repeater.release_gamepad(0);

        assert_eq!(
            repeater.due(start + Duration::from_millis(400)),
            vec![(1, GamepadAction::NavigateDown)]
        );
    }

    #[test]
    fn unrelated_buttons_are_ignored() {
        assert_eq!(button_action(Button::North), None);
        assert_eq!(button_action(Button::West), None);
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
            vec![(GamepadAction::NavigateUp, true)]
        );
        assert_eq!(
            state.update(Axis::LeftStickX, 0.8),
            vec![(GamepadAction::NavigateRight, true)]
        );
    }

    #[test]
    fn dpad_axes_map_to_navigation() {
        let mut state = AxisState::default();

        assert_eq!(
            state.update(Axis::DPadY, -0.8),
            vec![(GamepadAction::NavigateDown, true)]
        );
        assert_eq!(
            state.update(Axis::DPadX, -0.8),
            vec![(GamepadAction::NavigateLeft, true)]
        );
    }

    #[test]
    fn axis_neutral_transition_does_not_emit_an_action() {
        let mut state = AxisState::default();

        assert_eq!(
            state.update(Axis::LeftStickY, 0.8),
            vec![(GamepadAction::NavigateUp, true)]
        );
        assert_eq!(
            state.update(Axis::LeftStickY, 0.1),
            vec![(GamepadAction::NavigateUp, false)]
        );
    }
}

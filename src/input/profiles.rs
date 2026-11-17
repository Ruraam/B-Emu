//! Per-controller action profiles.
//!
//! Profiles are data only: they remap normalized actions for controllers whose
//! button layout differs from the default mapping. They match a controller by
//! device name substring or by 16-byte SDL-style GUID.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::GamepadAction;

impl GamepadAction {
    pub fn as_key(self) -> &'static str {
        match self {
            Self::NavigateUp => "up",
            Self::NavigateDown => "down",
            Self::NavigateLeft => "left",
            Self::NavigateRight => "right",
            Self::Confirm => "confirm",
            Self::Back => "back",
            Self::ToggleFavorite => "favorite",
            Self::MarkPlayed => "played",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "up" => Some(Self::NavigateUp),
            "down" => Some(Self::NavigateDown),
            "left" => Some(Self::NavigateLeft),
            "right" => Some(Self::NavigateRight),
            "confirm" => Some(Self::Confirm),
            "back" => Some(Self::Back),
            "favorite" => Some(Self::ToggleFavorite),
            "played" => Some(Self::MarkPlayed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub device_name_contains: Option<String>,
    #[serde(default)]
    pub guid: Option<String>,
    /// Map from the default action key to the desired action key.
    #[serde(default)]
    pub remap: BTreeMap<String, String>,
}

impl ControllerProfile {
    pub fn matches(&self, device_name: &str, guid_hex: &str) -> bool {
        let name_match = self
            .device_name_contains
            .as_ref()
            .is_some_and(|needle| device_name.to_lowercase().contains(&needle.to_lowercase()));
        let guid_match = self
            .guid
            .as_ref()
            .is_some_and(|expected| expected.eq_ignore_ascii_case(guid_hex));

        match (self.guid.is_some(), self.device_name_contains.is_some()) {
            (true, true) => name_match && guid_match,
            (true, false) => guid_match,
            (false, true) => name_match,
            (false, false) => false,
        }
    }

    pub fn apply(&self, action: GamepadAction) -> GamepadAction {
        self.remap
            .get(action.as_key())
            .and_then(|key| GamepadAction::from_key(key))
            .unwrap_or(action)
    }
}

pub fn match_profile<'a>(
    profiles: &'a [ControllerProfile],
    device_name: &str,
    guid_hex: &str,
) -> Option<&'a ControllerProfile> {
    profiles
        .iter()
        .find(|profile| profile.matches(device_name, guid_hex))
}

pub fn format_guid(uuid: [u8; 16]) -> String {
    let mut text = String::with_capacity(32);
    for byte in uuid {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{ControllerProfile, format_guid, match_profile};
    use crate::input::GamepadAction;
    use std::collections::BTreeMap;

    #[test]
    fn action_keys_roundtrip() {
        for action in [
            GamepadAction::NavigateUp,
            GamepadAction::NavigateDown,
            GamepadAction::NavigateLeft,
            GamepadAction::NavigateRight,
            GamepadAction::Confirm,
            GamepadAction::Back,
            GamepadAction::ToggleFavorite,
            GamepadAction::MarkPlayed,
        ] {
            assert_eq!(GamepadAction::from_key(action.as_key()), Some(action));
        }
        assert_eq!(GamepadAction::from_key("unknown"), None);
    }

    #[test]
    fn matches_by_name_or_guid() {
        let profile = ControllerProfile {
            id: "inverted".into(),
            name: "Inverted A/B".into(),
            device_name_contains: Some("GPIO Controller 2".into()),
            guid: None,
            remap: BTreeMap::new(),
        };
        assert!(profile.matches("GPIO Controller 2", "anything"));
        assert!(!profile.matches("Xbox 360 Controller", "anything"));

        let by_guid = ControllerProfile {
            guid: Some("ABCDEF0123456789".into()),
            device_name_contains: None,
            ..profile
        };
        assert!(by_guid.matches("anything", "abcdef0123456789"));
    }

    #[test]
    fn remaps_only_declared_actions() {
        let profile = ControllerProfile {
            id: "inverted".into(),
            name: "Inverted A/B".into(),
            device_name_contains: None,
            guid: None,
            remap: BTreeMap::from([
                ("confirm".into(), "back".into()),
                ("back".into(), "confirm".into()),
            ]),
        };
        assert_eq!(profile.apply(GamepadAction::Confirm), GamepadAction::Back);
        assert_eq!(profile.apply(GamepadAction::Back), GamepadAction::Confirm);
        assert_eq!(
            profile.apply(GamepadAction::NavigateUp),
            GamepadAction::NavigateUp
        );
    }

    #[test]
    fn first_matching_profile_wins() {
        let first = ControllerProfile {
            id: "first".into(),
            name: "First".into(),
            device_name_contains: Some("pad".into()),
            guid: None,
            remap: BTreeMap::new(),
        };
        let second = ControllerProfile {
            id: "second".into(),
            name: "Second".into(),
            ..first.clone()
        };
        assert_eq!(
            match_profile(&[first, second], "Gamepad", "").unwrap().id,
            "first"
        );
    }

    #[test]
    fn formats_guid_as_lowercase_hex() {
        assert_eq!(format_guid([0xab; 16]), "abababababababababababababababab");
    }
}

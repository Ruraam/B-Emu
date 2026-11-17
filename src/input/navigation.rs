//! Pure navigation-state transitions shared by input backends.

use super::GamepadAction;

/// Default page count used when no UI state is available (e.g. in unit tests).
/// The running application passes the real count from its Slint page model.
pub const DEFAULT_PAGE_COUNT: i32 = 5;

/// Which part of the shell currently owns keyboard/gamepad focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusZone {
    Sidebar,
    Content,
}

/// Complete navigation state of the shell.
///
/// This is deliberately independent from Slint so transitions can be tested
/// without creating a window. The worker reads the current state from the UI,
/// applies one action, and writes the result back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Navigation {
    pub focused_page: i32,
    pub active_page: i32,
    pub page_count: i32,
    pub zone: FocusZone,
    pub focused_item: i32,
    pub selected_item: i32,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            focused_page: 0,
            active_page: 0,
            page_count: DEFAULT_PAGE_COUNT,
            zone: FocusZone::Sidebar,
            focused_item: 0,
            selected_item: -1,
        }
    }
}

fn wrap(value: i32, count: i32) -> i32 {
    if count <= 0 {
        0
    } else {
        value.rem_euclid(count)
    }
}

/// Applies a normalized controller action to the shell navigation state.
///
/// `item_count` is the number of focusable items in the active screen. It is
/// provided by the UI because each screen owns its own model.
pub fn apply_action(state: Navigation, action: GamepadAction, item_count: i32) -> Navigation {
    // Normalize page indices once so every transition sees valid values.
    let state = Navigation {
        focused_page: wrap(state.focused_page, state.page_count),
        active_page: wrap(state.active_page, state.page_count),
        ..state
    };

    match state.zone {
        FocusZone::Sidebar => match action {
            GamepadAction::NavigateUp => Navigation {
                focused_page: wrap(state.focused_page - 1, state.page_count),
                ..state
            },
            GamepadAction::NavigateDown => Navigation {
                focused_page: wrap(state.focused_page + 1, state.page_count),
                ..state
            },
            GamepadAction::Confirm => Navigation {
                active_page: state.focused_page,
                zone: FocusZone::Content,
                focused_item: 0,
                selected_item: -1,
                ..state
            },
            GamepadAction::Back => Navigation {
                focused_page: 0,
                active_page: 0,
                focused_item: 0,
                selected_item: -1,
                ..state
            },
            GamepadAction::NavigateLeft
            | GamepadAction::NavigateRight
            | GamepadAction::ToggleFavorite
            | GamepadAction::MarkPlayed => state,
        },
        FocusZone::Content => match action {
            GamepadAction::NavigateUp => Navigation {
                focused_item: wrap(state.focused_item - 1, item_count),
                ..state
            },
            GamepadAction::NavigateDown => Navigation {
                focused_item: wrap(state.focused_item + 1, item_count),
                ..state
            },
            GamepadAction::Confirm => Navigation {
                selected_item: state.focused_item,
                ..state
            },
            GamepadAction::Back => Navigation {
                zone: FocusZone::Sidebar,
                focused_item: 0,
                selected_item: -1,
                ..state
            },
            GamepadAction::NavigateLeft
            | GamepadAction::NavigateRight
            | GamepadAction::ToggleFavorite
            | GamepadAction::MarkPlayed => state,
        },
    }
}

/// Sidebar-only transition kept as a small convenience wrapper.
pub fn apply_sidebar_action(
    focused_page: i32,
    active_page: i32,
    action: GamepadAction,
) -> (i32, i32) {
    let state = Navigation {
        focused_page,
        active_page,
        ..Navigation::default()
    };
    let next = apply_action(state, action, 0);
    (next.focused_page, next.active_page)
}

#[cfg(test)]
mod tests {
    use super::{FocusZone, Navigation, apply_action, apply_sidebar_action};
    use crate::input::GamepadAction;

    #[test]
    fn vertical_navigation_wraps_around() {
        assert_eq!(
            apply_sidebar_action(0, 2, GamepadAction::NavigateUp),
            (4, 2)
        );
        assert_eq!(
            apply_sidebar_action(4, 2, GamepadAction::NavigateDown),
            (0, 2)
        );
    }

    #[test]
    fn confirm_activates_the_focused_page() {
        assert_eq!(apply_sidebar_action(3, 1, GamepadAction::Confirm), (3, 3));
    }

    #[test]
    fn back_returns_to_home() {
        assert_eq!(apply_sidebar_action(4, 3, GamepadAction::Back), (0, 0));
    }

    #[test]
    fn horizontal_actions_do_not_change_sidebar_state() {
        assert_eq!(
            apply_sidebar_action(2, 1, GamepadAction::NavigateLeft),
            (2, 1)
        );
        assert_eq!(
            apply_sidebar_action(2, 1, GamepadAction::NavigateRight),
            (2, 1)
        );
    }

    #[test]
    fn invalid_page_indices_are_normalized() {
        assert_eq!(apply_sidebar_action(-1, 7, GamepadAction::Confirm), (4, 4));
    }

    #[test]
    fn page_count_comes_from_the_state() {
        let state = Navigation {
            focused_page: 0,
            page_count: 3,
            ..Navigation::default()
        };
        assert_eq!(
            apply_action(state, GamepadAction::NavigateUp, 0).focused_page,
            2
        );
        assert_eq!(
            apply_action(state, GamepadAction::NavigateDown, 0).focused_page,
            1
        );
    }

    #[test]
    fn confirm_moves_focus_into_the_content() {
        let state = Navigation {
            focused_page: 2,
            ..Navigation::default()
        };
        let next = apply_action(state, GamepadAction::Confirm, 4);
        assert_eq!(next.active_page, 2);
        assert_eq!(next.zone, FocusZone::Content);
        assert_eq!(next.focused_item, 0);
    }

    #[test]
    fn content_navigation_wraps_within_the_screen_item_count() {
        let state = Navigation {
            zone: FocusZone::Content,
            focused_item: 0,
            ..Navigation::default()
        };
        assert_eq!(
            apply_action(state, GamepadAction::NavigateUp, 3).focused_item,
            2
        );
        assert_eq!(
            apply_action(state, GamepadAction::NavigateDown, 3).focused_item,
            1
        );
    }

    #[test]
    fn content_confirm_selects_the_focused_item() {
        let state = Navigation {
            zone: FocusZone::Content,
            focused_item: 2,
            ..Navigation::default()
        };
        let next = apply_action(state, GamepadAction::Confirm, 4);
        assert_eq!(next.selected_item, 2);
    }

    #[test]
    fn content_back_returns_focus_to_the_sidebar() {
        let state = Navigation {
            active_page: 3,
            zone: FocusZone::Content,
            focused_item: 2,
            selected_item: 2,
            ..Navigation::default()
        };
        let next = apply_action(state, GamepadAction::Back, 4);
        assert_eq!(next.zone, FocusZone::Sidebar);
        assert_eq!(next.active_page, 3);
        assert_eq!(next.selected_item, -1);
    }

    #[test]
    fn empty_screens_keep_item_index_at_zero() {
        let state = Navigation {
            zone: FocusZone::Content,
            focused_item: 0,
            ..Navigation::default()
        };
        assert_eq!(
            apply_action(state, GamepadAction::NavigateDown, 0).focused_item,
            0
        );
    }
}

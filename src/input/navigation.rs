//! Pure navigation-state transitions shared by input backends.

use super::GamepadAction;

/// Applies a normalized controller action to the five-item application sidebar.
///
/// The function is deliberately independent from Slint so its wraparound and
/// activation behavior can be tested without creating a window.
pub fn apply_sidebar_action(
    focused_page: i32,
    active_page: i32,
    action: GamepadAction,
) -> (i32, i32) {
    let focused_page = focused_page.rem_euclid(5);
    let active_page = active_page.rem_euclid(5);

    match action {
        GamepadAction::NavigateUp => ((focused_page + 4) % 5, active_page),
        GamepadAction::NavigateDown => ((focused_page + 1) % 5, active_page),
        GamepadAction::Confirm => (focused_page, focused_page),
        GamepadAction::Back => (0, 0),
        GamepadAction::NavigateLeft | GamepadAction::NavigateRight => {
            (focused_page, active_page)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_sidebar_action;
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
        assert_eq!(
            apply_sidebar_action(3, 1, GamepadAction::Confirm),
            (3, 3)
        );
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
        assert_eq!(
            apply_sidebar_action(-1, 7, GamepadAction::Confirm),
            (4, 4)
        );
    }
}

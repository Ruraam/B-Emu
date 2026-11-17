//! Background gamepad polling and safe delivery to the Slint event loop.

use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use b_emu::input::{
    GamepadAction, GamepadEvent, GamepadInput, diagnostics, profiles::ControllerProfile,
};

use crate::{MainWindow, NavAction};

/// Owns the controller polling thread and stops it when the application exits.
pub struct GamepadWorker {
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl GamepadWorker {
    /// Starts polling outside the UI thread.
    ///
    /// Failure to spawn the thread is returned to the caller. Failure to
    /// initialize the optional gamepad subsystem is logged by the worker and
    /// does not prevent keyboard/mouse use of the application.
    pub fn start(
        window: slint::Weak<MainWindow>,
        profiles: Arc<RwLock<Vec<ControllerProfile>>>,
        vibration: Arc<AtomicBool>,
    ) -> std::io::Result<Self> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("b-emu-gamepad".into())
            .spawn(move || run_worker(window, stop_rx, profiles, vibration))?;

        Ok(Self {
            stop: Some(stop_tx),
            thread: Some(thread),
        })
    }
}

impl Drop for GamepadWorker {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }

        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                eprintln!("[b-emu] gamepad worker terminated unexpectedly");
            }
        }
    }
}

fn nav_action(action: GamepadAction) -> NavAction {
    match action {
        GamepadAction::NavigateUp => NavAction::Up,
        GamepadAction::NavigateDown => NavAction::Down,
        GamepadAction::NavigateLeft => NavAction::Left,
        GamepadAction::NavigateRight => NavAction::Right,
        GamepadAction::Confirm => NavAction::Confirm,
        GamepadAction::Back => NavAction::Back,
        GamepadAction::ToggleFavorite => NavAction::Favorite,
        GamepadAction::MarkPlayed => NavAction::Played,
    }
}

/// Tracks which controller owns navigation. The first connected controller
/// wins; another one is only adopted after it disconnects. This prevents two
/// controllers from fighting over the same focus state.
#[derive(Debug, Default)]
struct ActiveController(Option<usize>);

impl ActiveController {
    fn observe_connected(&mut self, id: usize) {
        if self.0.is_none() {
            self.0 = Some(id);
        }
    }

    fn observe_disconnected(&mut self, id: usize) {
        if self.0 == Some(id) {
            self.0 = None;
        }
    }

    fn accepts(&mut self, id: usize) -> bool {
        if self.0.is_none() {
            self.0 = Some(id);
        }
        self.0 == Some(id)
    }
}

fn run_worker(
    window: slint::Weak<MainWindow>,
    stop_rx: Receiver<()>,
    profiles: Arc<RwLock<Vec<ControllerProfile>>>,
    vibration: Arc<AtomicBool>,
) {
    let mut input = match GamepadInput::new_with_profiles(profiles) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("[b-emu] gamepad support unavailable: {error}");
            diagnostics::log_input_diagnostic();
            return;
        }
    };

    let mut active = ActiveController::default();
    for gamepad in input.connected_gamepads() {
        eprintln!(
            "[b-emu] controller connected: {} (id {})",
            gamepad.name, gamepad.id
        );
        active.observe_connected(gamepad.id);
    }

    loop {
        match stop_rx.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        let mut actions = Vec::new();
        let mut last_action_id = None;

        for event in input.poll_events() {
            match event {
                GamepadEvent::Connected(gamepad) => {
                    eprintln!(
                        "[b-emu] controller connected: {} (id {})",
                        gamepad.name, gamepad.id
                    );
                    active.observe_connected(gamepad.id);
                }
                GamepadEvent::Disconnected { id } => {
                    eprintln!("[b-emu] controller disconnected (id {id})");
                    active.observe_disconnected(id);
                }
                GamepadEvent::Action { id, action } => {
                    if active.accepts(id) {
                        last_action_id = Some(id);
                        actions.push(nav_action(action));
                    }
                }
            }
        }

        // Queue at most one UI callback per polling batch. This preserves
        // action order while preventing rapid input from flooding Slint's
        // event queue with one callback per low-level controller event.
        if !actions.is_empty() {
            if vibration.load(Ordering::Relaxed) {
                if let Some(id) = last_action_id {
                    let strong = actions.iter().any(|action| {
                        matches!(
                            action,
                            NavAction::Confirm | NavAction::Favorite | NavAction::Played
                        )
                    });
                    input.pulse(id, strong);
                }
            }

            let action_count = actions.len();
            let result = window.upgrade_in_event_loop(move |ui| {
                for action in actions {
                    ui.invoke_request_action(action);
                }
            });

            if let Err(error) = result {
                eprintln!(
                    "[b-emu] could not dispatch batch of {action_count} controller actions: {error}; stopping worker"
                );
                break;
            }
        }

        // Avoid busy-spinning while keeping controller input responsive.
        thread::sleep(Duration::from_millis(8));
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveController;

    #[test]
    fn first_connected_controller_owns_navigation() {
        let mut active = ActiveController::default();
        active.observe_connected(3);
        active.observe_connected(7);

        assert!(active.accepts(3));
        assert!(!active.accepts(7));
    }

    #[test]
    fn another_controller_is_adopted_after_disconnect() {
        let mut active = ActiveController::default();
        active.observe_connected(3);
        active.observe_disconnected(3);

        assert!(active.accepts(7));
        assert!(!active.accepts(3));
    }

    #[test]
    fn actions_from_a_non_active_controller_are_ignored() {
        let mut active = ActiveController::default();
        assert!(active.accepts(1));

        active.observe_disconnected(2);
        assert!(active.accepts(1));
    }
}

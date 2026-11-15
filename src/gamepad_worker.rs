//! Background gamepad polling and safe delivery to the Slint event loop.

use std::{
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread::{self, JoinHandle},
    time::Duration,
};

use b_emu::input::{GamepadAction, GamepadEvent, GamepadInput, navigation::apply_sidebar_action};

use crate::MainWindow;

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
    pub fn start(window: slint::Weak<MainWindow>) -> std::io::Result<Self> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("b-emu-gamepad".into())
            .spawn(move || run_worker(window, stop_rx))?;

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

fn run_worker(window: slint::Weak<MainWindow>, stop_rx: Receiver<()>) {
    let mut input = match GamepadInput::new() {
        Ok(input) => input,
        Err(error) => {
            eprintln!("[b-emu] gamepad support unavailable: {error}");
            return;
        }
    };

    for gamepad in input.connected_gamepads() {
        eprintln!(
            "[b-emu] controller connected: {} (id {})",
            gamepad.name, gamepad.id
        );
    }

    loop {
        match stop_rx.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        let mut actions = Vec::new();

        for event in input.poll_events() {
            match event {
                GamepadEvent::Connected(gamepad) => eprintln!(
                    "[b-emu] controller connected: {} (id {})",
                    gamepad.name, gamepad.id
                ),
                GamepadEvent::Disconnected { id } => {
                    eprintln!("[b-emu] controller disconnected (id {id})");
                }
                GamepadEvent::Action { id, action } => actions.push((id, action)),
            }
        }

        // Queue at most one UI callback per polling batch. This preserves
        // action order while preventing rapid input from flooding Slint's
        // event queue with one callback per low-level controller event.
        if !actions.is_empty() {
            let action_count = actions.len();
            let result = window.upgrade_in_event_loop(move |ui| {
                let mut focused_page = ui.get_focused_page();
                let mut active_page = ui.get_active_page();

                for (_, action) in actions {
                    (focused_page, active_page) =
                        apply_sidebar_action(focused_page, active_page, action);
                }

                ui.set_focused_page(focused_page);
                ui.set_active_page(active_page);
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

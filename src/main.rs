use b_emu::{
    input::{
        GamepadAction,
        navigation::{FocusZone, Navigation, apply_action},
    },
    wayland,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

mod gamepad_worker;
mod shell;

slint::include_modules!();

fn main() {
    if let Err(error) = wayland::wayland_socket_from_env() {
        eprintln!("B-Emu requires a usable native Wayland session: {error}");
        std::process::exit(1);
    }

    if let Err(error) = run() {
        eprintln!("B-Emu could not start: {error}");
        std::process::exit(1);
    }
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn action_from_nav(action: NavAction) -> GamepadAction {
    match action {
        NavAction::Up => GamepadAction::NavigateUp,
        NavAction::Down => GamepadAction::NavigateDown,
        NavAction::Left => GamepadAction::NavigateLeft,
        NavAction::Right => GamepadAction::NavigateRight,
        NavAction::Confirm => GamepadAction::Confirm,
        NavAction::Back => GamepadAction::Back,
    }
}

fn zone_from_ui(value: i32) -> FocusZone {
    if value == 1 {
        FocusZone::Content
    } else {
        FocusZone::Sidebar
    }
}

/// Number of focusable items in the active screen.
fn item_count(window: &MainWindow) -> i32 {
    match window.get_active_page() {
        0 => 1 + window.get_recent_games().row_count() as i32, // hero card + recent games
        1 => window.get_systems().row_count() as i32,
        2 => window.get_library_games().row_count() as i32,
        3 => window.get_emulators().row_count() as i32,
        4 => 3, // settings rows
        _ => 0,
    }
}

fn set_motion_duration(window: &MainWindow, reduced: bool) {
    // Slint duration properties are expressed in nanoseconds on the Rust side.
    let nanos: i64 = if reduced { 0 } else { 180_000_000 };
    window.invoke_set_motion_duration(nanos);
}

fn toggle_setting(window: &MainWindow, index: i32) {
    match index {
        0 => {
            let value = window.get_setting_auto_scan();
            window.set_setting_auto_scan(!value);
        }
        1 => {
            let value = window.get_setting_vibration();
            window.set_setting_vibration(!value);
        }
        2 => {
            let value = window.get_setting_reduced_motion();
            window.set_setting_reduced_motion(!value);
            set_motion_duration(window, !value);
        }
        _ => {}
    }
}

/// Single navigation path shared by keyboard and gamepad input.
fn apply_nav_action(window: &MainWindow, action: NavAction) {
    let gamepad_action = action_from_nav(action);
    let old_zone = zone_from_ui(window.get_focus_zone());

    let state = Navigation {
        focused_page: window.get_focused_page(),
        active_page: window.get_active_page(),
        page_count: window.get_pages().row_count() as i32,
        zone: old_zone,
        focused_item: window.get_focused_item(),
        selected_item: window.get_selected_item(),
    };

    let next = apply_action(state, gamepad_action, item_count(window));

    window.set_focused_page(next.focused_page);
    window.set_active_page(next.active_page);
    window.set_focused_item(next.focused_item);
    window.set_selected_item(next.selected_item);
    window.set_focus_zone(match next.zone {
        FocusZone::Sidebar => 0,
        FocusZone::Content => 1,
    });

    if next.zone != old_zone {
        match next.zone {
            FocusZone::Sidebar => window.invoke_focus_sidebar(),
            FocusZone::Content => window.invoke_focus_content(),
        }
    }

    // The settings screen toggles its focused row on Confirm.
    if gamepad_action == GamepadAction::Confirm
        && next.zone == FocusZone::Content
        && next.active_page == 4
    {
        toggle_setting(window, next.focused_item);
    }
}

fn seed_models(window: &MainWindow) {
    let games = shell::all_games();
    let emulators = shell::emulators();

    window.set_pages(model(shell::pages()));
    window.set_last_played(model(shell::last_played()));
    window.set_recent_games(model(shell::recent_games()));
    window.set_library_games(model(games.clone()));
    window.set_systems(model(shell::systems()));
    window.set_emulators(model(emulators.clone()));
    window.set_games_count(games.len() as i32);
    window.set_emulators_ready_count(emulators.iter().filter(|row| row.ready).count() as i32);
}

fn wire_callbacks(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_request_action(move |action| {
        if let Some(window) = weak.upgrade() {
            apply_nav_action(&window, action);
        }
    });

    let weak = window.as_weak();
    window.on_toggle_setting(move |index| {
        if let Some(window) = weak.upgrade() {
            toggle_setting(&window, index);
        }
    });

    let weak = window.as_weak();
    let games = shell::all_games();
    window.on_search_changed(move |text: SharedString| {
        if let Some(window) = weak.upgrade() {
            let filtered = shell::filter_games(&games, &text);
            window.set_search_text(text);
            window.set_library_games(model(filtered));
            window.set_focused_item(0);
            window.set_selected_item(-1);
        }
    });
}

fn run() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;
    seed_models(&window);
    wire_callbacks(&window);

    let _gamepad_worker = match gamepad_worker::GamepadWorker::start(window.as_weak()) {
        Ok(worker) => Some(worker),
        Err(error) => {
            eprintln!("[b-emu] could not start gamepad worker: {error}");
            None
        }
    };

    window.run()
}

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
};

use b_emu::{
    config::Config,
    input::{
        GamepadAction,
        navigation::{FocusZone, Navigation, apply_action},
    },
    library::{GameRecord, Library},
    paths::AppPaths,
    wayland,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

mod gamepad_worker;
mod scan_worker;
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
        NavAction::Favorite => GamepadAction::ToggleFavorite,
        NavAction::Played => GamepadAction::MarkPlayed,
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
        0 => 1 + window.get_played_games().row_count() as i32, // hero card + played games
        1 => window.get_systems().row_count() as i32,
        2 => 2 + window.get_library_games().row_count() as i32, // two filter pills + games
        3 => window.get_emulators().row_count() as i32,
        4 => 5 + window.get_game_dirs().row_count() as i32, // 3 toggles + dirs + add + scan
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

/// Applies one shared navigation action coming from keyboard or gamepad.
fn apply_nav_action(window: &MainWindow, db_path: &Path, action: NavAction) {
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

    let mut next = apply_action(state, gamepad_action, item_count(window));

    // The library has two filter pills before the game rows.
    if next.active_page == 2 && next.zone == FocusZone::Content {
        match gamepad_action {
            GamepadAction::Confirm if next.focused_item < 2 => {
                window.set_show_favorites_only(next.focused_item == 1);
                window.set_focused_item(0);
                window.set_selected_item(-1);
                if let Err(error) = refresh_library(window, db_path) {
                    eprintln!("[b-emu] could not refresh library: {error}");
                }
                return;
            }
            GamepadAction::Confirm => {
                next.selected_item = (next.focused_item - 2).max(0);
            }
            GamepadAction::ToggleFavorite | GamepadAction::MarkPlayed => {
                if next.selected_item < 0 {
                    window.set_selected_item((next.focused_item - 2).max(0));
                }
                library_item_action(window, db_path, gamepad_action);
                return;
            }
            _ => {}
        }
    } else if gamepad_action == GamepadAction::ToggleFavorite
        || gamepad_action == GamepadAction::MarkPlayed
    {
        // Library-only actions are ignored on other screens.
        return;
    }

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

    if gamepad_action == GamepadAction::Confirm
        && next.zone == FocusZone::Content
        && next.active_page == 4
    {
        activate_settings_item(window, next.focused_item);
    }
}

fn game_size(bytes: i64) -> SharedString {
    if bytes >= 1024 * 1024 {
        format!("{} Mo", bytes / (1024 * 1024)).into()
    } else if bytes >= 1024 {
        format!("{} Ko", bytes / 1024).into()
    } else {
        format!("{} o", bytes).into()
    }
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn to_indexed_games(
    records: Vec<GameRecord>,
    duplicates: &HashMap<String, usize>,
    favorites: &HashSet<String>,
) -> Vec<IndexedGame> {
    records
        .into_iter()
        .map(|game| IndexedGame {
            title: SharedString::from(game.title),
            system: SharedString::from(game.system.clone()),
            path: SharedString::from(path_key(&game.path)),
            size: game_size(game.size),
            accent: shell::system_accent(&game.system),
            duplicate: duplicates.get(&game.checksum).copied().unwrap_or(0) > 1,
            favorite: favorites.contains(&path_key(&game.path)),
        })
        .collect()
}

fn refresh_library(window: &MainWindow, db_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let library = Library::open(db_path)?;
    let all = library.games()?;
    let counts = library.counts()?;
    let favorites = library.favorite_paths()?;

    let mut duplicates: HashMap<String, usize> = HashMap::new();
    for game in &all {
        *duplicates.entry(game.checksum.clone()).or_insert(0) += 1;
    }

    let recent = library.recent(4)?;
    let mut played = library.recently_played(4)?;
    if played.is_empty() {
        played = recent.clone();
    }
    let mut last_played = played.clone();
    last_played.truncate(1);

    window.set_library_games(model(to_indexed_games(
        filtered_games(&library, window)?,
        &duplicates,
        &favorites,
    )));
    window.set_recent_games(model(to_indexed_games(recent, &duplicates, &favorites)));
    window.set_played_games(model(to_indexed_games(played, &duplicates, &favorites)));
    window.set_last_played(model(to_indexed_games(
        last_played,
        &duplicates,
        &favorites,
    )));

    let systems: Vec<SystemStat> = library
        .systems()?
        .into_iter()
        .map(|system| SystemStat {
            name: SharedString::from(system.name.clone()),
            game_count: system.game_count,
            accent: shell::system_accent(&system.name),
        })
        .collect();
    window.set_systems(model(systems));

    window.set_games_count(counts.games);
    window.set_systems_count(counts.systems);
    window.set_duplicates_count(counts.duplicates);
    window.set_favorites_count(favorites.len() as i32);
    Ok(())
}

fn filtered_games(
    library: &Library,
    window: &MainWindow,
) -> Result<Vec<GameRecord>, Box<dyn std::error::Error>> {
    let search = window.get_search_text();
    let mut records = if search.is_empty() {
        library.games()?
    } else {
        library.search(&search)?
    };

    if window.get_show_favorites_only() {
        let favorites = library.favorite_paths()?;
        records.retain(|game| favorites.contains(&path_key(&game.path)));
    }
    Ok(records)
}

fn library_item_action(window: &MainWindow, db_path: &Path, action: GamepadAction) {
    let index = window.get_selected_item();
    let games = window.get_library_games();
    let Some(game) = games.row_data(index.max(0) as usize) else {
        return;
    };

    let result = match action {
        GamepadAction::ToggleFavorite => Library::open(db_path)
            .and_then(|library| library.toggle_favorite(&game.path))
            .map(|_| ()),
        GamepadAction::MarkPlayed => Library::open(db_path)
            .and_then(|library| library.mark_played(&game.path))
            .map(|_| ()),
        _ => Ok(()),
    };

    if let Err(error) = result {
        eprintln!("[b-emu] library action failed: {error}");
    }
    if let Err(error) = refresh_library(window, db_path) {
        eprintln!("[b-emu] could not refresh library: {error}");
    }
}

fn refresh_dirs(window: &MainWindow, config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(config_path)?;
    let dirs: Vec<SharedString> = config
        .game_dirs
        .iter()
        .map(|path| SharedString::from(path.to_string_lossy().as_ref()))
        .collect();
    window.set_game_dirs(model(dirs));
    Ok(())
}

fn add_dir(window: &MainWindow, config_path: &Path, text: SharedString) {
    let path = PathBuf::from(text.as_str());
    if path.as_os_str().is_empty() {
        window.set_scan_status("Chemin vide".into());
        return;
    }
    if !path.is_absolute() {
        window.set_scan_status("Chemin absolu requis".into());
        return;
    }
    if !path.is_dir() {
        window.set_scan_status("Dossier introuvable".into());
        return;
    }

    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut config = Config::load(config_path)?;
        let added = config.add_dir(path)?;
        config.save(config_path)?;
        refresh_dirs(window, config_path)?;
        window.set_scan_status(if added {
            "Dossier ajouté".into()
        } else {
            "Dossier déjà configuré".into()
        });
        Ok(())
    })();

    if let Err(error) = result {
        eprintln!("[b-emu] could not add game directory: {error}");
        window.set_scan_status("Erreur lors de l'ajout".into());
    }
}

fn remove_dir(window: &MainWindow, config_path: &Path, index: i32) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut config = Config::load(config_path)?;
        if index >= 0 && (index as usize) < config.game_dirs.len() {
            config.game_dirs.remove(index as usize);
            config.save(config_path)?;
            refresh_dirs(window, config_path)?;
            window.set_scan_status("Dossier retiré".into());
        }
        Ok(())
    })();

    if let Err(error) = result {
        eprintln!("[b-emu] could not remove game directory: {error}");
    }

    let count = 5 + window.get_game_dirs().row_count() as i32;
    if window.get_focused_item() >= count {
        window.set_focused_item(count - 1);
    }
}

fn start_scan(
    window: &MainWindow,
    config_path: &Path,
    db_path: &Path,
    slot: &Rc<RefCell<Option<scan_worker::ScanWorker>>>,
) {
    let config = match Config::load(config_path) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("[b-emu] could not read configuration: {error}");
            return;
        }
    };

    if config.game_dirs.is_empty() {
        window.set_scan_status("Ajoutez un dossier de jeux".into());
        return;
    }

    let weak = window.as_weak();
    let refresh_db = db_path.to_path_buf();
    let on_finished = Box::new(move || {
        let weak = weak.clone();
        let refresh_db = refresh_db.clone();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if let Err(error) = refresh_library(&ui, &refresh_db) {
                eprintln!("[b-emu] could not refresh library view: {error}");
            }
        });
    });

    let worker = scan_worker::ScanWorker::start(
        window.as_weak(),
        db_path.to_path_buf(),
        config.game_dirs,
        on_finished,
    );
    match worker {
        Ok(worker) => {
            *slot.borrow_mut() = Some(worker);
            window.set_scan_status("Analyse…".into());
        }
        Err(error) => {
            eprintln!("[b-emu] could not start scan worker: {error}");
            window.set_scan_status("Échec du démarrage".into());
        }
    }
}

fn activate_settings_item(window: &MainWindow, index: i32) {
    let dirs = window.get_game_dirs().row_count() as i32;
    if index < 3 {
        toggle_setting(window, index);
    } else if index < 3 + dirs {
        window.invoke_remove_focused_dir();
    } else if index == 3 + dirs {
        let text = window.get_new_dir_text();
        window.invoke_add_dir(text);
    } else if index == 4 + dirs {
        window.invoke_scan_now();
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let paths = AppPaths::from_env()?;
    let config_path = paths.config_dir.join("config.json");
    let db_path = paths.data_dir.join("library.sqlite3");

    let window = MainWindow::new()?;
    window.set_pages(model(shell::pages()));
    window.set_emulators(model(shell::emulators()));
    window.set_emulators_ready_count(
        shell::emulators().iter().filter(|row| row.ready).count() as i32,
    );

    if let Err(error) = refresh_dirs(&window, &config_path) {
        eprintln!("[b-emu] could not load game directories: {error}");
    }
    if let Err(error) = refresh_library(&window, &db_path) {
        eprintln!("[b-emu] could not load library index: {error}");
    }

    let scan_slot: Rc<RefCell<Option<scan_worker::ScanWorker>>> = Rc::new(RefCell::new(None));

    let weak = window.as_weak();
    let db = db_path.clone();
    window.on_request_action(move |action| {
        if let Some(window) = weak.upgrade() {
            apply_nav_action(&window, &db, action);
        }
    });

    let weak = window.as_weak();
    window.on_toggle_setting(move |index| {
        if let Some(window) = weak.upgrade() {
            toggle_setting(&window, index);
        }
    });

    let weak = window.as_weak();
    let config = config_path.clone();
    window.on_add_dir(move |text| {
        if let Some(window) = weak.upgrade() {
            add_dir(&window, &config, text);
        }
    });

    let weak = window.as_weak();
    let config = config_path.clone();
    window.on_remove_dir(move |index| {
        if let Some(window) = weak.upgrade() {
            remove_dir(&window, &config, index);
        }
    });

    let weak = window.as_weak();
    let config = config_path.clone();
    let db = db_path.clone();
    let slot = Rc::clone(&scan_slot);
    window.on_scan_now(move || {
        if let Some(window) = weak.upgrade() {
            start_scan(&window, &config, &db, &slot);
        }
    });

    let weak = window.as_weak();
    let db = db_path.clone();
    window.on_search_changed(move |text: SharedString| {
        if let Some(window) = weak.upgrade() {
            window.set_search_text(text);
            window.set_focused_item(0);
            window.set_selected_item(-1);
            if let Err(error) = refresh_library(&window, &db) {
                eprintln!("[b-emu] library search failed: {error}");
            }
        }
    });

    let weak = window.as_weak();
    let db = db_path.clone();
    window.on_set_favorites_filter(move |enabled| {
        if let Some(window) = weak.upgrade() {
            window.set_show_favorites_only(enabled);
            window.set_focused_item(0);
            window.set_selected_item(-1);
            if let Err(error) = refresh_library(&window, &db) {
                eprintln!("[b-emu] could not filter favorites: {error}");
            }
        }
    });

    let weak = window.as_weak();
    let db = db_path.clone();
    window.on_toggle_favorite(move |path: SharedString| {
        if let Some(window) = weak.upgrade() {
            match Library::open(&db).and_then(|library| library.toggle_favorite(&path)) {
                Ok(_) => {
                    if let Err(error) = refresh_library(&window, &db) {
                        eprintln!("[b-emu] could not refresh favorites: {error}");
                    }
                }
                Err(error) => eprintln!("[b-emu] could not toggle favorite: {error}"),
            }
        }
    });

    let weak = window.as_weak();
    let db = db_path.clone();
    window.on_mark_played(move |path: SharedString| {
        if let Some(window) = weak.upgrade() {
            match Library::open(&db).and_then(|library| library.mark_played(&path)) {
                Ok(()) => {
                    if let Err(error) = refresh_library(&window, &db) {
                        eprintln!("[b-emu] could not refresh play history: {error}");
                    }
                }
                Err(error) => eprintln!("[b-emu] could not record play: {error}"),
            }
        }
    });

    if window.get_setting_auto_scan()
        && Config::load(&config_path)
            .map(|config| !config.game_dirs.is_empty())
            .unwrap_or(false)
    {
        start_scan(&window, &config_path, &db_path, &scan_slot);
    }

    let _gamepad_worker = match gamepad_worker::GamepadWorker::start(window.as_weak()) {
        Ok(worker) => Some(worker),
        Err(error) => {
            eprintln!("[b-emu] could not start gamepad worker: {error}");
            None
        }
    };

    window.run()?;
    Ok(())
}

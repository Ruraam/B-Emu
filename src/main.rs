use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use b_emu::{
    bios::{self, BiosStatus},
    config::Config,
    emulators::{self, EmulatorInstallation, adapters::BiosRequirement},
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
mod launch_worker;
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
        3 => {
            window.get_emulators().row_count() as i32 + window.get_capabilities().row_count() as i32
        }
        4 => 5 + window.get_game_dirs().row_count() as i32, // 3 toggles + dirs + add + scan
        _ => 0,
    }
}

fn set_motion_duration(window: &MainWindow, reduced: bool) {
    // Slint duration properties are expressed in nanoseconds on the Rust side.
    let nanos: i64 = if reduced { 0 } else { 180_000_000 };
    window.invoke_set_motion_duration(nanos);
}

fn toggle_setting(
    window: &MainWindow,
    index: i32,
    vibration: &Arc<AtomicBool>,
    config_path: &Path,
) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut config = Config::load(config_path)?;
        match index {
            0 => {
                config.auto_scan = !window.get_setting_auto_scan();
                window.set_setting_auto_scan(config.auto_scan);
            }
            1 => {
                config.vibration = !window.get_setting_vibration();
                window.set_setting_vibration(config.vibration);
                vibration.store(config.vibration, Ordering::Relaxed);
            }
            2 => {
                config.reduced_motion = !window.get_setting_reduced_motion();
                window.set_setting_reduced_motion(config.reduced_motion);
                set_motion_duration(window, config.reduced_motion);
            }
            _ => return Ok(()),
        }
        config.save(config_path)?;
        Ok(())
    })();

    if let Err(error) = result {
        eprintln!("[b-emu] could not save preference: {error}");
    }
}

/// Applies one shared navigation action coming from keyboard or gamepad.
fn apply_nav_action(
    window: &MainWindow,
    config_path: &Path,
    db_path: &Path,
    installations: &[EmulatorInstallation],
    bios_store: &Rc<RefCell<Vec<BiosStatus>>>,
    vibration: &Arc<AtomicBool>,
    action: NavAction,
) {
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

    // Capability settings follow the emulator rows in the Emulators screen.
    if next.active_page == 3
        && next.zone == FocusZone::Content
        && gamepad_action == GamepadAction::Confirm
        && next.focused_item >= window.get_emulators().row_count() as i32
    {
        let index = next.focused_item - window.get_emulators().row_count() as i32;
        toggle_capability_at(window, config_path, installations, index);
        return;
    }

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

    if gamepad_action == GamepadAction::Confirm && next.zone == FocusZone::Content {
        match next.active_page {
            3 => {
                window.set_selected_item(next.focused_item);
                refresh_capabilities(window, config_path, installations);
                refresh_selected_bios(window, installations, bios_store);
            }
            4 => activate_settings_item(window, next.focused_item, vibration, config_path),
            _ => {}
        }
    }
}

fn refresh_capabilities(
    window: &MainWindow,
    config_path: &Path,
    installations: &[EmulatorInstallation],
) {
    let index = window.get_selected_item();
    let Some(install) = usize::try_from(index)
        .ok()
        .and_then(|value| installations.get(value))
    else {
        window.set_capabilities(model(Vec::<CapabilityRow>::new()));
        return;
    };

    let enabled = match Config::load(config_path) {
        Ok(config) => config.enabled_capabilities(&install.id),
        Err(error) => {
            eprintln!("[b-emu] could not load emulator settings: {error}");
            Default::default()
        }
    };

    let rows: Vec<CapabilityRow> = install
        .capabilities
        .iter()
        .map(|capability| CapabilityRow {
            key: SharedString::from(capability.key),
            label: SharedString::from(capability.label),
            flag: SharedString::from(capability.flag),
            enabled: enabled.contains(capability.key),
        })
        .collect();
    window.set_capabilities(model(rows));
}

fn refresh_emulators(
    window: &MainWindow,
    config_path: &Path,
    installations: &[EmulatorInstallation],
) {
    let rows: Vec<EmulatorRow> = installations
        .iter()
        .map(|install| EmulatorRow {
            id: SharedString::from(install.id.as_str()),
            name: SharedString::from(install.name.as_str()),
            version: SharedString::from(install.version.as_deref().unwrap_or("inconnue")),
            status: SharedString::from(if install.version.is_some() {
                "Détecté"
            } else {
                "Version ?"
            }),
            systems: SharedString::from(install.systems.join(" · ").as_str()),
            bios: SharedString::from(
                install
                    .adapter
                    .bios
                    .iter()
                    .map(|requirement| requirement.name)
                    .collect::<Vec<_>>()
                    .join(", ")
                    .as_str(),
            ),
            ready: true,
        })
        .collect();
    window.set_emulators(model(rows));
    refresh_capabilities(window, config_path, installations);
}

fn bios_rows(statuses: &[BiosStatus]) -> Vec<BiosRow> {
    statuses
        .iter()
        .map(|status| BiosRow {
            name: SharedString::from(status.name.as_str()),
            system: SharedString::from(status.system.as_str()),
            present: status.present,
            checksum: SharedString::from(status.checksum.as_deref().unwrap_or("")),
        })
        .collect()
}

fn refresh_bios(
    window: &MainWindow,
    config_path: &Path,
    data_dir: &Path,
    installations: &[EmulatorInstallation],
    store: &Rc<RefCell<Vec<BiosStatus>>>,
) {
    let config = match Config::load(config_path) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("[b-emu] could not load BIOS settings: {error}");
            return;
        }
    };
    let dir = config
        .bios_dir
        .clone()
        .unwrap_or_else(|| data_dir.join("bios"));
    let requirements: Vec<(String, BiosRequirement)> = installations
        .iter()
        .flat_map(|install| {
            install
                .adapter
                .bios
                .iter()
                .map(move |requirement| (install.id.clone(), *requirement))
        })
        .collect();

    let statuses = bios::inventory(&dir, &requirements);
    *store.borrow_mut() = statuses.clone();
    window.set_bios(model(bios_rows(&statuses)));
}

fn refresh_selected_bios(
    window: &MainWindow,
    installations: &[EmulatorInstallation],
    store: &Rc<RefCell<Vec<BiosStatus>>>,
) {
    let selected = installations
        .get(window.get_selected_item().max(0) as usize)
        .map(|install| install.id.as_str());
    let filtered: Vec<BiosStatus> = store
        .borrow()
        .iter()
        .filter(|status| Some(status.emulator_id.as_str()) == selected)
        .cloned()
        .collect();
    window.set_bios(model(bios_rows(&filtered)));
}

fn toggle_capability_at(
    window: &MainWindow,
    config_path: &Path,
    installations: &[EmulatorInstallation],
    index: i32,
) {
    let Some(install) = installations.get(window.get_selected_item().max(0) as usize) else {
        return;
    };
    let Some(capability) = install.capabilities.get(index.max(0) as usize) else {
        return;
    };

    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut config = Config::load(config_path)?;
        config.toggle_capability(&install.id, capability.key);
        config.save(config_path)?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("[b-emu] could not save emulator setting: {error}");
        return;
    }
    refresh_capabilities(window, config_path, installations);
}

fn launch_game(
    window: &MainWindow,
    config_path: &Path,
    db_path: &Path,
    installations: &[EmulatorInstallation],
    slots: &Rc<RefCell<Vec<launch_worker::LaunchWorker>>>,
    path: SharedString,
    system: SharedString,
) {
    let Some(install) = emulators::for_system(&system, installations) else {
        window.set_launch_status("Aucun émulateur pour ce système".into());
        return;
    };
    let install = install.clone();
    let config = match Config::load(config_path) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("[b-emu] could not load configuration: {error}");
            window.set_launch_status("Configuration illisible".into());
            return;
        }
    };

    // Launch only regular files located inside a configured game directory.
    // Canonicalizing also resolves symlinks that would point outside the roots.
    let rom = match PathBuf::from(path.as_str()).canonicalize() {
        Ok(path) if path.is_file() => path,
        _ => {
            window.set_launch_status("Fichier de jeu introuvable".into());
            return;
        }
    };
    let inside_root = config.game_dirs.iter().any(|root| {
        root.canonicalize()
            .map(|root| rom.starts_with(root))
            .unwrap_or(false)
    });
    if !inside_root {
        window.set_launch_status("Jeu hors des dossiers configurés".into());
        return;
    }

    let enabled = config.enabled_capabilities(&install.id);

    let weak = window.as_weak();
    let refresh_db = db_path.to_path_buf();
    let on_finished = Box::new(move || {
        let weak = weak.clone();
        let refresh_db = refresh_db.clone();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if let Err(error) = refresh_library(&ui, &refresh_db) {
                eprintln!("[b-emu] could not refresh play history: {error}");
            }
        });
    });

    match launch_worker::LaunchWorker::start(
        window.as_weak(),
        install,
        rom,
        enabled,
        db_path.to_path_buf(),
        on_finished,
    ) {
        Ok(worker) => slots.borrow_mut().push(worker),
        Err(error) => {
            eprintln!("[b-emu] could not start launch worker: {error}");
            window.set_launch_status("Échec du lancement".into());
        }
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
    metadata: &HashMap<String, b_emu::metadata::Metadata>,
    images: &HashMap<String, slint::Image>,
) -> Vec<IndexedGame> {
    records
        .into_iter()
        .map(|game| {
            let key = path_key(&game.path);
            let detail = metadata.get(&key);
            let image = images.get(&key);
            IndexedGame {
                title: SharedString::from(game.title),
                system: SharedString::from(game.system.clone()),
                path: SharedString::from(key.as_str()),
                size: game_size(game.size),
                accent: shell::system_accent(&game.system),
                duplicate: duplicates.get(&game.checksum).copied().unwrap_or(0) > 1,
                favorite: favorites.contains(&key),
                description: SharedString::from(
                    detail.map(|value| value.description.as_str()).unwrap_or(""),
                ),
                year: SharedString::from(
                    detail
                        .and_then(|value| value.year)
                        .map(|year| year.to_string())
                        .unwrap_or_default()
                        .as_str(),
                ),
                region: SharedString::from(detail.map(|value| value.region.as_str()).unwrap_or("")),
                provider: SharedString::from(
                    detail.map(|value| value.provider.as_str()).unwrap_or(""),
                ),
                artwork: image.cloned().unwrap_or_default(),
                has_artwork: image.is_some(),
            }
        })
        .collect()
}

fn refresh_library(window: &MainWindow, db_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let library = Library::open(db_path)?;
    let all = library.games()?;
    let counts = library.counts()?;
    let favorites = library.favorite_paths()?;
    let metadata = library.metadata_map()?;
    let images: HashMap<String, slint::Image> = library
        .artwork_map()?
        .into_iter()
        .filter_map(|(key, value)| {
            slint::Image::load_from_path(&value.cache_path)
                .ok()
                .map(|image| (key, image))
        })
        .collect();

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
        &metadata,
        &images,
    )));
    window.set_recent_games(model(to_indexed_games(
        recent,
        &duplicates,
        &favorites,
        &metadata,
        &images,
    )));
    window.set_played_games(model(to_indexed_games(
        played,
        &duplicates,
        &favorites,
        &metadata,
        &images,
    )));
    window.set_last_played(model(to_indexed_games(
        last_played,
        &duplicates,
        &favorites,
        &metadata,
        &images,
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
    cache_dir: &Path,
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
    let refresh_cache = cache_dir.to_path_buf();
    let on_finished = Box::new(move || {
        let weak = weak.clone();
        let refresh_db = refresh_db.clone();
        let refresh_cache = refresh_cache.clone();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            if let Ok(library) = Library::open(&refresh_db) {
                if let Err(error) = library.enrich_all(&refresh_cache) {
                    eprintln!("[b-emu] could not enrich metadata: {error}");
                }
            }
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

fn activate_settings_item(
    window: &MainWindow,
    index: i32,
    vibration: &Arc<AtomicBool>,
    config_path: &Path,
) {
    let dirs = window.get_game_dirs().row_count() as i32;
    if index < 3 {
        toggle_setting(window, index, vibration, config_path);
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

    let cache_dir = paths.cache_dir.join("artwork");
    let installations: Rc<RefCell<Vec<EmulatorInstallation>>> =
        Rc::new(RefCell::new(emulators::discovery::discover()));
    let bios_store: Rc<RefCell<Vec<BiosStatus>>> = Rc::new(RefCell::new(Vec::new()));

    let window = MainWindow::new()?;
    window.set_pages(model(shell::pages()));
    refresh_emulators(&window, &config_path, &installations.borrow());
    refresh_bios(
        &window,
        &config_path,
        &paths.data_dir,
        &installations.borrow(),
        &bios_store,
    );

    if let Err(error) = Library::open(&db_path).and_then(|library| {
        library.enrich_all(&cache_dir)?;
        Ok(())
    }) {
        eprintln!("[b-emu] could not enrich metadata: {error}");
    }

    if let Err(error) = refresh_dirs(&window, &config_path) {
        eprintln!("[b-emu] could not load game directories: {error}");
    }
    if let Err(error) = refresh_library(&window, &db_path) {
        eprintln!("[b-emu] could not load library index: {error}");
    }

    let preferences = Config::load(&config_path).unwrap_or_default();
    window.set_setting_auto_scan(preferences.auto_scan);
    window.set_setting_vibration(preferences.vibration);
    window.set_setting_reduced_motion(preferences.reduced_motion);
    set_motion_duration(&window, preferences.reduced_motion);

    let vibration = Arc::new(AtomicBool::new(preferences.vibration));
    let scan_slot: Rc<RefCell<Option<scan_worker::ScanWorker>>> = Rc::new(RefCell::new(None));
    let launch_slots: Rc<RefCell<Vec<launch_worker::LaunchWorker>>> =
        Rc::new(RefCell::new(Vec::new()));

    let weak = window.as_weak();
    let config = config_path.clone();
    let db = db_path.clone();
    let installs = Rc::clone(&installations);
    let bios = Rc::clone(&bios_store);
    let vibration_for_nav = Arc::clone(&vibration);
    window.on_request_action(move |action| {
        if let Some(window) = weak.upgrade() {
            apply_nav_action(
                &window,
                &config,
                &db,
                &installs.borrow(),
                &bios,
                &vibration_for_nav,
                action,
            );
        }
    });

    let weak = window.as_weak();
    let vibration_for_setting = Arc::clone(&vibration);
    let config_for_setting = config_path.clone();
    window.on_toggle_setting(move |index| {
        if let Some(window) = weak.upgrade() {
            toggle_setting(&window, index, &vibration_for_setting, &config_for_setting);
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
    let cache_dir_for_scan = cache_dir.clone();
    let slot = Rc::clone(&scan_slot);
    window.on_scan_now(move || {
        if let Some(window) = weak.upgrade() {
            start_scan(&window, &config, &db, &cache_dir_for_scan, &slot);
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

    let weak = window.as_weak();
    let config = config_path.clone();
    let installs = Rc::clone(&installations);
    let bios = Rc::clone(&bios_store);
    window.on_select_emulator(move |index| {
        if let Some(window) = weak.upgrade() {
            window.set_selected_item(index);
            refresh_capabilities(&window, &config, &installs.borrow());
            refresh_selected_bios(&window, &installs.borrow(), &bios);
        }
    });

    let weak = window.as_weak();
    let config = config_path.clone();
    let save_path = config_path.clone();
    let installs = Rc::clone(&installations);
    window.on_toggle_capability(move |key: SharedString| {
        if let Some(window) = weak.upgrade() {
            let installs = installs.borrow();
            let Some(install) = installs.get(window.get_selected_item().max(0) as usize) else {
                return;
            };
            let Some(capability) = install.capabilities.iter().find(|cap| key == cap.key) else {
                return;
            };
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                let mut config = Config::load(&config)?;
                config.toggle_capability(&install.id, capability.key);
                config.save(&save_path)?;
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("[b-emu] could not save emulator setting: {error}");
            }
            refresh_capabilities(&window, &config, &installs);
        }
    });

    let weak = window.as_weak();
    let config = config_path.clone();
    let db = db_path.clone();
    let installs = Rc::clone(&installations);
    let slots = Rc::clone(&launch_slots);
    window.on_launch_game(move |path: SharedString, system: SharedString| {
        if let Some(window) = weak.upgrade() {
            launch_game(
                &window,
                &config,
                &db,
                &installs.borrow(),
                &slots,
                path,
                system,
            );
        }
    });

    if window.get_setting_auto_scan()
        && Config::load(&config_path)
            .map(|config| !config.game_dirs.is_empty())
            .unwrap_or(false)
    {
        start_scan(&window, &config_path, &db_path, &cache_dir, &scan_slot);
    }

    let profiles = Arc::new(RwLock::new(
        Config::load(&config_path)
            .map(|config| config.controller_profiles)
            .unwrap_or_default(),
    ));
    let _gamepad_worker =
        match gamepad_worker::GamepadWorker::start(window.as_weak(), profiles, vibration) {
            Ok(worker) => Some(worker),
            Err(error) => {
                eprintln!("[b-emu] could not start gamepad worker: {error}");
                None
            }
        };

    window.run()?;
    Ok(())
}

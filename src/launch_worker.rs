//! Emulator launch on a background thread with process supervision.

use std::{
    collections::BTreeSet,
    io,
    path::PathBuf,
    thread::{self, JoinHandle},
};

use b_emu::{
    emulators::{EmulatorInstallation, launch},
    library::Library,
};

use crate::MainWindow;

pub struct LaunchWorker {
    // Kept alive so the supervising thread is detached only at process exit.
    _thread: Option<JoinHandle<()>>,
}

impl LaunchWorker {
    pub fn start(
        window: slint::Weak<MainWindow>,
        install: EmulatorInstallation,
        rom: PathBuf,
        enabled: BTreeSet<String>,
        db_path: PathBuf,
        on_finished: Box<dyn FnOnce() + Send>,
    ) -> io::Result<Self> {
        let thread = thread::Builder::new()
            .name("b-emu-launch".into())
            .spawn(move || {
                let name = install.name.clone();
                let weak = window.clone();
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_launch_status(format!("Lancement de {name}…").into());
                });

                let spawned = match launch::launch(&install, &rom, &enabled) {
                    Ok(spawned) => spawned,
                    Err(error) => {
                        eprintln!("[b-emu] launch failed: {error}");
                        let weak = window.clone();
                        let _ = weak.upgrade_in_event_loop(move |ui| {
                            ui.set_launch_status("Échec du lancement".into());
                        });
                        on_finished();
                        return;
                    }
                };

                let pid = spawned.pid();
                let name = install.name.clone();
                let rom_key = rom.to_string_lossy().to_string();
                if let Err(error) =
                    Library::open(&db_path).and_then(|library| library.mark_played(&rom_key))
                {
                    eprintln!("[b-emu] could not record play history: {error}");
                }

                let weak = window.clone();
                let status_name = name.clone();
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_launch_status(format!("{status_name} lancé (pid {pid})").into());
                });
                on_finished();

                match spawned.wait() {
                    Ok(result) => {
                        eprintln!("[b-emu] {name} exited with {:?}", result.status.code());
                        if !result.stderr.is_empty() {
                            eprintln!("[b-emu] {name} stderr: {}", result.stderr.trim());
                        }
                        let weak = window.clone();
                        let status_name = name.clone();
                        let code = result.status.code().unwrap_or(-1);
                        let _ = weak.upgrade_in_event_loop(move |ui| {
                            ui.set_launch_status(
                                format!("{status_name} terminé (code {code})").into(),
                            );
                        });
                    }
                    Err(error) => {
                        eprintln!("[b-emu] could not supervise {name}: {error}");
                        let weak = window.clone();
                        let _ = weak.upgrade_in_event_loop(move |ui| {
                            ui.set_launch_status("Erreur de supervision".into());
                        });
                    }
                }
            })?;

        Ok(Self {
            _thread: Some(thread),
        })
    }
}

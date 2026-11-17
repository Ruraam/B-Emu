//! Background library scanner. Progress reaches Slint through a weak handle.

use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use b_emu::library::Library;

use crate::MainWindow;

/// Owns the library scanning thread and cancels it on drop.
pub struct ScanWorker {
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ScanWorker {
    pub fn start(
        window: slint::Weak<MainWindow>,
        db_path: PathBuf,
        roots: Vec<PathBuf>,
        on_finished: Box<dyn FnOnce() + Send>,
    ) -> io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);

        let thread = thread::Builder::new()
            .name("b-emu-scan".into())
            .spawn(move || {
                let mut library = match Library::open(&db_path) {
                    Ok(library) => library,
                    Err(error) => {
                        eprintln!("[b-emu] could not open library index: {error}");
                        let weak = window.clone();
                        let _ = weak.upgrade_in_event_loop(move |ui| {
                            ui.set_scan_status("Impossible d'ouvrir l'index".into());
                        });
                        on_finished();
                        return;
                    }
                };

                let report = library.scan(&roots, &worker_cancel, &mut |progress| {
                    // Throttle UI updates: one callback every 16 files.
                    if progress.scanned % 16 != 0 && progress.scanned != progress.total {
                        return;
                    }
                    let scanned = progress.scanned;
                    let total = progress.total;
                    let name = progress
                        .current
                        .file_name()
                        .map(|value| value.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let weak = window.clone();
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        ui.set_scan_progress(scanned as f32 / total.max(1) as f32);
                        ui.set_scan_status(format!("Analyse… {scanned}/{total} · {name}").into());
                    });
                });

                match report {
                    Ok(report) => {
                        let weak = window.clone();
                        let _ = weak.upgrade_in_event_loop(move |ui| {
                            ui.set_scan_progress(1.0);
                            if report.cancelled {
                                ui.set_scan_status("Analyse annulée".into());
                            } else {
                                ui.set_scan_status(
                                    format!(
                                        "Terminé : {} ajoutés, {} mis à jour, {} supprimés",
                                        report.added, report.updated, report.removed
                                    )
                                    .into(),
                                );
                            }
                        });
                    }
                    Err(error) => {
                        eprintln!("[b-emu] library scan failed: {error}");
                        let weak = window.clone();
                        let _ = weak.upgrade_in_event_loop(move |ui| {
                            ui.set_scan_status("Échec de l'analyse".into());
                        });
                    }
                }

                on_finished();
            })?;

        Ok(Self {
            cancel,
            thread: Some(thread),
        })
    }
}

impl Drop for ScanWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                eprintln!("[b-emu] scan worker terminated unexpectedly");
            }
        }
    }
}

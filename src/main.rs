use b_emu::wayland;

mod gamepad_worker;

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

fn run() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;
    let _gamepad_worker = match gamepad_worker::GamepadWorker::start(window.as_weak()) {
        Ok(worker) => Some(worker),
        Err(error) => {
            eprintln!("[b-emu] could not start gamepad worker: {error}");
            None
        }
    };

    window.run()
}

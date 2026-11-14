use b_emu::wayland;

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
    window.run()
}

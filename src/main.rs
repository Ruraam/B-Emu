slint::include_modules!();

fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("B-Emu requires a native Wayland session. X11 and XWayland are not supported.");
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

//! Launch coordinator. Commands are built from argument arrays only.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
};

use super::discovery::EmulatorInstallation;

#[derive(Debug)]
pub enum LaunchError {
    Io(io::Error),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "could not launch emulator: {error}"),
        }
    }
}

impl std::error::Error for LaunchError {}

impl From<io::Error> for LaunchError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug)]
pub struct ExitResult {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

pub struct SpawnedEmulator {
    child: Child,
    pub adapter_id: String,
    pub rom: PathBuf,
}

impl SpawnedEmulator {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn wait(self) -> Result<ExitResult, io::Error> {
        let output = self.child.wait_with_output()?;
        Ok(ExitResult {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Full argument array for a launch, exposed for diagnostics and tests.
pub fn command_args(
    install: &EmulatorInstallation,
    rom: &Path,
    enabled_capabilities: &BTreeSet<String>,
) -> Vec<OsString> {
    (install.adapter.build_args)(rom, enabled_capabilities)
}

pub fn launch(
    install: &EmulatorInstallation,
    rom: &Path,
    enabled_capabilities: &BTreeSet<String>,
) -> Result<SpawnedEmulator, LaunchError> {
    let child = Command::new(&install.executable)
        .args(command_args(install, rom, enabled_capabilities))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    Ok(SpawnedEmulator {
        child,
        adapter_id: install.id.clone(),
        rom: rom.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::{command_args, launch};
    use crate::emulators::adapters::Adapter;
    use crate::emulators::discovery::EmulatorInstallation;
    use std::{collections::BTreeSet, ffi::OsString, path::Path};

    fn fake_adapter() -> &'static Adapter {
        const CAPS: &[crate::emulators::adapters::Capability] = &[];
        const ADAPTER: Adapter = Adapter {
            id: "fake",
            name: "Fake",
            executables: &["/bin/echo"],
            systems: &["Test"],
            extensions: &["rom"],
            version_args: &[],
            capabilities: CAPS,
            bios: &[],
            build_args: |rom, _| vec![OsString::from("launch"), rom.as_os_str().to_owned()],
        };
        &ADAPTER
    }

    fn installation(executable: &str) -> EmulatorInstallation {
        EmulatorInstallation {
            adapter: fake_adapter(),
            id: "fake".into(),
            name: "Fake".into(),
            executable: executable.into(),
            version: Some("1.0.0".into()),
            available: true,
            systems: vec!["Test".into()],
            capabilities: vec![],
        }
    }

    #[test]
    fn builds_argument_arrays_only() {
        let args = command_args(
            &installation("/bin/echo"),
            Path::new("/roms/a & b.rom"),
            &BTreeSet::new(),
        );
        let values: Vec<_> = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        assert_eq!(values, vec!["launch", "/roms/a & b.rom"]);
    }

    #[test]
    fn launches_a_fake_adapter_and_captures_output() {
        let spawned = launch(
            &installation("/bin/echo"),
            Path::new("/roms/game.rom"),
            &BTreeSet::new(),
        )
        .unwrap();
        assert!(spawned.pid() > 0);
        let result = spawned.wait().unwrap();
        assert!(result.status.success());
        assert!(result.stdout.contains("launch /roms/game.rom"));
    }

    #[test]
    fn reports_non_zero_exit_status() {
        let spawned = launch(
            &installation("/bin/false"),
            Path::new("/roms/game.rom"),
            &BTreeSet::new(),
        )
        .unwrap();
        assert!(!spawned.wait().unwrap().status.success());
    }

    #[test]
    fn missing_executable_is_an_error() {
        assert!(
            launch(
                &installation("/does/not/exist"),
                Path::new("/roms/game.rom"),
                &BTreeSet::new(),
            )
            .is_err()
        );
    }
}

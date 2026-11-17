//! Emulator discovery on PATH and version probing.

use std::{
    collections::BTreeMap,
    env,
    ffi::OsStr,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::{config::Config, paths::AppPaths};

use super::adapters::{ADAPTERS, Adapter, Capability};

#[derive(Debug, Clone)]
pub struct EmulatorInstallation {
    pub adapter: &'static Adapter,
    pub id: String,
    pub name: String,
    pub executable: PathBuf,
    pub version: Option<String>,
    pub available: bool,
    pub systems: Vec<String>,
    pub capabilities: Vec<Capability>,
}

/// Extracts the first version-like token from version command output.
pub fn parse_version(output: &str) -> Option<String> {
    for line in output.lines() {
        for raw in line.split_whitespace() {
            let token = raw.trim_matches(|c: char| {
                !c.is_ascii_alphanumeric() && c != '.' && c != '-' && c != '+'
            });
            let bytes = token.as_bytes();
            if bytes.first().is_some_and(u8::is_ascii_digit) && (3..=24).contains(&token.len()) {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn find_executable(name: &str, path_var: Option<&OsStr>) -> Option<PathBuf> {
    let path_var = path_var?;
    for dir in env::split_paths(path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn wait_with_timeout(mut child: Child, timeout: Duration) -> Option<std::process::Output> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => {
                let _ = child.kill();
                return None;
            }
        }
    }
}

fn probe_version(executable: &Path, args: &[&str]) -> Option<String> {
    let child = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let output = wait_with_timeout(child, Duration::from_secs(2))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_version(&format!("{stdout}\n{stderr}"))
}

/// Discovers emulators. `probe` is injectable so tests never spawn real
/// emulators. Explicit config overrides take precedence over PATH aliases.
pub fn discover_with_overrides(
    path_var: Option<&OsStr>,
    overrides: &BTreeMap<String, String>,
    probe: &dyn Fn(&Path, &[&str]) -> Option<String>,
) -> Vec<EmulatorInstallation> {
    ADAPTERS
        .iter()
        .filter_map(|adapter| {
            let executable = overrides
                .get(adapter.id)
                .and_then(|value| {
                    let path = PathBuf::from(value);
                    if path.is_absolute() {
                        is_executable(&path).then_some(path)
                    } else {
                        find_executable(value, path_var)
                    }
                })
                .or_else(|| {
                    adapter
                        .executables
                        .iter()
                        .find_map(|name| find_executable(name, path_var))
                })?;
            let version = probe(&executable, adapter.version_args);
            Some(EmulatorInstallation {
                adapter,
                id: adapter.id.to_owned(),
                name: adapter.name.to_owned(),
                available: true,
                executable,
                version,
                systems: adapter
                    .systems
                    .iter()
                    .map(|system| (*system).to_owned())
                    .collect(),
                capabilities: adapter.capabilities.to_vec(),
            })
        })
        .collect()
}

pub fn discover_with(
    path_var: Option<&OsStr>,
    probe: &dyn Fn(&Path, &[&str]) -> Option<String>,
) -> Vec<EmulatorInstallation> {
    discover_with_overrides(path_var, &BTreeMap::new(), probe)
}

pub fn discover() -> Vec<EmulatorInstallation> {
    let overrides = AppPaths::from_env()
        .ok()
        .and_then(|paths| Config::load(&paths.config_dir.join("config.json")).ok())
        .map(|config| config.emulator_executables)
        .unwrap_or_default();
    discover_with_overrides(env::var_os("PATH").as_deref(), &overrides, &probe_version)
}

#[cfg(test)]
mod tests {
    use super::{discover_with, find_executable, parse_version};
    use std::{ffi::OsString, fs, os::unix::fs::PermissionsExt};

    #[test]
    fn parses_versions_from_multiple_formats() {
        assert_eq!(parse_version("RetroArch 1.16.0"), Some("1.16.0".into()));
        assert_eq!(parse_version("Dolphin 2407\n"), Some("2407".into()));
        assert_eq!(parse_version("no version here"), None);
    }

    #[test]
    fn finds_only_executable_files() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("fake-emulator");
        fs::write(&binary, b"ignored").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let non_executable = dir.path().join("other");
        fs::write(&non_executable, b"ignored").unwrap();

        let path = OsString::from(dir.path());
        assert_eq!(find_executable("fake-emulator", Some(&path)), Some(binary));
        assert_eq!(find_executable("other", Some(&path)), None);
    }

    #[test]
    fn discovery_uses_injected_probe() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["retroarch", "dolphin-emu"] {
            let path = dir.path().join(name);
            fs::write(&path, b"ignored").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = OsString::from(dir.path());
        let installations = discover_with(Some(&path), &|_, _| Some("9.9.9".into()));

        assert_eq!(installations.len(), 2);
        assert!(
            installations
                .iter()
                .all(|install| install.version.as_deref() == Some("9.9.9"))
        );
        assert!(installations.iter().all(|install| install.available));
    }

    #[test]
    fn executable_override_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let custom = dir.path().join("my-retroarch");
        fs::write(&custom, b"ignored").unwrap();
        fs::set_permissions(&custom, fs::Permissions::from_mode(0o755)).unwrap();
        let path = OsString::from(dir.path());
        let overrides = std::collections::BTreeMap::from([(
            "retroarch".to_string(),
            custom.to_string_lossy().to_string(),
        )]);

        let found = super::discover_with_overrides(Some(&path), &overrides, &|_, _| None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].executable, custom);
    }

    #[test]
    fn missing_emulators_are_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = OsString::from(dir.path());
        assert!(discover_with(Some(&path), &|_, _| None).is_empty());
    }
}

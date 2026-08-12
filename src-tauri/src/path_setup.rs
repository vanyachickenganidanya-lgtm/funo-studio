use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStatus {
    pub installed: bool,
    pub bin_dir: String,
    pub launcher: String,
    pub path_contains_bin: bool,
}

#[cfg(windows)]
fn locations() -> Result<(PathBuf, PathBuf), String> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join("AppData").join("Local")))
        .ok_or("Не найдена папка LocalAppData")?;
    let bin = base.join("Funo").join("bin");
    Ok((bin.clone(), bin.join("funo.cmd")))
}

#[cfg(unix)]
fn locations() -> Result<(PathBuf, PathBuf), String> {
    let home = dirs::home_dir().ok_or("Не найдена домашняя папка")?;
    let bin = home.join(".local").join("bin");
    Ok((bin.clone(), bin.join("funo")))
}

#[cfg(unix)]
fn path_has(bin: &std::path::Path) -> bool {
    env::var_os("PATH")
        .map(|value| env::split_paths(&value).any(|item| paths_equal(&item, bin)))
        .unwrap_or(false)
}

fn paths_equal(left: &std::path::Path, right: &std::path::Path) -> bool {
    #[cfg(windows)]
    { left.to_string_lossy().trim_end_matches(['\\', '/']).eq_ignore_ascii_case(right.to_string_lossy().trim_end_matches(['\\', '/'])) }
    #[cfg(not(windows))]
    { left == right }
}

pub fn status() -> Result<PathStatus, String> {
    let (bin, launcher) = locations()?;
    let stored = user_path()?.iter().any(|item| paths_equal(item, &bin));
    #[cfg(windows)]
    let path_contains_bin = stored;
    #[cfg(not(windows))]
    let path_contains_bin = stored || path_has(&bin);
    Ok(PathStatus {
        installed: launcher.exists() && path_contains_bin,
        bin_dir: bin.to_string_lossy().into(),
        launcher: launcher.to_string_lossy().into(),
        path_contains_bin,
    })
}

pub fn install() -> Result<PathStatus, String> {
    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let (bin, launcher) = locations()?;
    fs::create_dir_all(&bin).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    {
        // The installer ships the console-subsystem `funo.exe` next to Studio.
        // Calling it directly preserves stdin/stdout and the real exit code.
        // Development builds fall back to the GUI binary's maintenance entry.
        let sibling_cli = executable.parent().map(|parent| parent.join("funo.exe"));
        let (target, cli_flag) = match sibling_cli.filter(|path| path.exists() && path != &executable) {
            Some(path) => (path, ""),
            None => (executable.clone(), " --cli"),
        };
        let quoted = target.to_string_lossy().replace('%', "%%");
        fs::write(&launcher, format!("@echo off\r\n\"{quoted}\"{cli_flag} %*\r\nexit /b %errorlevel%\r\n"))
            .map_err(|error| error.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let script = format!("#!/bin/sh\nexec {} --cli \"$@\"\n", shell_quote(&executable.to_string_lossy()));
        fs::write(&launcher, script).map_err(|error| error.to_string())?;
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).map_err(|error| error.to_string())?;
    }
    let mut values = user_path()?;
    if !values.iter().any(|item| paths_equal(item, &bin)) {
        values.push(bin);
        set_user_path(&values)?;
    }
    status()
}

pub fn uninstall() -> Result<PathStatus, String> {
    let (bin, launcher) = locations()?;
    if launcher.exists() {
        fs::remove_file(&launcher).map_err(|error| error.to_string())?;
    }
    let values = user_path()?.into_iter().filter(|item| !paths_equal(item, &bin)).collect::<Vec<_>>();
    set_user_path(&values)?;
    status()
}

#[cfg(unix)]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn user_path() -> Result<Vec<PathBuf>, String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let environment = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", winreg::enums::KEY_READ | winreg::enums::KEY_WRITE)
        .map_err(|error| format!("Не удалось открыть HKCU\\Environment: {error}"))?;
    let value: String = environment.get_value("Path").unwrap_or_default();
    Ok(env::split_paths(&value).collect())
}

#[cfg(windows)]
fn set_user_path(values: &[PathBuf]) -> Result<(), String> {
    use winreg::{
        enums::{HKEY_CURRENT_USER, REG_EXPAND_SZ},
        RegKey, RegValue,
    };
    let environment = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", winreg::enums::KEY_READ | winreg::enums::KEY_WRITE)
        .map_err(|error| format!("Не удалось открыть HKCU\\Environment: {error}"))?;
    let joined = env::join_paths(values).map_err(|error| error.to_string())?;
    // Keep PATH expandable: writing a Rust String through winreg would change
    // REG_EXPAND_SZ to REG_SZ and break entries such as %USERPROFILE%\\bin.
    let mut bytes = joined
        .to_string_lossy()
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    bytes.extend_from_slice(&[0, 0]);
    environment
        .set_raw_value("Path", &RegValue { bytes, vtype: REG_EXPAND_SZ })
        .map_err(|error| format!("Не удалось обновить пользовательский PATH: {error}"))?;
    broadcast_environment_change();
    Ok(())
}

#[cfg(windows)]
fn broadcast_environment_change() {
    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutW(
            window: isize,
            message: u32,
            wparam: usize,
            lparam: isize,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }
    const HWND_BROADCAST: isize = 0xffff;
    const WM_SETTINGCHANGE: u32 = 0x001a;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    let value: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut result = 0usize;
    // Failure here should not undo a registry update; new terminals still read
    // the correct value even if one unresponsive window misses the broadcast.
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            value.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5_000,
            &mut result,
        );
    }
}

#[cfg(unix)]
fn user_path() -> Result<Vec<PathBuf>, String> {
    let (bin, _) = locations()?;
    let mut values = env::var_os("PATH").map(|value| env::split_paths(&value).collect()).unwrap_or_else(Vec::new);
    let home = dirs::home_dir().ok_or("Не найдена домашняя папка")?;
    for file in [home.join(".profile"), home.join(".zshrc")] {
        if fs::read_to_string(file).unwrap_or_default().contains("$HOME/.local/bin") && !values.iter().any(|item| paths_equal(item, &bin)) {
            values.push(bin.clone());
        }
    }
    Ok(values)
}

#[cfg(unix)]
fn set_user_path(values: &[PathBuf]) -> Result<(), String> {
    use std::io::Write;
    let (bin, _) = locations()?;
    let should_add = values.iter().any(|item| paths_equal(item, &bin));
    let home = dirs::home_dir().ok_or("Не найдена домашняя папка")?;
    let marker_start = "# >>> Funo CLI >>>";
    let marker_end = "# <<< Funo CLI <<<";
    for profile in [home.join(".profile"), home.join(".zshrc")] {
        let existing = fs::read_to_string(&profile).unwrap_or_default();
        let mut output = existing.clone();
        if let Some(start) = output.find(marker_start) {
            if let Some(relative_end) = output[start..].find(marker_end) {
                let end = start + relative_end + marker_end.len();
                output.replace_range(start..end, "");
            }
        }
        if should_add {
            output.push_str("\n# >>> Funo CLI >>>\nexport PATH=\"$HOME/.local/bin:$PATH\"\n# <<< Funo CLI <<<\n");
        }
        let mut file = fs::File::create(&profile).map_err(|error| error.to_string())?;
        file.write_all(output.as_bytes()).map_err(|error| error.to_string())?;
    }
    Ok(())
}

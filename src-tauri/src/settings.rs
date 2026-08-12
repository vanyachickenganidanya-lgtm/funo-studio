use crate::models::StudioSettings;
use std::{fs, path::PathBuf};

fn settings_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .ok_or("Не удалось найти папку настроек пользователя")?;
    Ok(base.join("Funo Studio").join("settings.json"))
}

pub fn load() -> Result<StudioSettings, String> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(StudioSettings::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("Не удалось прочитать {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("Настройки Funo повреждены: {error}"))
}

pub fn save(settings: &StudioSettings) -> Result<StudioSettings, String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(&path, text)
        .map_err(|error| format!("Не удалось сохранить {}: {error}", path.display()))?;
    Ok(settings.clone())
}

/// Used by the NSIS installer to carry the beginner choice into Studio. The
/// first-launch wizard is deliberately left incomplete so it is still shown.
pub fn set_installer_beginner(beginner: bool) -> Result<(), String> {
    let mut settings = load()?;
    settings.beginner = beginner;
    settings.installer_beginner_choice = Some(beginner);
    save(&settings).map(|_| ())
}

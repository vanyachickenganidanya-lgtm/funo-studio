use crate::{
    models::{MinecraftToolStatus, MinecraftToolchainStatus, StorageVolume},
    process, project,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const FREE_SPACE_RESERVE: u64 = 30 * 1024 * 1024 * 1024;
const JDK_INSTALL_ESTIMATE: u64 = 900 * 1024 * 1024;
const GRADLE_INSTALL_ESTIMATE: u64 = 550 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManagedTool {
    kind: String,
    requirement: String,
    version: String,
    home: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ManagedConfig {
    #[serde(default)]
    tools: Vec<ManagedTool>,
}

#[derive(Debug, Clone)]
struct DownloadPackage {
    version: String,
    url: String,
    checksum: String,
    size: u64,
    extension: &'static str,
}

fn config_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().or_else(dirs::home_dir).ok_or("Не найдена папка настроек")?;
    Ok(base.join("Funo Studio").join("minecraft-tools.json"))
}

fn load_config() -> ManagedConfig {
    config_path()
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|source| serde_json::from_str(&source).ok())
        .unwrap_or_default()
}

fn save_config(config: &ManagedConfig) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Не удалось создать папку настроек инструментов: {error}"))?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(config).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("Не удалось сохранить настройки инструментов: {error}"))
}

fn record_tool(tool: ManagedTool) -> Result<(), String> {
    let mut config = load_config();
    config
        .tools
        .retain(|value| value.kind != tool.kind || value.requirement != tool.requirement);
    config.tools.push(tool);
    save_config(&config)
}

fn executable(home: &Path, relative: &str) -> PathBuf {
    if cfg!(windows) {
        home.join(format!("{relative}.exe"))
    } else {
        home.join(relative)
    }
}

fn gradle_executable(home: &Path) -> PathBuf {
    home.join("bin").join(if cfg!(windows) { "gradle.bat" } else { "gradle" })
}

fn command_text(program: &Path, arguments: &[&str]) -> Option<String> {
    let output = process::command(program).args(arguments).output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{stdout}\n{stderr}").trim().to_string();
    if output.status.success() && !text.is_empty() { Some(text) } else { None }
}

fn java_version(text: &str) -> Option<String> {
    Regex::new(r#"(?i)(?:javac|version)\s+\"?([0-9][0-9A-Za-z._+\-]*)"#)
        .ok()?
        .captures(text)
        .map(|captures| captures[1].to_string())
}

fn java_major(version: &str) -> Option<u8> {
    let numbers: Vec<u16> = Regex::new(r"\d+")
        .ok()?
        .find_iter(version)
        .filter_map(|value| value.as_str().parse().ok())
        .collect();
    match numbers.as_slice() {
        [1, major, ..] => u8::try_from(*major).ok(),
        [major, ..] => u8::try_from(*major).ok(),
        _ => None,
    }
}

fn gradle_version(text: &str) -> Option<String> {
    Regex::new(r"(?m)^Gradle\s+([0-9][0-9A-Za-z._+\-]*)")
        .ok()?
        .captures(text)
        .map(|captures| captures[1].to_string())
}

fn managed_tool(kind: &str, requirement: &str, relative_executable: &str) -> Option<ManagedTool> {
    load_config()
        .tools
        .into_iter()
        .rev()
        .find(|tool| {
            tool.kind == kind
                && tool.requirement == requirement
                && Path::new(&tool.home).join(relative_executable).is_file()
        })
}

fn detect_jdk(required: u8) -> MinecraftToolStatus {
    let mut candidates: Vec<(String, String, bool)> = Vec::new();
    if let Some(tool) = managed_tool(
        "jdk",
        &required.to_string(),
        if cfg!(windows) { "bin/javac.exe" } else { "bin/javac" },
    ) {
        let javac = executable(&PathBuf::from(&tool.home).join("bin"), "javac");
        if let Some(text) = command_text(&javac, &["-version"]) {
            if let Some(version) = java_version(&text) {
                candidates.push((version, tool.home, true));
            }
        }
    }
    if let Some(home) = env::var_os("JAVA_HOME").map(PathBuf::from) {
        let javac = executable(&home.join("bin"), "javac");
        if let Some(text) = command_text(&javac, &["-version"]) {
            if let Some(version) = java_version(&text) {
                candidates.push((version, home.to_string_lossy().to_string(), false));
            }
        }
    }
    let system_javac = PathBuf::from(if cfg!(windows) { "javac.exe" } else { "javac" });
    if let Some(text) = command_text(&system_javac, &["-version"]) {
        if let Some(version) = java_version(&text) {
            candidates.push((version, "PATH".into(), false));
        }
    }

    let selected = candidates
        .iter()
        .find(|(version, _, _)| java_major(version) == Some(required))
        .or_else(|| candidates.first());
    if let Some((version, path, managed)) = selected {
        let compatible = java_major(version) == Some(required);
        MinecraftToolStatus {
            found: true,
            compatible,
            managed: *managed,
            version: version.clone(),
            latest_version: String::new(),
            path: path.clone(),
            detail: if compatible {
                format!("JDK {required} готов для этого Minecraft-проекта")
            } else {
                format!("Найден JDK {}, но проекту нужен JDK {required}", java_major(version).unwrap_or(0))
            },
            update_available: false,
        }
    } else {
        MinecraftToolStatus {
            found: false,
            compatible: false,
            managed: false,
            version: String::new(),
            latest_version: String::new(),
            path: String::new(),
            detail: format!("JDK {required} не найден"),
            update_available: false,
        }
    }
}

fn wrapper_version(project_root: &Path) -> Option<(String, String)> {
    let launcher = project_root.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" });
    if !launcher.is_file() {
        return None;
    }
    let properties = project_root.join("gradle/wrapper/gradle-wrapper.properties");
    let source = fs::read_to_string(&properties).ok()?;
    let version = Regex::new(r"gradle-([0-9][0-9A-Za-z._+\-]*)-(?:bin|all)\.zip")
        .ok()?
        .captures(&source)
        .map(|captures| captures[1].to_string())?;
    Some((version, properties.to_string_lossy().to_string()))
}

fn apply_managed_jdk(command: &mut Command, required_java: u8) {
    let Some(jdk) = managed_tool(
        "jdk",
        &required_java.to_string(),
        if cfg!(windows) { "bin/javac.exe" } else { "bin/javac" },
    ) else {
        return;
    };
    let home = PathBuf::from(jdk.home);
    let javac = executable(&home.join("bin"), "javac");
    let valid = command_text(&javac, &["-version"])
        .and_then(|text| java_version(&text))
        .and_then(|version| java_major(&version))
        == Some(required_java);
    if !valid {
        return;
    }
    let mut paths = vec![home.join("bin")];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(path) = env::join_paths(paths) {
        command.env("PATH", path);
    }
    command.env("JAVA_HOME", home);
}

fn version_triplet(version: &str) -> (u64, u64, u64) {
    let values: Vec<u64> = Regex::new(r"\d+")
        .unwrap()
        .find_iter(version)
        .take(3)
        .filter_map(|value| value.as_str().parse().ok())
        .collect();
    (
        *values.first().unwrap_or(&0),
        *values.get(1).unwrap_or(&0),
        *values.get(2).unwrap_or(&0),
    )
}

fn gradle_compatible(version: &str, recommended: &str) -> bool {
    let actual = version_triplet(version);
    let minimum = version_triplet(recommended);
    match minimum.0 {
        // Legacy ForgeGradle releases reject newer major Gradle versions.
        4 | 6 | 7 | 8 | 9 => actual.0 == minimum.0 && actual >= minimum,
        _ => false,
    }
}

fn release_numbers(version: &str) -> Vec<u64> {
    let mut values: Vec<u64> = Regex::new(r"\d+")
        .unwrap()
        .find_iter(version)
        .take(5)
        .filter_map(|value| value.as_str().parse().ok())
        .collect();
    if values.first() == Some(&1) && values.get(1) == Some(&8) {
        values.remove(0);
    }
    // Build metadata is often omitted by `javac -version`; compare the public
    // feature/minor/security release so an installed current JDK is not offered forever.
    values.resize(3, 0);
    values.truncate(3);
    values
}

fn release_newer(latest: &str, current: &str) -> bool {
    release_numbers(latest) > release_numbers(current)
}

fn detect_gradle(project_root: &Path, recommended: &str, required_java: u8) -> MinecraftToolStatus {
    let mut candidates: Vec<(String, String, bool, bool)> = Vec::new();
    if let Some(value) = wrapper_version(project_root) {
        // A checked-in wrapper is authoritative for custom and third-party loaders.
        candidates.push((value.0, value.1, false, true));
    }
    if let Some(tool) = managed_tool(
        "gradle",
        recommended,
        if cfg!(windows) { "bin/gradle.bat" } else { "bin/gradle" },
    ) {
        // Managed records are written only after a verified archive is activated.
        let compatible = gradle_compatible(&tool.version, recommended);
        candidates.push((tool.version, tool.home, true, compatible));
    }
    let system = PathBuf::from(if cfg!(windows) { "gradle.bat" } else { "gradle" });
    let mut command = process::command(&system);
    apply_managed_jdk(&mut command, required_java);
    if let Ok(output) = command.arg("--version").output() {
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if output.status.success() {
            if let Some(version) = gradle_version(&text) {
                let compatible = gradle_compatible(&version, recommended);
                candidates.push((version, "PATH".into(), false, compatible));
            }
        }
    }

    let selected = candidates
        .iter()
        .find(|(_, _, _, compatible)| *compatible)
        .or_else(|| candidates.first());
    if let Some((version, path, managed, compatible)) = selected {
        MinecraftToolStatus {
            found: true,
            compatible: *compatible,
            managed: *managed,
            version: version.clone(),
            latest_version: recommended.into(),
            path: path.clone(),
            detail: if path.ends_with("gradle-wrapper.properties") {
                "Проект использует собственный Gradle Wrapper".into()
            } else if *compatible {
                "Gradle готов для сборки и запуска".into()
            } else {
                format!("Найден Gradle {version}, но проекту нужен совместимый Gradle {recommended}")
            },
            update_available: false,
        }
    } else {
        MinecraftToolStatus {
            found: false,
            compatible: false,
            managed: false,
            version: String::new(),
            latest_version: recommended.into(),
            path: String::new(),
            detail: format!("Gradle {recommended} не найден"),
            update_available: false,
        }
    }
}

fn version_at_least(version: &str, major: u64, minor: u64, patch: u64) -> bool {
    let values: Vec<u64> = Regex::new(r"\d+")
        .unwrap()
        .find_iter(version)
        .take(3)
        .filter_map(|value| value.as_str().parse().ok())
        .collect();
    (
        *values.first().unwrap_or(&0),
        *values.get(1).unwrap_or(&0),
        *values.get(2).unwrap_or(&0),
    ) >= (major, minor, patch)
}

pub fn recommended_gradle(loader: &str, minecraft_version: &str, java: u8) -> String {
    if loader == "forge" {
        if java <= 8 && !version_at_least(minecraft_version, 1, 16, 0) {
            return "4.10.3".into();
        }
        if java <= 8 {
            return "6.9.4".into();
        }
        if java <= 16 {
            return "7.3.3".into();
        }
        if java <= 17 {
            return "8.8".into();
        }
        if java <= 21 {
            return "8.14.3".into();
        }
        return "9.4.0".into();
    }
    if loader == "neoforge" {
        if java <= 17 {
            return "8.8".into();
        }
        if java <= 21 {
            return "8.14.3".into();
        }
        return "9.4.0".into();
    }
    match java {
        0..=8 => "8.14.3",
        9..=16 => "8.14.3",
        17..=21 => "8.14.3",
        _ => "9.4.0",
    }
    .into()
}

fn parse_u64(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .unwrap_or(0)
}

#[cfg(windows)]
fn windows_volumes() -> Result<Vec<(String, PathBuf, u64, u64)>, String> {
    let script = "Get-CimInstance Win32_LogicalDisk -Filter \"DriveType=3\" | Select-Object DeviceID,FreeSpace,Size | ConvertTo-Json -Compress";
    let mut last_error = String::new();
    for shell in ["powershell.exe", "pwsh.exe"] {
        let output = process::command(shell)
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
            .output();
        let output = match output {
            Ok(value) => value,
            Err(error) => {
                last_error = error.to_string();
                continue;
            }
        };
        if !output.status.success() {
            last_error = String::from_utf8_lossy(&output.stderr).trim().to_string();
            continue;
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("Windows вернула неверный список дисков: {error}"))?;
        let values = value.as_array().cloned().unwrap_or_else(|| vec![value]);
        let result = values
            .into_iter()
            .filter_map(|item| {
                let id = item.get("DeviceID")?.as_str()?.to_string();
                let free = parse_u64(item.get("FreeSpace")?);
                let total = parse_u64(item.get("Size")?);
                Some((id.clone(), PathBuf::from(format!("{id}\\")), free, total))
            })
            .filter(|(_, _, _, total)| *total > 0)
            .collect::<Vec<_>>();
        if !result.is_empty() {
            return Ok(result);
        }
    }
    Err(format!("Не удалось получить список локальных дисков: {last_error}"))
}

#[cfg(not(windows))]
fn unix_volumes() -> Result<Vec<(String, PathBuf, u64, u64)>, String> {
    let output = process::command("df")
        .args(["-Pk"])
        .output()
        .map_err(|error| format!("Не удалось проверить свободное место: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut values = Vec::new();
    for line in text.lines().skip(1) {
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() < 6 || !columns[0].starts_with('/') {
            continue;
        }
        let total = columns[1].parse::<u64>().unwrap_or(0).saturating_mul(1024);
        let free = columns[3].parse::<u64>().unwrap_or(0).saturating_mul(1024);
        let root = PathBuf::from(columns[5]);
        if total > 0 {
            values.push((columns[0].into(), root, free, total));
        }
    }
    if values.is_empty() {
        Err("Не найден ни один диск для установки".into())
    } else {
        Ok(values)
    }
}

fn path_on_volume(path: &Path, root: &Path) -> bool {
    if cfg!(windows) {
        path.to_string_lossy()
            .to_ascii_lowercase()
            .starts_with(&root.to_string_lossy().to_ascii_lowercase())
    } else {
        path.starts_with(root)
    }
}

fn available_after_install(free_bytes: u64, install_bytes: u64) -> u64 {
    free_bytes.saturating_sub(install_bytes)
}

fn preserves_free_space_reserve(free_bytes: u64, install_bytes: u64) -> bool {
    available_after_install(free_bytes, install_bytes) >= FREE_SPACE_RESERVE
}

fn storage_volumes(project_root: &Path, install_bytes: u64) -> Result<Vec<StorageVolume>, String> {
    #[cfg(windows)]
    let raw = windows_volumes()?;
    #[cfg(not(windows))]
    let raw = unix_volumes()?;

    let data_tools = dirs::data_dir()
        .or_else(dirs::home_dir)
        .ok_or("Не найдена папка данных")?
        .join("Funo Studio")
        .join("tools");
    let mut current_index = None;
    let mut current_root_length = 0;
    for (index, (_, root, _, _)) in raw.iter().enumerate() {
        if path_on_volume(project_root, root) && root.as_os_str().len() >= current_root_length {
            current_index = Some(index);
            current_root_length = root.as_os_str().len();
        }
    }
    if current_index.is_none() {
        current_index = raw.iter().position(|(_, root, _, _)| path_on_volume(&data_tools, root));
    }

    Ok(raw
        .into_iter()
        .enumerate()
        .map(|(index, (id, root, free, total))| {
            let install_root = if path_on_volume(&data_tools, &root) {
                data_tools.clone()
            } else {
                root.join("Funo Studio").join("tools")
            };
            StorageVolume {
                id,
                root: root.to_string_lossy().to_string(),
                install_root: install_root.to_string_lossy().to_string(),
                free_bytes: free,
                total_bytes: total,
                available_after_bytes: available_after_install(free, install_bytes),
                eligible: preserves_free_space_reserve(free, install_bytes),
                current: current_index == Some(index),
            }
        })
        .collect())
}

fn status_message(ready: bool, volumes: &[StorageVolume]) -> String {
    if ready {
        return "JDK и Gradle готовы. Funo автоматически использует их для Minecraft.".into();
    }
    let current = volumes.iter().find(|volume| volume.current);
    let alternative = volumes.iter().find(|volume| !volume.current && volume.eligible);
    if current.is_some_and(|volume| !volume.eligible) {
        if let Some(volume) = alternative {
            return format!(
                "На текущем диске нельзя сохранить резерв 30 ГиБ. Можно установить инструменты на диск {}.",
                volume.id
            );
        }
        return "После установки на доступных дисках не останется обязательных 30 ГиБ.".into();
    }
    "Установите недостающие инструменты. После установки на диске останется не менее 30 ГиБ.".into()
}

fn build_status(
    project_root: &Path,
    minecraft_version: &str,
    loader: &str,
    include_volumes: bool,
) -> Result<MinecraftToolchainStatus, String> {
    let required_java = project::java_for_minecraft(minecraft_version);
    let recommended = recommended_gradle(loader, minecraft_version, required_java);
    let jdk = detect_jdk(required_java);
    let gradle = detect_gradle(project_root, &recommended, required_java);
    let ready = jdk.compatible && gradle.compatible;
    let install_bytes = JDK_INSTALL_ESTIMATE + GRADLE_INSTALL_ESTIMATE;
    let volumes = if include_volumes {
        storage_volumes(project_root, install_bytes)?
    } else {
        Vec::new()
    };
    let recommended_install_root = volumes
        .iter()
        .find(|volume| volume.current && volume.eligible)
        .or_else(|| volumes.iter().find(|volume| volume.eligible))
        .map(|volume| volume.install_root.clone())
        .unwrap_or_default();
    Ok(MinecraftToolchainStatus {
        required_java,
        recommended_gradle: recommended,
        reserve_bytes: FREE_SPACE_RESERVE,
        estimated_install_bytes: install_bytes,
        jdk,
        gradle,
        volumes: volumes.clone(),
        recommended_install_root,
        ready,
        has_updates: false,
        message: status_message(ready, &volumes),
    })
}

pub fn local_status(
    project_root: &Path,
    minecraft_version: &str,
    loader: &str,
) -> Result<MinecraftToolchainStatus, String> {
    build_status(project_root, minecraft_version, loader, false)
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("Funo-Studio/1.0.0")
        .redirect(reqwest::redirect::Policy::limited(8))
        .timeout(Duration::from_secs(1800))
        .build()
        .map_err(|error| error.to_string())
}

fn platform() -> Result<(&'static str, &'static str), String> {
    let os = match env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "mac",
        value => return Err(format!("Автоустановка JDK пока не поддерживает {value}")),
    };
    let architecture = match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        "x86" => "x32",
        value => return Err(format!("Автоустановка JDK пока не поддерживает архитектуру {value}")),
    };
    Ok((os, architecture))
}

async fn jdk_package(required: u8) -> Result<DownloadPackage, String> {
    let (os, architecture) = platform()?;
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{required}/hotspot?architecture={architecture}&image_type=jdk&os={os}&vendor=eclipse"
    );
    let response = http_client()?
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Не удалось получить каталог Eclipse Temurin: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Eclipse Temurin вернул HTTP {} для JDK {required}", response.status()));
    }
    let assets: Value = response
        .json()
        .await
        .map_err(|error| format!("Eclipse Temurin вернул неверные данные: {error}"))?;
    let asset = assets
        .as_array()
        .and_then(|values| values.first())
        .ok_or_else(|| format!("Eclipse Temurin не публикует JDK {required} для этой системы"))?;
    let binary = asset
        .get("binary")
        .or_else(|| asset.get("binaries").and_then(Value::as_array).and_then(|values| values.first()))
        .ok_or("В каталоге Eclipse Temurin нет подходящего JDK")?;
    let package = binary.get("package").ok_or("В каталоге Eclipse Temurin нет архива JDK")?;
    let version = asset
        .pointer("/version_data/openjdk_version")
        .and_then(Value::as_str)
        .or_else(|| asset.pointer("/version_data/semver").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let name = package.get("name").and_then(Value::as_str).unwrap_or_default();
    Ok(DownloadPackage {
        version,
        url: package.get("link").and_then(Value::as_str).ok_or("У JDK отсутствует HTTPS-ссылка")?.into(),
        checksum: package.get("checksum").and_then(Value::as_str).ok_or("У JDK отсутствует SHA-256")?.into(),
        size: parse_u64(package.get("size").unwrap_or(&Value::Null)),
        extension: if name.ends_with(".zip") { "zip" } else { "tar.gz" },
    })
}

async fn current_gradle_version() -> Result<String, String> {
    let response = http_client()?
        .get("https://services.gradle.org/versions/current")
        .send()
        .await
        .map_err(|error| format!("Не удалось проверить обновления Gradle: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Gradle вернул HTTP {}", response.status()));
    }
    let value: Value = response.json().await.map_err(|error| format!("Gradle вернул неверные данные: {error}"))?;
    value
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "Gradle не сообщил номер текущей версии".into())
}

async fn gradle_package(version: &str) -> Result<DownloadPackage, String> {
    let url = format!("https://services.gradle.org/distributions/gradle-{version}-bin.zip");
    let checksum_url = format!("{url}.sha256");
    let client = http_client()?;
    let checksum_response = client
        .get(checksum_url)
        .send()
        .await
        .map_err(|error| format!("Не удалось получить SHA-256 Gradle {version}: {error}"))?;
    if !checksum_response.status().is_success() {
        return Err(format!("Gradle не публикует дистрибутив {version} (HTTP {})", checksum_response.status()));
    }
    let checksum = checksum_response
        .text()
        .await
        .map_err(|error| error.to_string())?
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    if !Regex::new(r"^[0-9a-fA-F]{64}$").unwrap().is_match(&checksum) {
        return Err("Gradle вернул некорректную контрольную сумму".into());
    }
    let size = client
        .head(&url)
        .send()
        .await
        .ok()
        .and_then(|response| response.content_length())
        .unwrap_or(GRADLE_INSTALL_ESTIMATE / 3);
    Ok(DownloadPackage {
        version: version.into(),
        url,
        checksum,
        size,
        extension: "zip",
    })
}

pub async fn status(
    project_root: &str,
    minecraft_version: &str,
    loader: &str,
    check_updates: bool,
) -> Result<MinecraftToolchainStatus, String> {
    let root = PathBuf::from(project_root);
    let mut status = build_status(&root, minecraft_version, loader, true)?;
    if !check_updates {
        return Ok(status);
    }

    let mut update_errors = Vec::new();
    match jdk_package(status.required_java).await {
        Ok(package) => {
            status.jdk.latest_version = package.version.clone();
            status.jdk.update_available = status.jdk.compatible
                && !status.jdk.version.is_empty()
                && release_newer(&package.version, &status.jdk.version);
        }
        Err(error) => update_errors.push(format!("JDK: {error}")),
    }
    if status.required_java >= 25 {
        match current_gradle_version().await {
            Ok(version) if gradle_compatible(&version, &status.recommended_gradle) => {
                status.recommended_gradle = version.clone();
                status.gradle.latest_version = version.clone();
                status.gradle.update_available = status.gradle.compatible
                    && !status.gradle.version.is_empty()
                    && release_newer(&version, &status.gradle.version);
            }
            Ok(_) => update_errors.push("Gradle: официальный выпуск несовместим с проектом".into()),
            Err(error) => update_errors.push(format!("Gradle: {error}")),
        }
    } else {
        status.gradle.latest_version = status.recommended_gradle.clone();
        status.gradle.update_available = status.gradle.managed
            && status.gradle.compatible
            && release_newer(&status.recommended_gradle, &status.gradle.version);
    }
    status.has_updates = status.jdk.update_available || status.gradle.update_available;
    status.message = if status.has_updates {
        "Для инструментов Minecraft доступны обновления. Выберите диск и нажмите «Обновить JDK и Gradle».".into()
    } else if !update_errors.is_empty() {
        format!(
            "Локальные версии проверены, но каталог обновлений сейчас недоступен: {}",
            update_errors.join("; ")
        )
    } else if status.ready {
        "JDK и Gradle готовы, доступных совместимых обновлений нет.".into()
    } else {
        status_message(false, &status.volumes)
    };
    Ok(status)
}

async fn download_verified(package: &DownloadPackage, destination: &Path) -> Result<(), String> {
    if !package.url.starts_with("https://")
        || !Regex::new(r"^[0-9a-fA-F]{64}$").unwrap().is_match(&package.checksum)
    {
        return Err("Источник инструмента не прошёл проверку безопасности".into());
    }
    let mut response = http_client()?
        .get(&package.url)
        .send()
        .await
        .map_err(|error| format!("Не удалось начать загрузку: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Сервер загрузки вернул HTTP {}", response.status()));
    }
    let mut file = fs::File::create(destination)
        .map_err(|error| format!("Не удалось создать временный архив: {error}"))?;
    let mut hash = Sha256::new();
    let mut received = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Загрузка была прервана: {error}"))?
    {
        received = received.saturating_add(chunk.len() as u64);
        if package.size > 0 && received > package.size.saturating_add(16 * 1024 * 1024) {
            let _ = fs::remove_file(destination);
            return Err("Загруженный архив оказался больше размера из официального каталога".into());
        }
        hash.update(&chunk);
        file.write_all(&chunk).map_err(|error| format!("Не удалось сохранить архив: {error}"))?;
    }
    file.flush().map_err(|error| error.to_string())?;
    let actual = hex::encode(hash.finalize());
    if !actual.eq_ignore_ascii_case(&package.checksum) {
        let _ = fs::remove_file(destination);
        return Err("SHA-256 загруженного инструмента не совпал. Архив удалён.".into());
    }
    Ok(())
}

fn extract_archive(archive: &Path, destination: &Path, extension: &str) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let output = if cfg!(windows) {
        process::command("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Expand-Archive -LiteralPath $env:FUNO_ARCHIVE -DestinationPath $env:FUNO_DESTINATION -Force",
            ])
            .env("FUNO_ARCHIVE", archive)
            .env("FUNO_DESTINATION", destination)
            .output()
    } else if extension == "zip" {
        process::command("unzip").arg("-q").arg(archive).arg("-d").arg(destination).output()
    } else {
        process::command("tar").arg("-xzf").arg(archive).arg("-C").arg(destination).output()
    }
    .map_err(|error| format!("Не удалось запустить распаковку архива: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("Не удалось распаковать инструмент: {stderr}"))
    }
}

fn find_home(root: &Path, relative_executable: &Path, depth: usize) -> Option<PathBuf> {
    if root.join(relative_executable).is_file() {
        return Some(root.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .find_map(|entry| find_home(&entry.path(), relative_executable, depth - 1))
}

fn replace_directory(source: &Path, destination: &Path) -> Result<(), String> {
    let backup = destination.with_extension("funo-backup");
    let _ = fs::remove_dir_all(&backup);
    if destination.exists() {
        fs::rename(destination, &backup)
            .map_err(|error| format!("Не удалось подготовить обновление {}: {error}", destination.display()))?;
    }
    if let Err(error) = fs::rename(source, destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, destination);
        }
        return Err(format!("Не удалось активировать инструмент: {error}"));
    }
    let _ = fs::remove_dir_all(backup);
    Ok(())
}

async fn install_package(
    kind: &str,
    requirement: &str,
    package: &DownloadPackage,
    tools_root: &Path,
) -> Result<ManagedTool, String> {
    fs::create_dir_all(tools_root)
        .map_err(|error| format!("Не удалось создать {}: {error}", tools_root.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let archive = tools_root.join(format!(".funo-{kind}-{stamp}.{}", package.extension));
    let staging = tools_root.join(format!(".funo-{kind}-{stamp}-extract"));
    let destination = tools_root.join(if kind == "jdk" {
        format!("jdk-{requirement}")
    } else {
        // Keep a stable requirement directory so updates atomically replace the
        // previous compatible Gradle instead of accumulating old distributions.
        format!("gradle-{requirement}")
    });
    let relative = if kind == "jdk" {
        PathBuf::from("bin").join(if cfg!(windows) { "javac.exe" } else { "javac" })
    } else {
        PathBuf::from("bin").join(if cfg!(windows) { "gradle.bat" } else { "gradle" })
    };

    download_verified(package, &archive).await?;
    if let Err(error) = extract_archive(&archive, &staging, package.extension) {
        let _ = fs::remove_file(&archive);
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let home = find_home(&staging, &relative, 4).ok_or_else(|| {
        format!("В официальном архиве {kind} не найден {}", relative.display())
    })?;
    if let Err(error) = replace_directory(&home, &destination) {
        let _ = fs::remove_file(&archive);
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let _ = fs::remove_file(&archive);
    let _ = fs::remove_dir_all(&staging);
    Ok(ManagedTool {
        kind: kind.into(),
        requirement: requirement.into(),
        version: package.version.clone(),
        home: destination.to_string_lossy().to_string(),
    })
}

fn same_path(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .trim_end_matches(|character| character == '\\' || character == '/')
            .to_ascii_lowercase()
            == right
                .to_string_lossy()
                .trim_end_matches(|character| character == '\\' || character == '/')
                .to_ascii_lowercase()
    } else {
        left == right
    }
}

pub async fn install(
    project_root: &str,
    minecraft_version: &str,
    loader: &str,
    destination_root: &str,
) -> Result<MinecraftToolchainStatus, String> {
    let project_root = PathBuf::from(project_root);
    let required_java = project::java_for_minecraft(minecraft_version);
    let gradle_requirement = recommended_gradle(loader, minecraft_version, required_java);
    let mut gradle_version = gradle_requirement.clone();
    if required_java >= 25 {
        if let Ok(current) = current_gradle_version().await {
            if gradle_compatible(&current, &gradle_requirement) {
                gradle_version = current;
            }
        }
    }
    let jdk = jdk_package(required_java).await?;
    let gradle = gradle_package(&gradle_version).await?;
    let required_bytes = jdk
        .size
        .saturating_mul(3)
        .saturating_add(gradle.size.saturating_mul(3))
        .max(JDK_INSTALL_ESTIMATE + GRADLE_INSTALL_ESTIMATE);
    let volumes = storage_volumes(&project_root, required_bytes)?;
    let selected = volumes
        .iter()
        .find(|volume| same_path(Path::new(&volume.install_root), Path::new(destination_root)))
        .ok_or("Выберите папку установки из списка локальных дисков Funo")?;
    if !selected.eligible {
        let alternatives = volumes
            .iter()
            .filter(|volume| volume.eligible)
            .map(|volume| volume.id.as_str())
            .collect::<Vec<_>>();
        return Err(if alternatives.is_empty() {
            "Установка отменена: ни на одном диске после установки не останется 30 ГиБ.".into()
        } else {
            format!(
                "На диске {} не останется 30 ГиБ. Выберите диск: {}.",
                selected.id,
                alternatives.join(", ")
            )
        });
    }

    let tools_root = PathBuf::from(destination_root);
    let installed_jdk = install_package("jdk", &required_java.to_string(), &jdk, &tools_root).await?;
    record_tool(installed_jdk)?;
    let installed_gradle = install_package("gradle", &gradle_requirement, &gradle, &tools_root).await?;
    record_tool(installed_gradle)?;
    status(
        project_root.to_string_lossy().as_ref(),
        minecraft_version,
        loader,
        false,
    )
    .await
}

fn manifest_value(source: &str, section: &str, key: &str) -> Option<String> {
    let mut current = "";
    for raw_line in source.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') && line.ends_with(']') {
            current = line.trim_matches(|value| value == '[' || value == ']').trim();
            continue;
        }
        if current != section {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            if name.trim() == key {
                return Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

pub fn project_requirements(project_root: &Path) -> Result<(String, String), String> {
    let source = fs::read_to_string(project_root.join("funo.toml"))
        .map_err(|error| format!("Не удалось прочитать funo.toml: {error}"))?;
    let version = manifest_value(&source, "minecraft", "version")
        .ok_or("В funo.toml не указана версия Minecraft")?;
    let loader = manifest_value(&source, "minecraft", "loader")
        .ok_or("В funo.toml не указан загрузчик Minecraft")?;
    Ok((version, loader))
}

pub fn prepare_gradle_command(
    project_root: &Path,
    minecraft_version: &str,
    loader: &str,
) -> Result<Command, String> {
    let required_java = project::java_for_minecraft(minecraft_version);
    let recommended = recommended_gradle(loader, minecraft_version, required_java);
    let wrapper = project_root.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" });
    let mut command = if wrapper.is_file() {
        if cfg!(windows) {
            process::command(&wrapper)
        } else {
            let mut value = process::command("sh");
            value.arg(&wrapper);
            value
        }
    } else if let Some(tool) = managed_tool(
        "gradle",
        &recommended,
        if cfg!(windows) { "bin/gradle.bat" } else { "bin/gradle" },
    ) {
        process::command(gradle_executable(Path::new(&tool.home)))
    } else {
        process::command(if cfg!(windows) { "gradle.bat" } else { "gradle" })
    };

    apply_managed_jdk(&mut command, required_java);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_java_versions() {
        assert_eq!(java_major(&java_version("javac 1.8.0_442").unwrap()), Some(8));
        assert_eq!(java_major(&java_version("javac 21.0.7").unwrap()), Some(21));
        assert_eq!(java_major(&java_version("openjdk version \"25.0.1\" 2025-10-21").unwrap()), Some(25));
    }

    #[test]
    fn chooses_loader_compatible_gradle() {
        assert_eq!(recommended_gradle("forge", "1.15.2", 8), "4.10.3");
        assert_eq!(recommended_gradle("forge", "1.16.5", 8), "6.9.4");
        assert_eq!(recommended_gradle("forge", "1.20.1", 17), "8.8");
        assert_eq!(recommended_gradle("neoforge", "1.21.1", 21), "8.14.3");
        assert_eq!(recommended_gradle("fabric", "26.2", 25), "9.4.0");
    }

    #[test]
    fn identifies_compatible_gradle_ranges() {
        assert!(gradle_compatible("4.10.3", "4.10.3"));
        assert!(!gradle_compatible("6.9.4", "4.10.3"));
        assert!(gradle_compatible("8.14.3", "8.8"));
        assert!(!gradle_compatible("8.7", "8.8"));
        assert!(!gradle_compatible("9.4", "8.14.3"));
    }

    #[test]
    fn compares_java_and_gradle_updates_numerically() {
        assert!(release_newer("1.8.0_452-b09", "1.8.0_442"));
        assert!(release_newer("21.0.8+9-LTS", "21.0.7"));
        assert!(!release_newer("21.0.7+6", "21.0.7"));
        assert!(release_newer("9.4.0", "9.3.1"));
    }

    #[test]
    fn reserve_is_checked_after_installation() {
        let install = JDK_INSTALL_ESTIMATE + GRADLE_INSTALL_ESTIMATE;
        let exact = FREE_SPACE_RESERVE + install;
        assert_eq!(available_after_install(exact, install), FREE_SPACE_RESERVE);
        assert!(preserves_free_space_reserve(exact, install));
        assert!(!preserves_free_space_reserve(exact - 1, install));
        assert!(!preserves_free_space_reserve(install - 1, install));
    }
}

use crate::models::{MinecraftVersion, Project, ProjectFile};
use regex::Regex;
use serde::Deserialize;
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    time::Duration,
};

const FABRIC_GAME_META: &str = "https://meta.fabricmc.net/v2/versions/game";
const FABRIC_LOOM_META: &str = "https://maven.fabricmc.net/net/fabricmc/fabric-loom/maven-metadata.xml";
const FORGE_META: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const NEOFORGE_META: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

#[derive(Debug, Clone)]
struct MinecraftProfile {
    loader: String,
    minecraft: String,
    loader_version: String,
    api_version: Option<String>,
    mappings: Option<String>,
    build_plugin: String,
    java: u8,
}

#[derive(Debug, Deserialize)]
struct FabricGameVersion {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderVersion {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoaderVersion,
}

#[derive(Debug, Deserialize)]
struct FabricYarnVersion {
    version: String,
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    version_number: String,
    version_type: String,
}

fn projects_home() -> Result<PathBuf, String> {
    let base = dirs::document_dir()
        .or_else(dirs::home_dir)
        .ok_or("Не удалось найти домашнюю папку")?;
    Ok(base.join("FunoProjects"))
}

fn safe_relative(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("Путь файла должен находиться внутри проекта".into());
    }
    Ok(candidate)
}

fn write_if_missing(path: &Path, content: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, content)
        .map_err(|error| format!("Не удалось записать {}: {error}", path.display()))
}

pub fn ensure_demo_project() -> Result<Project, String> {
    let root = projects_home()?.join("hello-funo");
    fs::create_dir_all(&root).map_err(|error| format!("Не удалось создать проект: {error}"))?;
    write_if_missing(
        &root.join("main.fun"),
        r#"fun fib(n: int) -> int = if n < 2 then n else fib(n - 1) + fib(n - 2)

fun main() {
    text title = "Привет из Funo Studio!"
    int answer = fib(10)
    bool ready = answer == 55

    println(title)
    println(answer)
    if ready {
        println("Типы и условия работают")
    }
    return(200)
}"#,
    )?;
    write_if_missing(
        &root.join("funo.toml"),
        r#"[project]
name = "Мой первый проект"
kind = "console"
target = "jvm-21"

[success]
code = 200

[registry]
official = "https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL"
"#,
    )?;
    write_if_missing(
        &root.join("src/minecraft.fun"),
        r#"use minecraft.fabric

mod "hello_funo" {
    on start {
        log("Мод Funo загружен")
    }

    on server_start {
        broadcast("Сервер готов!")
    }

    on player_join(player) {
        tell("Добро пожаловать!")
    }
}
"#,
    )?;
    load_project(&root)
}

pub fn write_project_file(project_root: &str, relative_path: &str, content: &str) -> Result<(), String> {
    let root = PathBuf::from(project_root);
    if !root.is_absolute() {
        return Err("Некорректная папка проекта".into());
    }
    let relative = safe_relative(relative_path)?;
    let destination = root.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(destination, content).map_err(|error| format!("Не удалось сохранить файл: {error}"))
}

pub fn load_project(root: &Path) -> Result<Project, String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files, 0)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = files
        .iter()
        .find(|file| file.path == "funo.toml")
        .map(|file| file.content.as_str())
        .unwrap_or("");
    let name_re = Regex::new(r#"(?m)^name\s*=\s*"([^"]+)""#).unwrap();
    let kind_re = Regex::new(r#"(?m)^kind\s*=\s*"([^"]+)""#).unwrap();
    let name = name_re
        .captures(manifest)
        .map(|captures| captures[1].to_string())
        .unwrap_or_else(|| root.file_name().unwrap_or_default().to_string_lossy().to_string());
    let kind = kind_re
        .captures(manifest)
        .map(|captures| captures[1].to_string())
        .unwrap_or_else(|| "console".into());
    Ok(Project {
        root: root.to_string_lossy().to_string(),
        name,
        kind,
        files,
    })
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<ProjectFile>, depth: usize) -> Result<(), String> {
    if depth > 6 {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|error| format!("Не удалось прочитать проект: {error}"))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".funo" || name == ".gradle" || name == "build" || name == "run" {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out, depth + 1)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("fun" | "toml" | "json" | "gradle" | "properties" | "md")
        ) || path.file_name().and_then(|value| value.to_str()) == Some("settings.gradle")
        {
            if let Ok(content) = fs::read_to_string(&path) {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(ProjectFile { path: relative, content });
            }
        }
    }
    Ok(())
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .user_agent(format!("Funo-Studio/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("Не удалось создать сетевой клиент: {error}"))
}

async fn fetch_text(url: &str, label: &str) -> Result<String, String> {
    let response = http_client()?
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Не удалось получить каталог {label}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Каталог {label} ответил HTTP {}", response.status()));
    }
    response
        .text()
        .await
        .map_err(|error| format!("Не удалось прочитать каталог {label}: {error}"))
}

fn numeric_parts(value: &str) -> Vec<u64> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left_parts = numeric_parts(left);
    let right_parts = numeric_parts(right);
    for index in 0..left_parts.len().max(right_parts.len()) {
        let left_value = *left_parts.get(index).unwrap_or(&0);
        let right_value = *right_parts.get(index).unwrap_or(&0);
        match left_value.cmp(&right_value) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    left.cmp(right)
}

fn is_prerelease(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    ["snapshot", "alpha", "beta", "-rc", "_rc", "pre"].iter().any(|marker| value.contains(marker))
}

fn version_at_least(version: &str, major: u64, minor: u64, patch: u64) -> bool {
    let parts = numeric_parts(version);
    let candidate = (
        *parts.first().unwrap_or(&0),
        *parts.get(1).unwrap_or(&0),
        *parts.get(2).unwrap_or(&0),
    );
    candidate >= (major, minor, patch)
}

fn java_for_minecraft(version: &str) -> u8 {
    if !version.starts_with("1.") || version_at_least(version, 26, 1, 0) {
        25
    } else if version_at_least(version, 1, 20, 5) {
        21
    } else if version_at_least(version, 1, 18, 0) {
        17
    } else if version_at_least(version, 1, 17, 0) {
        16
    } else {
        8
    }
}

fn parse_xml_versions(xml: &str) -> Vec<String> {
    Regex::new(r"<version>\s*([^<\s]+)\s*</version>")
        .unwrap()
        .captures_iter(xml)
        .map(|captures| captures[1].to_string())
        .collect()
}

fn forge_minecraft_version(coordinate: &str) -> Option<String> {
    Regex::new(r"^((?:1\.\d+(?:\.\d+)?)|(?:\d{2}\.\d+(?:\.\d+)?))-")
        .unwrap()
        .captures(coordinate)
        .map(|captures| captures[1].to_string())
}

fn neoforge_minecraft_version(coordinate: &str) -> Option<String> {
    let parts = numeric_parts(coordinate);
    match parts.as_slice() {
        [20, minor, ..] => Some(format!("1.20.{minor}")),
        [21, minor, ..] => {
            if *minor == 0 { Some("1.21".into()) } else { Some(format!("1.21.{minor}")) }
        }
        [year, release, point, ..] if *year >= 26 => {
            if *point == 0 { Some(format!("{year}.{release}")) } else { Some(format!("{year}.{release}.{point}")) }
        }
        _ => None,
    }
}

fn versions_from_coordinates(loader: &str, coordinates: &[String]) -> Vec<MinecraftVersion> {
    let mut versions = BTreeMap::<String, bool>::new();
    for coordinate in coordinates {
        let minecraft = if loader == "forge" {
            forge_minecraft_version(coordinate)
        } else {
            neoforge_minecraft_version(coordinate)
        };
        if let Some(minecraft) = minecraft {
            let supported = if loader == "forge" {
                // Official Mojang mappings, used by maintained ForgeGradle, begin at 1.14.4.
                !minecraft.starts_with("1.") || version_at_least(&minecraft, 1, 14, 4)
            } else {
                !minecraft.starts_with("1.") || version_at_least(&minecraft, 1, 20, 2)
            };
            if supported {
                let stable = !is_prerelease(coordinate) && !is_prerelease(&minecraft);
                versions.entry(minecraft).and_modify(|current| *current |= stable).or_insert(stable);
            }
        }
    }
    let mut result: Vec<_> = versions
        .into_iter()
        .map(|(version, stable)| MinecraftVersion {
            java: java_for_minecraft(&version),
            id: version.clone(),
            label: version,
            stable,
        })
        .collect();
    result.sort_by(|left, right| {
        right
            .stable
            .cmp(&left.stable)
            .then_with(|| natural_cmp(&right.id, &left.id))
    });
    result
}

fn fallback_versions(loader: &str) -> Vec<MinecraftVersion> {
    let values: &[&str] = match loader {
        "fabric" => &["26.2", "26.1.2", "1.21.11", "1.21.1", "1.20.6", "1.20.1", "1.19.4", "1.18.2", "1.17.1", "1.16.5", "1.15.2", "1.14.4"],
        "forge" => &["26.2", "26.1.2", "1.21.11", "1.21.1", "1.20.6", "1.20.1", "1.19.4", "1.18.2", "1.17.1", "1.16.5", "1.15.2", "1.14.4"],
        "neoforge" => &["26.2", "26.1.2", "1.21.11", "1.21.10", "1.21.8", "1.21.6", "1.21.5", "1.21.4", "1.21.3", "1.21.1", "1.21", "1.20.6", "1.20.4", "1.20.3", "1.20.2"],
        _ => &[],
    };
    values
        .iter()
        .map(|version| MinecraftVersion {
            id: (*version).to_string(),
            label: (*version).to_string(),
            stable: true,
            java: java_for_minecraft(version),
        })
        .collect()
}

pub async fn minecraft_versions(loader: &str) -> Result<Vec<MinecraftVersion>, String> {
    match loader {
        "fabric" => {
            let response = http_client()?.get(FABRIC_GAME_META).send().await;
            let games: Vec<FabricGameVersion> = match response {
                Ok(response) if response.status().is_success() => response
                    .json()
                    .await
                    .map_err(|error| format!("Fabric Meta вернул повреждённый каталог: {error}"))?,
                _ => return Ok(fallback_versions(loader)),
            };
            let mut result: Vec<_> = games
                .into_iter()
                .filter(|game| {
                    !game.version.starts_with("1.") || version_at_least(&game.version, 1, 14, 0)
                })
                .map(|game| MinecraftVersion {
                    java: java_for_minecraft(&game.version),
                    id: game.version.clone(),
                    label: game.version,
                    stable: game.stable,
                })
                .collect();
            result.sort_by(|left, right| {
                right
                    .stable
                    .cmp(&left.stable)
                    .then_with(|| natural_cmp(&right.id, &left.id))
            });
            Ok(result)
        }
        "forge" | "neoforge" => {
            let url = if loader == "forge" { FORGE_META } else { NEOFORGE_META };
            match fetch_text(url, loader).await {
                Ok(xml) => Ok(versions_from_coordinates(loader, &parse_xml_versions(&xml))),
                Err(_) => Ok(fallback_versions(loader)),
            }
        }
        _ => Err("Загрузчик должен быть fabric, forge или neoforge".into()),
    }
}

fn newest_matching_coordinate(loader: &str, minecraft: &str, coordinates: &[String]) -> Option<String> {
    let mut matching: Vec<_> = coordinates
        .iter()
        .filter(|coordinate| {
            if loader == "forge" {
                forge_minecraft_version(coordinate).as_deref() == Some(minecraft)
            } else {
                neoforge_minecraft_version(coordinate).as_deref() == Some(minecraft)
            }
        })
        .cloned()
        .collect();
    if matching.iter().any(|coordinate| !is_prerelease(coordinate)) {
        matching.retain(|coordinate| !is_prerelease(coordinate));
    }
    matching.into_iter().max_by(|left, right| natural_cmp(left, right))
}

async fn resolve_fabric(minecraft: &str) -> Result<MinecraftProfile, String> {
    let client = http_client()?;
    let loader_url = format!("https://meta.fabricmc.net/v2/versions/loader/{minecraft}");
    let loader_response = client
        .get(loader_url)
        .send()
        .await
        .map_err(|error| format!("Не удалось подобрать Fabric Loader для Minecraft {minecraft}: {error}"))?;
    if !loader_response.status().is_success() {
        return Err(format!("Fabric не публикует Loader для Minecraft {minecraft}"));
    }
    let loaders: Vec<FabricLoaderEntry> = loader_response
        .json()
        .await
        .map_err(|error| format!("Fabric Meta вернул неверные данные Loader: {error}"))?;
    let loader_version = loaders
        .iter()
        .find(|entry| entry.loader.stable)
        .or_else(|| loaders.first())
        .map(|entry| entry.loader.version.clone())
        .ok_or_else(|| format!("Для Minecraft {minecraft} не найден Fabric Loader"))?;

    let yarn_url = format!("https://meta.fabricmc.net/v2/versions/yarn/{minecraft}");
    let mappings = if !minecraft.starts_with("1.") || version_at_least(minecraft, 26, 1, 0) {
        None
    } else {
        let response = client
            .get(yarn_url)
            .send()
            .await
            .map_err(|error| format!("Не удалось подобрать Yarn для Minecraft {minecraft}: {error}"))?;
        let yarn: Vec<FabricYarnVersion> = response
            .json()
            .await
            .map_err(|error| format!("Fabric Meta вернул неверные данные Yarn: {error}"))?;
        Some(
            yarn.first()
                .map(|entry| entry.version.clone())
                .ok_or_else(|| format!("Для Minecraft {minecraft} не найдены Yarn mappings"))?,
        )
    };

    let game_versions = serde_json::to_string(&vec![minecraft]).unwrap();
    let loaders_filter = serde_json::to_string(&vec!["fabric"]).unwrap();
    let api_response = client
        .get("https://api.modrinth.com/v2/project/P7dR8mSH/version")
        .query(&[("game_versions", game_versions), ("loaders", loaders_filter)])
        .send()
        .await
        .map_err(|error| format!("Не удалось подобрать Fabric API для Minecraft {minecraft}: {error}"))?;
    if !api_response.status().is_success() {
        return Err(format!("Modrinth не смог подобрать Fabric API (HTTP {})", api_response.status()));
    }
    let api_versions: Vec<ModrinthVersion> = api_response
        .json()
        .await
        .map_err(|error| format!("Modrinth вернул неверные данные Fabric API: {error}"))?;
    let api_version = api_versions
        .iter()
        .find(|entry| entry.version_type == "release")
        .or_else(|| api_versions.first())
        .map(|entry| entry.version_number.clone())
        .ok_or_else(|| format!("Для Minecraft {minecraft} не опубликован совместимый Fabric API"))?;

    let loom_xml = fetch_text(FABRIC_LOOM_META, "Fabric Loom").await?;
    let build_plugin = parse_xml_versions(&loom_xml)
        .into_iter()
        .filter(|version| !version.to_ascii_lowercase().contains("snapshot"))
        .max_by(|left, right| natural_cmp(left, right))
        .ok_or("Не удалось подобрать стабильную версию Fabric Loom")?;

    Ok(MinecraftProfile {
        loader: "fabric".into(),
        minecraft: minecraft.into(),
        loader_version,
        api_version: Some(api_version),
        mappings,
        build_plugin,
        java: java_for_minecraft(minecraft),
    })
}

async fn resolve_maven_loader(loader: &str, minecraft: &str) -> Result<MinecraftProfile, String> {
    let url = if loader == "forge" { FORGE_META } else { NEOFORGE_META };
    let xml = fetch_text(url, loader).await?;
    let coordinate = newest_matching_coordinate(loader, minecraft, &parse_xml_versions(&xml))
        .ok_or_else(|| format!("{loader} не публикует сборку для Minecraft {minecraft}"))?;
    let build_plugin = if loader == "neoforge" {
        if version_at_least(minecraft, 1, 20, 4) || !minecraft.starts_with("1.") {
            "2.0.143".into()
        } else {
            "7.0.116".into()
        }
    } else if !minecraft.starts_with("1.") || version_at_least(minecraft, 26, 1, 0) {
        "[7.0.17,8)".into()
    } else if version_at_least(minecraft, 1, 20, 0) {
        "6.0.54".into()
    } else if version_at_least(minecraft, 1, 17, 0) {
        "5.1.77".into()
    } else if version_at_least(minecraft, 1, 16, 0) {
        "4.1.16".into()
    } else {
        "3.0.197".into()
    };
    Ok(MinecraftProfile {
        loader: loader.into(),
        minecraft: minecraft.into(),
        loader_version: coordinate,
        api_version: None,
        mappings: None,
        build_plugin,
        java: java_for_minecraft(minecraft),
    })
}

async fn resolve_profile(loader: &str, minecraft: &str) -> Result<MinecraftProfile, String> {
    if !Regex::new(r"^[0-9A-Za-z._+-]{1,48}$").unwrap().is_match(minecraft) {
        return Err("Некорректная версия Minecraft".into());
    }
    match loader {
        "fabric" => resolve_fabric(minecraft).await,
        "forge" | "neoforge" => resolve_maven_loader(loader, minecraft).await,
        _ => Err("Поддерживаются Fabric, Forge и NeoForge".into()),
    }
}

fn escape_toml(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', " ")
}

fn escape_funo_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub async fn create_minecraft_project(
    name: &str,
    mod_id: &str,
    loader: &str,
    minecraft_version: &str,
) -> Result<Project, String> {
    if !Regex::new(r"^[a-z][a-z0-9_]{2,63}$").unwrap().is_match(mod_id) {
        return Err("ID мода должен содержать маленькие латинские буквы, цифры и _".into());
    }
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err("Укажите название мода без управляющих символов".into());
    }
    let profile = resolve_profile(loader, minecraft_version).await?;
    let root = projects_home()?.join(mod_id);
    if root.exists() {
        return Err(format!("Проект {} уже существует", root.display()));
    }
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let result = create_minecraft_files(&root, name.trim(), mod_id, &profile).and_then(|_| load_project(&root));
    if result.is_err() {
        let _ = fs::remove_dir_all(&root);
    }
    result
}

fn create_minecraft_files(root: &Path, name: &str, mod_id: &str, profile: &MinecraftProfile) -> Result<(), String> {
    let loader = &profile.loader;
    let minecraft = &profile.minecraft;
    let escaped_name = escape_funo_string(name);
    let main_fun = format!(
        r#"use minecraft.{loader}

mod "{mod_id}" {{
    on start {{
        log("Мод {escaped_name} загружен")
    }}

    on server_start {{
        broadcast("Сервер Minecraft {minecraft} запущен с модом {escaped_name}!")
        run_command("time set day")
    }}

    on player_join(player) {{
        tell("Добро пожаловать на сервер!")
        // give("minecraft:diamond", 1)
    }}
}}
"#
    );
    fs::write(root.join("main.fun"), main_fun).map_err(|error| error.to_string())?;

    let manifest = format!(
        r#"[project]
name = "{}"
kind = "minecraft-{loader}"
target = "jvm-{}"

[minecraft]
mod_id = "{mod_id}"
loader = "{loader}"
version = "{minecraft}"
loader_version = "{}"

[registry]
official = "https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL"
"#,
        escape_toml(name),
        profile.java,
        escape_toml(&profile.loader_version),
    );
    fs::write(root.join("funo.toml"), manifest).map_err(|error| error.to_string())?;

    match loader.as_str() {
        "fabric" => create_fabric_files(root, name, mod_id, profile)?,
        "forge" => create_forge_files(root, name, mod_id, profile)?,
        "neoforge" => create_neoforge_files(root, name, mod_id, profile)?,
        _ => return Err("Неизвестный загрузчик".into()),
    }

    fs::write(
        root.join("README.md"),
        format!(
            "# {name}\n\nMinecraft-мод на Funo: **{loader}**, Minecraft **{minecraft}**, Java **{}**.\n\nГлавный исходник — `main.fun`. Funo Studio обновляет Java-мост перед каждой сборкой.\n\n```bash\nfuno minecraft build\n# или, если Gradle установлен отдельно:\ngradle build\n```\n\nТочные координаты загрузчика разрешены генератором из официального каталога: `{}`.\n",
            profile.java, profile.loader_version
        ),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn create_fabric_files(root: &Path, name: &str, mod_id: &str, profile: &MinecraftProfile) -> Result<(), String> {
    let unobfuscated = !profile.minecraft.starts_with("1.") || version_at_least(&profile.minecraft, 26, 1, 0);
    let plugin_id = if unobfuscated { "net.fabricmc.fabric-loom" } else { "net.fabricmc.fabric-loom-remap" };
    let mappings = profile
        .mappings
        .as_ref()
        .map(|version| format!("    mappings 'net.fabricmc:yarn:{version}:v2'\n"))
        .unwrap_or_default();
    let api_version = profile.api_version.as_deref().ok_or("Не указана версия Fabric API")?;
    let gradle = format!(
        r#"plugins {{
    id '{plugin_id}' version '{loom}'
    id 'maven-publish'
}}

group = 'funo.mods'
version = '1.0.0'
base {{ archivesName = '{mod_id}' }}

repositories {{
    mavenCentral()
    maven {{ url = 'https://maven.fabricmc.net/' }}
}}

dependencies {{
    minecraft 'com.mojang:minecraft:{minecraft}'
{mappings}    modImplementation 'net.fabricmc:fabric-loader:{loader}'
    modImplementation 'net.fabricmc.fabric-api:fabric-api:{api}'
}}

processResources {{
    inputs.property 'version', project.version
    filesMatching('fabric.mod.json') {{ expand 'version': project.version }}
}}

java {{
    toolchain.languageVersion = JavaLanguageVersion.of({java})
    withSourcesJar()
}}

tasks.withType(JavaCompile).configureEach {{ options.encoding = 'UTF-8' }}
"#,
        loom = profile.build_plugin,
        minecraft = profile.minecraft,
        loader = profile.loader_version,
        api = api_version,
        java = profile.java,
    );
    fs::write(root.join("build.gradle"), gradle).map_err(|error| error.to_string())?;
    fs::write(
        root.join("settings.gradle"),
        format!(
            "pluginManagement {{ repositories {{ gradlePluginPortal(); maven {{ url = 'https://maven.fabricmc.net/' }} }} }}\nrootProject.name = '{mod_id}'\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        root.join("gradle.properties"),
        "org.gradle.jvmargs=-Xmx2G\norg.gradle.parallel=true\norg.gradle.caching=true\n",
    )
    .map_err(|error| error.to_string())?;
    let json_name = serde_json::to_string(name).map_err(|error| error.to_string())?;
    let json = format!(
        r#"{{
  "schemaVersion": 1,
  "id": "{mod_id}",
  "version": "${{version}}",
  "name": {json_name},
  "environment": "*",
  "entrypoints": {{ "main": ["funo.generated.FunoMod"] }},
  "depends": {{
    "fabricloader": ">={loader}",
    "minecraft": "={minecraft}",
    "java": ">={java}",
    "fabric-api": "*"
  }}
}}
"#,
        loader = profile.loader_version,
        minecraft = profile.minecraft,
        java = profile.java,
    );
    write_generated_files(root, "fabric.mod.json", &json, FABRIC_BRIDGE)
}

fn create_forge_files(root: &Path, name: &str, mod_id: &str, profile: &MinecraftProfile) -> Result<(), String> {
    let current_gradle = !profile.minecraft.starts_with("1.") || version_at_least(&profile.minecraft, 26, 1, 0);
    let gradle = if current_gradle {
        format!(
            r#"plugins {{
    id 'java'
    id 'idea'
    id 'eclipse'
    id 'net.minecraftforge.gradle' version '{plugin}'
}}

version = '1.0.0'
group = 'funo.mods'
base {{ archivesName = '{mod_id}' }}
java.toolchain.languageVersion = JavaLanguageVersion.of({java})
sourceSets.main.resources {{ srcDir 'src/generated/resources' }}

minecraft {{
    runs {{
        configureEach {{
            workingDir = layout.projectDirectory.dir('run')
            systemProperty 'forge.enabledGameTestNamespaces', '{mod_id}'
        }}
        register('client')
        register('server') {{ args '--nogui' }}
    }}
}}

repositories {{
    minecraft.mavenizer(it)
    maven fg.forgeMaven
    maven fg.minecraftLibsMaven
    mavenCentral()
}}

dependencies {{ implementation minecraft.dependency('net.minecraftforge:forge:{coordinate}') }}
tasks.withType(JavaCompile).configureEach {{ options.encoding = 'UTF-8' }}
"#,
            plugin = profile.build_plugin,
            java = profile.java,
            coordinate = profile.loader_version,
        )
    } else {
        let legacy_gradle = profile.build_plugin.starts_with("3.");
        let archive_config = if legacy_gradle {
            format!("archivesBaseName = '{mod_id}'")
        } else {
            format!("base {{ archivesName = '{mod_id}' }}")
        };
        let java_config = if legacy_gradle {
            "sourceCompatibility = targetCompatibility = '1.8'".to_string()
        } else {
            format!("java.toolchain.languageVersion = JavaLanguageVersion.of({})", profile.java)
        };
        format!(
            r#"buildscript {{
    repositories {{ maven {{ url = 'https://maven.minecraftforge.net' }}; mavenCentral() }}
    dependencies {{ classpath 'net.minecraftforge.gradle:ForgeGradle:{plugin}' }}
}}
apply plugin: 'net.minecraftforge.gradle'
apply plugin: 'java'
apply plugin: 'eclipse'
apply plugin: 'idea'

version = '1.0.0'
group = 'funo.mods'
{archive_config}
{java_config}

minecraft {{
    mappings channel: 'official', version: '{minecraft}'
    runs {{
        client {{ workingDirectory project.file('run'); mods {{ {mod_id} {{ source sourceSets.main }} }} }}
        server {{ workingDirectory project.file('run'); args '--nogui'; mods {{ {mod_id} {{ source sourceSets.main }} }} }}
    }}
}}

repositories {{ mavenCentral() }}
dependencies {{ minecraft 'net.minecraftforge:forge:{coordinate}' }}
tasks.withType(JavaCompile).configureEach {{ options.encoding = 'UTF-8' }}
"#,
            plugin = profile.build_plugin,
            minecraft = profile.minecraft,
            coordinate = profile.loader_version,
        )
    };
    fs::write(root.join("build.gradle"), gradle).map_err(|error| error.to_string())?;
    fs::write(
        root.join("settings.gradle"),
        format!(
            "pluginManagement {{ repositories {{ gradlePluginPortal(); maven {{ url = 'https://maven.minecraftforge.net/' }} }} }}\nrootProject.name = '{mod_id}'\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        root.join("gradle.properties"),
        "org.gradle.jvmargs=-Xmx2G\norg.gradle.parallel=true\norg.gradle.caching=true\n",
    )
    .map_err(|error| error.to_string())?;
    let loader_major = forge_minecraft_version(&profile.loader_version)
        .and_then(|minecraft| profile.loader_version.strip_prefix(&(minecraft + "-")).map(str::to_string))
        .and_then(|loader| numeric_parts(&loader).first().copied())
        .unwrap_or(1);
    let required = "mandatory=true";
    let toml = format!(
        r#"modLoader="javafml"
loaderVersion="[{loader_major},)"
license="All Rights Reserved"

[[mods]]
modId="{mod_id}"
version="1.0.0"
displayName="{}"
description='''Minecraft-мод на языке Funo.'''

[[dependencies.{mod_id}]]
modId="forge"
{required}
versionRange="[{loader_major},)"
ordering="NONE"
side="BOTH"

[[dependencies.{mod_id}]]
modId="minecraft"
{required}
versionRange="[{}]"
ordering="NONE"
side="BOTH"
"#,
        escape_toml(name),
        profile.minecraft,
    );
    write_generated_files(root, "META-INF/mods.toml", &toml, &forge_bridge(mod_id, false, &profile.minecraft))
}

fn create_neoforge_files(root: &Path, name: &str, mod_id: &str, profile: &MinecraftProfile) -> Result<(), String> {
    let old_neogradle = profile.minecraft.starts_with("1.") && !version_at_least(&profile.minecraft, 1, 20, 4);
    let gradle = if old_neogradle {
        format!(
            r#"plugins {{
    id 'java-library'
    id 'eclipse'
    id 'idea'
    id 'net.neoforged.gradle.userdev' version '{plugin}'
}}
version = '1.0.0'
group = 'funo.mods'
base {{ archivesName = '{mod_id}' }}
java.toolchain.languageVersion = JavaLanguageVersion.of({java})

runs {{
    configureEach {{ modSource project.sourceSets.main }}
    client {{}}
    server {{ programArgument '--nogui' }}
}}

dependencies {{ implementation 'net.neoforged:neoforge:{coordinate}' }}
tasks.withType(JavaCompile).configureEach {{ options.encoding = 'UTF-8' }}
"#,
            plugin = profile.build_plugin,
            java = profile.java,
            coordinate = profile.loader_version,
        )
    } else {
        format!(
            r#"plugins {{
    id 'java-library'
    id 'maven-publish'
    id 'net.neoforged.moddev' version '{plugin}'
    id 'idea'
}}
version = '1.0.0'
group = 'funo.mods'
base {{ archivesName = '{mod_id}' }}
java.toolchain.languageVersion = JavaLanguageVersion.of({java})
sourceSets.main.resources {{ srcDir('src/generated/resources') }}

neoForge {{
    version = '{coordinate}'
    runs {{
        client {{ client() }}
        server {{ server(); programArgument '--nogui' }}
    }}
    mods {{
        "{mod_id}" {{ sourceSet(sourceSets.main) }}
    }}
}}

tasks.withType(JavaCompile).configureEach {{ options.encoding = 'UTF-8' }}
"#,
            plugin = profile.build_plugin,
            java = profile.java,
            coordinate = profile.loader_version,
        )
    };
    fs::write(root.join("build.gradle"), gradle).map_err(|error| error.to_string())?;
    let foojay_version = if old_neogradle { "0.8.0" } else { "1.0.0" };
    fs::write(
        root.join("settings.gradle"),
        format!(
            "pluginManagement {{ repositories {{ gradlePluginPortal(); maven {{ url = 'https://maven.neoforged.net/releases' }} }} }}\nplugins {{ id 'org.gradle.toolchains.foojay-resolver-convention' version '{foojay_version}' }}\nrootProject.name = '{mod_id}'\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        root.join("gradle.properties"),
        "org.gradle.jvmargs=-Xmx2G\norg.gradle.parallel=true\norg.gradle.caching=true\n",
    )
    .map_err(|error| error.to_string())?;
    let old_manifest = profile.minecraft.starts_with("1.") && !version_at_least(&profile.minecraft, 1, 20, 5);
    let required = if old_manifest { "mandatory=true" } else { "type=\"required\"" };
    let resource = if old_manifest { "META-INF/mods.toml" } else { "META-INF/neoforge.mods.toml" };
    let toml = format!(
        r#"modLoader="javafml"
loaderVersion="[1,)"
license="All Rights Reserved"

[[mods]]
modId="{mod_id}"
version="1.0.0"
displayName="{}"
description='''Minecraft-мод на языке Funo.'''

[[dependencies.{mod_id}]]
modId="neoforge"
{required}
versionRange="[{},)"
ordering="NONE"
side="BOTH"

[[dependencies.{mod_id}]]
modId="minecraft"
{required}
versionRange="[{}]"
ordering="NONE"
side="BOTH"
"#,
        escape_toml(name),
        profile.loader_version,
        profile.minecraft,
    );
    write_generated_files(root, resource, &toml, &forge_bridge(mod_id, true, &profile.minecraft))
}

fn forge_bridge(mod_id: &str, neoforge: bool, minecraft: &str) -> String {
    let (mod_import, bus_import, subscribe_import, server_import, player_import) = if neoforge {
        (
            "net.neoforged.fml.common.Mod",
            "net.neoforged.neoforge.common.NeoForge",
            "net.neoforged.bus.api.SubscribeEvent",
            "net.neoforged.neoforge.event.server.ServerStartedEvent",
            "net.neoforged.neoforge.event.entity.player.PlayerEvent.PlayerLoggedInEvent",
        )
    } else {
        let server_import = if version_at_least(minecraft, 1, 17, 0) || !minecraft.starts_with("1.") {
            "net.minecraftforge.event.server.ServerStartedEvent"
        } else {
            "net.minecraftforge.fml.event.server.FMLServerStartedEvent"
        };
        (
            "net.minecraftforge.fml.common.Mod",
            "net.minecraftforge.common.MinecraftForge",
            "net.minecraftforge.eventbus.api.SubscribeEvent",
            server_import,
            "net.minecraftforge.event.entity.player.PlayerEvent.PlayerLoggedInEvent",
        )
    };
    let server_type = server_import.rsplit('.').next().unwrap_or("ServerStartedEvent");
    let old_forge = !neoforge && minecraft.starts_with("1.") && !version_at_least(minecraft, 1, 17, 0);
    let extra_import = if old_forge {
        "import net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext;"
    } else {
        ""
    };
    let registration = if old_forge {
        "MinecraftForge.EVENT_BUS.register(this);\n        FMLJavaModLoadingContext.get().getModEventBus().addListener(this::onServerStarted);"
    } else if neoforge {
        "NeoForge.EVENT_BUS.register(this);"
    } else {
        "MinecraftForge.EVENT_BUS.register(this);"
    };
    format!(
        r#"package funo.generated;

import {mod_import};
import {bus_import};
import {subscribe_import};
import {server_import};
import {player_import};
{extra_import}

@Mod("{mod_id}")
public final class FunoMod {{
    public FunoMod() {{
        FunoMain.start();
        {registration}
    }}

    @SubscribeEvent
    public void onServerStarted({server_type} event) {{
        FunoMain.serverStart(value(event, new String[] {{ "getServer" }}, new String[] {{ "server" }}));
    }}

    @SubscribeEvent
    public void onPlayerJoin(PlayerLoggedInEvent event) {{
        FunoMain.playerJoin(value(event, new String[] {{ "getEntity", "getPlayer" }}, new String[] {{ "player", "entity" }}));
    }}

    private static Object value(Object target, String[] methods, String[] fields) {{
        for (String method : methods) try {{ return target.getClass().getMethod(method).invoke(target); }} catch (ReflectiveOperationException ignored) {{}}
        for (String field : fields) try {{ return target.getClass().getField(field).get(target); }} catch (ReflectiveOperationException ignored) {{}}
        return target;
    }}
}}
"#
    )
}

fn write_generated_files(root: &Path, resource: &str, resource_content: &str, bridge: &str) -> Result<(), String> {
    let resource_path = root.join("src/main/resources").join(resource);
    let java_dir = root.join("src/main/java/funo/generated");
    fs::create_dir_all(resource_path.parent().unwrap()).map_err(|error| error.to_string())?;
    fs::create_dir_all(&java_dir).map_err(|error| error.to_string())?;
    fs::write(resource_path, resource_content).map_err(|error| error.to_string())?;
    fs::write(java_dir.join("FunoMod.java"), bridge).map_err(|error| error.to_string())?;
    fs::write(
        java_dir.join("FunoMain.java"),
        r#"package funo.generated;
/** Этот файл обновляется компилятором Funo перед сборкой. */
public final class FunoMain {
    public static void start() { FunoMinecraft.log("Minecraft-мод Funo запущен!"); }
    public static void serverStart(Object server) { FunoMinecraft.bindServer(server); }
    public static void playerJoin(Object player) {}
}
"#,
    )
    .map_err(|error| error.to_string())?;
    fs::write(java_dir.join("FunoMinecraft.java"), FUNO_MINECRAFT_RUNTIME)
        .map_err(|error| error.to_string())?;
    Ok(())
}

const FABRIC_BRIDGE: &str = r#"package funo.generated;

import net.fabricmc.api.ModInitializer;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;

public final class FunoMod implements ModInitializer {
    private interface Callback { void call(Object[] arguments); }

    @Override public void onInitialize() {
        FunoMain.start();
        boolean server = register("net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents", "SERVER_STARTED", args -> FunoMain.serverStart(args[0]));
        if (!server) register("net.fabricmc.fabric.api.event.server.ServerStartCallback", "EVENT", args -> FunoMain.serverStart(args[0]));
        register("net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents", "JOIN", args -> FunoMain.playerJoin(player(args[0])));
    }

    private static boolean register(String ownerName, String fieldName, Callback callback) {
        try {
            Class<?> owner = Class.forName(ownerName);
            Object event = owner.getField(fieldName).get(null);
            Object invoker = event.getClass().getMethod("invoker").invoke(event);
            Class<?> listener = callbackInterface(invoker.getClass());
            if (listener == null) return false;
            Object proxy = Proxy.newProxyInstance(listener.getClassLoader(), new Class<?>[] { listener }, (object, method, args) -> {
                if (method.getDeclaringClass() == Object.class) {
                    if (method.getName().equals("toString")) return "FunoFabricEventProxy";
                    if (method.getName().equals("hashCode")) return System.identityHashCode(object);
                    if (method.getName().equals("equals")) return object == args[0];
                }
                callback.call(args == null ? new Object[0] : args);
                return null;
            });
            for (Method method : event.getClass().getMethods()) {
                if (method.getName().equals("register") && method.getParameterCount() == 1) {
                    method.invoke(event, proxy);
                    return true;
                }
            }
        } catch (ReflectiveOperationException error) {
            FunoMinecraft.log("Событие Fabric " + fieldName + " недоступно: " + error.getMessage());
        }
        return false;
    }

    private static Class<?> callbackInterface(Class<?> type) {
        for (Class<?> candidate : type.getInterfaces()) {
            if (!candidate.getName().equals("net.fabricmc.fabric.api.event.Event")) return candidate;
        }
        Class<?> parent = type.getSuperclass();
        return parent == null ? null : callbackInterface(parent);
    }

    private static Object player(Object handler) {
        for (String method : new String[] { "getPlayer", "player" }) try { return handler.getClass().getMethod(method).invoke(handler); } catch (ReflectiveOperationException ignored) {}
        for (String field : new String[] { "player", "field_14140" }) try { Field value = handler.getClass().getField(field); return value.get(handler); } catch (ReflectiveOperationException ignored) {}
        return handler;
    }
}
"#;

const FUNO_MINECRAFT_RUNTIME: &str = r#"package funo.generated;

import java.lang.reflect.Method;

/** Небольшой Funo API поверх Fabric, Forge и NeoForge. */
public final class FunoMinecraft {
    private static Object server;
    private FunoMinecraft() {}

    public static void bindServer(Object value) {
        server = value;
        log("Minecraft server API подключён");
    }

    public static void log(Object value) {
        System.out.println("[Funo] " + String.valueOf(value));
    }

    public static void broadcast(Object value) {
        if (!command("tellraw @a " + json(value))) log(value);
    }

    public static void actionbar(Object value) {
        command("title @a actionbar " + json(value));
    }

    public static void tell(Object player, Object value) {
        String name = playerName(player);
        if (!command("tellraw " + name + " " + json(value))) log(name + ": " + value);
    }

    public static void give(Object player, Object item, Object count) {
        command("give " + playerName(player) + " " + item + " " + count);
    }

    public static boolean command(Object value) {
        if (server == null) {
            log("Команда отложена до запуска сервера: " + value);
            return false;
        }
        try {
            Object manager = call(server, new String[] { "getCommandManager", "getCommands" });
            Object source = call(server, new String[] { "getCommandSource", "createCommandSourceStack" });
            invoke(manager, new String[] { "executeWithPrefix", "performPrefixedCommand" }, source, String.valueOf(value));
            return true;
        } catch (ReflectiveOperationException error) {
            log("Не удалось выполнить команду: " + error.getMessage());
            return false;
        }
    }

    private static String playerName(Object player) {
        try {
            Object profile = call(player, new String[] { "getGameProfile" });
            return String.valueOf(call(profile, new String[] { "getName" }));
        } catch (ReflectiveOperationException ignored) {
            try {
                return String.valueOf(call(player, new String[] { "getScoreboardName", "getName" }));
            } catch (ReflectiveOperationException error) {
                return "@s";
            }
        }
    }

    private static String json(Object value) {
        String text = String.valueOf(value).replace("\\", "\\\\").replace("\"", "\\\"");
        return "{\"text\":\"" + text + "\"}";
    }

    private static Object call(Object target, String[] names) throws ReflectiveOperationException {
        return invoke(target, names);
    }

    private static Object invoke(Object target, String[] names, Object... args) throws ReflectiveOperationException {
        for (String name : names) {
            for (Method method : target.getClass().getMethods()) {
                if (!method.getName().equals(name) || method.getParameterCount() != args.length) continue;
                try { return method.invoke(target, args); }
                catch (IllegalArgumentException ignored) { /* попробовать другую перегрузку */ }
            }
        }
        throw new NoSuchMethodException(String.join(" / ", names));
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_paths() {
        assert!(safe_relative("../secret").is_err());
    }

    #[test]
    fn accepts_source_path() {
        assert!(safe_relative("src/main.fun").is_ok());
    }

    #[test]
    fn parses_loader_coordinates() {
        assert_eq!(forge_minecraft_version("1.21.1-52.1.3").as_deref(), Some("1.21.1"));
        assert_eq!(forge_minecraft_version("26.2-65.1.1").as_deref(), Some("26.2"));
        assert_eq!(neoforge_minecraft_version("21.1.200").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_minecraft_version("26.1.2.95").as_deref(), Some("26.1.2"));
        assert_eq!(neoforge_minecraft_version("26.2.0.59").as_deref(), Some("26.2"));
    }

    #[test]
    fn selects_newest_loader_build() {
        let coordinates = vec!["1.21.1-52.0.1".into(), "1.21.1-52.1.3".into(), "1.20.1-47.4.0".into()];
        assert_eq!(newest_matching_coordinate("forge", "1.21.1", &coordinates).as_deref(), Some("1.21.1-52.1.3"));
    }

    #[test]
    fn java_matches_minecraft_era() {
        assert_eq!(java_for_minecraft("1.16.5"), 8);
        assert_eq!(java_for_minecraft("1.18.2"), 17);
        assert_eq!(java_for_minecraft("1.21.1"), 21);
        assert_eq!(java_for_minecraft("26.2"), 25);
    }

    #[test]
    fn modern_forge_manifest_keeps_forge_dependency_schema() {
        let root = std::env::temp_dir().join(format!("funo-forge-manifest-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let profile = MinecraftProfile {
            loader: "forge".into(),
            minecraft: "26.2".into(),
            loader_version: "26.2-65.1.1".into(),
            api_version: None,
            mappings: None,
            build_plugin: "[7.0.17,8)".into(),
            java: 25,
        };

        create_forge_files(&root, "Current Forge", "current_forge", &profile).unwrap();
        let manifest = fs::read_to_string(root.join("src/main/resources/META-INF/mods.toml")).unwrap();
        assert_eq!(manifest.matches("mandatory=true").count(), 2);
        assert!(!manifest.contains("type=\"required\""));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn generates_all_loader_layouts() {
        let root = std::env::temp_dir().join(format!("funo-generator-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let cases = [
            MinecraftProfile {
                loader: "fabric".into(),
                minecraft: "1.16.5".into(),
                loader_version: "0.16.14".into(),
                api_version: Some("0.42.0+1.16".into()),
                mappings: Some("1.16.5+build.10".into()),
                build_plugin: "1.15.4".into(),
                java: 8,
            },
            MinecraftProfile {
                loader: "forge".into(),
                minecraft: "1.20.1".into(),
                loader_version: "1.20.1-47.4.10".into(),
                api_version: None,
                mappings: None,
                build_plugin: "6.0.54".into(),
                java: 17,
            },
            MinecraftProfile {
                loader: "neoforge".into(),
                minecraft: "26.2".into(),
                loader_version: "26.2.0.59".into(),
                api_version: None,
                mappings: None,
                build_plugin: "2.0.143".into(),
                java: 25,
            },
        ];

        for profile in cases {
            let project = root.join(&profile.loader);
            fs::create_dir_all(&project).unwrap();
            create_minecraft_files(&project, "Generator Test", "generator_test", &profile).unwrap();
            assert!(project.join("build.gradle").is_file());
            assert!(project.join("src/main/java/funo/generated/FunoMod.java").is_file());
            let manifest = fs::read_to_string(project.join("funo.toml")).unwrap();
            assert_eq!(manifest.matches("loader =").count(), 1);
            assert!(manifest.contains(&format!("version = \"{}\"", profile.minecraft)));
        }

        assert!(root.join("fabric/src/main/resources/fabric.mod.json").is_file());
        assert!(root.join("forge/src/main/resources/META-INF/mods.toml").is_file());
        assert!(root.join("neoforge/src/main/resources/META-INF/neoforge.mods.toml").is_file());
        let _ = fs::remove_dir_all(root);
    }
}

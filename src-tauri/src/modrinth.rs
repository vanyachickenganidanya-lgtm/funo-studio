use crate::{launcher, models::{InstalledMod, MinecraftInstance, ModrinthProject}};
use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::{fs, path::PathBuf};

const API: &str = "https://api.modrinth.com/v2";
const USER_AGENT: &str = "FunoStudio/1.0 (https://github.com/vanyachickenganidanya-lgtm/funo-studio)";

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<ModrinthProject>,
}

#[derive(Deserialize)]
struct Version {
    id: String,
    project_id: String,
    name: String,
    files: Vec<VersionFile>,
}

#[derive(Deserialize)]
struct VersionFile {
    hashes: FileHashes,
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct FileHashes {
    sha512: String,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| error.to_string())
}

pub async fn search(query: &str, loader: &str, game_version: &str) -> Result<Vec<ModrinthProject>, String> {
    let facets = serde_json::to_string(&vec![
        vec!["project_type:mod".to_string()],
        vec![format!("categories:{loader}")],
        vec![format!("versions:{game_version}")],
    ])
    .map_err(|error| error.to_string())?;
    let response = client()?
        .get(format!("{API}/search"))
        .query(&[("query", query), ("facets", facets.as_str()), ("limit", "30")])
        .send()
        .await
        .map_err(|error| format!("Modrinth недоступен: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Modrinth вернул ошибку {}", response.status()));
    }
    response.json::<SearchResponse>().await.map(|value| value.hits).map_err(|error| error.to_string())
}

pub async fn install(instance_id: &str, project_id: &str) -> Result<MinecraftInstance, String> {
    let mut instances = launcher::load_instances()?;
    let instance = instances.iter_mut().find(|value| value.id == instance_id).ok_or("Сборка не найдена")?;
    let loaders = serde_json::to_string(&vec![&instance.loader]).map_err(|error| error.to_string())?;
    let versions = serde_json::to_string(&vec![&instance.minecraft_version]).map_err(|error| error.to_string())?;
    let response = client()?
        .get(format!("{API}/project/{project_id}/version"))
        .query(&[("loaders", loaders.as_str()), ("game_versions", versions.as_str())])
        .send()
        .await
        .map_err(|error| format!("Modrinth недоступен: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Не удалось получить версию мода: {}", response.status()));
    }
    let releases = response.json::<Vec<Version>>().await.map_err(|error| error.to_string())?;
    let release = releases.first().ok_or_else(|| {
        format!("Нет совместимой версии для {} / {}", instance.loader, instance.minecraft_version)
    })?;
    let file = release.files.iter().find(|value| value.primary).or_else(|| release.files.first()).ok_or("В версии мода нет файла")?;
    let file_name = PathBuf::from(&file.filename);
    if file_name.components().count() != 1 || !file.filename.to_ascii_lowercase().ends_with(".jar") {
        return Err("Modrinth вернул небезопасное имя файла".into());
    }
    if let Some(existing) = instance.mods.iter().find(|value| value.project_id == release.project_id && value.sha512 == file.hashes.sha512) {
        let existing_path = PathBuf::from(&instance.game_dir).join("mods").join(&existing.file_name);
        if existing_path.exists() {
            return Ok(instance.clone());
        }
    }
    let bytes = client()?
        .get(&file.url)
        .send()
        .await
        .map_err(|error| format!("Не удалось скачать мод: {error}"))?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    let hash = hex::encode(Sha512::digest(&bytes));
    if !hash.eq_ignore_ascii_case(&file.hashes.sha512) {
        return Err("SHA-512 скачанного мода не совпал с данными Modrinth".into());
    }
    let mods_dir = PathBuf::from(&instance.game_dir).join("mods");
    fs::create_dir_all(&mods_dir).map_err(|error| error.to_string())?;
    for old in instance.mods.iter().filter(|value| value.project_id == release.project_id) {
        let old_path = mods_dir.join(&old.file_name);
        if old_path.exists() {
            fs::remove_file(old_path).map_err(|error| error.to_string())?;
        }
    }
    instance.mods.retain(|value| value.project_id != release.project_id);
    let destination = mods_dir.join(&file.filename);
    let partial = mods_dir.join(format!(".{}.download", file.filename));
    fs::write(&partial, bytes).map_err(|error| error.to_string())?;
    if destination.exists() {
        fs::remove_file(&destination).map_err(|error| error.to_string())?;
    }
    fs::rename(&partial, &destination).map_err(|error| error.to_string())?;
    instance.mods.push(InstalledMod {
        project_id: release.project_id.clone(),
        version_id: release.id.clone(),
        name: release.name.clone(),
        file_name: file.filename.clone(),
        sha512: hash,
        source: "modrinth".into(),
    });
    let installed = instance.clone();
    launcher::save_instances(&instances)?;
    Ok(installed)
}

pub fn remove(instance_id: &str, project_id: &str) -> Result<MinecraftInstance, String> {
    let mut instances = launcher::load_instances()?;
    let instance = instances.iter_mut().find(|value| value.id == instance_id).ok_or("Сборка не найдена")?;
    let installed = instance.mods.iter().find(|value| value.project_id == project_id).cloned().ok_or("Мод не установлен")?;
    let file = PathBuf::from(&instance.game_dir).join("mods").join(&installed.file_name);
    if file.exists() {
        fs::remove_file(file).map_err(|error| error.to_string())?;
    }
    instance.mods.retain(|value| value.project_id != project_id);
    let result = instance.clone();
    launcher::save_instances(&instances)?;
    Ok(result)
}

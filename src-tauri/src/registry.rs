use crate::models::{RegistryIndex, RegistryPackage, RegistryResponse};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, time::Duration};
use url::Url;

const OFFICIAL_REPOSITORY: &str = "https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL";
const MAX_PACKAGE_BYTES: usize = 100 * 1024 * 1024;

fn index_url(repository: &str) -> Result<String, String> {
    let url = Url::parse(repository).map_err(|_| "Некорректный адрес реестра")?;
    if url.scheme() != "https" || url.host_str() != Some("github.com") {
        return Err("Официальный реестр должен быть HTTPS-репозиторием GitHub".into());
    }
    let parts: Vec<_> = url
        .path_segments()
        .map(|x| x.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();
    if parts.len() < 2 {
        return Err("Ожидался адрес github.com/владелец/репозиторий".into());
    }
    Ok(format!(
        "https://raw.githubusercontent.com/{}/{}/main/index.json",
        parts[0],
        parts[1].trim_end_matches(".git")
    ))
}

pub async fn fetch_registry(repository: Option<String>) -> Result<RegistryResponse, String> {
    let source = repository.unwrap_or_else(|| OFFICIAL_REPOSITORY.into());
    let raw_url = index_url(&source)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Funo-Studio/0.2")
        .build()
        .map_err(|e| e.to_string())?;
    let response = match client.get(&raw_url).send().await {
        Ok(response) => response,
        Err(error) => {
            return Ok(RegistryResponse {
                source,
                status: "offline".into(),
                message: format!("Не удалось связаться с GitHub: {error}"),
                packages: Vec::new(),
            })
        }
    };
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(RegistryResponse {
            source, status: "empty".into(),
            message: "Репозиторий найден, но index.json пока не добавлен. Используйте registry-template/index.json из Funo Studio.".into(),
            packages: Vec::new(),
        });
    }
    if !response.status().is_success() {
        return Ok(RegistryResponse {
            source,
            status: "offline".into(),
            message: format!("GitHub вернул статус {}", response.status()),
            packages: Vec::new(),
        });
    }
    let mut index: RegistryIndex = response
        .json()
        .await
        .map_err(|e| format!("index.json имеет неправильный формат: {e}"))?;
    if index.schema != 1 {
        return Err(format!(
            "Версия схемы {} не поддерживается. Нужна schema: 1",
            index.schema
        ));
    }
    index.packages.retain(|p| valid_package(p));
    for package in &mut index.packages {
        // Значок проверки означает: источник HTTPS и в индексе закреплена SHA-256.
        package.verified =
            package.verified && package.sha256.len() == 64 && package.source_url.starts_with("https://");
    }
    Ok(RegistryResponse {
        source,
        status: "ready".into(),
        message: format!("Загружено пакетов: {}", index.packages.len()),
        packages: index.packages,
    })
}

fn valid_package(package: &RegistryPackage) -> bool {
    Regex::new(r"^[a-z0-9][a-z0-9._-]{1,80}$")
        .unwrap()
        .is_match(&package.id)
        && Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?$")
            .unwrap()
            .is_match(&package.version)
        && matches!(package.kind.as_str(), "funo" | "java" | "minecraft")
        && Url::parse(&package.source_url)
            .map(|u| u.scheme() == "https")
            .unwrap_or(false)
}

pub async fn install_package(
    project_root: &str,
    package: RegistryPackage,
    allow_unsafe: bool,
) -> Result<String, String> {
    if !valid_package(&package) {
        return Err("Пакет имеет некорректное имя, версию или URL".into());
    }
    if !package.verified && !allow_unsafe {
        return Err("Пакет не подтверждён SHA-256. Включите установку непроверенных пакетов только если доверяете автору.".into());
    }
    let root = PathBuf::from(project_root);
    if !root.is_absolute() {
        return Err("Некорректная папка проекта".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Funo-Studio/0.2")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(&package.source_url)
        .send()
        .await
        .map_err(|e| format!("Не удалось скачать пакет: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Сервер пакета вернул {}", response.status()));
    }
    if response.content_length().unwrap_or(0) > MAX_PACKAGE_BYTES as u64 {
        return Err("Пакет больше 100 МБ".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Ошибка загрузки: {e}"))?;
    if bytes.len() > MAX_PACKAGE_BYTES {
        return Err("Пакет больше 100 МБ".into());
    }

    let hash = hex::encode(Sha256::digest(&bytes));
    if !package.sha256.is_empty() && !hash.eq_ignore_ascii_case(&package.sha256) {
        return Err(format!(
            "SHA-256 не совпала. Ожидалось {}, получено {}. Установка отменена.",
            package.sha256, hash
        ));
    }
    let directory = root
        .join(".funo")
        .join("packages")
        .join(&package.id)
        .join(&package.version);
    fs::create_dir_all(&directory).map_err(|e| format!("Не удалось создать папку пакета: {e}"))?;
    let extension = if package.kind == "java" { "jar" } else { "funpkg" };
    fs::write(directory.join(format!("package.{extension}")), &bytes)
        .map_err(|e| format!("Не удалось сохранить пакет: {e}"))?;
    fs::write(
        directory.join("package.json"),
        serde_json::to_vec_pretty(&package).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    update_lock(&root, &package, &hash)?;
    Ok(format!(
        "{} {} установлен. SHA-256 проверена.",
        package.name, package.version
    ))
}

fn update_lock(root: &std::path::Path, package: &RegistryPackage, actual_hash: &str) -> Result<(), String> {
    let lock_path = root.join("funo.lock");
    let mut lock: serde_json::Value = if lock_path.exists() {
        serde_json::from_slice(&fs::read(&lock_path).map_err(|e| e.to_string())?)
            .unwrap_or_else(|_| serde_json::json!({ "schema": 1, "packages": [] }))
    } else {
        serde_json::json!({ "schema": 1, "packages": [] })
    };
    let packages = lock
        .get_mut("packages")
        .and_then(|x| x.as_array_mut())
        .ok_or("Повреждён funo.lock")?;
    packages.retain(|entry| entry.get("id").and_then(|x| x.as_str()) != Some(&package.id));
    packages.push(serde_json::json!({
        "id": package.id, "version": package.version, "kind": package.kind,
        "source_url": package.source_url, "sha256": actual_hash
    }));
    fs::write(
        lock_path,
        serde_json::to_vec_pretty(&lock).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creates_raw_github_url() {
        let result = index_url(OFFICIAL_REPOSITORY).unwrap();
        assert_eq!(
            result,
            "https://raw.githubusercontent.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL/main/index.json"
        );
    }
}

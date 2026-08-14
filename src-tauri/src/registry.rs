use crate::models::{RegistryIndex, RegistryPackage, RegistryResponse};
use flate2::read::DeflateDecoder;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::Duration,
};
use url::Url;

pub const OFFICIAL_REPOSITORY: &str =
    "https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL/tree/main";
const MAX_PACKAGE_BYTES: usize = 100 * 1024 * 1024;
const MAX_UNPACKED_BYTES: usize = 100 * 1024 * 1024;
const MAX_PACKAGE_FILES: usize = 128;

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

    // A normal repository URL reads main. GitHub's /tree/<branch> URLs are
    // accepted too, including Arena branch names containing a slash.
    let reference = if parts.get(2) == Some(&"tree") && parts.len() > 3 {
        parts[3..].join("/")
    } else {
        "main".into()
    };
    let valid_reference = Regex::new(r"^[A-Za-z0-9._/-]+$").unwrap();
    if reference.contains("..") || !valid_reference.is_match(&reference) {
        return Err("Некорректное имя ветки реестра".into());
    }

    Ok(format!(
        "https://raw.githubusercontent.com/{}/{}/{reference}/index.json",
        parts[0],
        parts[1].trim_end_matches(".git")
    ))
}

pub async fn fetch_registry(repository: Option<String>) -> Result<RegistryResponse, String> {
    let source = repository.unwrap_or_else(|| OFFICIAL_REPOSITORY.into());
    let raw_url = index_url(&source)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Funo-Studio/0.3")
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
        .user_agent("Funo-Studio/0.3")
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
    let package_parent = root
        .join(".funo")
        .join("packages")
        .join(&package.id);
    let directory = package_parent.join(&package.version);
    let staging = package_parent.join(format!(
        ".{}.{}.installing",
        package.version,
        &hash[..12]
    ));
    fs::create_dir_all(&package_parent)
        .map_err(|e| format!("Не удалось создать папку пакета: {e}"))?;
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|e| format!("Не удалось очистить временную папку пакета: {e}"))?;
    }
    fs::create_dir_all(&staging)
        .map_err(|e| format!("Не удалось создать временную папку пакета: {e}"))?;

    let install_result = (|| -> Result<(), String> {
        let extension = if package.kind == "java" { "jar" } else { "funpkg" };
        fs::write(staging.join(format!("package.{extension}")), &bytes)
            .map_err(|e| format!("Не удалось сохранить пакет: {e}"))?;
        if package.kind == "funo" || package.kind == "minecraft" {
            unpack_funpkg(&staging, &bytes, &package)?;
        }
        fs::write(
            staging.join("package.json"),
            serde_json::to_vec_pretty(&package).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = install_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if directory.exists() {
        fs::remove_dir_all(&directory)
            .map_err(|e| format!("Не удалось обновить установленный пакет: {e}"))?;
    }
    fs::rename(&staging, &directory)
        .map_err(|e| format!("Не удалось завершить установку пакета: {e}"))?;
    update_lock(&root, &package, &hash)?;
    Ok(format!(
        "{} {} установлен. SHA-256 проверена.",
        package.name, package.version
    ))
}

fn unpack_funpkg(
    directory: &Path,
    bytes: &[u8],
    package: &RegistryPackage,
) -> Result<(), String> {
    if bytes.starts_with(b"PK\x03\x04") {
        unpack_zip_funpkg(directory, bytes, package)
    } else {
        unpack_json_funpkg(directory, bytes)
    }
}

fn unpack_json_funpkg(directory: &Path, bytes: &[u8]) -> Result<(), String> {
    let archive: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("Пакет .funpkg должен быть ZIP- или JSON-архивом Funo: {e}"))?;
    if archive.get("schema").and_then(|v| v.as_u64()) != Some(1) {
        return Err("Пакет использует неподдерживаемую схему .funpkg".into());
    }
    let entry = archive
        .get("entry")
        .and_then(|v| v.as_str())
        .ok_or("В .funpkg не указан entry")?;
    let files = archive
        .get("files")
        .and_then(|v| v.as_object())
        .ok_or("В .funpkg нет объекта files")?;
    if files.len() > MAX_PACKAGE_FILES || !files.contains_key(entry) {
        return Err("В .funpkg слишком много файлов или отсутствует entry".into());
    }
    let source_root = directory.join("src");
    let mut total_size = 0usize;
    for (name, value) in files {
        let relative = safe_archive_path(name)?;
        let content = value
            .as_str()
            .ok_or_else(|| format!("Файл {name} в .funpkg должен быть строкой"))?;
        total_size = total_size
            .checked_add(content.len())
            .ok_or("Распакованный пакет слишком большой")?;
        if total_size > MAX_UNPACKED_BYTES {
            return Err("Распакованный пакет больше 100 МБ".into());
        }
        let destination = source_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(destination, content).map_err(|e| e.to_string())?;
    }
    safe_archive_path(entry)?;
    fs::write(directory.join("entry.txt"), entry).map_err(|e| e.to_string())?;
    Ok(())
}

fn unpack_zip_funpkg(
    directory: &Path,
    bytes: &[u8],
    package: &RegistryPackage,
) -> Result<(), String> {
    let entries = read_zip_entries(bytes)?;
    let manifest_bytes = entries
        .iter()
        .find(|(name, _)| name == "manifest.json")
        .map(|(_, content)| content)
        .ok_or("В ZIP-пакете нет manifest.json")?;
    let manifest: serde_json::Value = serde_json::from_slice(manifest_bytes)
        .map_err(|e| format!("manifest.json имеет неправильный формат: {e}"))?;
    if manifest.get("schema").and_then(|value| value.as_u64()) != Some(1) {
        return Err("Пакет использует неподдерживаемую схему manifest.json".into());
    }
    for (field, expected) in [
        ("id", package.id.as_str()),
        ("version", package.version.as_str()),
        ("kind", package.kind.as_str()),
    ] {
        if manifest.get(field).and_then(|value| value.as_str()) != Some(expected) {
            return Err(format!(
                "Поле {field} в manifest.json не совпадает с index.json"
            ));
        }
    }
    if !entries
        .iter()
        .any(|(name, _)| name.starts_with("src/") && name.ends_with(".fun"))
    {
        return Err("В ZIP-пакете нет исходников src/*.fun".into());
    }

    for (name, content) in entries {
        let relative = safe_archive_path(&name)?;
        if matches!(
            name.as_str(),
            "package.funpkg" | "package.jar" | "package.json" | "entry.txt"
        ) {
            return Err(format!("Зарезервированный путь в .funpkg: {name}"));
        }
        let destination = directory.join(relative);
        if name.ends_with('/') {
            fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(destination, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn safe_archive_path(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('\\') || name.contains('\0') {
        return Err(format!("Небезопасный путь в .funpkg: {name}"));
    }
    let relative = PathBuf::from(name);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
                    | Component::CurDir
            )
        })
    {
        return Err(format!("Небезопасный путь в .funpkg: {name}"));
    }
    Ok(relative)
}

fn little_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or("Повреждён заголовок ZIP-пакета")?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn little_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or("Повреждён заголовок ZIP-пакета")?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_zip_entries(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut entries = Vec::new();
    let mut names = HashSet::new();
    let mut cursor = 0usize;
    let mut total_size = 0usize;

    while cursor < bytes.len() {
        let signature = little_u32(bytes, cursor)?;
        if signature == 0x0201_4b50 || signature == 0x0605_4b50 {
            break;
        }
        if signature != 0x0403_4b50 {
            return Err("Повреждён ZIP-пакет: ожидался заголовок файла".into());
        }
        if entries.len() >= MAX_PACKAGE_FILES {
            return Err("В .funpkg больше 128 файлов".into());
        }
        let flags = little_u16(bytes, cursor + 6)?;
        if flags & 0x0001 != 0 || flags & 0x0008 != 0 {
            return Err("Зашифрованные ZIP и ZIP с data descriptor не поддерживаются".into());
        }
        let method = little_u16(bytes, cursor + 8)?;
        if method != 0 && method != 8 {
            return Err(format!("Метод сжатия ZIP {method} не поддерживается"));
        }
        let compressed_size = little_u32(bytes, cursor + 18)? as usize;
        let unpacked_size = little_u32(bytes, cursor + 22)? as usize;
        let name_size = little_u16(bytes, cursor + 26)? as usize;
        let extra_size = little_u16(bytes, cursor + 28)? as usize;
        let name_start = cursor
            .checked_add(30)
            .ok_or("Повреждён размер ZIP-пакета")?;
        let name_end = name_start
            .checked_add(name_size)
            .ok_or("Повреждён размер имени в ZIP-пакете")?;
        let data_start = name_end
            .checked_add(extra_size)
            .ok_or("Повреждён размер заголовка ZIP-пакета")?;
        let data_end = data_start
            .checked_add(compressed_size)
            .ok_or("Повреждён размер файла в ZIP-пакете")?;
        let name = String::from_utf8(
            bytes
                .get(name_start..name_end)
                .ok_or("Повреждено имя файла в ZIP-пакете")?
                .to_vec(),
        )
        .map_err(|_| "Имя файла в ZIP-пакете должно быть UTF-8")?;
        safe_archive_path(&name)?;
        if !names.insert(name.clone()) {
            return Err(format!("Повторяющийся путь в ZIP-пакете: {name}"));
        }
        total_size = total_size
            .checked_add(unpacked_size)
            .ok_or("Распакованный пакет слишком большой")?;
        if total_size > MAX_UNPACKED_BYTES {
            return Err("Распакованный пакет больше 100 МБ".into());
        }
        let compressed = bytes
            .get(data_start..data_end)
            .ok_or("Повреждены данные файла в ZIP-пакете")?;
        let content = if method == 0 {
            compressed.to_vec()
        } else {
            let mut decoded = Vec::with_capacity(unpacked_size.min(1024 * 1024));
            DeflateDecoder::new(compressed)
                .take(unpacked_size as u64 + 1)
                .read_to_end(&mut decoded)
                .map_err(|e| format!("Не удалось распаковать ZIP-пакет: {e}"))?;
            decoded
        };
        if content.len() != unpacked_size {
            return Err(format!("Неверный размер файла {name} в ZIP-пакете"));
        }
        entries.push((name, content));
        cursor = data_end;
    }
    if entries.is_empty() {
        return Err("ZIP-пакет не содержит файлов".into());
    }
    Ok(entries)
}

pub fn remove_package(project_root: &str, package_id: &str) -> Result<String, String> {
    if !Regex::new(r"^[a-z0-9][a-z0-9._-]{1,80}$")
        .unwrap()
        .is_match(package_id)
    {
        return Err("Некорректный ID пакета".into());
    }
    let root = PathBuf::from(project_root);
    if !root.is_absolute() {
        return Err("Некорректная папка проекта".into());
    }
    let directory = root.join(".funo").join("packages").join(package_id);
    if !directory.exists() {
        return Err(format!("Пакет {package_id} не установлен"));
    }
    fs::remove_dir_all(directory).map_err(|e| format!("Не удалось удалить пакет: {e}"))?;
    let lock_path = root.join("funo.lock");
    if lock_path.exists() {
        let mut lock: serde_json::Value = serde_json::from_slice(
            &fs::read(&lock_path).map_err(|e| e.to_string())?,
        )
        .unwrap_or_else(|_| serde_json::json!({ "schema": 1, "packages": [] }));
        if let Some(packages) = lock.get_mut("packages").and_then(|v| v.as_array_mut()) {
            packages.retain(|entry| entry.get("id").and_then(|v| v.as_str()) != Some(package_id));
        }
        fs::write(
            lock_path,
            serde_json::to_vec_pretty(&lock).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(format!("Пакет {package_id} удалён"))
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
    packages.retain(|entry| {
        entry.get("id").and_then(|x| x.as_str()) != Some(package.id.as_str())
    });
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

    #[test]
    fn supports_github_tree_urls_with_slashes_in_branch() {
        let result = index_url(
            "https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL/tree/arena/019fffef-funo-libsoffical",
        )
        .unwrap();
        assert_eq!(
            result,
            "https://raw.githubusercontent.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL/arena/019fffef-funo-libsoffical/index.json"
        );
    }

    fn package() -> RegistryPackage {
        RegistryPackage {
            id: "funo.hello".into(),
            name: "Hello Funo".into(),
            version: "1.0.0".into(),
            description: "Test".into(),
            kind: "funo".into(),
            source_url: "https://example.com/funo.hello.funpkg".into(),
            sha256: "00".repeat(32),
            verified: true,
            author: Some("Funo".into()),
        }
    }

    fn add_zip_entry(archive: &mut Vec<u8>, name: &str, content: &[u8], deflate: bool) {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;

        let compressed = if deflate {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(content).unwrap();
            encoder.finish().unwrap()
        } else {
            content.to_vec()
        };
        archive.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        archive.extend_from_slice(&20u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&(if deflate { 8u16 } else { 0u16 }).to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u32.to_le_bytes());
        archive.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(content.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(name.as_bytes());
        archive.extend_from_slice(&compressed);
    }

    #[test]
    fn unpacks_official_zip_funpkg_layout() {
        let manifest = br#"{
            "schema": 1,
            "id": "funo.hello",
            "version": "1.0.0",
            "kind": "funo"
        }"#;
        let mut archive = Vec::new();
        add_zip_entry(&mut archive, "manifest.json", manifest, true);
        add_zip_entry(
            &mut archive,
            "src/hello.fun",
            b"fun greet(name) = \"Hello, \" + name\n",
            true,
        );
        archive.extend_from_slice(&0x0201_4b50u32.to_le_bytes());

        let directory = std::env::temp_dir().join(format!(
            "funo-registry-zip-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        unpack_funpkg(&directory, &archive, &package()).unwrap();
        assert_eq!(
            fs::read_to_string(directory.join("src/hello.fun")).unwrap(),
            "fun greet(name) = \"Hello, \" + name\n"
        );
        assert!(directory.join("manifest.json").is_file());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_path_traversal_in_zip_funpkg() {
        let manifest = br#"{
            "schema": 1,
            "id": "funo.hello",
            "version": "1.0.0",
            "kind": "funo"
        }"#;
        let mut archive = Vec::new();
        add_zip_entry(&mut archive, "manifest.json", manifest, false);
        add_zip_entry(&mut archive, "src/hello.fun", b"fun hello() = 1", false);
        add_zip_entry(&mut archive, "../outside.fun", b"bad", false);
        assert!(read_zip_entries(&archive).is_err());
    }
}

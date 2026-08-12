use crate::{models::{MinecraftAccount, MicrosoftAuthChallenge}, settings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

const MICROSOFT_DEVICE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MICROSOFT_TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAuth {
    refresh_token: String,
    minecraft_access_token: String,
    minecraft_expires_at: u64,
    username: String,
    uuid: String,
}

#[derive(Debug, Deserialize)]
struct MicrosoftToken {
    access_token: String,
    refresh_token: String,
}

fn auth_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().or_else(dirs::home_dir).ok_or("Не найдена папка настроек")?;
    Ok(base.join("Funo Studio").join("microsoft-auth.json"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_secs()).unwrap_or(0)
}

fn client_id() -> Result<String, String> {
    let value = settings::load()?.microsoft_client_id.trim().to_string();
    if value.is_empty() {
        Err("Сначала укажите Microsoft OAuth Client ID в настройках. Приложение должно быть зарегистрировано как public client с XboxLive.signin.".into())
    } else {
        Ok(value)
    }
}

fn form(values: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new()).extend_pairs(values.iter().copied()).finish()
}

fn load() -> Result<Option<StoredAuth>, String> {
    let path = auth_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let value = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&value).map(Some).map_err(|error| error.to_string())
}

fn save(value: &StoredAuth) -> Result<(), String> {
    let path = auth_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, serde_json::to_string_pretty(value).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
}

async fn token_request(body: String) -> Result<MicrosoftToken, String> {
    let response = reqwest::Client::new()
        .post(MICROSOFT_TOKEN)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|error| format!("Microsoft недоступен: {error}"))?;
    let status = response.status();
    let value = response.json::<Value>().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        let code = value.get("error").and_then(Value::as_str).unwrap_or("oauth_error");
        let description = value.get("error_description").and_then(Value::as_str).unwrap_or("Авторизация не завершена");
        return Err(format!("{code}: {description}"));
    }
    serde_json::from_value(value).map_err(|error| error.to_string())
}

pub async fn begin() -> Result<MicrosoftAuthChallenge, String> {
    let client_id = client_id()?;
    let response = reqwest::Client::new()
        .post(MICROSOFT_DEVICE)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form(&[("client_id", &client_id), ("scope", "XboxLive.signin offline_access")]))
        .send()
        .await
        .map_err(|error| format!("Microsoft недоступен: {error}"))?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    response.json().await.map_err(|error| error.to_string())
}

async fn minecraft_login(microsoft_access_token: &str) -> Result<(String, u64, String, String), String> {
    let client = reqwest::Client::new();
    let xbox = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={microsoft_access_token}") },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| format!("Xbox Live не принял учётную запись: {error}"))?
        .json::<Value>().await.map_err(|error| error.to_string())?;
    let xbox_token = xbox.get("Token").and_then(Value::as_str).ok_or("Xbox Live не вернул токен")?;
    let user_hash = xbox.pointer("/DisplayClaims/xui/0/uhs").and_then(Value::as_str).ok_or("Xbox Live не вернул user hash")?;
    let xsts = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbox_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| format!("XSTS не разрешил вход. Проверьте профиль Xbox и возрастные ограничения: {error}"))?
        .json::<Value>().await.map_err(|error| error.to_string())?;
    let xsts_token = xsts.get("Token").and_then(Value::as_str).ok_or("XSTS не вернул токен")?;
    let login = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({ "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}") }))
        .send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| format!("Minecraft Services отклонил вход: {error}"))?
        .json::<Value>().await.map_err(|error| error.to_string())?;
    let access = login.get("access_token").and_then(Value::as_str).ok_or("Minecraft Services не вернул токен")?.to_string();
    let expires = login.get("expires_in").and_then(Value::as_u64).unwrap_or(86_400);
    let entitlements = client
        .get("https://api.minecraftservices.com/entitlements/mcstore")
        .bearer_auth(&access)
        .send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| format!("Не удалось проверить лицензию Minecraft: {error}"))?
        .json::<Value>().await.map_err(|error| error.to_string())?;
    if entitlements.get("items").and_then(Value::as_array).map(Vec::is_empty).unwrap_or(true) {
        return Err("На этой Microsoft-учётной записи не найдена лицензия Minecraft: Java Edition".into());
    }
    let profile = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&access)
        .send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| format!("Профиль Minecraft недоступен: {error}"))?
        .json::<Value>().await.map_err(|error| error.to_string())?;
    let username = profile.get("name").and_then(Value::as_str).ok_or("В профиле Minecraft нет имени")?.to_string();
    let uuid = profile.get("id").and_then(Value::as_str).ok_or("В профиле Minecraft нет UUID")?.to_string();
    Ok((access, expires, username, uuid))
}

pub async fn complete(device_code: &str) -> Result<MinecraftAccount, String> {
    let client_id = client_id()?;
    let token = token_request(form(&[
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ("client_id", &client_id),
        ("device_code", device_code),
    ])).await?;
    let (minecraft_access_token, expires, username, uuid) = minecraft_login(&token.access_token).await?;
    save(&StoredAuth {
        refresh_token: token.refresh_token,
        minecraft_access_token,
        minecraft_expires_at: now() + expires.saturating_sub(60),
        username: username.clone(),
        uuid: uuid.clone(),
    })?;
    Ok(MinecraftAccount { username, uuid, authenticated: true })
}

pub fn current() -> Result<Option<MinecraftAccount>, String> {
    Ok(load()?.map(|value| MinecraftAccount { username: value.username, uuid: value.uuid, authenticated: true }))
}

pub fn logout() -> Result<(), String> {
    let path = auth_path()?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub async fn credentials() -> Result<Option<(String, String, String)>, String> {
    let Some(mut stored) = load()? else { return Ok(None) };
    if stored.minecraft_expires_at <= now() {
        let client_id = client_id()?;
        let token = token_request(form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("refresh_token", &stored.refresh_token),
            ("scope", "XboxLive.signin offline_access"),
        ])).await?;
        let (access, expires, username, uuid) = minecraft_login(&token.access_token).await?;
        stored = StoredAuth {
            refresh_token: token.refresh_token,
            minecraft_access_token: access,
            minecraft_expires_at: now() + expires.saturating_sub(60),
            username,
            uuid,
        };
        save(&stored)?;
    }
    Ok(Some((stored.username, stored.uuid, stored.minecraft_access_token)))
}

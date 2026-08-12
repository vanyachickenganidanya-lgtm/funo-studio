use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftVersion {
    pub id: String,
    pub label: String,
    pub stable: bool,
    pub java: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub root: String,
    pub name: String,
    pub kind: String,
    pub files: Vec<ProjectFile>,
    #[serde(default)]
    pub directories: Vec<String>,
    #[serde(default)]
    pub hidden_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: String,
    pub line: usize,
    pub column: usize,
    pub end_column: usize,
    pub code: String,
    pub title: String,
    pub message: String,
    pub example: Option<String>,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub generated_java: String,
    pub elapsed_ms: u128,
    pub diagnostics: Vec<Diagnostic>,
    pub artifact: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub kind: String,
    pub source_url: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub verified: bool,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub schema: u32,
    #[serde(default)]
    pub packages: Vec<RegistryPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryResponse {
    pub source: String,
    pub status: String,
    pub message: String,
    pub packages: Vec<RegistryPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioSettings {
    #[serde(default)]
    pub onboarding_completed: bool,
    #[serde(default = "default_true")]
    pub beginner: bool,
    #[serde(default)]
    pub installer_beginner_choice: Option<bool>,
    #[serde(default)]
    pub tutorial_step: usize,
    #[serde(default = "default_backend")]
    pub compiler_backend: String,
    #[serde(default)]
    pub microsoft_client_id: String,
}

fn default_true() -> bool {
    true
}

fn default_backend() -> String {
    "jvm".into()
}

impl Default for StudioSettings {
    fn default() -> Self {
        Self {
            onboarding_completed: false,
            beginner: true,
            installer_beginner_choice: None,
            tutorial_step: 0,
            compiler_backend: default_backend(),
            microsoft_client_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledMod {
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub file_name: String,
    pub sha512: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftInstance {
    pub id: String,
    pub name: String,
    pub project_root: String,
    pub minecraft_version: String,
    pub loader: String,
    pub game_dir: String,
    #[serde(default = "default_jvm_args")]
    pub jvm_args: String,
    #[serde(default)]
    pub game_args: String,
    #[serde(default = "default_launch_task")]
    pub launch_task: String,
    #[serde(default)]
    pub mods: Vec<InstalledMod>,
}

fn default_jvm_args() -> String {
    "-Xmx2G".into()
}

fn default_launch_task() -> String {
    "runClient".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthProject {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosoftAuthChallenge {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftAccount {
    pub username: String,
    pub uuid: String,
    pub authenticated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginProject {
    pub id: String,
    pub name: String,
    pub language: String,
    pub kind: String,
    pub root: String,
    pub repository_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCheck {
    pub success: bool,
    pub summary: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftToolStatus {
    pub found: bool,
    pub compatible: bool,
    pub managed: bool,
    pub version: String,
    pub latest_version: String,
    pub path: String,
    pub detail: String,
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageVolume {
    pub id: String,
    pub root: String,
    pub install_root: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub available_after_bytes: u64,
    pub eligible: bool,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftToolchainStatus {
    pub required_java: u8,
    pub recommended_gradle: String,
    pub reserve_bytes: u64,
    pub estimated_install_bytes: u64,
    pub jdk: MinecraftToolStatus,
    pub gradle: MinecraftToolStatus,
    pub volumes: Vec<StorageVolume>,
    pub recommended_install_root: String,
    pub ready: bool,
    pub has_updates: bool,
    pub message: String,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub root: String,
    pub name: String,
    pub kind: String,
    pub files: Vec<ProjectFile>,
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

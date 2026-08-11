mod compiler;
mod models;
mod project;
mod registry;

use models::{BuildResult, Diagnostic, Project, RegistryPackage, RegistryResponse};

#[tauri::command]
fn ensure_demo_project() -> Result<Project, String> {
    project::ensure_demo_project()
}

#[tauri::command]
fn write_project_file(project_root: String, relative_path: String, content: String) -> Result<(), String> {
    project::write_project_file(&project_root, &relative_path, &content)
}

#[tauri::command]
fn check_source(source: String) -> Vec<Diagnostic> {
    compiler::check_source(&source)
}

#[tauri::command]
async fn compile_and_run(project_root: String, source: String, classpath: Vec<String>) -> BuildResult {
    // javac/java are blocking processes; run them outside Tauri's async UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        compiler::compile_and_run(&project_root, &source, &classpath)
    })
    .await
    .unwrap_or_else(|error| BuildResult {
        success: false,
        stdout: String::new(),
        stderr: format!("Внутренняя ошибка задачи компилятора: {error}"),
        generated_java: String::new(),
        elapsed_ms: 0,
        diagnostics: Vec::new(),
        artifact: None,
    })
}

#[tauri::command]
async fn build_minecraft(project_root: String, source: String) -> BuildResult {
    tauri::async_runtime::spawn_blocking(move || compiler::build_minecraft(&project_root, &source))
        .await
        .unwrap_or_else(|error| BuildResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Внутренняя ошибка Minecraft-сборки: {error}"),
            generated_java: String::new(),
            elapsed_ms: 0,
            diagnostics: Vec::new(),
            artifact: None,
        })
}

#[tauri::command]
async fn fetch_registry(repository: Option<String>) -> Result<RegistryResponse, String> {
    registry::fetch_registry(repository).await
}

#[tauri::command]
async fn install_package(
    project_root: String,
    package: RegistryPackage,
    allow_unsafe: bool,
) -> Result<String, String> {
    registry::install_package(&project_root, package, allow_unsafe).await
}

#[tauri::command]
fn create_minecraft_project(name: String, mod_id: String, loader: String) -> Result<Project, String> {
    project::create_minecraft_project(&name, &mod_id, &loader)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            ensure_demo_project,
            write_project_file,
            check_source,
            compile_and_run,
            build_minecraft,
            fetch_registry,
            install_package,
            create_minecraft_project
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить Funo Studio");
}

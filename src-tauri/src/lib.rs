pub mod auth;
pub mod cli;
pub mod compiler;
pub mod launcher;
pub mod models;
pub mod modrinth;
pub mod native;
pub mod path_setup;
pub mod plugins;
pub mod process;
pub mod project;
pub mod registry;
pub mod settings;
pub mod toolchains;

use models::{
    BuildResult, Diagnostic, MinecraftAccount, MinecraftInstance, MinecraftToolchainStatus,
    MicrosoftAuthChallenge, ModrinthProject, PluginCheck, PluginProject, Project,
    RegistryPackage, RegistryResponse, StudioSettings,
};

fn task_error(label: &str, error: impl std::fmt::Display) -> BuildResult {
    BuildResult {
        success: false,
        stdout: String::new(),
        stderr: format!("Внутренняя ошибка {label}: {error}"),
        generated_java: String::new(),
        elapsed_ms: 0,
        diagnostics: Vec::new(),
        artifact: None,
    }
}

#[tauri::command]
fn ensure_demo_project() -> Result<Project, String> {
    project::ensure_demo_project()
}

#[tauri::command]
fn write_project_file(project_root: String, relative_path: String, content: String) -> Result<(), String> {
    project::write_project_file(&project_root, &relative_path, &content)
}

#[tauri::command]
fn create_project_folder(project_root: String, relative_path: String) -> Result<Project, String> {
    project::create_project_folder(&project_root, &relative_path)
}

#[tauri::command]
fn set_project_path_hidden(project_root: String, relative_path: String, hidden: bool) -> Result<Project, String> {
    project::set_project_path_hidden(&project_root, &relative_path, hidden)
}

#[tauri::command]
fn reload_project(project_root: String) -> Result<Project, String> {
    project::reload_project(&project_root)
}

#[tauri::command]
fn check_source(source: String) -> Vec<Diagnostic> {
    compiler::check_source(&source)
}

#[tauri::command]
async fn compile_and_run(project_root: String, source: String, classpath: Vec<String>) -> BuildResult {
    tauri::async_runtime::spawn_blocking(move || compiler::compile_and_run(&project_root, &source, &classpath))
        .await
        .unwrap_or_else(|error| task_error("задачи компилятора", error))
}

#[tauri::command]
async fn build_backend(project_root: String, source: String, target: String, run: bool) -> BuildResult {
    tauri::async_runtime::spawn_blocking(move || native::build_backend(&project_root, &source, &target, run))
        .await
        .unwrap_or_else(|error| task_error("native backend", error))
}

#[tauri::command]
async fn build_minecraft(project_root: String, source: String) -> BuildResult {
    tauri::async_runtime::spawn_blocking(move || compiler::build_minecraft(&project_root, &source))
        .await
        .unwrap_or_else(|error| task_error("Minecraft-сборки", error))
}

#[tauri::command]
async fn fetch_registry(repository: Option<String>) -> Result<RegistryResponse, String> {
    registry::fetch_registry(repository).await
}

#[tauri::command]
async fn install_package(project_root: String, package: RegistryPackage, allow_unsafe: bool) -> Result<String, String> {
    registry::install_package(&project_root, package, allow_unsafe).await
}

#[tauri::command]
async fn minecraft_versions(loader: String) -> Result<Vec<models::MinecraftVersion>, String> {
    project::minecraft_versions(&loader).await
}

#[tauri::command]
async fn create_minecraft_project(name: String, mod_id: String, loader: String, minecraft_version: String) -> Result<Project, String> {
    project::create_minecraft_project(&name, &mod_id, &loader, &minecraft_version).await
}

#[tauri::command]
fn get_settings() -> Result<StudioSettings, String> {
    settings::load()
}

#[tauri::command]
fn save_settings(value: StudioSettings) -> Result<StudioSettings, String> {
    settings::save(&value)
}

#[tauri::command]
fn path_status() -> Result<path_setup::PathStatus, String> {
    path_setup::status()
}

#[tauri::command]
fn install_path() -> Result<path_setup::PathStatus, String> {
    path_setup::install()
}

#[tauri::command]
fn uninstall_path() -> Result<path_setup::PathStatus, String> {
    path_setup::uninstall()
}

#[tauri::command]
fn list_instances() -> Result<Vec<MinecraftInstance>, String> {
    launcher::load_instances()
}

#[tauri::command]
fn create_instance(name: String, project_root: String, minecraft_version: String, loader: String) -> Result<MinecraftInstance, String> {
    launcher::create_instance(&name, &project_root, &minecraft_version, &loader)
}

#[tauri::command]
fn update_instance(instance: MinecraftInstance) -> Result<MinecraftInstance, String> {
    launcher::update_instance(instance)
}

#[tauri::command]
fn delete_instance(id: String) -> Result<(), String> {
    launcher::delete_instance(&id)
}

#[tauri::command]
async fn launch_instance(id: String) -> Result<String, String> {
    launcher::launch_instance(&id).await
}

#[tauri::command]
async fn minecraft_toolchain_status(
    project_root: String,
    minecraft_version: String,
    loader: String,
    check_updates: bool,
) -> Result<MinecraftToolchainStatus, String> {
    toolchains::status(
        &project_root,
        &minecraft_version,
        &loader,
        check_updates,
    )
    .await
}

#[tauri::command]
async fn install_minecraft_toolchain(
    project_root: String,
    minecraft_version: String,
    loader: String,
    destination_root: String,
) -> Result<MinecraftToolchainStatus, String> {
    toolchains::install(
        &project_root,
        &minecraft_version,
        &loader,
        &destination_root,
    )
    .await
}

#[tauri::command]
async fn search_modrinth(query: String, loader: String, game_version: String) -> Result<Vec<ModrinthProject>, String> {
    modrinth::search(&query, &loader, &game_version).await
}

#[tauri::command]
async fn install_modrinth(instance_id: String, project_id: String) -> Result<MinecraftInstance, String> {
    modrinth::install(&instance_id, &project_id).await
}

#[tauri::command]
fn remove_instance_mod(instance_id: String, project_id: String) -> Result<MinecraftInstance, String> {
    modrinth::remove(&instance_id, &project_id)
}

#[tauri::command]
fn create_plugin(parent: String, name: String, language: String, kind: String) -> Result<PluginProject, String> {
    plugins::create_plugin(&parent, &name, &language, &kind)
}

#[tauri::command]
async fn check_plugin(root: String) -> Result<PluginCheck, String> {
    tauri::async_runtime::spawn_blocking(move || plugins::check_plugin(&root))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn install_plugin(root: String) -> Result<PluginProject, String> {
    plugins::install_plugin(&root)
}

#[tauri::command]
fn list_plugins() -> Result<Vec<PluginProject>, String> {
    plugins::list_plugins()
}

#[tauri::command]
async fn begin_microsoft_auth() -> Result<MicrosoftAuthChallenge, String> {
    auth::begin().await
}

#[tauri::command]
async fn complete_microsoft_auth(device_code: String) -> Result<MinecraftAccount, String> {
    auth::complete(&device_code).await
}

#[tauri::command]
fn current_microsoft_account() -> Result<Option<MinecraftAccount>, String> {
    auth::current()
}

#[tauri::command]
fn logout_microsoft() -> Result<(), String> {
    auth::logout()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            ensure_demo_project,
            write_project_file,
            create_project_folder,
            set_project_path_hidden,
            reload_project,
            check_source,
            compile_and_run,
            build_backend,
            build_minecraft,
            fetch_registry,
            install_package,
            minecraft_versions,
            create_minecraft_project,
            get_settings,
            save_settings,
            path_status,
            install_path,
            uninstall_path,
            list_instances,
            create_instance,
            update_instance,
            delete_instance,
            launch_instance,
            minecraft_toolchain_status,
            install_minecraft_toolchain,
            search_modrinth,
            install_modrinth,
            remove_instance_mod,
            create_plugin,
            check_plugin,
            install_plugin,
            list_plugins,
            begin_microsoft_auth,
            complete_microsoft_auth,
            current_microsoft_account,
            logout_microsoft
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить Funo Studio");
}

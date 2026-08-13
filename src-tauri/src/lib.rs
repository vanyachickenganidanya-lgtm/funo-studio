pub mod auth;
pub mod cli;
pub mod compiler;
pub mod interpreter;
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

#[cfg(target_os = "android")]
use tauri_plugin_funo_android::{BuildRequest as AndroidBuildRequest, FunoAndroidExt, ToolchainRequest as AndroidToolchainRequest};

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

fn mobile_task_error(feature: &str) -> BuildResult {
    BuildResult {
        success: false,
        stdout: String::new(),
        stderr: format!(
            "{feature} требует системные инструменты и доступна в desktop-версии Funo Studio. На Android можно редактировать и проверять исходный код."
        ),
        generated_java: String::new(),
        elapsed_ms: 0,
        diagnostics: Vec::new(),
        artifact: None,
    }
}

fn desktop_only(feature: &str) -> Result<(), String> {
    if cfg!(mobile) {
        Err(format!("{feature} доступна только в desktop-версии Funo Studio."))
    } else {
        Ok(())
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
fn transpile_source(source: String, minecraft: bool) -> BuildResult {
    let started = std::time::Instant::now();
    let result = if minecraft {
        compiler::transpile_minecraft_entry(&source)
    } else {
        compiler::transpile(&source)
    };
    match result {
        Ok(generated_java) => BuildResult {
            success: true,
            stdout: "Исходник проверен без запуска JDK.".into(),
            stderr: String::new(),
            generated_java,
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: None,
        },
        Err(diagnostics) => BuildResult {
            success: false,
            stdout: String::new(),
            stderr: diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.clone())
                .unwrap_or_else(|| "В исходнике есть ошибка".into()),
            generated_java: String::new(),
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics,
            artifact: None,
        },
    }
}

/// Executes ordinary Funo inside the application without a JDK or subprocess.
/// This command is available on every platform and intentionally refuses
/// Minecraft sources; their Gradle/JAR build remains desktop-only.
#[tauri::command]
async fn execute_source(source: String) -> BuildResult {
    tauri::async_runtime::spawn_blocking(move || interpreter::execute(&source))
        .await
        .unwrap_or_else(|error| task_error("встроенного интерпретатора", error))
}

#[tauri::command]
async fn compile_and_run(project_root: String, source: String, classpath: Vec<String>) -> BuildResult {
    if cfg!(mobile) {
        return mobile_task_error("Запуск JVM");
    }
    tauri::async_runtime::spawn_blocking(move || compiler::compile_and_run(&project_root, &source, &classpath))
        .await
        .unwrap_or_else(|error| task_error("задачи компилятора", error))
}

#[tauri::command]
async fn build_backend(project_root: String, source: String, target: String, run: bool) -> BuildResult {
    if cfg!(mobile) {
        return mobile_task_error("Native backend");
    }
    tauri::async_runtime::spawn_blocking(move || native::build_backend(&project_root, &source, &target, run))
        .await
        .unwrap_or_else(|error| task_error("native backend", error))
}

#[tauri::command]
async fn build_minecraft(
    app: tauri::AppHandle,
    project_root: String,
    source: String,
    project: Option<Project>,
) -> BuildResult {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let Some(project) = project else {
            return task_error("Android Minecraft-сборки", "Не переданы файлы мобильного проекта");
        };
        let base = match app.path().app_data_dir() {
            Ok(path) => path,
            Err(error) => return task_error("Android Minecraft-сборки", error),
        };
        let root = match crate::project::materialize_android_minecraft(&base, &project).await {
            Ok(path) => path,
            Err(error) => return task_error("подготовки Android Minecraft-проекта", error),
        };
        let source_for_prepare = source.clone();
        let root_for_prepare = root.to_string_lossy().to_string();
        let prepared = match tauri::async_runtime::spawn_blocking(move || {
            compiler::prepare_minecraft_mobile(&root_for_prepare, &source_for_prepare)
        })
        .await
        {
            Ok(value) => value,
            Err(error) => return task_error("генерации Android Minecraft-мода", error),
        };
        if !prepared.success {
            return prepared;
        }
        let (minecraft_version, loader) = match toolchains::project_requirements(&root) {
            Ok(value) => value,
            Err(error) => return task_error("чтения Android Minecraft-проекта", error),
        };
        let request = AndroidBuildRequest {
            project_root: root.to_string_lossy().to_string(),
            source,
            minecraft_version,
            loader,
        };
        let app_for_build = app.clone();
        let native = tauri::async_runtime::spawn_blocking(move || {
            app_for_build.funo_android().build_minecraft::<BuildResult>(request)
        })
        .await;
        return match native {
            Ok(Ok(mut result)) => {
                result.generated_java = prepared.generated_java;
                result
            }
            Ok(Err(error)) => task_error("встроенной Android Gradle-сборки", error),
            Err(error) => task_error("Android Gradle-сборки", error),
        };
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, project);
        tauri::async_runtime::spawn_blocking(move || compiler::build_minecraft(&project_root, &source))
            .await
            .unwrap_or_else(|error| task_error("Minecraft-сборки", error))
    }
}

#[tauri::command]
async fn fetch_registry(repository: Option<String>) -> Result<RegistryResponse, String> {
    registry::fetch_registry(repository).await
}

#[tauri::command]
async fn install_package(project_root: String, package: RegistryPackage, allow_unsafe: bool) -> Result<String, String> {
    desktop_only("Установка JVM-пакетов")?;
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
    desktop_only("PATH и консольная команда funo")?;
    path_setup::status()
}

#[tauri::command]
fn install_path() -> Result<path_setup::PathStatus, String> {
    desktop_only("Установка Funo в PATH")?;
    path_setup::install()
}

#[tauri::command]
fn uninstall_path() -> Result<path_setup::PathStatus, String> {
    desktop_only("Управление PATH")?;
    path_setup::uninstall()
}

#[tauri::command]
fn list_instances() -> Result<Vec<MinecraftInstance>, String> {
    desktop_only("Minecraft Launcher")?;
    launcher::load_instances()
}

#[tauri::command]
fn create_instance(name: String, project_root: String, minecraft_version: String, loader: String) -> Result<MinecraftInstance, String> {
    desktop_only("Minecraft Launcher")?;
    launcher::create_instance(&name, &project_root, &minecraft_version, &loader)
}

#[tauri::command]
fn update_instance(instance: MinecraftInstance) -> Result<MinecraftInstance, String> {
    desktop_only("Minecraft Launcher")?;
    launcher::update_instance(instance)
}

#[tauri::command]
fn delete_instance(id: String) -> Result<(), String> {
    desktop_only("Minecraft Launcher")?;
    launcher::delete_instance(&id)
}

#[tauri::command]
async fn launch_instance(id: String) -> Result<String, String> {
    desktop_only("Запуск Minecraft")?;
    launcher::launch_instance(&id).await
}

#[tauri::command]
async fn minecraft_toolchain_status(
    app: tauri::AppHandle,
    project_root: String,
    minecraft_version: String,
    loader: String,
    check_updates: bool,
) -> Result<MinecraftToolchainStatus, String> {
    #[cfg(target_os = "android")]
    {
        let request = AndroidToolchainRequest {
            project_root,
            minecraft_version,
            loader,
            check_updates,
            destination_root: String::new(),
        };
        let app_for_status = app.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            app_for_status
                .funo_android()
                .toolchain_status::<MinecraftToolchainStatus>(request)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        toolchains::status(&project_root, &minecraft_version, &loader, check_updates).await
    }
}

#[tauri::command]
async fn install_minecraft_toolchain(
    app: tauri::AppHandle,
    project_root: String,
    minecraft_version: String,
    loader: String,
    destination_root: String,
) -> Result<MinecraftToolchainStatus, String> {
    #[cfg(target_os = "android")]
    {
        let request = AndroidToolchainRequest {
            project_root,
            minecraft_version,
            loader,
            check_updates: true,
            destination_root,
        };
        let app_for_install = app.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            app_for_install
                .funo_android()
                .install_toolchain::<MinecraftToolchainStatus>(request)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        toolchains::install(&project_root, &minecraft_version, &loader, &destination_root).await
    }
}

#[tauri::command]
async fn open_android_launcher(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        return app
            .funo_android()
            .open_launcher()
            .map(|response| response.value)
            .map_err(|error| error.to_string());
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("Встроенный Android Launcher доступен только в APK".into())
    }
}

#[tauri::command]
async fn search_modrinth(query: String, loader: String, game_version: String) -> Result<Vec<ModrinthProject>, String> {
    modrinth::search(&query, &loader, &game_version).await
}

#[tauri::command]
async fn install_modrinth(instance_id: String, project_id: String) -> Result<MinecraftInstance, String> {
    desktop_only("Установка модов в Minecraft Launcher")?;
    modrinth::install(&instance_id, &project_id).await
}

#[tauri::command]
fn remove_instance_mod(instance_id: String, project_id: String) -> Result<MinecraftInstance, String> {
    desktop_only("Управление модами Minecraft Launcher")?;
    modrinth::remove(&instance_id, &project_id)
}

#[tauri::command]
fn create_plugin(parent: String, name: String, language: String, kind: String) -> Result<PluginProject, String> {
    desktop_only("Создание нативного плагина")?;
    plugins::create_plugin(&parent, &name, &language, &kind)
}

#[tauri::command]
async fn check_plugin(root: String) -> Result<PluginCheck, String> {
    desktop_only("Проверка нативного плагина")?;
    tauri::async_runtime::spawn_blocking(move || plugins::check_plugin(&root))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn install_plugin(root: String) -> Result<PluginProject, String> {
    desktop_only("Установка нативного плагина")?;
    plugins::install_plugin(&root)
}

#[tauri::command]
fn list_plugins() -> Result<Vec<PluginProject>, String> {
    desktop_only("Plugin SDK")?;
    plugins::list_plugins()
}

#[tauri::command]
async fn begin_microsoft_auth() -> Result<MicrosoftAuthChallenge, String> {
    desktop_only("Microsoft-авторизация Minecraft Launcher")?;
    auth::begin().await
}

#[tauri::command]
async fn complete_microsoft_auth(device_code: String) -> Result<MinecraftAccount, String> {
    desktop_only("Microsoft-авторизация Minecraft Launcher")?;
    auth::complete(&device_code).await
}

#[tauri::command]
fn current_microsoft_account() -> Result<Option<MinecraftAccount>, String> {
    desktop_only("Microsoft-авторизация Minecraft Launcher")?;
    auth::current()
}

#[tauri::command]
fn logout_microsoft() -> Result<(), String> {
    desktop_only("Microsoft-авторизация Minecraft Launcher")?;
    auth::logout()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_funo_android::init());

    builder
        .invoke_handler(tauri::generate_handler![
            ensure_demo_project,
            write_project_file,
            create_project_folder,
            set_project_path_hidden,
            reload_project,
            check_source,
            transpile_source,
            execute_source,
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
            open_android_launcher,
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

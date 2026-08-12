use crate::{models::{PluginCheck, PluginProject}, process};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    schema: u32,
    id: String,
    name: String,
    language: String,
    kind: String,
    entry: String,
    #[serde(default)]
    repository: String,
}

fn slug(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() { Some(character.to_ascii_lowercase()) }
            else if character == ' ' || character == '-' || character == '_' { Some('-') }
            else { None }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn plugins_root() -> Result<PathBuf, String> {
    let base = dirs::data_dir().or_else(dirs::home_dir).ok_or("Не найдена папка данных")?;
    Ok(base.join("Funo Studio").join("plugins"))
}

fn read_manifest(root: &Path) -> Result<Manifest, String> {
    let source = fs::read_to_string(root.join("funo.plugin.json"))
        .map_err(|error| format!("Не найден funo.plugin.json: {error}"))?;
    let manifest: Manifest = serde_json::from_str(&source).map_err(|error| format!("Некорректный manifest: {error}"))?;
    if manifest.schema != 1 || slug(&manifest.id) != manifest.id || manifest.id.is_empty() {
        return Err("Plugin manifest должен иметь schema=1 и безопасный id".into());
    }
    Ok(manifest)
}

fn project(root: &Path, manifest: &Manifest) -> PluginProject {
    PluginProject {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        language: manifest.language.clone(),
        kind: manifest.kind.clone(),
        root: root.to_string_lossy().into(),
        repository_hint: manifest.repository.clone(),
    }
}

pub fn create_plugin(parent: &str, name: &str, language: &str, kind: &str) -> Result<PluginProject, String> {
    let parent = PathBuf::from(parent);
    if !parent.is_absolute() || !parent.is_dir() {
        return Err("Выберите существующую абсолютную папку".into());
    }
    let id = slug(name);
    if id.is_empty() {
        return Err("Название должно содержать латинские буквы или цифры".into());
    }
    let language = language.to_ascii_lowercase();
    if !matches!(language.as_str(), "rust" | "cpp" | "c++" | "typescript" | "javascript" | "python") {
        return Err("Поддерживаются Rust, C++, TypeScript, JavaScript и Python".into());
    }
    let root = parent.join(&id);
    if root.exists() {
        return Err("Папка плагина уже существует".into());
    }
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let display_name = name.trim();
    let (entry, files): (&str, Vec<(&str, String)>) = match language.as_str() {
        "rust" => (
            "target/release",
            vec![
                ("Cargo.toml", format!("[package]\nname = \"{id}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n")),
                ("src/lib.rs", "#[no_mangle]\npub extern \"C\" fn funo_plugin_abi() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests { #[test] fn abi_is_current() { assert_eq!(super::funo_plugin_abi(), 1); } }\n".into()),
                ("README.md", format!("# {display_name}\n\nNative Rust plugin for Funo Studio. ABI version: 1.\n")),
            ],
        ),
        "cpp" | "c++" => (
            "build",
            vec![
                ("CMakeLists.txt", format!("cmake_minimum_required(VERSION 3.16)\nproject({id} LANGUAGES CXX)\nadd_library({id} SHARED src/plugin.cpp)\ntarget_compile_features({id} PRIVATE cxx_std_17)\n")),
                ("src/plugin.cpp", "extern \"C\" unsigned int funo_plugin_abi() { return 1; }\n".into()),
                ("README.md", format!("# {display_name}\n\nNative C++17 plugin for Funo Studio. ABI version: 1.\n")),
            ],
        ),
        "python" => (
            "plugin.py",
            vec![
                ("plugin.py", "FUNO_PLUGIN_ABI = 1\n\ndef activate(context):\n    return {\"message\": \"Plugin activated\", \"context\": context}\n".into()),
                ("test_plugin.py", "import plugin\nassert plugin.FUNO_PLUGIN_ABI == 1\n".into()),
                ("README.md", format!("# {display_name}\n\nPython tooling plugin for Funo Studio.\n")),
            ],
        ),
        "javascript" => (
            "index.js",
            vec![
                ("package.json", format!("{{\n  \"name\": \"{id}\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": {{ \"build\": \"node --check index.js\", \"test\": \"node test.js\" }}\n}}\n")),
                ("index.js", "export const FUNO_PLUGIN_ABI = 1;\nexport function activate(context) { return { message: 'Plugin activated', context }; }\n".into()),
                ("test.js", "import { FUNO_PLUGIN_ABI } from './index.js';\nif (FUNO_PLUGIN_ABI !== 1) throw new Error('Unsupported Funo plugin ABI');\n".into()),
                ("README.md", format!("# {display_name}\n\nJavaScript tooling plugin for Funo Studio.\n")),
            ],
        ),
        _ => (
            "dist/index.js",
            vec![
                ("package.json", format!("{{\n  \"name\": \"{id}\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"scripts\": {{ \"build\": \"tsc\", \"test\": \"npm run build\" }},\n  \"devDependencies\": {{ \"typescript\": \"^5.7.0\" }}\n}}\n")),
                ("tsconfig.json", "{\n  \"compilerOptions\": { \"target\": \"ES2022\", \"module\": \"ES2022\", \"outDir\": \"dist\", \"strict\": true },\n  \"include\": [\"src\"]\n}\n".into()),
                ("src/index.ts", "export const FUNO_PLUGIN_ABI = 1;\nexport function activate(context: unknown) { return { message: 'Plugin activated', context }; }\n".into()),
                ("README.md", format!("# {display_name}\n\nTypeScript tooling plugin for Funo Studio.\n")),
            ],
        ),
    };
    for (relative, source) in files {
        let path = root.join(relative);
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        fs::write(path, source).map_err(|error| error.to_string())?;
    }
    fs::write(root.join(".gitignore"), "build/\ntarget/\ndist/\nnode_modules/\n")
        .map_err(|error| error.to_string())?;
    let manifest = Manifest {
        schema: 1,
        id,
        name: display_name.into(),
        language,
        kind: if kind.trim().is_empty() { "tooling".into() } else { kind.trim().into() },
        entry: entry.into(),
        repository: "https://github.com/your-name/your-plugin".into(),
    };
    fs::write(root.join("funo.plugin.json"), serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    // Initialize locally when Git is available; the user can then publish this
    // ordinary repository to any provider without giving Studio credentials.
    let _ = process::command("git").args(["init", "--initial-branch=main"]).current_dir(&root).output();
    Ok(project(&root, &manifest))
}

fn check_output(output: std::io::Result<std::process::Output>, action: &str) -> PluginCheck {
    match output {
        Ok(value) => {
            let stdout = String::from_utf8_lossy(&value.stdout);
            let stderr = String::from_utf8_lossy(&value.stderr);
            PluginCheck {
                success: value.status.success(),
                summary: if value.status.success() { format!("{action}: успешно") } else { format!("{action}: ошибка") },
                output: format!("{stdout}{stderr}").trim().into(),
            }
        }
        Err(error) => PluginCheck { success: false, summary: format!("{action}: инструмент не найден"), output: error.to_string() },
    }
}

pub fn check_plugin(root: &str) -> Result<PluginCheck, String> {
    let root = PathBuf::from(root);
    let manifest = read_manifest(&root)?;
    let result = match manifest.language.as_str() {
        "rust" => check_output(process::command("cargo").args(["test", "--release"]).current_dir(&root).output(), "Cargo test"),
        "cpp" | "c++" => {
            let configure = process::command("cmake").args(["-S", ".", "-B", "build"]).current_dir(&root).output();
            let configured = check_output(configure, "CMake configure");
            if configured.success {
                check_output(process::command("cmake").args(["--build", "build", "--config", "Release"]).current_dir(&root).output(), "CMake build")
            } else { configured }
        }
        "typescript" | "javascript" => check_output(process::command("npm").args(["test"]).current_dir(&root).output(), "npm test"),
        "python" => check_output(process::command(if cfg!(windows) { "python" } else { "python3" }).args(["test_plugin.py"]).current_dir(&root).output(), "Python test"),
        _ => PluginCheck { success: false, summary: "Неизвестный язык".into(), output: manifest.language },
    };
    Ok(result)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for item in fs::read_dir(source).map_err(|error| error.to_string())? {
        let item = item.map_err(|error| error.to_string())?;
        let kind = item.file_type().map_err(|error| error.to_string())?;
        if kind.is_symlink() || matches!(item.file_name().to_str(), Some(".git" | "target" | "node_modules" | "build")) {
            continue;
        }
        let target = destination.join(item.file_name());
        if kind.is_dir() {
            copy_tree(&item.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(item.path(), target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub fn install_plugin(root: &str) -> Result<PluginProject, String> {
    let source = PathBuf::from(root);
    let manifest = read_manifest(&source)?;
    let destination = plugins_root()?.join(&manifest.id);
    if destination.exists() {
        let backup = destination.with_extension(format!("backup-{}", SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_secs()));
        fs::rename(&destination, backup).map_err(|error| error.to_string())?;
    }
    copy_tree(&source, &destination)?;
    Ok(project(&destination, &manifest))
}

pub fn list_plugins() -> Result<Vec<PluginProject>, String> {
    let root = plugins_root()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| error.to_string())?.flatten() {
        if let Ok(manifest) = read_manifest(&entry.path()) {
            values.push(project(&entry.path(), &manifest));
        }
    }
    values.sort_by(|left, right| left.name.to_ascii_lowercase().cmp(&right.name.to_ascii_lowercase()));
    Ok(values)
}

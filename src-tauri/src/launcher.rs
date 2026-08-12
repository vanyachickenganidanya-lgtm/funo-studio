use crate::{auth, models::MinecraftInstance, toolchains};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

fn instances_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().or_else(dirs::home_dir).ok_or("Не найдена папка настроек")?;
    Ok(base.join("Funo Studio").join("instances.json"))
}

fn instances_root() -> Result<PathBuf, String> {
    let base = dirs::data_dir().or_else(dirs::home_dir).ok_or("Не найдена папка данных")?;
    Ok(base.join("Funo Studio").join("instances"))
}

pub fn load_instances() -> Result<Vec<MinecraftInstance>, String> {
    let path = instances_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let value = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    serde_json::from_str(&value).map_err(|error| format!("Список сборок повреждён: {error}"))
}

pub fn save_instances(instances: &[MinecraftInstance]) -> Result<(), String> {
    let path = instances_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, serde_json::to_string_pretty(instances).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

pub fn create_instance(
    name: &str,
    project_root: &str,
    minecraft_version: &str,
    loader: &str,
) -> Result<MinecraftInstance, String> {
    if name.trim().is_empty() || project_root.trim().is_empty() || minecraft_version.trim().is_empty() || loader.trim().is_empty() {
        return Err("Укажите название, проект, версию Minecraft и загрузчик".into());
    }
    let project = PathBuf::from(project_root);
    if !project.is_absolute() || !project.join("funo.toml").exists() {
        return Err("Выберите существующий Minecraft-проект Funo".into());
    }
    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| error.to_string())?.as_millis();
    let slug: String = name
        .chars()
        .filter_map(|character| if character.is_ascii_alphanumeric() { Some(character.to_ascii_lowercase()) } else if character == ' ' || character == '-' { Some('-') } else { None })
        .take(32)
        .collect();
    let id = format!("{}-{suffix}", slug.trim_matches('-'));
    let game_dir = instances_root()?.join(&id).join("game");
    fs::create_dir_all(game_dir.join("mods")).map_err(|error| error.to_string())?;
    fs::create_dir_all(game_dir.join("config")).map_err(|error| error.to_string())?;
    let instance = MinecraftInstance {
        id,
        name: name.trim().into(),
        project_root: project.to_string_lossy().into(),
        minecraft_version: minecraft_version.trim().into(),
        loader: loader.trim().to_ascii_lowercase(),
        game_dir: game_dir.to_string_lossy().into(),
        jvm_args: "-Xmx2G".into(),
        game_args: String::new(),
        launch_task: "runClient".into(),
        mods: Vec::new(),
    };
    let mut instances = load_instances()?;
    instances.push(instance.clone());
    save_instances(&instances)?;
    Ok(instance)
}

pub fn update_instance(instance: MinecraftInstance) -> Result<MinecraftInstance, String> {
    if instance.jvm_args.contains('\n') || instance.game_args.contains('\n') || instance.launch_task.contains(char::is_whitespace) {
        return Err("Аргументы должны быть в одной строке, а задача Gradle — без пробелов".into());
    }
    let mut instances = load_instances()?;
    let found = instances.iter_mut().find(|value| value.id == instance.id).ok_or("Сборка не найдена")?;
    // The data directory and installed-mod inventory are managed by Studio and
    // cannot be redirected by a forged frontend payload.
    let game_dir = found.game_dir.clone();
    let mods = found.mods.clone();
    *found = MinecraftInstance { game_dir, mods, ..instance };
    let updated = found.clone();
    save_instances(&instances)?;
    Ok(updated)
}

pub fn delete_instance(id: &str) -> Result<(), String> {
    let mut instances = load_instances()?;
    let index = instances.iter().position(|value| value.id == id).ok_or("Сборка не найдена")?;
    let instance = instances.remove(index);
    let root = instances_root()?;
    let game = PathBuf::from(&instance.game_dir);
    if game.starts_with(&root) {
        let instance_dir = game.parent().ok_or("Некорректный путь сборки")?;
        if instance_dir.exists() {
            fs::remove_dir_all(instance_dir).map_err(|error| error.to_string())?;
        }
    }
    save_instances(&instances)
}

fn parse_arguments(source: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in source.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if quote == Some(character) {
            quote = None;
        } else if quote.is_none() && (character == '\'' || character == '"') {
            quote = Some(character);
        } else if quote.is_none() && character.is_whitespace() {
            if !current.is_empty() {
                values.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if quote.is_some() {
        return Err("В аргументах не закрыта кавычка".into());
    }
    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        values.push(current);
    }
    Ok(values)
}

fn groovy(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

pub async fn launch_instance(id: &str) -> Result<String, String> {
    let instance = load_instances()?.into_iter().find(|value| value.id == id).ok_or("Сборка не найдена")?;
    let project = PathBuf::from(&instance.project_root);
    if !project.join("funo.toml").exists() {
        return Err("Проект этой сборки был перемещён или удалён".into());
    }
    let toolchain = toolchains::local_status(
        &project,
        &instance.minecraft_version,
        &instance.loader,
    )?;
    if !toolchain.ready {
        return Err(format!(
            "{}. Откройте раздел Minecraft → «JDK и Gradle», выберите диск и установите недостающие инструменты.",
            toolchain.message
        ));
    }
    fs::create_dir_all(PathBuf::from(&instance.game_dir).join("mods")).map_err(|error| error.to_string())?;
    let jvm_args = parse_arguments(&instance.jvm_args)?;
    let mut game_args = parse_arguments(&instance.game_args)?;
    if let Some((username, uuid, access_token)) = auth::credentials().await? {
        // A licensed Microsoft account is optional for loader development runs,
        // but when connected we pass the official profile to Minecraft itself.
        game_args.extend([
            "--username".into(), username,
            "--uuid".into(), uuid,
            "--accessToken".into(), access_token,
            "--userType".into(), "msa".into(),
        ]);
    }
    let jvm_json = serde_json::to_string(&jvm_args).map_err(|error| error.to_string())?;
    let game_json = serde_json::to_string(&game_args).map_err(|error| error.to_string())?;
    let init_path = project.join(".funo-instance.gradle");
    let init = format!(
        r#"// Generated by Funo Studio. Applies isolation after loader plugins configure runClient.
import groovy.json.JsonSlurper

def funoJvmArgs = new JsonSlurper().parseText(System.getenv('FUNO_JVM_ARGS') ?: '[]')
def funoGameArgs = new JsonSlurper().parseText(System.getenv('FUNO_GAME_ARGS') ?: '[]')
gradle.projectsEvaluated {{
    allprojects {{ p ->
        p.tasks.matching {{ it.name == {task} }}.configureEach {{ run ->
            if (run.metaClass.respondsTo(run, 'workingDir', Object)) run.workingDir({directory})
            if (run.hasProperty('workingDirectory')) run.workingDirectory = p.file({directory})
            if (run.metaClass.respondsTo(run, 'jvmArgs', Object[])) run.jvmArgs(funoJvmArgs as Object[])
            if (run.metaClass.respondsTo(run, 'args', Object[])) run.args(funoGameArgs as Object[])
        }}
    }}
}}
"#,
        task = groovy(&instance.launch_task),
        directory = groovy(&instance.game_dir),
    );
    fs::write(&init_path, init).map_err(|error| error.to_string())?;
    let mut command = toolchains::prepare_gradle_command(
        &project,
        &instance.minecraft_version,
        &instance.loader,
    )?;
    let output = command
        .arg(&instance.launch_task)
        .arg("--init-script")
        .arg(&init_path)
        .env("FUNO_JVM_ARGS", jvm_json)
        .env("FUNO_GAME_ARGS", game_json)
        .current_dir(&project)
        .output()
        .map_err(|error| format!("Не удалось запустить Gradle: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim_end().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim_end().to_string();
    if output.status.success() {
        Ok(if stdout.is_empty() { "Minecraft завершил работу".into() } else { stdout })
    } else {
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_support_quotes() {
        assert_eq!(parse_arguments("-Xmx2G -Dname=\"Funo Test\"").unwrap(), vec!["-Xmx2G", "-Dname=Funo Test"]);
        assert!(parse_arguments("--broken='value").is_err());
    }
}

use crate::models::{Project, ProjectFile};
use regex::Regex;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

fn projects_home() -> Result<PathBuf, String> {
    let base = dirs::document_dir()
        .or_else(dirs::home_dir)
        .ok_or("Не удалось найти домашнюю папку")?;
    Ok(base.join("FunoProjects"))
}

fn safe_relative(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute()
        || candidate.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("Путь файла должен находиться внутри проекта".into());
    }
    Ok(candidate)
}

fn write_if_missing(path: &Path, content: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, content).map_err(|e| format!("Не удалось записать {}: {e}", path.display()))
}

pub fn ensure_demo_project() -> Result<Project, String> {
    let root = projects_home()?.join("hello-funo");
    fs::create_dir_all(&root).map_err(|e| format!("Не удалось создать проект: {e}"))?;
    write_if_missing(
        &root.join("main.fun"),
        r#"fun fib(n: int) -> int = if n < 2 then n else fib(n - 1) + fib(n - 2)

fun main() {
    text title = "Привет из Funo Studio!"
    int answer = fib(10)
    bool ready = answer == 55

    println(title)
    println(answer)
    if ready {
        println("Типы и условия работают")
    }
    return(200)
}"#,
    )?;
    write_if_missing(
        &root.join("funo.toml"),
        r#"[project]
name = "Мой первый проект"
kind = "console"
target = "jvm-21"

[success]
code = 200

[registry]
official = "https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL"
"#,
    )?;
    write_if_missing(
        &root.join("src/minecraft.fun"),
        r#"use minecraft.fabric

mod "hello_funo" {
    on start {
        println("Мод Funo запущен!")
    }
}
"#,
    )?;
    load_project(&root)
}

pub fn write_project_file(project_root: &str, relative_path: &str, content: &str) -> Result<(), String> {
    let root = PathBuf::from(project_root);
    if !root.is_absolute() {
        return Err("Некорректная папка проекта".into());
    }
    let relative = safe_relative(relative_path)?;
    let destination = root.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(destination, content).map_err(|e| format!("Не удалось сохранить файл: {e}"))
}

pub fn load_project(root: &Path) -> Result<Project, String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files, 0)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let manifest = files
        .iter()
        .find(|f| f.path == "funo.toml")
        .map(|f| f.content.as_str())
        .unwrap_or("");
    let name_re = Regex::new(r#"(?m)^name\s*=\s*"([^"]+)""#).unwrap();
    let kind_re = Regex::new(r#"(?m)^kind\s*=\s*"([^"]+)""#).unwrap();
    let name = name_re
        .captures(manifest)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| root.file_name().unwrap_or_default().to_string_lossy().to_string());
    let kind = kind_re
        .captures(manifest)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "console".into());
    Ok(Project {
        root: root.to_string_lossy().to_string(),
        name,
        kind,
        files,
    })
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<ProjectFile>, depth: usize) -> Result<(), String> {
    if depth > 5 {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| format!("Не удалось прочитать проект: {e}"))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".funo" || name == ".gradle" || name == "build" {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out, depth + 1)?;
        } else if matches!(
            path.extension().and_then(|x| x.to_str()),
            Some("fun" | "toml" | "json" | "gradle" | "properties" | "md")
        ) || path.file_name().and_then(|x| x.to_str()) == Some("settings.gradle")
        {
            if let Ok(content) = fs::read_to_string(&path) {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(ProjectFile {
                    path: relative,
                    content,
                });
            }
        }
    }
    Ok(())
}

pub fn create_minecraft_project(name: &str, mod_id: &str, loader: &str) -> Result<Project, String> {
    if !Regex::new(r"^[a-z][a-z0-9_]{2,63}$").unwrap().is_match(mod_id) {
        return Err("ID мода должен содержать маленькие латинские буквы, цифры и _".into());
    }
    if loader != "fabric" && loader != "forge" {
        return Err("Поддерживаются Fabric и Forge".into());
    }
    let root = projects_home()?.join(mod_id);
    if root.exists() {
        return Err(format!("Проект {} уже существует", root.display()));
    }
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    let main_fun = format!(
        r#"use minecraft.{loader}

mod "{mod_id}" {{
    on start {{
        log("Мод {name} загружен")
    }}

    on server_start {{
        broadcast("Сервер запущен с модом {name}!")
        run_command("time set day")
    }}

    on player_join(player) {{
        tell("Добро пожаловать на сервер!")
        // give("minecraft:diamond", 1)
    }}
}}
"#
    );
    fs::write(root.join("main.fun"), main_fun).map_err(|e| e.to_string())?;
    fs::write(root.join("settings.gradle"), format!("pluginManagement {{ repositories {{ gradlePluginPortal(); maven {{ url = 'https://maven.fabricmc.net/' }}; maven {{ url = 'https://maven.minecraftforge.net/' }} }} }}\nrootProject.name = '{mod_id}'\n")).map_err(|e| e.to_string())?;
    fs::write(
        root.join("gradle.properties"),
        "org.gradle.jvmargs=-Xmx2G\norg.gradle.parallel=true\n",
    )
    .map_err(|e| e.to_string())?;

    let manifest = format!(
        r#"[project]
name = "{name}"
kind = "minecraft-{loader}"
target = "jvm-21"

[minecraft]
mod_id = "{mod_id}"
loader = "{loader}"
version = "1.21.1"

[registry]
official = "https://github.com/vanyachickenganidanya-lgtm/funo_libsOFFICAL"
"#
    );
    fs::write(root.join("funo.toml"), manifest).map_err(|e| e.to_string())?;

    if loader == "fabric" {
        create_fabric_files(&root, name, mod_id)?;
    } else {
        create_forge_files(&root, name, mod_id)?;
    }

    fs::write(
        root.join("README.md"),
        format!(
            r#"# {name}

Minecraft-мод на Funo ({loader}). Главный исходник — `main.fun`.

Перед первой сборкой установите JDK 21 и Gradle, затем выполните `gradle build`.
Funo Studio при сборке обновляет Java-мост из `main.fun`.
"#
        ),
    )
    .map_err(|e| e.to_string())?;
    load_project(&root)
}

fn create_fabric_files(root: &Path, name: &str, mod_id: &str) -> Result<(), String> {
    let gradle = format!(
        r#"plugins {{
    id 'fabric-loom' version '1.7-SNAPSHOT'
    id 'maven-publish'
}}
version = '1.0.0'
group = 'funo.mods'
base {{ archivesName = '{mod_id}' }}
repositories {{ mavenCentral() }}
dependencies {{
    minecraft 'com.mojang:minecraft:1.21.1'
    mappings 'net.fabricmc:yarn:1.21.1+build.3:v2'
    modImplementation 'net.fabricmc:fabric-loader:0.16.10'
    modImplementation 'net.fabricmc.fabric-api:fabric-api:0.115.1+1.21.1'
}}
processResources {{ inputs.property 'version', project.version; filesMatching('fabric.mod.json') {{ expand 'version': project.version }} }}
java {{ toolchain.languageVersion = JavaLanguageVersion.of(21); withSourcesJar() }}
"#
    );
    fs::write(root.join("build.gradle"), gradle).map_err(|e| e.to_string())?;
    let json = format!(
        r#"{{
  "schemaVersion": 1,
  "id": "{mod_id}",
  "version": "${{version}}",
  "name": "{name}",
  "environment": "*",
  "entrypoints": {{ "main": ["funo.generated.FunoMod"] }},
  "depends": {{ "fabricloader": ">=0.16.0", "minecraft": "~1.21.1", "java": ">=21", "fabric-api": "*" }}
}}
"#
    );
    write_generated_files(
        root,
        "fabric.mod.json",
        &json,
        &format!(
            r#"package funo.generated;
import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;

public final class FunoMod implements ModInitializer {{
    @Override public void onInitialize() {{
        FunoMain.start();
        ServerLifecycleEvents.SERVER_STARTED.register(FunoMain::serverStart);
        ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> FunoMain.playerJoin(handler.player));
    }}
}}
"#
        ),
    )
}

fn create_forge_files(root: &Path, name: &str, mod_id: &str) -> Result<(), String> {
    let gradle = format!(
        r#"plugins {{ id 'net.minecraftforge.gradle' version '[6.0,6.2)' }}
group = 'funo.mods'
version = '1.0.0'
base {{ archivesName = '{mod_id}' }}
java.toolchain.languageVersion = JavaLanguageVersion.of(21)
minecraft {{ mappings channel: 'official', version: '1.21.1' }}
repositories {{ mavenCentral() }}
dependencies {{ minecraft 'net.minecraftforge:forge:1.21.1-52.0.16' }}
"#
    );
    fs::write(root.join("build.gradle"), gradle).map_err(|e| e.to_string())?;
    let toml = format!(
        r#"modLoader="javafml"
loaderVersion="[52,)"
license="All Rights Reserved"
[[mods]]
modId="{mod_id}"
version="1.0.0"
displayName="{name}"
[[dependencies.{mod_id}]]
modId="minecraft"
mandatory=true
versionRange="[1.21.1]"
ordering="NONE"
side="BOTH"
"#
    );
    write_generated_files(
        root,
        "META-INF/mods.toml",
        &toml,
        &format!(
            r#"package funo.generated;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.event.entity.player.PlayerEvent;
import net.minecraftforge.event.server.ServerStartedEvent;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;

@Mod("{mod_id}")
public final class FunoMod {{
    public FunoMod() {{
        FunoMain.start();
        MinecraftForge.EVENT_BUS.register(this);
    }}

    @SubscribeEvent
    public void onServerStarted(ServerStartedEvent event) {{ FunoMain.serverStart(event.getServer()); }}

    @SubscribeEvent
    public void onPlayerJoin(PlayerEvent.PlayerLoggedInEvent event) {{ FunoMain.playerJoin(event.getEntity()); }}
}}
"#
        ),
    )
}

fn write_generated_files(
    root: &Path,
    resource: &str,
    resource_content: &str,
    bridge: &str,
) -> Result<(), String> {
    let resource_path = root.join("src/main/resources").join(resource);
    let java_dir = root.join("src/main/java/funo/generated");
    fs::create_dir_all(resource_path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::create_dir_all(&java_dir).map_err(|e| e.to_string())?;
    fs::write(resource_path, resource_content).map_err(|e| e.to_string())?;
    fs::write(java_dir.join("FunoMod.java"), bridge).map_err(|e| e.to_string())?;
    fs::write(
        java_dir.join("FunoMain.java"),
        r#"package funo.generated;
/** Этот файл обновляется компилятором Funo перед сборкой. */
public final class FunoMain {
    public static void start() { FunoMinecraft.log("Minecraft-мод Funo запущен!"); }
    public static void serverStart(Object server) { FunoMinecraft.bindServer(server); }
    public static void playerJoin(Object player) {}
}
"#,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        java_dir.join("FunoMinecraft.java"),
        r#"package funo.generated;

import java.lang.reflect.Method;

/** Небольшой Funo API поверх Fabric/Forge. Не требует Java-кода в main.fun. */
public final class FunoMinecraft {
    private static Object server;
    private FunoMinecraft() {}

    public static void bindServer(Object value) {
        server = value;
        log("Minecraft server API подключён");
    }

    public static void log(Object value) {
        System.out.println("[Funo] " + String.valueOf(value));
    }

    public static void broadcast(Object value) {
        if (!command("tellraw @a " + json(value))) log(value);
    }

    public static void actionbar(Object value) {
        command("title @a actionbar " + json(value));
    }

    public static void tell(Object player, Object value) {
        String name = playerName(player);
        if (!command("tellraw " + name + " " + json(value))) log(name + ": " + value);
    }

    public static void give(Object player, Object item, Object count) {
        command("give " + playerName(player) + " " + item + " " + count);
    }

    public static boolean command(Object value) {
        if (server == null) {
            log("Команда отложена до запуска сервера: " + value);
            return false;
        }
        try {
            Object manager = call(server, new String[] { "getCommandManager", "getCommands" });
            Object source = call(server, new String[] { "getCommandSource", "createCommandSourceStack" });
            invoke(manager, new String[] { "executeWithPrefix", "performPrefixedCommand" }, source, String.valueOf(value));
            return true;
        } catch (ReflectiveOperationException error) {
            log("Не удалось выполнить команду: " + error.getMessage());
            return false;
        }
    }

    private static String playerName(Object player) {
        try {
            Object profile = call(player, new String[] { "getGameProfile" });
            return String.valueOf(call(profile, new String[] { "getName" }));
        } catch (ReflectiveOperationException ignored) {
            try {
                return String.valueOf(call(player, new String[] { "getScoreboardName", "getName" }));
            } catch (ReflectiveOperationException error) {
                return "@s";
            }
        }
    }

    private static String json(Object value) {
        String text = String.valueOf(value).replace("\\", "\\\\").replace("\"", "\\\"");
        return "{\"text\":\"" + text + "\"}";
    }

    private static Object call(Object target, String[] names) throws ReflectiveOperationException {
        return invoke(target, names);
    }

    private static Object invoke(Object target, String[] names, Object... args) throws ReflectiveOperationException {
        for (String name : names) {
            for (Method method : target.getClass().getMethods()) {
                if (!method.getName().equals(name) || method.getParameterCount() != args.length) continue;
                try { return method.invoke(target, args); }
                catch (IllegalArgumentException ignored) { /* попробовать другую перегрузку */ }
            }
        }
        throw new NoSuchMethodException(String.join(" / ", names));
    }
}
"#,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_parent_paths() {
        assert!(safe_relative("../secret").is_err());
    }
    #[test]
    fn accepts_source_path() {
        assert!(safe_relative("src/main.fun").is_ok());
    }
}

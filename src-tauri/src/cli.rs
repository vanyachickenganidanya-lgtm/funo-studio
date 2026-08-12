use crate::{compiler, project, registry};
use std::{
    env, fs,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn paint(code: &str, text: impl AsRef<str>) -> String {
    if io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none() {
        format!("\x1b[{code}m{}\x1b[0m", text.as_ref())
    } else {
        text.as_ref().to_string()
    }
}

fn ok(text: impl AsRef<str>) {
    println!("{} {}", paint("32;1", "✓"), text.as_ref());
}

fn fail(text: impl AsRef<str>) -> i32 {
    eprintln!("{} {}", paint("31;1", "Ошибка:"), text.as_ref());
    1
}

fn help() {
    println!(
        "{} {}\n{}\n\n{}\n  {}\n  {}\n  {}\n  {}\n\n{}\n  {}\n  {}\n  {}\n  {}\n\n{}\n  {}\n  {}\n  {}\n\n{}\n  {}\n",
        paint("36;1", "Funo"),
        VERSION,
        "Простой язык, JVM-компилятор и инструменты Minecraft",
        paint("1", "Программы"),
        "funo run [main.fun]             собрать и запустить",
        "funo build [main.fun] [-o app.jar]  собрать исполняемый JAR",
        "funo check [main.fun]           проверить код без JDK",
        "funo java [main.fun]            показать сгенерированный Java",
        paint("1", "Библиотеки"),
        "funo pkg list                   пакеты официального GitHub",
        "funo pkg search <текст>         поиск пакета",
        "funo pkg install <id>           скачать пакет и обновить funo.lock",
        "funo pkg remove <id>            удалить пакет из проекта",
        paint("1", "Minecraft"),
        "funo minecraft new <имя> <mod_id> [fabric|forge|neoforge] [версия]",
        "funo minecraft versions [loader]  показать доступные версии",
        "funo minecraft build [main.fun] собрать JAR мода через Gradle",
        paint("1", "Установка"),
        "funo setup                      установить CLI в пользовательский PATH"
    );
}

fn source_path(value: Option<&String>) -> Result<PathBuf, String> {
    let candidate = value
        .filter(|value| !value.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("main.fun"));
    let candidate = if candidate.is_dir() {
        candidate.join("main.fun")
    } else {
        candidate
    };
    candidate
        .canonicalize()
        .map_err(|e| format!("Не удалось открыть {}: {e}", candidate.display()))
}

fn read_source(value: Option<&String>) -> Result<(PathBuf, PathBuf, String), String> {
    let file = source_path(value)?;
    let root = file
        .parent()
        .ok_or("Не удалось определить папку проекта")?
        .to_path_buf();
    let source = fs::read_to_string(&file)
        .map_err(|e| format!("Не удалось прочитать {}: {e}", file.display()))?;
    Ok((file, root, source))
}

fn print_diagnostics(source_path: &Path, diagnostics: &[crate::models::Diagnostic]) {
    for diagnostic in diagnostics {
        eprintln!(
            "{}:{}:{}: {} [{}] {}",
            source_path.display(),
            diagnostic.line,
            diagnostic.column,
            paint(if diagnostic.severity == "error" { "31;1" } else { "33;1" }, &diagnostic.severity),
            diagnostic.code,
            diagnostic.title
        );
        eprintln!("  {}", diagnostic.message);
        if let Some(example) = &diagnostic.example {
            eprintln!("  Пример: {}", example.replace('\n', " "));
        }
    }
}

fn run_source(args: &[String]) -> i32 {
    let (file, root, source) = match read_source(args.first()) {
        Ok(value) => value,
        Err(error) => return fail(error),
    };
    println!("{} {}", paint("36", "› funo run"), file.display());
    let result = compiler::compile_and_run(&root.to_string_lossy(), &source, &[]);
    if !result.stdout.is_empty() {
        println!("{}", result.stdout);
    }
    if !result.diagnostics.is_empty() {
        print_diagnostics(&file, &result.diagnostics);
    }
    if !result.success {
        if !result.stderr.is_empty() {
            eprintln!("{}", result.stderr);
        }
        return 1;
    }
    ok(format!("Готово за {} мс", result.elapsed_ms));
    0
}

fn build_source_argument(args: &[String]) -> Option<&String> {
    // Values belonging to -o/--output are never source files. This also keeps
    // `funo build -o app.jar` using the default ./main.fun.
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "-o" | "--output" => index += 2,
            value if value.starts_with('-') => index += 1,
            _ => return args.get(index),
        }
    }
    None
}

fn build_source(args: &[String]) -> i32 {
    let (file, root, source) = match read_source(build_source_argument(args)) {
        Ok(value) => value,
        Err(error) => return fail(error),
    };
    println!("{} {}", paint("36", "› funo build"), file.display());
    let result = compiler::compile_only(&root.to_string_lossy(), &source, &[]);
    if !result.diagnostics.is_empty() {
        print_diagnostics(&file, &result.diagnostics);
    }
    if !result.success {
        return fail(result.stderr);
    }
    let mut artifact = result.artifact.map(PathBuf::from);
    if let Some(position) = args.iter().position(|arg| arg == "-o" || arg == "--output") {
        let Some(destination) = args.get(position + 1) else {
            return fail("После -o укажите путь к JAR-файлу");
        };
        if let Some(source_artifact) = artifact.as_ref() {
            let destination = PathBuf::from(destination);
            if let Some(parent) = destination.parent().filter(|path| !path.as_os_str().is_empty()) {
                if let Err(error) = fs::create_dir_all(parent) {
                    return fail(format!("Не удалось создать папку результата: {error}"));
                }
            }
            if let Err(error) = fs::copy(source_artifact, &destination) {
                return fail(format!("Не удалось записать {}: {error}", destination.display()));
            }
            artifact = Some(destination);
        }
    }
    ok(format!(
        "JAR собран за {} мс: {}",
        result.elapsed_ms,
        artifact.map(|path| path.display().to_string()).unwrap_or_default()
    ));
    0
}

fn check_source(args: &[String], show_java: bool) -> i32 {
    let (file, _, source) = match read_source(args.first()) {
        Ok(value) => value,
        Err(error) => return fail(error),
    };
    match compiler::transpile(&source) {
        Ok(java) if show_java => {
            print!("{java}");
            0
        }
        Ok(_) => {
            ok(format!("{}: ошибок нет", file.display()));
            0
        }
        Err(diagnostics) => {
            print_diagnostics(&file, &diagnostics);
            1
        }
    }
}

fn project_root() -> Result<PathBuf, String> {
    env::current_dir()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| format!("Не удалось открыть текущую папку: {e}"))
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Не удалось запустить сетевой клиент: {e}"))
}

fn fetch_packages() -> Result<crate::models::RegistryResponse, String> {
    runtime()?.block_on(registry::fetch_registry(None))
}

fn print_packages(query: Option<&str>) -> i32 {
    let response = match fetch_packages() {
        Ok(value) => value,
        Err(error) => return fail(error),
    };
    if response.status != "ready" {
        println!("{}", paint("33;1", &response.message));
        println!("Источник: {}", response.source);
        return if response.status == "empty" { 0 } else { 1 };
    }
    let query = query.unwrap_or("").to_lowercase();
    let packages: Vec<_> = response
        .packages
        .iter()
        .filter(|package| {
            query.is_empty()
                || format!("{} {} {}", package.id, package.name, package.description)
                    .to_lowercase()
                    .contains(&query)
        })
        .collect();
    println!("{}\n", paint("1", "Официальные библиотеки Funo"));
    for package in &packages {
        println!(
            "  {} {}  {}\n    {}",
            paint("36;1", &package.id),
            paint("2", &package.version),
            if package.verified { paint("32", "✓ SHA-256") } else { paint("33", "не проверен") },
            package.description
        );
    }
    println!("\nНайдено: {} · {}", packages.len(), registry::OFFICIAL_REPOSITORY);
    0
}

fn install_package(args: &[String]) -> i32 {
    let Some(requested) = args.first() else {
        return fail("Укажите ID: funo pkg install <id>");
    };
    let allow_unsafe = args.iter().any(|arg| arg == "--unsafe");
    let (id, version) = requested
        .split_once('@')
        .map(|(id, version)| (id, Some(version)))
        .unwrap_or((requested.as_str(), None));
    let response = match fetch_packages() {
        Ok(value) if value.status == "ready" => value,
        Ok(value) => return fail(value.message),
        Err(error) => return fail(error),
    };
    let package = response
        .packages
        .into_iter()
        .filter(|package| package.id == id && version.map(|v| v == package.version).unwrap_or(true))
        .max_by(|a, b| a.version.cmp(&b.version));
    let Some(package) = package else {
        return fail(format!("Пакет {requested} не найден. Выполните funo pkg search <имя>"));
    };
    let root = match project_root() {
        Ok(value) => value,
        Err(error) => return fail(error),
    };
    println!("Скачиваю {} {}…", paint("36;1", &package.id), package.version);
    let runtime = match runtime() {
        Ok(value) => value,
        Err(error) => return fail(error),
    };
    match runtime.block_on(registry::install_package(
        &root.to_string_lossy(),
        package,
        allow_unsafe,
    )) {
        Ok(message) => {
            ok(message);
            0
        }
        Err(error) => fail(error),
    }
}

fn package_command(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("list") | None => print_packages(None),
        Some("search") => print_packages(args.get(1).map(String::as_str)),
        Some("install") | Some("add") => install_package(&args[1..]),
        Some("remove") => {
            let Some(id) = args.get(1) else {
                return fail("Укажите ID: funo pkg remove <id>");
            };
            let root = match project_root() {
                Ok(value) => value,
                Err(error) => return fail(error),
            };
            match registry::remove_package(&root.to_string_lossy(), id) {
                Ok(message) => {
                    ok(message);
                    0
                }
                Err(error) => fail(error),
            }
        }
        Some(other) => fail(format!("Неизвестная команда pkg: {other}")),
    }
}

fn minecraft_command(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("new") => {
            let Some(name) = args.get(1) else {
                return fail("Пример: funo minecraft new \"Мой мод\" my_mod fabric 1.21.1");
            };
            let Some(mod_id) = args.get(2) else {
                return fail("Укажите mod_id маленькими латинскими буквами");
            };
            let loader = args.get(3).map(String::as_str).unwrap_or("fabric");
            let runtime = match runtime() {
                Ok(value) => value,
                Err(error) => return fail(error),
            };
            let selected_version = if let Some(version) = args.get(4) {
                version.clone()
            } else {
                match runtime.block_on(project::minecraft_versions(loader)) {
                    Ok(versions) => match versions.first() {
                        Some(version) => version.id.clone(),
                        None => return fail(format!("Для {loader} нет доступных версий Minecraft")),
                    },
                    Err(error) => return fail(error),
                }
            };
            match runtime.block_on(project::create_minecraft_project(name, mod_id, loader, &selected_version)) {
                Ok(project) => {
                    ok(format!(
                        "Проект создан: {} ({loader}, Minecraft {selected_version})",
                        project.root
                    ));
                    println!("Дальше: cd \"{}\" && funo minecraft build", project.root);
                    0
                }
                Err(error) => fail(error),
            }
        }
        Some("versions") => {
            let loader = args.get(1).map(String::as_str).unwrap_or("fabric");
            let runtime = match runtime() {
                Ok(value) => value,
                Err(error) => return fail(error),
            };
            match runtime.block_on(project::minecraft_versions(loader)) {
                Ok(versions) => {
                    println!("{}", paint("1", format!("Minecraft / {loader}")));
                    for version in versions {
                        let channel = if version.stable { "" } else { " preview" };
                        println!("  {:<14} Java {}{}", version.label, version.java, channel);
                    }
                    0
                }
                Err(error) => fail(error),
            }
        }
        Some("build") => {
            let (file, root, source) = match read_source(args.get(1)) {
                Ok(value) => value,
                Err(error) => return fail(error),
            };
            println!("{} {}", paint("36", "› funo minecraft build"), file.display());
            let result = compiler::build_minecraft(&root.to_string_lossy(), &source);
            if !result.stdout.is_empty() {
                println!("{}", result.stdout);
            }
            if result.success {
                ok(result.artifact.unwrap_or_else(|| "Мод собран".into()));
                0
            } else {
                fail(result.stderr)
            }
        }
        _ => fail("Команды: funo minecraft new … | versions | build"),
    }
}

fn install_to_path() -> Result<String, String> {
    let status = crate::path_setup::install()?;
    Ok(format!(
        "Funo установлен: {}. Откройте новый терминал и выполните funo --version.",
        status.launcher
    ))
}

pub fn run() -> i32 {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--cli") {
        args.remove(0);
    }
    match args.first().map(String::as_str) {
        None | Some("help") | Some("--help") | Some("-h") => {
            help();
            0
        }
        Some("--version") | Some("-V") | Some("version") => {
            println!("funo {VERSION}");
            0
        }
        Some("run") => run_source(&args[1..]),
        Some("build") | Some("compile") => build_source(&args[1..]),
        Some("check") => check_source(&args[1..], false),
        Some("java") => check_source(&args[1..], true),
        Some("pkg") | Some("package") | Some("lib") => package_command(&args[1..]),
        Some("minecraft") | Some("mc") => minecraft_command(&args[1..]),
        Some("setup") | Some("install-path") => match install_to_path() {
            Ok(message) => {
                ok(message);
                0
            }
            Err(error) => fail(error),
        },
        Some(value) if value.ends_with(".fun") => run_source(&args),
        Some(other) => {
            eprintln!("Неизвестная команда: {other}\n");
            help();
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_is_not_used_as_source() {
        let args = vec!["-o".to_string(), "app.jar".to_string()];
        assert!(build_source_argument(&args).is_none());
    }

    #[test]
    fn source_can_appear_before_or_after_output() {
        let before = vec!["main.fun".into(), "-o".into(), "app.jar".into()];
        let after = vec!["--output".into(), "app.jar".into(), "main.fun".into()];
        assert_eq!(build_source_argument(&before).map(String::as_str), Some("main.fun"));
        assert_eq!(build_source_argument(&after).map(String::as_str), Some("main.fun"));
    }
}

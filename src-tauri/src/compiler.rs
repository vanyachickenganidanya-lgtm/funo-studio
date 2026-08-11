use crate::models::{BuildResult, Diagnostic};
use regex::Regex;
use std::{fs, path::PathBuf, process::Command, time::Instant};

fn diagnostic(
    severity: &str,
    line: usize,
    column: usize,
    end_column: usize,
    code: &str,
    title: impl Into<String>,
    message: impl Into<String>,
    example: Option<&str>,
    replacement: Option<&str>,
) -> Diagnostic {
    Diagnostic {
        severity: severity.into(),
        line,
        column,
        end_column,
        code: code.into(),
        title: title.into(),
        message: message.into(),
        example: example.map(str::to_string),
        replacement: replacement.map(str::to_string),
    }
}

pub fn check_source(source: &str) -> Vec<Diagnostic> {
    let typo_re = Regex::new(r"\b(printn|pritnln|printl|prntln)\b").unwrap();
    if let Some(found) = typo_re.find(source) {
        let before = &source[..found.start()];
        let line = before.lines().count().max(1);
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        return vec![diagnostic(
            "error",
            line,
            column,
            column + found.as_str().chars().count(),
            "FUN001",
            format!("Похоже, в «{}» опечатка", found.as_str()),
            "Наверное, вы хотели вывести значение. Заменить это слово на println?",
            Some("println(\"Привет!\")"),
            Some("println"),
        )];
    }

    let mut stack: Vec<(char, usize, usize)> = Vec::new();
    let mut string_quote: Option<char> = None;
    let mut escaped = false;
    for (line_idx, line) in source.lines().enumerate() {
        let mut chars = line.chars().peekable();
        let mut column = 0usize;
        while let Some(ch) = chars.next() {
            column += 1;
            if string_quote.is_none() && ch == '/' && chars.peek() == Some(&'/') {
                break;
            }
            if let Some(quote) = string_quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote {
                    string_quote = None;
                }
                continue;
            }
            if ch == '"' || ch == '\'' {
                string_quote = Some(ch);
                continue;
            }
            match ch {
                '{' | '(' | '[' => stack.push((ch, line_idx + 1, column)),
                '}' | ')' | ']' => {
                    let expected = match ch {
                        '}' => '{',
                        ')' => '(',
                        ']' => '[',
                        _ => unreachable!(),
                    };
                    match stack.pop() {
                        Some((open, _, _)) if open == expected => {}
                        _ => {
                            return vec![diagnostic(
                                "error",
                                line_idx + 1,
                                column,
                                column + 1,
                                "FUN003",
                                format!("Скобка «{ch}» осталась без пары"),
                                "Можно удалить её или добавить подходящую открывающую скобку.",
                                Some("fun main() {\n    println(\"Готово\")\n}"),
                                None,
                            )]
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if let Some((open, line, column)) = stack.last().copied() {
        let close = match open {
            '{' => '}',
            '(' => ')',
            '[' => ']',
            _ => '}',
        };
        return vec![diagnostic(
            "error",
            line,
            column,
            column + 1,
            "FUN002",
            format!("Не хватает закрывающей скобки «{close}»"),
            "Блок начался, но пока не закончился. Добавить скобку в конец файла?",
            Some("fun main() {\n    println(\"Готово\")\n}"),
            Some(&format!("\n{close}")),
        )];
    }

    let bad_fun = Regex::new(r"(?m)^\s*fun\s+[A-Za-z_][A-Za-z0-9_]*[ \t]+[^( \t\r\n]").unwrap();
    if let Some(found) = bad_fun.find(source) {
        let line = source[..found.start()].lines().count().max(1);
        return vec![diagnostic(
            "error",
            line,
            1,
            4,
            "FUN004",
            "После имени функции нужны круглые скобки",
            "Даже если параметров нет, напишите ().",
            Some("fun hello() = println(\"Привет\")"),
            None,
        )];
    }

    Vec::new()
}

fn java_type(name: &str) -> &str {
    match name.trim() {
        "int" => "int",
        "text" | "string" | "String" => "String",
        "bool" | "boolean" => "boolean",
        "float" => "double",
        "void" => "void",
        "any" | "Object" => "Object",
        other if !other.is_empty() => other,
        _ => "int",
    }
}

fn infer_return(name: &str, declared: Option<&str>, expression: &str, block_source: &str) -> String {
    if name == "main" {
        return "void".into();
    }
    if let Some(kind) = declared {
        return java_type(kind).into();
    }
    let sample = if expression.is_empty() {
        block_source
    } else {
        expression
    };
    if expression.is_empty() && !sample.contains("return") {
        return "void".into();
    }
    if sample.contains('"') {
        return "String".into();
    }
    if sample.contains(" true") || sample.contains(" false") {
        return "boolean".into();
    }
    "int".into()
}

fn expression_to_java(value: &str) -> String {
    let mut expr = value.trim().trim_end_matches(';').to_string();
    let if_expr = Regex::new(r"^if\s+(.+?)\s+then\s+(.+?)\s+else\s+(.+)$").unwrap();
    if let Some(cap) = if_expr.captures(&expr) {
        expr = format!("({} ? {} : {})", &cap[1], &cap[2], &cap[3]);
    }
    expr
}

fn java_params(params: &str, body_hint: &str) -> String {
    params
        .split(',')
        .filter(|p| !p.trim().is_empty())
        .map(|param| {
            let parts: Vec<&str> = param.trim().split(':').collect();
            let name = parts[0].trim();
            let ty = if parts.len() > 1 {
                java_type(parts[1])
            } else if body_hint.contains('"') && body_hint.contains(name) {
                "String"
            } else {
                "int"
            };
            format!("{ty} {name}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn minecraft_body(source: &str) -> Vec<String> {
    let println_re = Regex::new(r#"println\s*\((.+)\)"#).unwrap();
    let assignment = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$").unwrap();
    let mut body = Vec::new();
    let mut in_start = false;
    let mut depth = 0isize;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("on start") && trimmed.ends_with('{') {
            in_start = true;
            depth = 1;
            continue;
        }
        if !in_start {
            continue;
        }
        depth += trimmed.matches('{').count() as isize;
        depth -= trimmed.matches('}').count() as isize;
        if depth <= 0 {
            break;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some(cap) = println_re.captures(trimmed) {
            body.push(format!(
                "        System.out.println({});",
                expression_to_java(&cap[1])
            ));
        } else if let Some(cap) = assignment.captures(trimmed) {
            body.push(format!(
                "        var {} = {};",
                &cap[1],
                expression_to_java(&cap[2])
            ));
        } else {
            body.push(format!("        {};", expression_to_java(trimmed)));
        }
    }
    if body.is_empty() {
        body.push("        // Добавьте команды в on start".into());
    }
    body
}

fn transpile_minecraft_preview(source: &str, imports: &[String]) -> String {
    let body = minecraft_body(source);
    let mut output = String::from("// Сгенерировано Funo для предпросмотра Minecraft\n");
    for import in imports {
        output.push_str(&format!("import {import};\n"));
    }
    output.push_str("\npublic final class Main {\n    public static void onStart() {\n");
    output.push_str(&body.join("\n"));
    output
        .push_str("\n    }\n\n    public static void main(String[] args) {\n        onStart();\n    }\n}\n");
    output
}

pub fn transpile_minecraft_entry(source: &str) -> Result<String, Vec<Diagnostic>> {
    let diagnostics = check_source(source);
    if diagnostics.iter().any(|d| d.severity == "error") {
        return Err(diagnostics);
    }
    let import_re = Regex::new(r#"^\s*use\s+java\s+\"([^\"]+)\"\s*$"#).unwrap();
    let imports: Vec<String> = source
        .lines()
        .filter_map(|line| import_re.captures(line).map(|c| c[1].to_string()))
        .collect();
    let mut java = String::from("package funo.generated;\n\n");
    for import in imports {
        java.push_str(&format!("import {import};\n"));
    }
    java.push_str("\n/** Автоматически создано из main.fun. */\npublic final class FunoMain {\n    private FunoMain() {}\n\n    public static void start() {\n");
    java.push_str(&minecraft_body(source).join("\n"));
    java.push_str("\n    }\n}\n");
    Ok(java)
}

pub fn transpile(source: &str) -> Result<String, Vec<Diagnostic>> {
    let diagnostics = check_source(source);
    if diagnostics.iter().any(|d| d.severity == "error") {
        return Err(diagnostics);
    }

    let java_import = Regex::new(r#"^\s*use\s+java\s+\"([^\"]+)\"\s*$"#).unwrap();
    let expression_fun = Regex::new(r"^\s*(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*([A-Za-z_][A-Za-z0-9_]*))?\s*=\s*(.+)$").unwrap();
    let block_fun = Regex::new(r"^\s*(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*([A-Za-z_][A-Za-z0-9_]*))?\s*\{\s*$").unwrap();
    let if_block = Regex::new(r"^if\s+(.+)\s*\{\s*$").unwrap();
    let assignment = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$").unwrap();

    let imports: Vec<String> = source
        .lines()
        .filter_map(|line| java_import.captures(line).map(|c| c[1].to_string()))
        .collect();
    if Regex::new("(?m)^\\s*mod\\s+\\\"").unwrap().is_match(source) {
        return Ok(transpile_minecraft_preview(source, &imports));
    }

    let mut java = String::from("// Сгенерировано компилятором Funo. Не редактируйте вручную.\n");
    for import in &imports {
        java.push_str(&format!("import {import};\n"));
    }
    if !imports.is_empty() {
        java.push('\n');
    }
    java.push_str("public final class Main {\n");

    let lines: Vec<&str> = source.lines().collect();
    let mut current_function = String::new();
    let mut function_depth = 0isize;

    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with("use ") || trimmed.starts_with("lib ") {
            continue;
        }
        if trimmed.starts_with("//") {
            java.push_str(&format!("    {trimmed}\n"));
            continue;
        }

        if let Some(cap) = expression_fun.captures(trimmed) {
            let name = &cap[1];
            let params = &cap[2];
            let declared = cap.get(3).map(|m| m.as_str());
            let expr = expression_to_java(&cap[4]);
            let ret = infer_return(name, declared, &expr, "");
            let java_name = if name == "main" { "main" } else { name };
            if name == "main" {
                java.push_str(&format!(
                    "    public static void main(String[] args) {{\n        {};\n    }}\n\n",
                    if expr.starts_with("println(") {
                        expr.replacen("println(", "System.out.println(", 1)
                    } else {
                        expr
                    }
                ));
            } else {
                java.push_str(&format!(
                    "    static {ret} {java_name}({}) {{\n        return {expr};\n    }}\n\n",
                    java_params(params, &expr)
                ));
            }
            continue;
        }

        if let Some(cap) = block_fun.captures(trimmed) {
            let name = cap[1].to_string();
            let params = &cap[2];
            let declared = cap.get(3).map(|m| m.as_str());
            let lookahead = lines[idx + 1..]
                .iter()
                .take(20)
                .copied()
                .collect::<Vec<_>>()
                .join("\n");
            let ret = infer_return(&name, declared, "", &lookahead);
            if name == "main" {
                java.push_str("    public static void main(String[] args) {\n");
            } else {
                java.push_str(&format!(
                    "    static {ret} {name}({}) {{\n",
                    java_params(params, &lookahead)
                ));
            }
            current_function = name;
            function_depth = 1;
            continue;
        }

        let indent = "    ".repeat((function_depth.max(1) + 1) as usize);
        if let Some(cap) = if_block.captures(trimmed) {
            java.push_str(&format!("{indent}if ({}) {{\n", expression_to_java(&cap[1])));
            function_depth += 1;
            continue;
        }
        if trimmed == "}" {
            function_depth -= 1;
            let close_indent = "    ".repeat((function_depth.max(0) + 1) as usize);
            java.push_str(&format!("{close_indent}}}\n"));
            if function_depth <= 0 {
                current_function.clear();
                java.push('\n');
                function_depth = 0;
            }
            continue;
        }
        if trimmed.starts_with("else") {
            java.push_str(&format!("{indent}else {{\n"));
            function_depth += 1;
            continue;
        }
        if let Some(inner) = trimmed.strip_prefix("println(").and_then(|x| x.strip_suffix(')')) {
            java.push_str(&format!(
                "{indent}System.out.println({});\n",
                expression_to_java(inner)
            ));
            continue;
        }
        if Regex::new(r"^return\s*\(\s*200\s*\)\s*;?$")
            .unwrap()
            .is_match(trimmed)
            && current_function == "main"
        {
            java.push_str(&format!(
                "{indent}// Funo return(200): успешное завершение\n{indent}return;\n"
            ));
            continue;
        }
        if let Some(value) = trimmed
            .strip_prefix("return ")
            .or_else(|| trimmed.strip_prefix("return(").and_then(|x| x.strip_suffix(')')))
        {
            java.push_str(&format!("{indent}return {};\n", expression_to_java(value)));
            continue;
        }
        if let Some(cap) = assignment.captures(trimmed) {
            java.push_str(&format!(
                "{indent}var {} = {};\n",
                &cap[1],
                expression_to_java(&cap[2])
            ));
            continue;
        }
        java.push_str(&format!("{indent}{};\n", expression_to_java(trimmed)));
    }
    java.push_str("}\n");
    Ok(java)
}

fn safe_project_root(project_root: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(project_root);
    if !root.is_absolute() {
        return Err("Путь проекта должен быть абсолютным".into());
    }
    fs::create_dir_all(&root).map_err(|e| format!("Не удалось открыть папку проекта: {e}"))?;
    Ok(root)
}

pub fn compile_and_run(project_root: &str, source: &str, classpath: &[String]) -> BuildResult {
    let started = Instant::now();
    let generated_java = match transpile(source) {
        Ok(java) => java,
        Err(diagnostics) => {
            return BuildResult {
                success: false,
                stdout: String::new(),
                stderr: "В исходнике есть ошибка".into(),
                generated_java: String::new(),
                elapsed_ms: started.elapsed().as_millis(),
                diagnostics,
                artifact: None,
            }
        }
    };

    let root = match safe_project_root(project_root) {
        Ok(root) => root,
        Err(error) => return failed(error, generated_java, started),
    };
    let build = root.join(".funo").join("build");
    let src_dir = build.join("src");
    let classes = build.join("classes");
    if let Err(error) = fs::create_dir_all(&src_dir).and_then(|_| fs::create_dir_all(&classes)) {
        return failed(
            format!("Не удалось создать папку сборки: {error}"),
            generated_java,
            started,
        );
    }
    let java_file = src_dir.join("Main.java");
    if let Err(error) = fs::write(&java_file, &generated_java) {
        return failed(
            format!("Не удалось записать Java-код: {error}"),
            generated_java,
            started,
        );
    }

    let mut javac = Command::new("javac");
    javac.arg("-encoding").arg("UTF-8").arg("-d").arg(&classes);
    if !classpath.is_empty() {
        javac.arg("-classpath").arg(join_classpath(classpath));
    }
    javac.arg(&java_file);
    let compile = match javac.output() {
        Ok(output) => output,
        Err(error) => {
            return failed(
                format!("Не найден javac. Установите JDK 17 или 21: {error}"),
                generated_java,
                started,
            )
        }
    };
    if !compile.status.success() {
        return BuildResult {
            success: false,
            stdout: String::new(),
            stderr: String::from_utf8_lossy(&compile.stderr).to_string(),
            generated_java,
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: None,
        };
    }

    let mut runtime_paths = vec![classes.to_string_lossy().to_string()];
    runtime_paths.extend(classpath.iter().cloned());
    let run = match Command::new("java")
        .arg("-cp")
        .arg(join_classpath(&runtime_paths))
        .arg("Main")
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return failed(
                format!("Не удалось запустить JVM: {error}"),
                generated_java,
                started,
            )
        }
    };
    BuildResult {
        success: run.status.success(),
        stdout: String::from_utf8_lossy(&run.stdout).trim_end().to_string(),
        stderr: String::from_utf8_lossy(&run.stderr).trim_end().to_string(),
        generated_java,
        elapsed_ms: started.elapsed().as_millis(),
        diagnostics: Vec::new(),
        artifact: Some(classes.join("Main.class").to_string_lossy().to_string()),
    }
}

pub fn build_minecraft(project_root: &str, source: &str) -> BuildResult {
    let started = Instant::now();
    let generated_java = match transpile_minecraft_entry(source) {
        Ok(java) => java,
        Err(diagnostics) => {
            return BuildResult {
                success: false,
                stdout: String::new(),
                stderr: "В исходнике есть ошибка".into(),
                generated_java: String::new(),
                elapsed_ms: started.elapsed().as_millis(),
                diagnostics,
                artifact: None,
            }
        }
    };
    let root = match safe_project_root(project_root) {
        Ok(root) => root,
        Err(error) => return failed(error, generated_java, started),
    };
    let generated_path = root.join("src/main/java/funo/generated/FunoMain.java");
    if let Some(parent) = generated_path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            return failed(
                format!("Не удалось создать Java-мост: {error}"),
                generated_java,
                started,
            );
        }
    }
    if let Err(error) = fs::write(&generated_path, &generated_java) {
        return failed(
            format!("Не удалось обновить Java-мост: {error}"),
            generated_java,
            started,
        );
    }

    let gradlew = root.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" });
    let mut command = if gradlew.exists() {
        if cfg!(windows) {
            let mut cmd = Command::new(&gradlew);
            cmd.arg("build");
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.arg(&gradlew).arg("build");
            cmd
        }
    } else {
        let mut cmd = Command::new("gradle");
        cmd.arg("build");
        cmd
    };
    let output = match command.current_dir(&root).output() {
        Ok(output) => output,
        Err(error) => {
            return failed(
                format!("Не найден Gradle. Установите Gradle или добавьте Gradle Wrapper в проект: {error}"),
                generated_java,
                started,
            )
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim_end().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim_end().to_string();
    let artifact = if output.status.success() {
        fs::read_dir(root.join("build/libs"))
            .ok()
            .and_then(|entries| {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .find(|p| p.extension().and_then(|x| x.to_str()) == Some("jar"))
            })
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };
    BuildResult {
        success: output.status.success(),
        stdout,
        stderr,
        generated_java,
        elapsed_ms: started.elapsed().as_millis(),
        diagnostics: Vec::new(),
        artifact,
    }
}

fn join_classpath(paths: &[String]) -> String {
    let separator = if cfg!(windows) { ";" } else { ":" };
    paths.join(separator)
}

fn failed(error: String, generated_java: String, started: Instant) -> BuildResult {
    BuildResult {
        success: false,
        stdout: String::new(),
        stderr: error,
        generated_java,
        elapsed_ms: started.elapsed().as_millis(),
        diagnostics: Vec::new(),
        artifact: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_short_fibonacci() {
        let source = "fun fib(n) = if n < 2 then n else fib(n - 1) + fib(n - 2)\n\nfun main() {\n println(fib(10))\n return(200)\n}";
        let java = transpile(source).unwrap();
        assert!(java.contains("static int fib(int n)"));
        assert!(java.contains("n < 2 ? n : fib(n - 1) + fib(n - 2)"));
        assert!(java.contains("public static void main"));
        assert!(java.contains("Funo return(200)"));
    }

    #[test]
    fn finds_friendly_typo() {
        let diagnostics = check_source("fun main() {\n printn(1)\n}");
        assert_eq!(diagnostics[0].code, "FUN001");
        assert_eq!(diagnostics[0].replacement.as_deref(), Some("println"));
    }
}

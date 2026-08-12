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

fn boxed_java_type(name: &str) -> String {
    match name.trim() {
        "byte" => "Byte".into(),
        "short" => "Short".into(),
        "int" | "integer" => "Integer".into(),
        "long" => "Long".into(),
        "float" => "Float".into(),
        "double" | "decimal" | "number" => "Double".into(),
        "bool" | "boolean" => "Boolean".into(),
        "char" => "Character".into(),
        "text" | "string" | "String" => "String".into(),
        "any" | "Object" => "Object".into(),
        other => java_type(other),
    }
}

fn java_type(name: &str) -> String {
    let name = name.trim();
    if let Some(inner) = name.strip_prefix("list<").and_then(|v| v.strip_suffix('>')) {
        return format!("java.util.ArrayList<{}>", boxed_java_type(inner));
    }
    if let Some(inner) = name.strip_prefix("set<").and_then(|v| v.strip_suffix('>')) {
        return format!("java.util.HashSet<{}>", boxed_java_type(inner));
    }
    if let Some(inner) = name.strip_prefix("map<").and_then(|v| v.strip_suffix('>')) {
        let mut parts = inner.splitn(2, ',');
        let key = boxed_java_type(parts.next().unwrap_or("any"));
        let value = boxed_java_type(parts.next().unwrap_or("any"));
        return format!("java.util.HashMap<{key}, {value}>");
    }
    if let Some(inner) = name.strip_suffix("[]") {
        return format!("{}[]", java_type(inner));
    }
    match name {
        "byte" => "byte".into(),
        "short" => "short".into(),
        "int" | "integer" => "int".into(),
        "long" => "long".into(),
        "float" => "float".into(),
        "double" | "decimal" | "number" => "double".into(),
        "text" | "string" | "String" => "String".into(),
        "bool" | "boolean" => "boolean".into(),
        "char" => "char".into(),
        "void" => "void".into(),
        "any" | "Object" => "Object".into(),
        other if !other.is_empty() => other.into(),
        _ => "Object".into(),
    }
}

fn infer_return(name: &str, declared: Option<&str>, expression: &str, block_source: &str) -> String {
    if name == "main" {
        return "void".into();
    }
    if let Some(kind) = declared {
        return java_type(kind);
    }
    let sample = if expression.is_empty() { block_source } else { expression };
    if expression.is_empty() && !sample.contains("return") {
        return "void".into();
    }
    let return_value = Regex::new(r#"return\s*\(?\s*([^\n;)]+)"#)
        .ok()
        .and_then(|re| re.captures(sample))
        .map(|capture| capture[1].trim().to_string())
        .unwrap_or_else(|| sample.trim().to_string());
    infer_expression_type(&return_value)
}

fn infer_expression_type(value: &str) -> String {
    let value = value.trim();
    // The condition of an `if ... then ... else ...` expression is boolean,
    // but the expression itself has the type of its result branches.
    if let Some(captures) = Regex::new(r"^if\s+.+?\s+then\s+(.+?)\s+else\s+(.+)$")
        .unwrap()
        .captures(value)
    {
        let then_type = infer_expression_type(&captures[1]);
        let else_type = infer_expression_type(&captures[2]);
        return if then_type == else_type {
            then_type
        } else if matches!(then_type.as_str(), "int" | "long" | "float" | "double")
            && matches!(else_type.as_str(), "int" | "long" | "float" | "double")
        {
            if then_type == "double" || else_type == "double" {
                "double".into()
            } else if then_type == "float" || else_type == "float" {
                "float".into()
            } else if then_type == "long" || else_type == "long" {
                "long".into()
            } else {
                "int".into()
            }
        } else {
            "Object".into()
        };
    }
    if value.starts_with('[') && value.ends_with(']') {
        let values = &value[1..value.len() - 1];
        if values.trim().is_empty() {
            return "Object[]".into();
        }
        if values.split(',').all(|item| item.trim().starts_with('"')) {
            return "String[]".into();
        }
        if values.split(',').all(|item| matches!(item.trim(), "true" | "false")) {
            return "boolean[]".into();
        }
        if values.split(',').any(|item| item.trim().contains('.')) {
            return "double[]".into();
        }
        return "int[]".into();
    }
    if value.starts_with('"')
        || value.contains(" + \"")
        || value.contains("\" + ")
        || value.starts_with("readln(")
    {
        "String".into()
    } else if value.starts_with('\'') && value.ends_with('\'') {
        "char".into()
    } else if value == "true"
        || value == "false"
        || value.contains("==")
        || value.contains("!=")
        || value.contains(" <= ")
        || value.contains(" >= ")
        || value.contains(" < ")
        || value.contains(" > ")
        || value.contains(" and ")
        || value.contains(" or ")
        || value.starts_with("not ")
        || value.starts_with("readBool(")
    {
        "boolean".into()
    } else if value.ends_with('L') || value.ends_with('l') || value.starts_with("readLong(") {
        "long".into()
    } else if value.ends_with('f') || value.ends_with('F') {
        "float".into()
    } else if value.starts_with("readDouble(") || Regex::new(r"\d+\.\d+").unwrap().is_match(value) {
        "double".into()
    } else {
        "int".into()
    }
}

fn replace_word_operators(value: &str) -> String {
    // Replace language operators without ever changing text inside literals.
    let mut result = String::with_capacity(value.len());
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let flush_word = |result: &mut String, word: &mut String| {
        match word.as_str() {
            "and" => result.push_str("&&"),
            "or" => result.push_str("||"),
            "not" => result.push('!'),
            _ => result.push_str(word),
        }
        word.clear();
    };

    for ch in value.chars() {
        if let Some(active_quote) = quote {
            result.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            flush_word(&mut result, &mut word);
            quote = Some(ch);
            result.push(ch);
        } else if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            flush_word(&mut result, &mut word);
            result.push(ch);
        }
    }
    flush_word(&mut result, &mut word);
    result
}

fn array_literal(value: &str, expected_type: Option<&str>) -> Option<String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let values = &trimmed[1..trimmed.len() - 1];
    if let Some(expected) = expected_type {
        let java = java_type(expected);
        if let Some(component) = java.strip_suffix("[]") {
            return Some(format!("new {component}[]{{{values}}}"));
        }
        if java.starts_with("java.util.ArrayList") {
            if values.trim().is_empty() {
                return Some("new java.util.ArrayList<>()".into());
            }
            return Some(format!("new java.util.ArrayList<>(java.util.List.of({values}))"));
        }
        if java.starts_with("java.util.HashSet") {
            return Some(format!("new java.util.HashSet<>(java.util.List.of({values}))"));
        }
    }
    let component = if values.trim().is_empty() {
        "Object"
    } else if values.split(',').all(|v| v.trim().starts_with('"')) {
        "String"
    } else if values.split(',').all(|v| matches!(v.trim(), "true" | "false")) {
        "boolean"
    } else if values.split(',').any(|v| v.trim().contains('.')) {
        "double"
    } else if values.split(',').all(|v| v.trim().parse::<i64>().is_ok()) {
        "int"
    } else {
        "Object"
    };
    Some(format!("new {component}[]{{{values}}}"))
}

fn expression_to_java_typed(value: &str, expected_type: Option<&str>) -> String {
    let mut expr = value.trim().trim_end_matches(';').to_string();
    let if_expr = Regex::new(r"^if\s+(.+?)\s+then\s+(.+?)\s+else\s+(.+)$").unwrap();
    if let Some(cap) = if_expr.captures(&expr) {
        expr = format!(
            "({} ? {} : {})",
            replace_word_operators(&cap[1]),
            expression_to_java_typed(&cap[2], expected_type),
            expression_to_java_typed(&cap[3], expected_type)
        );
    } else if let Some(array) = array_literal(&expr, expected_type) {
        expr = array;
    }
    expr = replace_word_operators(&expr);
    let trimmed = expr.trim_start();
    let leading = &expr[..expr.len() - trimmed.len()];
    if let Some(rest) = trimmed.strip_prefix("println(") {
        format!("{leading}System.out.println({rest}")
    } else if let Some(rest) = trimmed.strip_prefix("print(") {
        format!("{leading}System.out.print({rest}")
    } else {
        expr
    }
}

fn expression_to_java(value: &str) -> String {
    expression_to_java_typed(value, None)
}

fn java_params(params: &str, body_hint: &str) -> String {
    params
        .split(',')
        .filter(|p| !p.trim().is_empty())
        .map(|param| {
            let mut parts = param.trim().splitn(2, ':');
            let name = parts.next().unwrap_or("value").trim();
            let ty = parts
                .next()
                .map(java_type)
                .unwrap_or_else(|| {
                    if body_hint.contains('"') && body_hint.contains(name) {
                        "String".into()
                    } else {
                        "int".into()
                    }
                });
            format!("{ty} {name}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn minecraft_event_body(source: &str, event: &str, has_player: bool) -> Vec<String> {
    let header = Regex::new(&format!(
        r"^on\s+{}(?:\s*\([^)]*\))?\s*\{{\s*$",
        regex::escape(event)
    ))
    .unwrap();
    let typed_decl = Regex::new(r"^(byte|short|int|long|float|double|number|decimal|text|string|bool|boolean|char|any)(\[\])?\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$").unwrap();
    let assignment = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$").unwrap();
    let mut body = Vec::new();
    let mut in_event = false;
    let mut depth = 0isize;
    for line in source.lines() {
        let trimmed = line.trim();
        if !in_event && header.is_match(trimmed) {
            in_event = true;
            depth = 1;
            continue;
        }
        if !in_event {
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
        let line = if let Some(value) = trimmed.strip_prefix("log(").and_then(|v| v.strip_suffix(')')) {
            format!("FunoMinecraft.log({});", expression_to_java(value))
        } else if let Some(value) = trimmed.strip_prefix("broadcast(").and_then(|v| v.strip_suffix(')')) {
            format!("FunoMinecraft.broadcast({});", expression_to_java(value))
        } else if let Some(value) = trimmed.strip_prefix("run_command(").and_then(|v| v.strip_suffix(')')) {
            format!("FunoMinecraft.command({});", expression_to_java(value))
        } else if let Some(value) = trimmed.strip_prefix("actionbar(").and_then(|v| v.strip_suffix(')')) {
            format!("FunoMinecraft.actionbar({});", expression_to_java(value))
        } else if has_player {
            if let Some(value) = trimmed.strip_prefix("tell(").and_then(|v| v.strip_suffix(')')) {
                format!("FunoMinecraft.tell(player, {});", expression_to_java(value))
            } else if let Some(value) = trimmed.strip_prefix("give(").and_then(|v| v.strip_suffix(')')) {
                format!("FunoMinecraft.give(player, {});", expression_to_java(value))
            } else if let Some(cap) = typed_decl.captures(trimmed) {
                let source_type = format!("{}{}", &cap[1], cap.get(2).map(|v| v.as_str()).unwrap_or(""));
                format!("{} {} = {};", java_type(&source_type), &cap[3], expression_to_java_typed(&cap[4], Some(&source_type)))
            } else if let Some(cap) = assignment.captures(trimmed) {
                format!("var {} = {};", &cap[1], expression_to_java(&cap[2]))
            } else {
                format!("{};", expression_to_java(trimmed))
            }
        } else if let Some(cap) = typed_decl.captures(trimmed) {
            let source_type = format!("{}{}", &cap[1], cap.get(2).map(|v| v.as_str()).unwrap_or(""));
            format!("{} {} = {};", java_type(&source_type), &cap[3], expression_to_java_typed(&cap[4], Some(&source_type)))
        } else if let Some(cap) = assignment.captures(trimmed) {
            format!("var {} = {};", &cap[1], expression_to_java(&cap[2]))
        } else {
            format!("{};", expression_to_java(trimmed))
        };
        body.push(format!("        {line}"));
    }
    body
}

fn minecraft_body(source: &str) -> Vec<String> {
    let mut body = minecraft_event_body(source, "start", false);
    if body.is_empty() {
        body.push("        // Добавьте команды в on start".into());
    }
    body
}

fn transpile_minecraft_preview(source: &str, imports: &[String]) -> String {
    let start = minecraft_body(source);
    let server_start = minecraft_event_body(source, "server_start", false);
    let player_join = minecraft_event_body(source, "player_join", true);
    let mut output = String::from("// Сгенерировано Funo для предпросмотра Minecraft\n");
    for import in imports {
        output.push_str(&format!("import {import};\n"));
    }
    output.push_str("\npublic final class Main {\n    public static void onStart() {\n");
    output.push_str(&start.join("\n"));
    output.push_str("\n    }\n\n    public static void onServerStart(Object server) {\n");
    output.push_str(&server_start.join("\n"));
    output.push_str("\n    }\n\n    public static void onPlayerJoin(Object player) {\n");
    output.push_str(&player_join.join("\n"));
    output.push_str("\n    }\n\n    public static void main(String[] args) {\n        onStart();\n    }\n}\n");
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
    java.push_str("\n/** Автоматически создано из main.fun. */\npublic final class FunoMain {\n    private FunoMain() {}\n\n");
    java.push_str("    public static void start() {\n");
    java.push_str(&minecraft_body(source).join("\n"));
    java.push_str("\n    }\n\n");
    java.push_str("    public static void serverStart(Object server) {\n        FunoMinecraft.bindServer(server);\n");
    java.push_str(&minecraft_event_body(source, "server_start", false).join("\n"));
    java.push_str("\n    }\n\n");
    java.push_str("    public static void playerJoin(Object player) {\n");
    java.push_str(&minecraft_event_body(source, "player_join", true).join("\n"));
    java.push_str("\n    }\n}\n");
    Ok(java)
}

fn function_body_hint(lines: &[&str], start: usize) -> String {
    let mut depth = 1isize;
    let mut body = Vec::new();
    for line in lines.iter().skip(start) {
        let trimmed = line.trim();
        depth += trimmed.matches('{').count() as isize;
        depth -= trimmed.matches('}').count() as isize;
        if depth <= 0 {
            break;
        }
        body.push(*line);
    }
    body.join("\n")
}

pub fn transpile(source: &str) -> Result<String, Vec<Diagnostic>> {
    let diagnostics = check_source(source);
    if diagnostics.iter().any(|d| d.severity == "error") {
        return Err(diagnostics);
    }

    let java_import = Regex::new(r#"^\s*use\s+java\s+\"([^\"]+)\"\s*$"#).unwrap();
    let expression_fun = Regex::new(r"^\s*(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*([^\s=]+))?\s*=\s*(.+)$").unwrap();
    let block_fun = Regex::new(r"^\s*(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*([^\s{]+))?\s*\{\s*$").unwrap();
    let typed_decl = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*(?:<[^>]+>)?(?:\[\])?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$").unwrap();
    let named_decl = Regex::new(r"^(let|var|const)\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*:\s*([^=\s]+))?\s*=\s*(.+)$").unwrap();
    let assignment = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$").unwrap();
    let if_block = Regex::new(r"^if\s+(.+)\s*\{\s*$").unwrap();
    let else_if = Regex::new(r"^(?:}\s*)?else\s+if\s+(.+)\s*\{\s*$").unwrap();
    let while_block = Regex::new(r"^while\s+(.+)\s*\{\s*$").unwrap();
    let range_for = Regex::new(r"^for\s+([A-Za-z_][A-Za-z0-9_]*)\s+in\s+(.+?)\.\.(=)?(.+?)\s*\{\s*$").unwrap();
    let each_for = Regex::new(r"^for\s+([A-Za-z_][A-Za-z0-9_]*)\s+in\s+(.+?)\s*\{\s*$").unwrap();
    let repeat_block = Regex::new(r"^repeat\s+(.+?)\s*\{\s*$").unwrap();

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
    java.push_str(
        "public final class Main {\n\
         \n    private static final java.util.Scanner __funoInput = new java.util.Scanner(System.in);\n\
         \n    static String readln() { return __funoInput.nextLine(); }\n\
             static int readInt() { return Integer.parseInt(readln().trim()); }\n\
             static long readLong() { return Long.parseLong(readln().trim()); }\n\
             static double readDouble() { return Double.parseDouble(readln().trim()); }\n\
             static boolean readBool() { return Boolean.parseBoolean(readln().trim()); }\n\
             static int toInt(Object value) { return Integer.parseInt(String.valueOf(value)); }\n\
             static double toDouble(Object value) { return Double.parseDouble(String.valueOf(value)); }\n\
             static int len(String value) { return value.length(); }\n\
             static int len(java.util.Collection<?> value) { return value.size(); }\n\
             static int len(Object value) { return java.lang.reflect.Array.getLength(value); }\n\
             @SafeVarargs static <T> java.util.ArrayList<T> list(T... values) { return new java.util.ArrayList<>(java.util.List.of(values)); }\n\
             @SafeVarargs static <T> java.util.HashSet<T> set(T... values) { return new java.util.HashSet<>(java.util.List.of(values)); }\n\
             static <K, V> java.util.HashMap<K, V> map() { return new java.util.HashMap<>(); }\n\n",
    );

    let lines: Vec<&str> = source.lines().collect();
    let mut current_function = String::new();
    let mut function_depth = 0isize;
    let mut declared_variables = std::collections::HashSet::<String>::new();
    let mut repeat_index = 0usize;

    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim().trim_end_matches(';').trim();
        if trimmed.is_empty()
            || trimmed.starts_with("use ")
            || trimmed.starts_with("lib ")
            || trimmed.starts_with("package ")
        {
            continue;
        }
        if trimmed.starts_with("//") {
            let indent = if current_function.is_empty() {
                "    ".into()
            } else {
                "    ".repeat((function_depth + 1).max(1) as usize)
            };
            java.push_str(&format!("{indent}{trimmed}\n"));
            continue;
        }

        if let Some(cap) = expression_fun.captures(trimmed) {
            let name = &cap[1];
            let params = &cap[2];
            let declared = cap.get(3).map(|m| m.as_str());
            let ret = infer_return(name, declared, &cap[4], "");
            let expr = expression_to_java_typed(&cap[4], declared);
            if name == "main" {
                java.push_str(&format!(
                    "    public static void main(String[] args) {{\n        {expr};\n    }}\n\n"
                ));
            } else if ret == "void" {
                java.push_str(&format!(
                    "    static void {name}({}) {{\n        {expr};\n    }}\n\n",
                    java_params(params, &expr)
                ));
            } else {
                java.push_str(&format!(
                    "    static {ret} {name}({}) {{\n        return {expr};\n    }}\n\n",
                    java_params(params, &expr)
                ));
            }
            continue;
        }

        if let Some(cap) = block_fun.captures(trimmed) {
            let name = cap[1].to_string();
            let params = &cap[2];
            let declared = cap.get(3).map(|m| m.as_str());
            let lookahead = function_body_hint(&lines, idx + 1);
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
            declared_variables.clear();
            for parameter in params.split(',').filter(|p| !p.trim().is_empty()) {
                if let Some(name) = parameter.trim().split(':').next() {
                    declared_variables.insert(name.trim().to_string());
                }
            }
            continue;
        }

        // A closing brace with else must be handled as one Java construct.
        if trimmed.starts_with('}') && trimmed.contains("else") {
            function_depth = (function_depth - 1).max(0);
            let indent = "    ".repeat((function_depth + 1).max(1) as usize);
            if let Some(cap) = else_if.captures(trimmed) {
                java.push_str(&format!("{indent}}} else if ({}) {{\n", expression_to_java(&cap[1])));
            } else {
                java.push_str(&format!("{indent}}} else {{\n"));
            }
            function_depth += 1;
            continue;
        }
        if trimmed == "}" {
            function_depth = (function_depth - 1).max(0);
            let indent = "    ".repeat((function_depth + 1).max(1) as usize);
            java.push_str(&format!("{indent}}}\n"));
            if function_depth == 0 {
                current_function.clear();
                declared_variables.clear();
                java.push('\n');
            }
            continue;
        }

        let indent = if current_function.is_empty() {
            "    ".to_string()
        } else {
            "    ".repeat((function_depth + 1).max(2) as usize)
        };

        if let Some(cap) = else_if.captures(trimmed) {
            java.push_str(&format!("{indent}else if ({}) {{\n", expression_to_java(&cap[1])));
            function_depth += 1;
            continue;
        }
        if trimmed == "else {" || trimmed == "else" {
            java.push_str(&format!("{indent}else {{\n"));
            function_depth += 1;
            continue;
        }
        if let Some(cap) = if_block.captures(trimmed) {
            java.push_str(&format!("{indent}if ({}) {{\n", expression_to_java(&cap[1])));
            function_depth += 1;
            continue;
        }
        if let Some(cap) = while_block.captures(trimmed) {
            java.push_str(&format!("{indent}while ({}) {{\n", expression_to_java(&cap[1])));
            function_depth += 1;
            continue;
        }
        if let Some(cap) = range_for.captures(trimmed) {
            let variable = &cap[1];
            let start = expression_to_java(&cap[2]);
            let inclusive = cap.get(3).is_some();
            let end = expression_to_java(&cap[4]);
            let operator = if inclusive { "<=" } else { "<" };
            java.push_str(&format!(
                "{indent}for (int {variable} = {start}; {variable} {operator} {end}; {variable}++) {{\n"
            ));
            declared_variables.insert(variable.to_string());
            function_depth += 1;
            continue;
        }
        if let Some(cap) = each_for.captures(trimmed) {
            java.push_str(&format!(
                "{indent}for (var {} : {}) {{\n",
                &cap[1],
                expression_to_java(&cap[2])
            ));
            declared_variables.insert(cap[1].to_string());
            function_depth += 1;
            continue;
        }
        if let Some(cap) = repeat_block.captures(trimmed) {
            repeat_index += 1;
            let counter = format!("__repeat{repeat_index}");
            java.push_str(&format!(
                "{indent}for (int {counter} = 0; {counter} < {}; {counter}++) {{\n",
                expression_to_java(&cap[1])
            ));
            function_depth += 1;
            continue;
        }

        if trimmed == "break" || trimmed == "continue" {
            java.push_str(&format!("{indent}{trimmed};\n"));
            continue;
        }
        if Regex::new(r"^return\s*\(\s*200\s*\)$").unwrap().is_match(trimmed)
            && current_function == "main"
        {
            java.push_str(&format!(
                "{indent}// Funo return(200): успешное завершение\n{indent}return;\n"
            ));
            continue;
        }
        if current_function == "main" {
            if let Some(value) = trimmed.strip_prefix("return(").and_then(|v| v.strip_suffix(')')) {
                java.push_str(&format!("{indent}System.exit({});\n{indent}return;\n", expression_to_java(value)));
                continue;
            }
        }
        if let Some(value) = trimmed.strip_prefix("return ") {
            java.push_str(&format!("{indent}return {};\n", expression_to_java(value)));
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("return(").and_then(|v| v.strip_suffix(')')) {
            java.push_str(&format!("{indent}return {};\n", expression_to_java(value)));
            continue;
        }

        if let Some(cap) = named_decl.captures(trimmed) {
            let keyword = &cap[1];
            let name = &cap[2];
            let source_type = cap.get(3).map(|v| v.as_str());
            let value = expression_to_java_typed(&cap[4], source_type);
            let kind = source_type.map(java_type).unwrap_or_else(|| {
                if current_function.is_empty() {
                    // Java does not allow `var` for fields, so top-level values
                    // still need the compiler's best inferred concrete type.
                    infer_expression_type(&cap[4])
                } else {
                    "var".into()
                }
            });
            let final_keyword = if keyword == "let" || keyword == "const" { "final " } else { "" };
            let static_keyword = if current_function.is_empty() { "static " } else { "" };
            java.push_str(&format!("{indent}{static_keyword}{final_keyword}{kind} {name} = {value};\n"));
            declared_variables.insert(name.to_string());
            continue;
        }
        if let Some(cap) = typed_decl.captures(trimmed) {
            let source_type = &cap[1];
            let name = &cap[2];
            let value = expression_to_java_typed(&cap[3], Some(source_type));
            let static_keyword = if current_function.is_empty() { "static " } else { "" };
            java.push_str(&format!(
                "{indent}{static_keyword}{} {name} = {value};\n",
                java_type(source_type)
            ));
            declared_variables.insert(name.to_string());
            continue;
        }
        if let Some(cap) = assignment.captures(trimmed) {
            let name = &cap[1];
            let value = expression_to_java(&cap[2]);
            if declared_variables.contains(name) {
                java.push_str(&format!("{indent}{name} = {value};\n"));
            } else if current_function.is_empty() {
                java.push_str(&format!(
                    "{indent}static {} {name} = {value};\n",
                    infer_expression_type(&cap[2])
                ));
                declared_variables.insert(name.to_string());
            } else {
                java.push_str(&format!("{indent}var {name} = {value};\n"));
                declared_variables.insert(name.to_string());
            }
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

fn installed_funo_sources(root: &std::path::Path) -> String {
    let packages = root.join(".funo").join("packages");
    let mut result = String::new();
    let Ok(ids) = fs::read_dir(packages) else {
        return result;
    };
    for id in ids.flatten().filter(|entry| entry.path().is_dir()) {
        let Ok(versions) = fs::read_dir(id.path()) else {
            continue;
        };
        for version in versions.flatten().filter(|entry| entry.path().is_dir()) {
            let archive_path = version.path().join("package.funpkg");
            let Ok(bytes) = fs::read(&archive_path) else {
                continue;
            };
            let Ok(archive) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            let Some(entry) = archive.get("entry").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(source) = archive
                .get("files")
                .and_then(|value| value.as_object())
                .and_then(|files| files.get(entry))
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            result.push_str(source);
            result.push_str("\n\n");
        }
    }
    result
}

pub fn discover_classpath(project_root: &str) -> Vec<String> {
    let packages = PathBuf::from(project_root).join(".funo").join("packages");
    let mut paths = Vec::new();
    let Ok(ids) = fs::read_dir(packages) else {
        return paths;
    };
    for id in ids.flatten().filter(|entry| entry.path().is_dir()) {
        let Ok(versions) = fs::read_dir(id.path()) else {
            continue;
        };
        for version in versions.flatten().filter(|entry| entry.path().is_dir()) {
            let jar = version.path().join("package.jar");
            if jar.is_file() {
                paths.push(jar.to_string_lossy().to_string());
            }
        }
    }
    paths.sort();
    paths
}

fn compile_program(
    project_root: &str,
    source: &str,
    classpath: &[String],
    run_program: bool,
) -> BuildResult {
    let started = Instant::now();
    let root = match safe_project_root(project_root) {
        Ok(root) => root,
        Err(error) => return failed(error, String::new(), started),
    };
    let mut complete_source = installed_funo_sources(&root);
    complete_source.push_str(source);
    let generated_java = match transpile(&complete_source) {
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

    let build = root.join(".funo").join("build");
    let src_dir = build.join("src");
    let classes = build.join("classes");
    if classes.exists() {
        let _ = fs::remove_dir_all(&classes);
    }
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

    let mut all_classpath = discover_classpath(project_root);
    for path in classpath {
        if !all_classpath.contains(path) {
            all_classpath.push(path.clone());
        }
    }
    let mut javac = Command::new("javac");
    javac
        .arg("-encoding")
        .arg("UTF-8")
        .arg("-d")
        .arg(&classes);
    if !all_classpath.is_empty() {
        javac.arg("-classpath").arg(join_classpath(&all_classpath));
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

    if run_program {
        let mut runtime_paths = vec![classes.to_string_lossy().to_string()];
        runtime_paths.extend(all_classpath);
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
        return BuildResult {
            success: run.status.success(),
            stdout: String::from_utf8_lossy(&run.stdout).trim_end().to_string(),
            stderr: String::from_utf8_lossy(&run.stderr).trim_end().to_string(),
            generated_java,
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: Some(classes.join("Main.class").to_string_lossy().to_string()),
        };
    }

    let jar_path = build.join("app.jar");
    let jar = Command::new("jar")
        .arg("--create")
        .arg("--file")
        .arg(&jar_path)
        .arg("--main-class")
        .arg("Main")
        .arg("-C")
        .arg(&classes)
        .arg(".")
        .output();
    match jar {
        Ok(output) if output.status.success() => BuildResult {
            success: true,
            stdout: format!("Готово: {}", jar_path.display()),
            stderr: String::new(),
            generated_java,
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: Some(jar_path.to_string_lossy().to_string()),
        },
        Ok(output) => BuildResult {
            success: false,
            stdout: String::new(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            generated_java,
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: None,
        },
        Err(error) => failed(
            format!("Не найдена команда jar из JDK: {error}"),
            generated_java,
            started,
        ),
    }
}

pub fn compile_and_run(project_root: &str, source: &str, classpath: &[String]) -> BuildResult {
    compile_program(project_root, source, classpath, true)
}

pub fn compile_only(project_root: &str, source: &str, classpath: &[String]) -> BuildResult {
    compile_program(project_root, source, classpath, false)
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
    fn compiles_types_collections_and_loops() {
        let source = r#"fun main() {
    int score = 1
    int[] rewards = [2, 3]
    list<text> names = ["Alex", "Steve"]
    for i in 0..2 {
        score = score + rewards[i]
    }
    println(names)
}"#;
        let java = transpile(source).unwrap();
        assert!(java.contains("int[] rewards = new int[]{2, 3};"));
        assert!(java.contains("java.util.ArrayList<String> names"));
        assert!(java.contains("for (int i = 0; i < 2; i++)"));
        assert!(java.contains("score = score + rewards[i];"));
    }

    #[test]
    fn operators_do_not_modify_string_literals() {
        let java = expression_to_java(r#"println("and or not: " + (true and not false))"#);
        assert_eq!(
            java,
            r#"System.out.println("and or not: " + (true && ! false))"#
        );
    }

    #[test]
    fn block_return_inference_stops_at_function_boundary() {
        let source = "fun hello() {\n println(\"hello\")\n}\nfun value() -> int {\n return 3\n}";
        let java = transpile(source).unwrap();
        assert!(java.contains("static void hello()"));
        assert!(java.contains("static int value()"));
    }

    #[test]
    fn finds_friendly_typo() {
        let diagnostics = check_source("fun main() {\n printn(1)\n}");
        assert_eq!(diagnostics[0].code, "FUN001");
        assert_eq!(diagnostics[0].replacement.as_deref(), Some("println"));
    }
}

use crate::{models::BuildResult, process};
use regex::Regex;
use std::{collections::HashSet, fs, path::PathBuf, time::Instant};

fn split_generic(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, character) in value.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(value[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(value[start..].trim());
    parts
}

fn generic_inner<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    value.trim().strip_prefix(name)?.strip_prefix('<')?.strip_suffix('>')
}

fn cpp_type(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value.strip_suffix("[]") {
        return format!("std::vector<{}>", cpp_type(inner));
    }
    if let Some(inner) = generic_inner(value, "list") {
        return format!("std::vector<{}>", cpp_type(inner));
    }
    if let Some(inner) = generic_inner(value, "set") {
        return format!("std::unordered_set<{}>", cpp_type(inner));
    }
    if let Some(inner) = generic_inner(value, "map") {
        let parts = split_generic(inner);
        if parts.len() == 2 {
            return format!("std::unordered_map<{}, {}>", cpp_type(parts[0]), cpp_type(parts[1]));
        }
    }
    match value {
        "text" | "string" => "std::string",
        "bool" | "boolean" => "bool",
        "byte" => "signed char",
        "short" => "short",
        "long" => "long long",
        "float" => "float",
        "double" | "number" | "decimal" => "double",
        "char" => "char",
        "void" => "void",
        _ => "int",
    }
    .into()
}

fn rust_type(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value.strip_suffix("[]") {
        return format!("Vec<{}>", rust_type(inner));
    }
    if let Some(inner) = generic_inner(value, "list") {
        return format!("Vec<{}>", rust_type(inner));
    }
    if let Some(inner) = generic_inner(value, "set") {
        return format!("HashSet<{}>", rust_type(inner));
    }
    if let Some(inner) = generic_inner(value, "map") {
        let parts = split_generic(inner);
        if parts.len() == 2 {
            return format!("HashMap<{}, {}>", rust_type(parts[0]), rust_type(parts[1]));
        }
    }
    match value {
        "text" | "string" => "String",
        "bool" | "boolean" => "bool",
        "byte" => "i8",
        "short" => "i16",
        "long" => "i64",
        "float" => "f32",
        "double" | "number" | "decimal" => "f64",
        "char" => "char",
        "void" => "()",
        _ => "i32",
    }
    .into()
}

fn rust_parameters(value: &str) -> (String, String) {
    let mut parameters = Vec::new();
    let mut conversions = String::new();
    for part in split_generic(value).into_iter().filter(|part| !part.trim().is_empty()) {
        let (name, kind) = part.split_once(':').unwrap_or((part, "int"));
        let name = name.trim();
        let kind = kind.trim();
        if matches!(kind, "text" | "string") {
            parameters.push(format!("{name}: impl Into<String>"));
            conversions.push_str(&format!("    let {name}: String = {name}.into();\n"));
        } else {
            let mutable = if kind.ends_with("[]")
                || generic_inner(kind, "list").is_some()
                || generic_inner(kind, "set").is_some()
                || generic_inner(kind, "map").is_some()
            {
                "mut "
            } else {
                ""
            };
            parameters.push(format!("{mutable}{name}: {}", rust_type(kind)));
        }
    }
    (parameters.join(", "), conversions)
}

fn operators(value: &str, target: &str) -> String {
    if target == "python" {
        // Preserve `!=`: replacing every exclamation mark would produce the
        // invalid Python operator `not =`.
        let value = value
            .replace(" && ", " and ")
            .replace(" || ", " or ")
            .replace("not ", "not ");
        let value = Regex::new(r"\btrue\b").unwrap().replace_all(&value, "True").into_owned();
        return Regex::new(r"\bfalse\b").unwrap().replace_all(&value, "False").into_owned();
    }
    value
        .replace(" and ", " && ")
        .replace(" or ", " || ")
        .replace("not ", "!")
}

fn translate_expression(value: &str, target: &str) -> String {
    let mut value = operators(value.trim(), target);
    let long_suffix = Regex::new(r"\b(\d+)L\b").unwrap();
    value = long_suffix
        .replace_all(&value, if target == "cpp" { "${1}LL" } else { "${1}" })
        .into_owned();
    let float_suffix = Regex::new(r"\b(\d+(?:\.\d+)?)f\b").unwrap();
    if matches!(target, "javascript" | "python") {
        value = float_suffix.replace_all(&value, "${1}").into_owned();
    } else if target == "rust" {
        value = float_suffix.replace_all(&value, "${1}f32").into_owned();
    }

    if target == "rust" && value.starts_with('"') && value.ends_with('"') && !value[1..value.len() - 1].contains('"') {
        return format!("{value}.to_string()");
    }
    if value.starts_with('[') && value.ends_with(']') {
        value = match target {
            "cpp" => format!("{{{}}}", &value[1..value.len() - 1]),
            "rust" => {
                let values = split_generic(&value[1..value.len() - 1])
                    .into_iter()
                    .filter(|item| !item.is_empty())
                    .map(|item| translate_expression(item, "rust"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("vec![{values}]")
            },
            _ => value,
        };
    }
    let constructors = [
        (Regex::new(r"\bmap\(\)").unwrap(), match target { "cpp" | "python" => "{}", "rust" => "HashMap::new()", "javascript" => "new Map()", _ => "map()" }),
        (Regex::new(r"\bset\(\)").unwrap(), match target { "cpp" => "{}", "rust" => "HashSet::new()", "javascript" => "new Set()", _ => "set()" }),
        (Regex::new(r"\blist\(\)").unwrap(), match target { "cpp" => "{}", "rust" => "Vec::new()", "javascript" | "python" => "[]", _ => "list()" }),
    ];
    for (pattern, replacement) in constructors {
        value = pattern.replace_all(&value, replacement).into_owned();
    }

    let put = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\.put\((.+),\s*(.+)\)$").unwrap();
    if let Some(parts) = put.captures(&value) {
        let key = translate_expression(&parts[2], target);
        let item = translate_expression(&parts[3], target);
        return match target {
            "cpp" | "python" => format!("{}[{key}] = {item}", &parts[1]),
            "rust" => format!("{}.insert(({key}).into(), ({item}).into())", &parts[1]),
            _ => format!("{}.set({key}, {item})", &parts[1]),
        };
    }
    let add = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\.add\((.+)\)$").unwrap();
    if let Some(parts) = add.captures(&value) {
        let item = translate_expression(&parts[2], target);
        return match target {
            "cpp" => format!("{}.push_back({item})", &parts[1]),
            "rust" => format!("{}.push(({item}).into())", &parts[1]),
            "python" => format!("{}.append({item})", &parts[1]),
            _ => format!("{}.push({item})", &parts[1]),
        };
    }
    let len = Regex::new(r"\blen\(([^()]+)\)").unwrap();
    value = match target {
        "cpp" => len.replace_all(&value, "$1.size()").into_owned(),
        "rust" => len.replace_all(&value, "$1.len()").into_owned(),
        "javascript" => len.replace_all(&value, "$1.length").into_owned(),
        _ => value,
    };

    if value.contains('"') && value.contains(" + ") {
        let pieces = value.split(" + ").collect::<Vec<_>>();
        let rendered = pieces
            .iter()
            .map(|piece| translate_expression(piece, target))
            .collect::<Vec<_>>()
            .join(", ");
        return match target {
            "cpp" => format!("funo_concat({rendered})"),
            "rust" => {
                let placeholders = "{}".repeat(pieces.len());
                format!("format!(\"{placeholders}\", {rendered})")
            }
            "python" => format!("funo_concat({rendered})"),
            _ => value,
        };
    }
    value
}

fn translate_statement(value: &str, target: &str, set_names: &HashSet<String>) -> String {
    let add = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\.add\((.+)\)$").unwrap();
    if let Some(parts) = add.captures(value.trim()) {
        if set_names.contains(&parts[1]) {
            return match target {
                "cpp" => format!("{}.insert({})", &parts[1], translate_expression(&parts[2], target)),
                "rust" => format!("{}.insert(({}).into())", &parts[1], translate_expression(&parts[2], target)),
                _ => format!("{}.add({})", &parts[1], translate_expression(&parts[2], target)),
            };
        }
    }
    translate_expression(value, target)
}

fn update_collection_kind(
    line: &str,
    typed: &Regex,
    named: &Regex,
    set_names: &mut HashSet<String>,
) {
    let declaration = typed
        .captures(line)
        .map(|parts| (parts[2].to_string(), parts[1].to_string()))
        .or_else(|| named.captures(line).map(|parts| (parts[1].to_string(), parts[2].to_string())));
    if let Some((name, kind)) = declaration {
        if kind == "set" {
            set_names.insert(name);
        } else {
            set_names.remove(&name);
        }
    }
}

fn update_parameter_collections(parameters: &str, set_names: &mut HashSet<String>) {
    set_names.clear();
    for parameter in split_generic(parameters) {
        let Some((name, kind)) = parameter.split_once(':') else { continue };
        if generic_inner(kind.trim(), "set").is_some() {
            set_names.insert(name.trim().to_string());
        }
    }
}

fn rust_expression_for_type(value: &str, kind: Option<&str>) -> String {
    let kind = kind.map(str::trim);
    let conditional = Regex::new(r"^if\s+(.+?)\s+then\s+(.+?)\s+else\s+(.+)$").unwrap();
    if let Some(parts) = conditional.captures(value.trim()) {
        let condition = translate_expression(&parts[1], "rust");
        let yes = rust_expression_for_type(&parts[2], kind);
        let no = rust_expression_for_type(&parts[3], kind);
        return format!("if {condition} {{ {yes} }} else {{ {no} }}");
    }
    if matches!(kind, Some("float" | "double" | "number" | "decimal")) && value.contains(" / ") {
        let parts = value.splitn(2, " / ").collect::<Vec<_>>();
        let target = if kind == Some("float") { "f32" } else { "f64" };
        return format!("({} as {target}) / ({} as {target})", translate_expression(parts[0], "rust"), translate_expression(parts[1], "rust"));
    }
    translate_expression(value, "rust")
}

fn conditional_expression(value: &str, target: &str) -> String {
    let pattern = Regex::new(r"^if\s+(.+?)\s+then\s+(.+?)\s+else\s+(.+)$").unwrap();
    let Some(parts) = pattern.captures(value.trim()) else {
        return translate_expression(value, target);
    };
    let condition = translate_expression(&parts[1], target);
    let yes = conditional_expression(&parts[2], target);
    let no = conditional_expression(&parts[3], target);
    match target {
        "python" => format!("{yes} if {condition} else {no}"),
        "rust" => format!("if {condition} {{ {yes} }} else {{ {no} }}"),
        _ => format!("({condition}) ? ({yes}) : ({no})"),
    }
}

pub fn transpile_backend(source: &str, target: &str) -> Result<String, String> {
    match target.to_ascii_lowercase().as_str() {
        "cpp" | "c++" => transpile_cpp(source),
        "rust" | "rs" => transpile_rust(source),
        "javascript" | "js" => transpile_script(source, false),
        "python" | "py" => transpile_script(source, true),
        _ => Err("Цель должна быть cpp, rust, javascript или python".into()),
    }
}

fn transpile_cpp(source: &str) -> Result<String, String> {
    let function = Regex::new(
        r"^(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*([^{=]+?))?\s*\{",
    )
    .unwrap();
    let expression_function = Regex::new(
        r"^(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*([^=]+?))?\s*=\s*(.+)$",
    )
    .unwrap();
    let typed = Regex::new(
        r"^((?:byte|short|int|long|float|double|number|decimal|text|string|bool|boolean|char)(?:\[\])?|(?:list|set|map)<.+>)\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$",
    )
    .unwrap();
    let named = Regex::new(r"^(let|const|var)\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*:\s*([^=]+?))?\s*=\s*(.+)$").unwrap();
    let range = Regex::new(r"^for\s+(\w+)\s+in\s+(.+?)\.\.(=)?(.+?)\s*\{$").unwrap();
    let foreach = Regex::new(r"^for\s+(\w+)\s+in\s+(.+?)\s*\{$").unwrap();
    let collection_typed = Regex::new(r"^(list|set)<.+>\s+(\w+)\s*=").unwrap();
    let collection_named = Regex::new(r"^(?:let|const|var)\s+(\w+)\s*:\s*(list|set)<.+>\s*=").unwrap();
    let mut set_names = HashSet::new();
    let mut out = String::from(
        "// Generated by Funo 1.0 — C++17 backend\n#include <cmath>\n#include <iostream>\n#include <sstream>\n#include <string>\n#include <unordered_map>\n#include <unordered_set>\n#include <vector>\n\ntemplate <typename... Values> std::string funo_concat(const Values&... values) { std::ostringstream out; (out << ... << values); return out.str(); }\ntemplate <typename Value> void funo_write(const Value& value) { std::cout << value; }\ntemplate <typename Value> void funo_write(const std::vector<Value>& values) { std::cout << \"[\"; bool first = true; for (const auto& value : values) { if (!first) std::cout << \", \"; first = false; funo_write(value); } std::cout << \"]\"; }\ntemplate <typename Value> void funo_write(const std::unordered_set<Value>& values) { std::cout << \"{\"; bool first = true; for (const auto& value : values) { if (!first) std::cout << \", \"; first = false; funo_write(value); } std::cout << \"}\"; }\ntemplate <typename Key, typename Value> void funo_write(const std::unordered_map<Key, Value>& values) { std::cout << \"{\"; bool first = true; for (const auto& entry : values) { if (!first) std::cout << \", \"; first = false; funo_write(entry.first); std::cout << \": \"; funo_write(entry.second); } std::cout << \"}\"; }\ntemplate <typename Value> void funo_print(const Value& value, bool newline) { funo_write(value); if (newline) std::cout << std::endl; }\ninline std::string readln() { std::string value; std::getline(std::cin >> std::ws, value); return value; }\ninline int readInt() { return std::stoi(readln()); }\ninline long long readLong() { return std::stoll(readln()); }\ninline double readDouble() { return std::stod(readln()); }\ninline bool readBool() { const auto value = readln(); return value == \"true\" || value == \"1\"; }\ninline int toInt(const std::string& value) { return std::stoi(value); }\ninline double toDouble(const std::string& value) { return std::stod(value); }\n\n",
    );
    for raw in source.lines() {
        let line = raw.trim().trim_end_matches(';').trim();
        if line.is_empty() || line.starts_with("use ") || line.starts_with("package ") {
            continue;
        }
        let indent = "    ".repeat(raw.chars().take_while(|value| value.is_whitespace()).count() / 4);
        update_collection_kind(line, &collection_typed, &collection_named, &mut set_names);
        if line.starts_with("//") {
            out.push_str(&format!("{indent}{line}\n"));
        } else if let Some(cap) = function.captures(line) {
            update_parameter_collections(&cap[2], &mut set_names);
            let name = &cap[1];
            let params = split_generic(&cap[2])
                .into_iter()
                .filter(|part| !part.trim().is_empty())
                .map(|part| {
                    let (name, kind) = part.split_once(':').unwrap_or((part, "int"));
                    format!("{} {}", cpp_type(kind), name.trim())
                })
                .collect::<Vec<_>>()
                .join(", ");
            let result = if name == "main" { "int".into() } else { cpp_type(cap.get(3).map(|v| v.as_str()).unwrap_or("void")) };
            out.push_str(&format!("{result} {name}({params}) {{\n"));
        } else if let Some(cap) = expression_function.captures(line) {
            update_parameter_collections(&cap[2], &mut set_names);
            let name = &cap[1];
            let params = split_generic(&cap[2])
                .into_iter()
                .filter(|part| !part.trim().is_empty())
                .map(|part| {
                    let (name, kind) = part.split_once(':').unwrap_or((part, "int"));
                    format!("{} {}", cpp_type(kind), name.trim())
                })
                .collect::<Vec<_>>()
                .join(", ");
            let result = if name == "main" {
                "int".to_string()
            } else {
                cap.get(3).map(|value| cpp_type(value.as_str())).unwrap_or_else(|| "auto".into())
            };
            let expression = cap[4].trim();
            out.push_str(&format!("{result} {name}({params}) {{\n"));
            if let Some(value) = expression.strip_prefix("println(").and_then(|value| value.strip_suffix(')')) {
                out.push_str(&format!("    funo_print({}, true);\n", conditional_expression(value, "cpp")));
                if name == "main" { out.push_str("    return 0;\n"); }
            } else if let Some(value) = expression.strip_prefix("print(").and_then(|value| value.strip_suffix(')')) {
                out.push_str(&format!("    funo_print({}, false);\n", conditional_expression(value, "cpp")));
                if name == "main" { out.push_str("    return 0;\n"); }
            } else if name == "main" {
                out.push_str(&format!("    (void)({});\n    return 0;\n", conditional_expression(expression, "cpp")));
            } else {
                out.push_str(&format!("    return {};\n", conditional_expression(expression, "cpp")));
            }
            out.push_str("}\n");
        } else if line == "}" {
            out.push_str(&format!("{indent}}}\n"));
        } else if let Some(cap) = typed.captures(line) {
            out.push_str(&format!("{indent}{} {} = {};\n", cpp_type(&cap[1]), &cap[2], conditional_expression(&cap[3], "cpp")));
        } else if let Some(cap) = named.captures(line) {
            let final_prefix = if &cap[1] == "var" { "" } else { "const " };
            let kind = cap.get(3).map(|value| cpp_type(value.as_str())).unwrap_or_else(|| "auto".into());
            out.push_str(&format!("{indent}{final_prefix}{kind} {} = {};\n", &cap[2], conditional_expression(&cap[4], "cpp")));
        } else if let Some(cap) = range.captures(line) {
            let comparison = if cap.get(3).is_some() { "<=" } else { "<" };
            out.push_str(&format!("{indent}for (int {} = {}; {} {comparison} {}; ++{}) {{\n", &cap[1], translate_expression(&cap[2], "cpp"), &cap[1], translate_expression(&cap[4], "cpp"), &cap[1]));
        } else if let Some(cap) = foreach.captures(line) {
            out.push_str(&format!("{indent}for (const auto& {} : {}) {{\n", &cap[1], translate_expression(&cap[2], "cpp")));
        } else if let Some(value) = line.strip_prefix("println(").and_then(|v| v.strip_suffix(')')) {
            out.push_str(&format!("{indent}funo_print({}, true);\n", conditional_expression(value, "cpp")));
        } else if let Some(value) = line.strip_prefix("print(").and_then(|v| v.strip_suffix(')')) {
            out.push_str(&format!("{indent}funo_print({}, false);\n", conditional_expression(value, "cpp")));
        } else if line == "return(200)" {
            out.push_str(&format!("{indent}return 0;\n"));
        } else if let Some(value) = line.strip_prefix("return(").and_then(|v| v.strip_suffix(')')) {
            out.push_str(&format!("{indent}return {};\n", conditional_expression(value, "cpp")));
        } else if let Some(condition) = line.strip_prefix("if ").and_then(|v| v.strip_suffix('{')) {
            out.push_str(&format!("{indent}if ({}) {{\n", conditional_expression(condition.trim(), "cpp")));
        } else if let Some(condition) = line.strip_prefix("while ").and_then(|v| v.strip_suffix('{')) {
            out.push_str(&format!("{indent}while ({}) {{\n", conditional_expression(condition.trim(), "cpp")));
        } else if let Some(condition) = line.strip_prefix("} else if ").and_then(|v| v.strip_suffix('{')) {
            out.push_str(&format!("{indent}}} else if ({}) {{\n", conditional_expression(condition.trim(), "cpp")));
        } else if line == "} else {" || line == "else {" {
            out.push_str(&format!("{indent}{}\n", if line.starts_with('}') { "} else {" } else { "else {" }));
        } else {
            out.push_str(&format!("{indent}{};\n", translate_statement(line, "cpp", &set_names)));
        }
    }
    if !out.contains("int main(") {
        return Err("Для C++ backend нужна функция fun main()".into());
    }
    Ok(out)
}

fn transpile_rust(source: &str) -> Result<String, String> {
    let function = Regex::new(
        r"^(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*([^{=]+?))?\s*\{",
    )
    .unwrap();
    let expression_function = Regex::new(
        r"^(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*([^=]+?))?\s*=\s*(.+)$",
    )
    .unwrap();
    let typed = Regex::new(
        r"^((?:byte|short|int|long|float|double|number|decimal|text|string|bool|boolean|char)(?:\[\])?|(?:list|set|map)<.+>)\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$",
    )
    .unwrap();
    let named = Regex::new(r"^(let|const|var)\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*:\s*([^=]+?))?\s*=\s*(.+)$").unwrap();
    let range = Regex::new(r"^for\s+(\w+)\s+in\s+(.+?)\.\.(=)?(.+?)\s*\{$").unwrap();
    let foreach = Regex::new(r"^for\s+(\w+)\s+in\s+(.+?)\s*\{$").unwrap();
    let collection_typed = Regex::new(r"^(list|set)<.+>\s+(\w+)\s*=").unwrap();
    let collection_named = Regex::new(r"^(?:let|const|var)\s+(\w+)\s*:\s*(list|set)<.+>\s*=").unwrap();
    let mut set_names = HashSet::new();
    let mut current_return_type: Option<String> = None;
    let mut out = String::from("// Generated by Funo 1.0 — Rust backend\nuse std::collections::{HashMap, HashSet};\nuse std::hash::Hash;\nuse std::io::{self, Write};\n\ntrait FunoDisplay { fn funo_display(&self) -> String; }\nmacro_rules! funo_display_value { ($($kind:ty),* $(,)?) => { $(impl FunoDisplay for $kind { fn funo_display(&self) -> String { self.to_string() } })* }; }\nfuno_display_value!(i8, i16, i32, i64, u8, u16, u32, u64, usize, f32, f64, bool, char, String);\nimpl FunoDisplay for str { fn funo_display(&self) -> String { self.to_string() } }\nimpl<Value: FunoDisplay + ?Sized> FunoDisplay for &Value { fn funo_display(&self) -> String { (*self).funo_display() } }\nimpl<Value: FunoDisplay> FunoDisplay for Vec<Value> { fn funo_display(&self) -> String { format!(\"[{}]\", self.iter().map(FunoDisplay::funo_display).collect::<Vec<_>>().join(\", \")) } }\nimpl<Value: FunoDisplay + Eq + Hash> FunoDisplay for HashSet<Value> { fn funo_display(&self) -> String { format!(\"{{{}}}\", self.iter().map(FunoDisplay::funo_display).collect::<Vec<_>>().join(\", \")) } }\nimpl<Key: FunoDisplay + Eq + Hash, Value: FunoDisplay> FunoDisplay for HashMap<Key, Value> { fn funo_display(&self) -> String { format!(\"{{{}}}\", self.iter().map(|(key, value)| format!(\"{}: {}\", key.funo_display(), value.funo_display())).collect::<Vec<_>>().join(\", \")) } }\nfn funo_print<Value: FunoDisplay>(value: &Value, newline: bool) { if newline { println!(\"{}\", value.funo_display()); } else { print!(\"{}\", value.funo_display()); let _ = io::stdout().flush(); } }\nfn readln() -> String { let mut value = String::new(); io::stdin().read_line(&mut value).expect(\"input failed\"); value.trim_end_matches(['\\r', '\\n']).to_string() }\nfn readInt() -> i32 { readln().parse().expect(\"expected int\") }\nfn readLong() -> i64 { readln().parse().expect(\"expected long\") }\nfn readDouble() -> f64 { readln().parse().expect(\"expected double\") }\nfn readBool() -> bool { matches!(readln().as_str(), \"true\" | \"1\") }\nfn toInt(value: impl AsRef<str>) -> i32 { value.as_ref().parse().expect(\"expected int\") }\nfn toDouble(value: impl AsRef<str>) -> f64 { value.as_ref().parse().expect(\"expected double\") }\n\n");
    for raw in source.lines() {
        let line = raw.trim().trim_end_matches(';').trim();
        if line.is_empty() || line.starts_with("use ") || line.starts_with("package ") {
            continue;
        }
        let indent = "    ".repeat(raw.chars().take_while(|value| value.is_whitespace()).count() / 4);
        update_collection_kind(line, &collection_typed, &collection_named, &mut set_names);
        if let Some(cap) = function.captures(line) {
            update_parameter_collections(&cap[2], &mut set_names);
            current_return_type = cap.get(3).map(|value| value.as_str().trim().to_string());
            let name = &cap[1];
            let (params, conversions) = rust_parameters(&cap[2]);
            let result = if name == "main" {
                String::new()
            } else {
                cap.get(3)
                    .map(|value| format!(" -> {}", rust_type(value.as_str())))
                    .unwrap_or_default()
            };
            out.push_str(&format!("fn {name}({params}){result} {{\n{conversions}"));
        } else if let Some(cap) = expression_function.captures(line) {
            update_parameter_collections(&cap[2], &mut set_names);
            let name = &cap[1];
            let (params, conversions) = rust_parameters(&cap[2]);
            let result = if name == "main" {
                String::new()
            } else {
                cap.get(3)
                    .map(|value| format!(" -> {}", rust_type(value.as_str())))
                    .unwrap_or_default()
            };
            let expression = cap[4].trim();
            out.push_str(&format!("fn {name}({params}){result} {{\n{conversions}"));
            if let Some(value) = expression.strip_prefix("println(").and_then(|value| value.strip_suffix(')')) {
                out.push_str(&format!("    funo_print(&({}), true);\n", conditional_expression(value, "rust")));
            } else if let Some(value) = expression.strip_prefix("print(").and_then(|value| value.strip_suffix(')')) {
                out.push_str(&format!("    funo_print(&({}), false);\n", conditional_expression(value, "rust")));
            } else if name == "main" {
                out.push_str(&format!("    let _ = {};\n", conditional_expression(expression, "rust")));
            } else {
                out.push_str(&format!("    {}\n", rust_expression_for_type(expression, cap.get(3).map(|value| value.as_str()))));
            }
            out.push_str("}\n");
        } else if let Some(cap) = typed.captures(line) {
            let value = rust_expression_for_type(&cap[3], Some(&cap[1]));
            out.push_str(&format!("{indent}let mut {}: {} = {value};\n", &cap[2], rust_type(&cap[1])));
        } else if let Some(cap) = named.captures(line) {
            let mutable = if &cap[1] == "var" { "mut " } else { "" };
            let declared_type = cap.get(3).map(|value| value.as_str());
            let annotation = declared_type.map(|value| format!(": {}", rust_type(value))).unwrap_or_default();
            let value = rust_expression_for_type(&cap[4], declared_type);
            out.push_str(&format!("{indent}let {mutable}{}{annotation} = {value};\n", &cap[2]));
        } else if let Some(cap) = range.captures(line) {
            let dots = if cap.get(3).is_some() { "..=" } else { ".." };
            out.push_str(&format!("{indent}for {} in {}{dots}{} {{\n", &cap[1], translate_expression(&cap[2], "rust"), translate_expression(&cap[4], "rust")));
        } else if let Some(cap) = foreach.captures(line) {
            out.push_str(&format!("{indent}for {} in {}.iter().cloned() {{\n", &cap[1], translate_expression(&cap[2], "rust")));
        } else if let Some(value) = line.strip_prefix("println(").and_then(|v| v.strip_suffix(')')) {
            out.push_str(&format!("{indent}funo_print(&({}), true);\n", conditional_expression(value, "rust")));
        } else if let Some(value) = line.strip_prefix("print(").and_then(|v| v.strip_suffix(')')) {
            out.push_str(&format!("{indent}funo_print(&({}), false);\n", conditional_expression(value, "rust")));
        } else if line == "return(200)" {
            out.push_str(&format!("{indent}return;\n"));
        } else if let Some(value) = line.strip_prefix("return(").and_then(|v| v.strip_suffix(')')) {
            out.push_str(&format!("{indent}return {};\n", rust_expression_for_type(value, current_return_type.as_deref())));
        } else if let Some(condition) = line.strip_prefix("if ").and_then(|v| v.strip_suffix('{')) {
            out.push_str(&format!("{indent}if {} {{\n", conditional_expression(condition.trim(), "rust")));
        } else if let Some(condition) = line.strip_prefix("while ").and_then(|v| v.strip_suffix('{')) {
            out.push_str(&format!("{indent}while {} {{\n", conditional_expression(condition.trim(), "rust")));
        } else if let Some(condition) = line.strip_prefix("} else if ").and_then(|v| v.strip_suffix('{')) {
            out.push_str(&format!("{indent}}} else if {} {{\n", conditional_expression(condition.trim(), "rust")));
        } else if line == "} else {" || line == "else {" {
            out.push_str(&format!("{indent}{}\n", if line.starts_with('}') { "} else {" } else { "else {" }));
        } else if line.starts_with("//") {
            out.push_str(&format!("{indent}{line}\n"));
        } else {
            out.push_str(&format!("{indent}{}{}\n", translate_statement(line, "rust", &set_names), if matches!(line, "{" | "}") || line.ends_with('{') { "" } else { ";" }));
        }
    }
    if !out.contains("fn main(") {
        return Err("Для Rust backend нужна функция fun main()".into());
    }
    Ok(out)
}

fn transpile_script(source: &str, python: bool) -> Result<String, String> {
    let function = Regex::new(r"^(?:public\s+)?fun\s+(\w+)\(([^)]*)\)(?:\s*->\s*[^{=]+?)?\s*\{").unwrap();
    let expression_function = Regex::new(r"^(?:public\s+)?fun\s+(\w+)\(([^)]*)\)(?:\s*->\s*[^=]+?)?\s*=\s*(.+)$").unwrap();
    let declaration = Regex::new(r"^(?:(?:let|var|const)\s+|(?:(?:byte|short|int|long|float|double|number|decimal|text|string|bool|boolean|char)(?:\[\])?|(?:list|set|map)<.+>)\s+)(\w+)(?:\s*:\s*[^=]+?)?\s*=\s*(.+)$").unwrap();
    let range = Regex::new(r"^for\s+(\w+)\s+in\s+(.+?)\.\.(=)?(.+?)\s*\{$").unwrap();
    let foreach = Regex::new(r"^for\s+(\w+)\s+in\s+(.+?)\s*\{$").unwrap();
    let collection_typed = Regex::new(r"^(list|set)<.+>\s+(\w+)\s*=").unwrap();
    let collection_named = Regex::new(r"^(?:let|const|var)\s+(\w+)\s*:\s*(list|set)<.+>\s*=").unwrap();
    let mut set_names = HashSet::new();
    let mut out = if python {
        "# Generated by Funo 1.0\ndef funo_concat(*values):\n    return \"\".join(str(value) for value in values)\n\ndef readln():\n    return input()\n\ndef readInt():\n    return int(input())\n\ndef readLong():\n    return int(input())\n\ndef readDouble():\n    return float(input())\n\ndef readBool():\n    return input().strip().lower() in (\"true\", \"1\")\n\ndef toInt(value):\n    return int(value)\n\ndef toDouble(value):\n    return float(value)\n\n".to_string()
    } else {
        "// Generated by Funo 1.0\nconst __funoLines = process.getBuiltinModule('fs').readFileSync(0, 'utf8').split(/\\r?\\n/);\nfunction readln() { return __funoLines.shift() ?? ''; }\nfunction readInt() { return Number.parseInt(readln(), 10); }\nfunction readLong() { return Number.parseInt(readln(), 10); }\nfunction readDouble() { return Number.parseFloat(readln()); }\nfunction readBool() { return ['true', '1'].includes(readln().trim().toLowerCase()); }\nfunction toInt(value) { return Number.parseInt(String(value), 10); }\nfunction toDouble(value) { return Number.parseFloat(String(value)); }\n\n".to_string()
    };
    let mut depth = 0usize;
    let mut saw_main = false;
    for raw in source.lines() {
        let mut line = raw.trim().trim_end_matches(';').trim();
        if line.is_empty() || line.starts_with("use ") || line.starts_with("package ") {
            continue;
        }
        if line.starts_with("//") {
            out.push_str(&format!("{}{}{}\n", "    ".repeat(if python { depth } else { 0 }), if python { "#" } else { "//" }, line.trim_start_matches("//")));
            continue;
        }
        let closes_then_opens = line.starts_with('}') && line[1..].trim_start().starts_with("else");
        if line == "}" || closes_then_opens {
            depth = depth.saturating_sub(1);
            if line == "}" {
                if !python { out.push_str("}\n"); }
                continue;
            }
            line = line.trim_start_matches('}').trim();
        }
        let indent = if python { "    ".repeat(depth) } else { String::new() };
        update_collection_kind(line, &collection_typed, &collection_named, &mut set_names);
        if let Some(cap) = function.captures(line) {
            update_parameter_collections(&cap[2], &mut set_names);
            if &cap[1] == "main" { saw_main = true; }
            let params = split_generic(&cap[2])
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.split_once(':').map(|pair| pair.0).unwrap_or(value).trim())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("{indent}{} {}({params}){}\n", if python { "def" } else { "function" }, &cap[1], if python { ":" } else { " {" }));
            depth += 1;
        } else if let Some(cap) = expression_function.captures(line) {
            update_parameter_collections(&cap[2], &mut set_names);
            let name = &cap[1];
            if name == "main" { saw_main = true; }
            let params = split_generic(&cap[2])
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.split_once(':').map(|pair| pair.0).unwrap_or(value).trim())
                .collect::<Vec<_>>()
                .join(", ");
            let expression = cap[3].trim();
            let target = if python { "python" } else { "javascript" };
            out.push_str(&format!("{indent}{} {}({params}){}\n", if python { "def" } else { "function" }, name, if python { ":" } else { " {" }));
            let body_indent = if python { format!("{indent}    ") } else { "    ".into() };
            if let Some(value) = expression.strip_prefix("println(").and_then(|value| value.strip_suffix(')')) {
                out.push_str(&format!("{body_indent}{}({}){}\n", if python { "print" } else { "console.log" }, conditional_expression(value, target), if python { "" } else { ";" }));
            } else if let Some(value) = expression.strip_prefix("print(").and_then(|value| value.strip_suffix(')')) {
                let value = conditional_expression(value, target);
                if python {
                    out.push_str(&format!("{body_indent}print({value}, end=\"\")\n"));
                } else {
                    out.push_str(&format!("{body_indent}process.stdout.write(String({value}));\n"));
                }
            } else if name == "main" {
                out.push_str(&format!("{body_indent}{}{}\n", conditional_expression(expression, target), if python { "" } else { ";" }));
            } else {
                out.push_str(&format!("{body_indent}return {}{}\n", conditional_expression(expression, target), if python { "" } else { ";" }));
            }
            if !python { out.push_str("}\n"); }
        } else if let Some(cap) = range.captures(line) {
            let target = if python { "python" } else { "javascript" };
            if python {
                let end = if cap.get(3).is_some() { format!("({}) + 1", translate_expression(&cap[4], target)) } else { translate_expression(&cap[4], target) };
                out.push_str(&format!("{indent}for {} in range({}, {end}):\n", &cap[1], translate_expression(&cap[2], target)));
            } else {
                let comparison = if cap.get(3).is_some() { "<=" } else { "<" };
                out.push_str(&format!("for (let {} = {}; {} {comparison} {}; {}++) {{\n", &cap[1], translate_expression(&cap[2], target), &cap[1], translate_expression(&cap[4], target), &cap[1]));
            }
            depth += 1;
        } else if let Some(cap) = foreach.captures(line) {
            if python {
                out.push_str(&format!("{indent}for {} in {}:\n", &cap[1], translate_expression(&cap[2], "python")));
            } else {
                out.push_str(&format!("for (const {} of {}) {{\n", &cap[1], translate_expression(&cap[2], "javascript")));
            }
            depth += 1;
        } else if let Some(condition) = line.strip_prefix("if ").and_then(|value| value.strip_suffix('{')) {
            let condition = conditional_expression(condition.trim(), if python { "python" } else { "javascript" });
            out.push_str(&format!("{indent}if {}{}\n", if python { condition } else { format!("({condition})") }, if python { ":" } else { " {" }));
            depth += 1;
        } else if let Some(condition) = line.strip_prefix("while ").and_then(|value| value.strip_suffix('{')) {
            let condition = conditional_expression(condition.trim(), if python { "python" } else { "javascript" });
            out.push_str(&format!("{indent}while {}{}\n", if python { condition } else { format!("({condition})") }, if python { ":" } else { " {" }));
            depth += 1;
        } else if let Some(condition) = line.strip_prefix("else if ").and_then(|value| value.strip_suffix('{')) {
            let condition = conditional_expression(condition.trim(), if python { "python" } else { "javascript" });
            out.push_str(&format!("{indent}{}{} ({condition}){}\n", if !python && closes_then_opens { "} " } else { "" }, if python { "elif" } else { "else if" }, if python { ":" } else { " {" }));
            depth += 1;
        } else if line.starts_with("else") {
            out.push_str(&format!("{indent}{}else{}\n", if !python && closes_then_opens { "} " } else { "" }, if python { ":" } else { " {" }));
            depth += 1;
        } else if let Some(cap) = declaration.captures(line) {
            let target = if python { "python" } else { "javascript" };
            let prefix = if python { "" } else { "let " };
            out.push_str(&format!("{indent}{prefix}{} = {}{}\n", &cap[1], conditional_expression(&cap[2], target), if python { "" } else { ";" }));
        } else if let Some(value) = line.strip_prefix("println(").and_then(|value| value.strip_suffix(')')) {
            let target = if python { "python" } else { "javascript" };
            out.push_str(&format!("{indent}{}({}){}\n", if python { "print" } else { "console.log" }, conditional_expression(value, target), if python { "" } else { ";" }));
        } else if let Some(value) = line.strip_prefix("print(").and_then(|value| value.strip_suffix(')')) {
            let value = conditional_expression(value, if python { "python" } else { "javascript" });
            if python {
                out.push_str(&format!("{indent}print({value}, end=\"\")\n"));
            } else {
                out.push_str(&format!("{indent}process.stdout.write(String({value}));\n"));
            }
        } else if line == "return(200)" {
            out.push_str(&format!("{indent}return{}\n", if python { "" } else { ";" }));
        } else if let Some(value) = line.strip_prefix("return(").and_then(|value| value.strip_suffix(')')) {
            out.push_str(&format!("{indent}return {}{}\n", conditional_expression(value, if python { "python" } else { "javascript" }), if python { "" } else { ";" }));
        } else {
            let target = if python { "python" } else { "javascript" };
            out.push_str(&format!("{indent}{}{}\n", translate_statement(line, target, &set_names), if python || line.ends_with('{') { "" } else { ";" }));
        }
    }
    if !saw_main {
        return Err(format!("Для {} backend нужна функция fun main()", if python { "Python" } else { "JavaScript" }));
    }
    out.push_str(if python { "\nif __name__ == \"__main__\":\n    main()\n" } else { "\nmain();\n" });
    Ok(out)
}

pub fn build_backend(project_root: &str, source: &str, target: &str, run: bool) -> BuildResult {
    let started = Instant::now();
    let generated = match transpile_backend(source, target) {
        Ok(value) => value,
        Err(error) => return failed(error, String::new(), started),
    };
    let root = PathBuf::from(project_root);
    if !root.is_absolute() {
        return failed("Путь проекта должен быть абсолютным".into(), generated, started);
    }
    let build = root.join(".funo").join("native").join(target.to_ascii_lowercase());
    if let Err(error) = fs::create_dir_all(&build) {
        return failed(error.to_string(), generated, started);
    }
    let normalized = target.to_ascii_lowercase();
    let (source_name, artifact_name) = match normalized.as_str() {
        "cpp" | "c++" => ("main.cpp", if cfg!(windows) { "funo-app.exe" } else { "funo-app" }),
        "rust" | "rs" => ("main.rs", if cfg!(windows) { "funo-app.exe" } else { "funo-app" }),
        "javascript" | "js" => ("main.js", "main.js"),
        _ => ("main.py", "main.py"),
    };
    let source_path = build.join(source_name);
    if let Err(error) = fs::write(&source_path, &generated) {
        return failed(error.to_string(), generated, started);
    }
    let artifact = build.join(artifact_name);
    let command_result = match normalized.as_str() {
        "cpp" | "c++" => process::command("c++")
            .arg("-std=c++17")
            .arg(&source_path)
            .arg("-o")
            .arg(&artifact)
            .output(),
        "rust" | "rs" => process::command("rustc")
            .arg("--edition=2021")
            .arg(&source_path)
            .arg("-o")
            .arg(&artifact)
            .output(),
        "javascript" | "js" if run => process::command("node").arg(&source_path).output(),
        "python" | "py" if run => process::command(if cfg!(windows) { "python" } else { "python3" }).arg(&source_path).output(),
        _ => {
            return BuildResult {
                success: true,
                stdout: format!("Исходник создан: {}", source_path.display()),
                stderr: String::new(),
                generated_java: generated,
                elapsed_ms: started.elapsed().as_millis(),
                diagnostics: Vec::new(),
                artifact: Some(source_path.to_string_lossy().to_string()),
            }
        }
    };
    let output = match command_result {
        Ok(value) => value,
        Err(error) => return failed(format!("Компилятор {target} не найден: {error}"), generated, started),
    };
    if !output.status.success() {
        return failed(String::from_utf8_lossy(&output.stderr).to_string(), generated, started);
    }
    if run && matches!(normalized.as_str(), "cpp" | "c++" | "rust" | "rs") {
        match process::command(&artifact).output() {
            Ok(value) => {
                return BuildResult {
                    success: value.status.success(),
                    stdout: String::from_utf8_lossy(&value.stdout).trim_end().into(),
                    stderr: String::from_utf8_lossy(&value.stderr).trim_end().into(),
                    generated_java: generated,
                    elapsed_ms: started.elapsed().as_millis(),
                    diagnostics: Vec::new(),
                    artifact: Some(artifact.to_string_lossy().into()),
                }
            }
            Err(error) => return failed(error.to_string(), generated, started),
        }
    }
    BuildResult {
        success: true,
        stdout: String::from_utf8_lossy(&output.stdout).trim_end().into(),
        stderr: String::new(),
        generated_java: generated,
        elapsed_ms: started.elapsed().as_millis(),
        diagnostics: Vec::new(),
        artifact: Some(artifact.to_string_lossy().into()),
    }
}

fn failed(error: String, generated: String, started: Instant) -> BuildResult {
    BuildResult {
        success: false,
        stdout: String::new(),
        stderr: error,
        generated_java: generated,
        elapsed_ms: started.elapsed().as_millis(),
        diagnostics: Vec::new(),
        artifact: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_cpp_and_rust_entrypoints() {
        let source = "fun main() {\n    int score = 2\n    score += 3\n    println(score)\n    return(200)\n}";
        assert!(transpile_backend(source, "cpp").unwrap().contains("int main()"));
        assert!(transpile_backend(source, "rust").unwrap().contains("fn main()"));
    }

    #[test]
    fn emits_valid_script_constructs() {
        let source = "fun double(n: int) -> int = if n != 0 then n * 2 else 0\nfun greet(name: text) {\n    println(name)\n}\nfun main() {\n    for i in 0..3 {\n        greet(\"Funo\")\n    }\n}";
        let javascript = transpile_backend(source, "javascript").unwrap();
        assert!(javascript.contains("function double(n) {"));
        assert!(javascript.contains("return (n != 0) ? (n * 2) : (0);"));
        assert!(javascript.contains("function greet(name) {"));
        assert!(javascript.contains("console.log(name);"));
        assert!(javascript.contains("for (let i = 0; i < 3; i++)"));
        let python = transpile_backend(source, "python").unwrap();
        assert!(python.contains("def double(n):"));
        assert!(python.contains("return n * 2 if n != 0 else 0"));
        assert!(python.contains("def greet(name):"));
        assert!(python.contains("print(name)"));
        assert!(python.contains("for i in range(0, 3):"));
    }

    #[test]
    fn lowers_collections_for_every_backend() {
        let source = r#"fun fill(tags: set<text>) {
    tags.add("from-function")
}
fun main() {
    list<text> names = ["Alex"]
    set<text> tags = set()
    map<text, list<int>> scores = map()
    names.add("Steve")
    tags.add("builder")
    scores.put("Alex", [1, 2])
    fill(tags)
    println(len(names))
}"#;
        let cpp = transpile_backend(source, "cpp").unwrap();
        assert!(cpp.contains("std::unordered_map<std::string, std::vector<int>> scores"));
        assert!(cpp.contains("names.push_back(\"Steve\")"));
        assert!(cpp.contains("tags.insert(\"builder\")"));
        assert!(cpp.contains("tags.insert(\"from-function\")"));

        let rust = transpile_backend(source, "rust").unwrap();
        assert!(rust.contains("mut tags: HashSet<String>"));
        assert!(rust.contains("names.push((\"Steve\".to_string()).into())"));
        assert!(rust.contains("tags.insert((\"builder\".to_string()).into())"));
        assert!(rust.contains("HashMap<String, Vec<i32>>"));

        let javascript = transpile_backend(source, "javascript").unwrap();
        assert!(javascript.contains("names.push(\"Steve\")"));
        assert!(javascript.contains("tags.add(\"builder\")"));
        assert!(javascript.contains("console.log(names.length)"));

        let python = transpile_backend(source, "python").unwrap();
        assert!(python.contains("names.append(\"Steve\")"));
        assert!(python.contains("tags.add(\"builder\")"));
    }

    #[test]
    fn emits_portable_example_sources() {
        let examples = [
            ("hello", include_str!("../../examples/hello.fun")),
            ("fibonacci", include_str!("../../examples/fibonacci.fun")),
            ("types-and-loops", include_str!("../../examples/types-and-loops.fun")),
        ];
        let root = std::env::temp_dir().join(format!("funo-native-tests-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();

        for (name, source) in examples {
            let rust = transpile_backend(source, "rust").unwrap();
            let rust_source = root.join(format!("{name}.rs"));
            let rust_artifact = root.join(format!("{name}-rust"));
            fs::write(&rust_source, rust).unwrap();
            let rust_output = std::process::Command::new("rustc")
                .arg("--edition=2021")
                .arg(&rust_source)
                .arg("-o")
                .arg(&rust_artifact)
                .output()
                .unwrap();
            assert!(rust_output.status.success(), "generated Rust for {name} failed:\n{}", String::from_utf8_lossy(&rust_output.stderr));

            let javascript = transpile_backend(source, "javascript").unwrap();
            let javascript_source = root.join(format!("{name}.js"));
            fs::write(&javascript_source, javascript).unwrap();
            let node_output = std::process::Command::new("node").arg("--check").arg(&javascript_source).output().unwrap();
            assert!(node_output.status.success(), "generated JavaScript for {name} failed:\n{}", String::from_utf8_lossy(&node_output.stderr));

            let python = transpile_backend(source, "python").unwrap();
            let python_source = root.join(format!("{name}.py"));
            fs::write(&python_source, python).unwrap();
            let python_command = if cfg!(windows) { "python" } else { "python3" };
            let python_output = std::process::Command::new(python_command)
                .arg("-m")
                .arg("py_compile")
                .arg(&python_source)
                .output()
                .unwrap();
            assert!(python_output.status.success(), "generated Python for {name} failed:\n{}", String::from_utf8_lossy(&python_output.stderr));
        }

        if let Some(compiler) = ["c++", "g++", "clang++"]
            .into_iter()
            .find(|candidate| std::process::Command::new(candidate).arg("--version").output().is_ok_and(|output| output.status.success()))
        {
            for (name, source) in examples {
                let cpp = transpile_backend(source, "cpp").unwrap();
                let cpp_source = root.join(format!("{name}.cpp"));
                let cpp_artifact = root.join(format!("{name}-cpp"));
                fs::write(&cpp_source, cpp).unwrap();
                let output = std::process::Command::new(compiler)
                    .arg("-std=c++17")
                    .arg(&cpp_source)
                    .arg("-o")
                    .arg(&cpp_artifact)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "generated C++ for {name} failed:\n{}", String::from_utf8_lossy(&output.stderr));
            }
        }

        let _ = fs::remove_dir_all(root);
    }
}

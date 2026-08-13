//! Process-free Funo interpreter used by the Android application.
//!
//! Android's ART runtime is not a desktop JDK: it cannot invoke `javac` or run
//! the Java source emitted by the regular compiler.  This module executes the
//! ordinary, documented Funo language directly and never starts a subprocess.
//! Minecraft projects are deliberately rejected because their event handlers
//! require a loader, Gradle and the Minecraft JVM runtime.

use crate::{compiler, models::BuildResult};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::mem::discriminant;
use std::time::Instant;

const MAX_STEPS: u64 = 250_000;
const MAX_CALL_DEPTH: usize = 128;
const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_COLLECTION_ITEMS: usize = 10_000;

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Null,
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

impl Value {
    fn display(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) if value.fract() == 0.0 => format!("{value:.1}"),
            Self::Float(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::List(values) => format!(
                "[{}]",
                values.iter().map(Value::display).collect::<Vec<_>>().join(", ")
            ),
            Self::Map(values) => format!(
                "{{{}}}",
                values
                    .iter()
                    .map(|(key, value)| format!("{key}={}", value.display()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    fn as_bool(&self) -> Result<bool, String> {
        match self {
            Self::Bool(value) => Ok(*value),
            _ => Err(format!(
                "условие должно иметь тип bool, получено «{}»",
                self.display()
            )),
        }
    }

    fn as_i64(&self, purpose: &str) -> Result<i64, String> {
        match self {
            Self::Int(value) => Ok(*value),
            _ => Err(format!("{purpose}: ожидалось целое число, получено «{}»", self.display())),
        }
    }
}

#[derive(Clone, Debug)]
struct Binding {
    value: Value,
    mutable: bool,
}

#[derive(Default)]
struct Environment {
    scopes: Vec<HashMap<String, Binding>>,
}

impl Environment {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str, value: Value, mutable: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), Binding { value, mutable });
        }
    }

    fn get(&self, name: &str, globals: &HashMap<String, Binding>) -> Option<Value> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).map(|binding| binding.value.clone()))
            .or_else(|| globals.get(name).map(|binding| binding.value.clone()))
    }

    fn assign(
        &mut self,
        name: &str,
        value: Value,
        globals: &mut HashMap<String, Binding>,
    ) -> Result<(), String> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.get_mut(name) {
                if !binding.mutable {
                    return Err(format!("значение «{name}» объявлено через let/const и неизменяемо"));
                }
                binding.value = value;
                return Ok(());
            }
        }
        if let Some(binding) = globals.get_mut(name) {
            if !binding.mutable {
                return Err(format!("значение «{name}» объявлено через let/const и неизменяемо"));
            }
            binding.value = value;
            return Ok(());
        }
        Err(format!("переменная «{name}» не объявлена"))
    }
}

#[derive(Clone, Debug)]
enum Expr {
    Literal(Value),
    Variable(String),
    Array(Vec<Expr>),
    Unary(Token, Box<Expr>),
    Binary(Box<Expr>, Token, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Method(Box<Expr>, String, Vec<Expr>),
    FString(String),
}

#[derive(Clone, Debug)]
struct Statement {
    kind: StatementKind,
    line: usize,
}

#[derive(Clone, Debug)]
enum StatementKind {
    Declare {
        name: String,
        value: Expr,
        mutable: bool,
    },
    Assign {
        name: String,
        value: Expr,
    },
    Expression(Expr),
    If {
        condition: Expr,
        then_body: Vec<Statement>,
        else_body: Vec<Statement>,
    },
    While {
        condition: Expr,
        body: Vec<Statement>,
    },
    RangeFor {
        variable: String,
        start: Expr,
        end: Expr,
        inclusive: bool,
        body: Vec<Statement>,
    },
    EachFor {
        variable: String,
        values: Expr,
        body: Vec<Statement>,
    },
    Repeat {
        count: Expr,
        body: Vec<Statement>,
    },
    Return(Option<Expr>),
    Break,
    Continue,
}

#[derive(Clone, Debug)]
struct Function {
    params: Vec<String>,
    body: FunctionBody,
}

#[derive(Clone, Debug)]
enum FunctionBody {
    Expression(Expr),
    Block(Vec<Statement>),
}

#[derive(Clone, Debug, Default)]
struct Program {
    functions: HashMap<String, Function>,
    top_level: Vec<Statement>,
}

#[derive(Clone, Debug)]
struct SourceLine {
    text: String,
    line: usize,
}

impl Program {
    fn parse(source: &str) -> Result<Self, String> {
        let lines = logical_lines(source)?;
        let expression_fun = Regex::new(
            r"^(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*[^\s=]+)?\s*=\s*(.+)$",
        )
        .unwrap();
        let block_fun = Regex::new(
            r"^(?:public\s+)?fun\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*(?:->\s*[^\s{]+)?\s*\{$",
        )
        .unwrap();
        let mut program = Program::default();
        let mut index = 0usize;

        while index < lines.len() {
            let current = &lines[index];
            if current.text.starts_with("use ")
                || current.text.starts_with("lib ")
                || current.text.starts_with("package ")
            {
                index += 1;
                continue;
            }
            if let Some(captures) = expression_fun.captures(&current.text) {
                let name = captures[1].to_string();
                let params = parse_params(&captures[2], current.line)?;
                let expression = parse_expression(&captures[3], current.line)?;
                if program
                    .functions
                    .insert(
                        name.clone(),
                        Function {
                            params,
                            body: FunctionBody::Expression(expression),
                        },
                    )
                    .is_some()
                {
                    return Err(format!("функция «{name}» объявлена повторно (строка {})", current.line));
                }
                index += 1;
                continue;
            }
            if let Some(captures) = block_fun.captures(&current.text) {
                let name = captures[1].to_string();
                let params = parse_params(&captures[2], current.line)?;
                index += 1;
                let body = parse_block(&lines, &mut index)?;
                if program
                    .functions
                    .insert(
                        name.clone(),
                        Function {
                            params,
                            body: FunctionBody::Block(body),
                        },
                    )
                    .is_some()
                {
                    return Err(format!("функция «{name}» объявлена повторно (строка {})", current.line));
                }
                continue;
            }
            if current.text == "}" {
                return Err(format!("лишняя закрывающая скобка (строка {})", current.line));
            }
            program.top_level.push(parse_statement(&lines, &mut index)?);
        }

        if !program.functions.contains_key("main") {
            return Err("обычной программе нужна функция main()".into());
        }
        Ok(program)
    }
}

fn parse_params(source: &str, line: usize) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    for parameter in split_top_level(source, ',')? {
        let name = parameter.split(':').next().unwrap_or("").trim();
        if name.is_empty()
            || !name
                .chars()
                .enumerate()
                .all(|(index, ch)| ch == '_' || ch.is_ascii_alphanumeric() && (index > 0 || !ch.is_ascii_digit()))
        {
            return Err(format!("неверный параметр «{parameter}» (строка {line})"));
        }
        result.push(name.to_string());
    }
    Ok(result)
}

fn parse_block(lines: &[SourceLine], index: &mut usize) -> Result<Vec<Statement>, String> {
    let mut body = Vec::new();
    while *index < lines.len() {
        if lines[*index].text == "}" {
            *index += 1;
            return Ok(body);
        }
        body.push(parse_statement(lines, index)?);
    }
    let line = lines.last().map(|value| value.line).unwrap_or(1);
    Err(format!("не хватает закрывающей скобки }} после строки {line}"))
}

fn parse_statement(lines: &[SourceLine], index: &mut usize) -> Result<Statement, String> {
    let current = lines
        .get(*index)
        .ok_or_else(|| "неожиданный конец исходника".to_string())?
        .clone();
    let text = current.text.as_str();

    if text.starts_with("if ") && text.ends_with('{') {
        let condition = text[3..text.len() - 1].trim();
        let condition = parse_expression(condition, current.line)?;
        *index += 1;
        let then_body = parse_block(lines, index)?;
        let mut else_body = Vec::new();
        if let Some(next) = lines.get(*index) {
            if next.text == "else {" {
                *index += 1;
                else_body = parse_block(lines, index)?;
            } else if next.text.starts_with("else if ") && next.text.ends_with('{') {
                let nested_text = next.text.trim_start_matches("else ").to_string();
                let mut nested_lines = lines.to_vec();
                nested_lines[*index].text = nested_text;
                else_body.push(parse_statement(&nested_lines, index)?);
            }
        }
        return Ok(Statement {
            kind: StatementKind::If {
                condition,
                then_body,
                else_body,
            },
            line: current.line,
        });
    }

    if text.starts_with("while ") && text.ends_with('{') {
        let condition = parse_expression(text[6..text.len() - 1].trim(), current.line)?;
        *index += 1;
        let body = parse_block(lines, index)?;
        return Ok(Statement {
            kind: StatementKind::While { condition, body },
            line: current.line,
        });
    }

    if text.starts_with("for ") && text.ends_with('{') {
        let header = text[4..text.len() - 1].trim();
        let (variable, source) = header
            .split_once(" in ")
            .ok_or_else(|| format!("ожидалось «for имя in значение» (строка {})", current.line))?;
        let variable = variable.trim();
        validate_name(variable, current.line)?;
        *index += 1;
        let body = parse_block(lines, index)?;
        if let Some((start, inclusive, end)) = split_range(source) {
            return Ok(Statement {
                kind: StatementKind::RangeFor {
                    variable: variable.to_string(),
                    start: parse_expression(start, current.line)?,
                    end: parse_expression(end, current.line)?,
                    inclusive,
                    body,
                },
                line: current.line,
            });
        }
        return Ok(Statement {
            kind: StatementKind::EachFor {
                variable: variable.to_string(),
                values: parse_expression(source, current.line)?,
                body,
            },
            line: current.line,
        });
    }

    if text.starts_with("repeat ") && text.ends_with('{') {
        let count = parse_expression(text[7..text.len() - 1].trim(), current.line)?;
        *index += 1;
        let body = parse_block(lines, index)?;
        return Ok(Statement {
            kind: StatementKind::Repeat { count, body },
            line: current.line,
        });
    }

    if text == "else {" || text.starts_with("else if ") {
        return Err(format!("else без соответствующего if (строка {})", current.line));
    }

    let kind = if text == "break" {
        StatementKind::Break
    } else if text == "continue" {
        StatementKind::Continue
    } else if text == "return" {
        StatementKind::Return(None)
    } else if let Some(value) = text.strip_prefix("return(").and_then(|value| value.strip_suffix(')')) {
        StatementKind::Return(Some(parse_expression(value, current.line)?))
    } else if let Some(value) = text.strip_prefix("return ") {
        StatementKind::Return(Some(parse_expression(value, current.line)?))
    } else if let Some((name, value, mutable)) = parse_declaration(text) {
        validate_name(&name, current.line)?;
        StatementKind::Declare {
            name,
            value: parse_expression(&value, current.line)?,
            mutable,
        }
    } else if let Some(captures) = Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$")
        .unwrap()
        .captures(text)
    {
        StatementKind::Assign {
            name: captures[1].to_string(),
            value: parse_expression(&captures[2], current.line)?,
        }
    } else if text == "}" {
        return Err(format!("лишняя закрывающая скобка (строка {})", current.line));
    } else if text.ends_with('{') {
        return Err(format!("неизвестный блок «{text}» (строка {})", current.line));
    } else {
        StatementKind::Expression(parse_expression(text, current.line)?)
    };
    *index += 1;
    Ok(Statement {
        kind,
        line: current.line,
    })
}

fn validate_name(name: &str, line: usize) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.chars().enumerate().all(|(index, ch)| {
            ch == '_' || ch.is_ascii_alphabetic() || index > 0 && ch.is_ascii_digit()
        });
    if valid {
        Ok(())
    } else {
        Err(format!("неверное имя «{name}» (строка {line})"))
    }
}

fn parse_declaration(text: &str) -> Option<(String, String, bool)> {
    let named = Regex::new(
        r"^(let|var|const)\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*:\s*[^=\s]+)?\s*=\s*(.+)$",
    )
    .unwrap();
    if let Some(captures) = named.captures(text) {
        return Some((
            captures[2].to_string(),
            captures[3].to_string(),
            &captures[1] == "var",
        ));
    }
    let typed = Regex::new(
        r"^[A-Za-z_][A-Za-z0-9_]*(?:<[^>]+>)?(?:\[\])?\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$",
    )
    .unwrap();
    typed.captures(text).map(|captures| {
        (
            captures[1].to_string(),
            captures[2].to_string(),
            true,
        )
    })
}

fn split_range(source: &str) -> Option<(&str, bool, &str)> {
    let chars = source.as_bytes();
    let mut quote = None;
    let mut depth = 0isize;
    let mut index = 0usize;
    while index + 1 < chars.len() {
        let ch = chars[index] as char;
        if let Some(expected) = quote {
            if ch == '\\' {
                index += 2;
                continue;
            }
            if ch == expected {
                quote = None;
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch == '(' || ch == '[' {
            depth += 1;
        } else if ch == ')' || ch == ']' {
            depth -= 1;
        } else if depth == 0 && ch == '.' && chars[index + 1] == b'.' {
            let inclusive = chars.get(index + 2) == Some(&b'=');
            let offset = if inclusive { 3 } else { 2 };
            return Some((source[..index].trim(), inclusive, source[index + offset..].trim()));
        }
        index += 1;
    }
    None
}

fn logical_lines(source: &str) -> Result<Vec<SourceLine>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut result = Vec::new();
    let mut current = String::new();
    let mut line = 1usize;
    let mut start_line = 1usize;
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    let mut index = 0usize;

    let flush = |current: &mut String, result: &mut Vec<SourceLine>, start_line: usize| {
        let text = current.trim().trim_end_matches(';').trim().to_string();
        current.clear();
        if !text.is_empty() {
            result.push(SourceLine {
                text,
                line: start_line,
            });
        }
    };

    while index < chars.len() {
        let ch = chars[index];
        if comment {
            if ch == '\n' {
                flush(&mut current, &mut result, start_line);
                comment = false;
                line += 1;
                start_line = line;
            }
            index += 1;
            continue;
        }
        if let Some(expected) = quote {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == expected {
                quote = None;
            }
            if ch == '\n' {
                line += 1;
            }
            index += 1;
            continue;
        }
        if ch == '/' && chars.get(index + 1) == Some(&'/') {
            comment = true;
            index += 2;
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                current.push(ch);
            }
            '{' => {
                current.push(ch);
                flush(&mut current, &mut result, start_line);
                start_line = line;
            }
            '}' => {
                flush(&mut current, &mut result, start_line);
                result.push(SourceLine {
                    text: "}".into(),
                    line,
                });
                start_line = line;
            }
            '\n' => {
                flush(&mut current, &mut result, start_line);
                line += 1;
                start_line = line;
            }
            ';' => {
                flush(&mut current, &mut result, start_line);
                start_line = line;
            }
            _ => current.push(ch),
        }
        index += 1;
    }
    if quote.is_some() {
        return Err(format!("строковый литерал не закрыт (строка {start_line})"));
    }
    flush(&mut current, &mut result, start_line);
    Ok(result)
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Int(i64),
    Float(f64),
    Text(String),
    FString(String),
    Identifier(String),
    True,
    False,
    Null,
    If,
    Then,
    Else,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Not,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Eof,
}

struct Lexer {
    chars: Vec<char>,
    index: usize,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
        }
    }

    fn scan(mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        while self.index < self.chars.len() {
            let ch = self.chars[self.index];
            if ch.is_whitespace() {
                self.index += 1;
                continue;
            }
            if ch == 'f' && self.chars.get(self.index + 1) == Some(&'"') {
                self.index += 1;
                tokens.push(Token::FString(self.string('"')?));
                continue;
            }
            if ch == '"' || ch == '\'' {
                tokens.push(Token::Text(self.string(ch)?));
                continue;
            }
            if ch.is_ascii_digit() {
                tokens.push(self.number()?);
                continue;
            }
            if ch.is_ascii_alphabetic() || ch == '_' {
                let start = self.index;
                self.index += 1;
                while self
                    .chars
                    .get(self.index)
                    .is_some_and(|value| value.is_ascii_alphanumeric() || *value == '_')
                {
                    self.index += 1;
                }
                let word: String = self.chars[start..self.index].iter().collect();
                tokens.push(match word.as_str() {
                    "true" => Token::True,
                    "false" => Token::False,
                    "null" => Token::Null,
                    "if" => Token::If,
                    "then" => Token::Then,
                    "else" => Token::Else,
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    _ => Token::Identifier(word),
                });
                continue;
            }
            let next = self.chars.get(self.index + 1).copied();
            let (token, advance) = match (ch, next) {
                ('=', Some('=')) => (Token::EqualEqual, 2),
                ('!', Some('=')) => (Token::NotEqual, 2),
                ('<', Some('=')) => (Token::LessEqual, 2),
                ('>', Some('=')) => (Token::GreaterEqual, 2),
                ('&', Some('&')) => (Token::And, 2),
                ('|', Some('|')) => (Token::Or, 2),
                ('+', _) => (Token::Plus, 1),
                ('-', _) => (Token::Minus, 1),
                ('*', _) => (Token::Star, 1),
                ('/', _) => (Token::Slash, 1),
                ('%', _) => (Token::Percent, 1),
                ('!', _) => (Token::Not, 1),
                ('<', _) => (Token::Less, 1),
                ('>', _) => (Token::Greater, 1),
                ('(', _) => (Token::LeftParen, 1),
                (')', _) => (Token::RightParen, 1),
                ('[', _) => (Token::LeftBracket, 1),
                (']', _) => (Token::RightBracket, 1),
                (',', _) => (Token::Comma, 1),
                ('.', _) => (Token::Dot, 1),
                _ => return Err(format!("неожиданный символ «{ch}»")),
            };
            self.index += advance;
            tokens.push(token);
        }
        tokens.push(Token::Eof);
        Ok(tokens)
    }

    fn string(&mut self, quote: char) -> Result<String, String> {
        self.index += 1;
        let mut result = String::new();
        while self.index < self.chars.len() {
            let ch = self.chars[self.index];
            self.index += 1;
            if ch == quote {
                return Ok(result);
            }
            if ch == '\\' {
                let escaped = self
                    .chars
                    .get(self.index)
                    .copied()
                    .ok_or_else(|| "незаконченная escape-последовательность".to_string())?;
                self.index += 1;
                result.push(match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\\' => '\\',
                    '"' => '"',
                    '\'' => '\'',
                    other => other,
                });
            } else {
                result.push(ch);
            }
        }
        Err("строковый литерал не закрыт".into())
    }

    fn number(&mut self) -> Result<Token, String> {
        let start = self.index;
        while self
            .chars
            .get(self.index)
            .is_some_and(|value| value.is_ascii_digit())
        {
            self.index += 1;
        }
        let mut floating = false;
        if self.chars.get(self.index) == Some(&'.')
            && self
                .chars
                .get(self.index + 1)
                .is_some_and(|value| value.is_ascii_digit())
        {
            floating = true;
            self.index += 1;
            while self
                .chars
                .get(self.index)
                .is_some_and(|value| value.is_ascii_digit())
            {
                self.index += 1;
            }
        }
        let raw: String = self.chars[start..self.index].iter().collect();
        if self
            .chars
            .get(self.index)
            .is_some_and(|value| matches!(*value, 'f' | 'F' | 'd' | 'D'))
        {
            floating = true;
            self.index += 1;
        } else if self
            .chars
            .get(self.index)
            .is_some_and(|value| matches!(*value, 'l' | 'L'))
        {
            self.index += 1;
        }
        if floating {
            raw.parse::<f64>()
                .map(Token::Float)
                .map_err(|_| format!("неверное число «{raw}»"))
        } else {
            raw.parse::<i64>()
                .map(Token::Int)
                .map_err(|_| format!("слишком большое целое число «{raw}»"))
        }
    }
}

struct ExpressionParser {
    tokens: Vec<Token>,
    index: usize,
}

impl ExpressionParser {
    fn parse(source: &str) -> Result<Expr, String> {
        let mut parser = Self {
            tokens: Lexer::new(source).scan()?,
            index: 0,
        };
        let expression = parser.conditional()?;
        if !parser.check(&Token::Eof) {
            return Err(format!("лишняя часть выражения рядом с {:?}", parser.peek()));
        }
        Ok(expression)
    }

    fn conditional(&mut self) -> Result<Expr, String> {
        if self.consume(&Token::If) {
            let condition = self.or()?;
            self.expect(&Token::Then, "после условия нужно слово then")?;
            let yes = self.or()?;
            self.expect(&Token::Else, "после первого результата нужно слово else")?;
            let no = self.conditional()?;
            Ok(Expr::Conditional(
                Box::new(condition),
                Box::new(yes),
                Box::new(no),
            ))
        } else {
            self.or()
        }
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut expr = self.and()?;
        while self.consume(&Token::Or) {
            expr = Expr::Binary(Box::new(expr), Token::Or, Box::new(self.and()?));
        }
        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut expr = self.equality()?;
        while self.consume(&Token::And) {
            expr = Expr::Binary(Box::new(expr), Token::And, Box::new(self.equality()?));
        }
        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.comparison()?;
        loop {
            let operator = if self.consume(&Token::EqualEqual) {
                Some(Token::EqualEqual)
            } else if self.consume(&Token::NotEqual) {
                Some(Token::NotEqual)
            } else {
                None
            };
            let Some(operator) = operator else { break };
            expr = Expr::Binary(Box::new(expr), operator, Box::new(self.comparison()?));
        }
        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.term()?;
        loop {
            let operator = [
                Token::LessEqual,
                Token::GreaterEqual,
                Token::Less,
                Token::Greater,
            ]
            .into_iter()
            .find(|candidate| self.consume(candidate));
            let Some(operator) = operator else { break };
            expr = Expr::Binary(Box::new(expr), operator, Box::new(self.term()?));
        }
        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut expr = self.factor()?;
        loop {
            let operator = if self.consume(&Token::Plus) {
                Some(Token::Plus)
            } else if self.consume(&Token::Minus) {
                Some(Token::Minus)
            } else {
                None
            };
            let Some(operator) = operator else { break };
            expr = Expr::Binary(Box::new(expr), operator, Box::new(self.factor()?));
        }
        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.unary()?;
        loop {
            let operator = if self.consume(&Token::Star) {
                Some(Token::Star)
            } else if self.consume(&Token::Slash) {
                Some(Token::Slash)
            } else if self.consume(&Token::Percent) {
                Some(Token::Percent)
            } else {
                None
            };
            let Some(operator) = operator else { break };
            expr = Expr::Binary(Box::new(expr), operator, Box::new(self.unary()?));
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.consume(&Token::Not) {
            return Ok(Expr::Unary(Token::Not, Box::new(self.unary()?)));
        }
        if self.consume(&Token::Minus) {
            return Ok(Expr::Unary(Token::Minus, Box::new(self.unary()?)));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;
        loop {
            if self.consume(&Token::LeftParen) {
                let arguments = self.arguments()?;
                if let Expr::Variable(name) = expr {
                    expr = Expr::Call(name, arguments);
                } else {
                    return Err("вызывать можно только функцию по имени".into());
                }
            } else if self.consume(&Token::LeftBracket) {
                let index = self.conditional()?;
                self.expect(&Token::RightBracket, "не хватает ] после индекса")?;
                expr = Expr::Index(Box::new(expr), Box::new(index));
            } else if self.consume(&Token::Dot) {
                let name = match self.advance() {
                    Token::Identifier(name) => name,
                    _ => return Err("после точки нужно имя метода".into()),
                };
                self.expect(&Token::LeftParen, "после имени метода нужна (")?;
                let arguments = self.arguments()?;
                expr = Expr::Method(Box::new(expr), name, arguments);
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn arguments(&mut self) -> Result<Vec<Expr>, String> {
        let mut values = Vec::new();
        if self.consume(&Token::RightParen) {
            return Ok(values);
        }
        loop {
            values.push(self.conditional()?);
            if self.consume(&Token::RightParen) {
                return Ok(values);
            }
            self.expect(&Token::Comma, "между аргументами нужна запятая")?;
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let token = self.advance();
        match token {
            Token::Int(value) => Ok(Expr::Literal(Value::Int(value))),
            Token::Float(value) => Ok(Expr::Literal(Value::Float(value))),
            Token::Text(value) => Ok(Expr::Literal(Value::Text(value))),
            Token::FString(value) => Ok(Expr::FString(value)),
            Token::True => Ok(Expr::Literal(Value::Bool(true))),
            Token::False => Ok(Expr::Literal(Value::Bool(false))),
            Token::Null => Ok(Expr::Literal(Value::Null)),
            Token::Identifier(value) => Ok(Expr::Variable(value)),
            Token::LeftParen => {
                let value = self.conditional()?;
                self.expect(&Token::RightParen, "не хватает )")?;
                Ok(value)
            }
            Token::LeftBracket => {
                let mut values = Vec::new();
                if self.consume(&Token::RightBracket) {
                    return Ok(Expr::Array(values));
                }
                loop {
                    values.push(self.conditional()?);
                    if self.consume(&Token::RightBracket) {
                        break;
                    }
                    self.expect(&Token::Comma, "между элементами массива нужна запятая")?;
                }
                Ok(Expr::Array(values))
            }
            other => Err(format!("ожидалось выражение, получено {other:?}")),
        }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.index).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        if !matches!(token, Token::Eof) {
            self.index += 1;
        }
        token
    }

    fn check(&self, expected: &Token) -> bool {
        discriminant(self.peek()) == discriminant(expected)
    }

    fn consume(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: &Token, message: &str) -> Result<(), String> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(message.into())
        }
    }
}

fn parse_expression(source: &str, line: usize) -> Result<Expr, String> {
    ExpressionParser::parse(source).map_err(|error| format!("{error} (строка {line})"))
}

fn split_top_level(source: &str, separator: char) -> Result<Vec<String>, String> {
    if source.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0isize;
    let mut quote = None;
    let mut escaped = false;
    for ch in source.chars() {
        if let Some(expected) = quote {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == expected {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            current.push(ch);
        } else if ch == '(' || ch == '[' || ch == '<' {
            depth += 1;
            current.push(ch);
        } else if ch == ')' || ch == ']' || ch == '>' {
            depth -= 1;
            current.push(ch);
        } else if ch == separator && depth == 0 {
            result.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if depth != 0 || quote.is_some() {
        return Err("несбалансированные скобки или кавычки".into());
    }
    result.push(current.trim().to_string());
    Ok(result)
}

#[derive(Debug)]
enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
}

struct Runtime {
    program: Program,
    globals: HashMap<String, Binding>,
    output: String,
    steps: u64,
    call_depth: usize,
}

impl Runtime {
    fn new(program: Program) -> Self {
        Self {
            program,
            globals: HashMap::new(),
            output: String::new(),
            steps: 0,
            call_depth: 0,
        }
    }

    fn run(&mut self) -> Result<Value, String> {
        let mut global_env = Environment::new();
        let top_level = self.program.top_level.clone();
        match self.execute_block(&top_level, &mut global_env, false)? {
            Flow::Normal => {}
            _ => return Err("return/break/continue нельзя использовать вне функции или цикла".into()),
        }
        self.globals = global_env.scopes.pop().unwrap_or_default();
        self.call("main", Vec::new())
    }

    fn tick(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            Err(format!(
                "программа остановлена: превышен безопасный лимит {MAX_STEPS} операций"
            ))
        } else {
            Ok(())
        }
    }

    fn execute_block(
        &mut self,
        statements: &[Statement],
        env: &mut Environment,
        scoped: bool,
    ) -> Result<Flow, String> {
        if scoped {
            env.push();
        }
        let result = (|| {
            for statement in statements {
                let flow = self
                    .execute_statement(statement, env)
                    .map_err(|error| format!("{error} (строка {})", statement.line))?;
                if !matches!(flow, Flow::Normal) {
                    return Ok(flow);
                }
            }
            Ok(Flow::Normal)
        })();
        if scoped {
            env.pop();
        }
        result
    }

    fn execute_statement(
        &mut self,
        statement: &Statement,
        env: &mut Environment,
    ) -> Result<Flow, String> {
        self.tick()?;
        match &statement.kind {
            StatementKind::Declare {
                name,
                value,
                mutable,
            } => {
                let value = self.evaluate(value, env)?;
                env.declare(name, value, *mutable);
                Ok(Flow::Normal)
            }
            StatementKind::Assign { name, value } => {
                let value = self.evaluate(value, env)?;
                env.assign(name, value, &mut self.globals)?;
                Ok(Flow::Normal)
            }
            StatementKind::Expression(expression) => {
                self.evaluate(expression, env)?;
                Ok(Flow::Normal)
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                if self.evaluate(condition, env)?.as_bool()? {
                    self.execute_block(then_body, env, true)
                } else {
                    self.execute_block(else_body, env, true)
                }
            }
            StatementKind::While { condition, body } => {
                loop {
                    self.tick()?;
                    if !self.evaluate(condition, env)?.as_bool()? {
                        break;
                    }
                    match self.execute_block(body, env, true)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        value @ Flow::Return(_) => return Ok(value),
                    }
                }
                Ok(Flow::Normal)
            }
            StatementKind::RangeFor {
                variable,
                start,
                end,
                inclusive,
                body,
            } => {
                let mut current = self.evaluate(start, env)?.as_i64("начало диапазона")?;
                let end = self.evaluate(end, env)?.as_i64("конец диапазона")?;
                while if *inclusive { current <= end } else { current < end } {
                    self.tick()?;
                    env.push();
                    env.declare(variable, Value::Int(current), true);
                    let flow = self.execute_block(body, env, false);
                    env.pop();
                    match flow? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        value @ Flow::Return(_) => return Ok(value),
                    }
                    current = current
                        .checked_add(1)
                        .ok_or_else(|| "переполнение счётчика цикла".to_string())?;
                }
                Ok(Flow::Normal)
            }
            StatementKind::EachFor {
                variable,
                values,
                body,
            } => {
                let values = match self.evaluate(values, env)? {
                    Value::List(values) => values,
                    Value::Text(value) => value
                        .chars()
                        .map(|character| Value::Text(character.to_string()))
                        .collect(),
                    Value::Map(values) => values.keys().cloned().map(Value::Text).collect(),
                    other => return Err(format!("for .. in не поддерживает «{}»", other.display())),
                };
                for value in values {
                    self.tick()?;
                    env.push();
                    env.declare(variable, value, true);
                    let flow = self.execute_block(body, env, false);
                    env.pop();
                    match flow? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        value @ Flow::Return(_) => return Ok(value),
                    }
                }
                Ok(Flow::Normal)
            }
            StatementKind::Repeat { count, body } => {
                let count = self.evaluate(count, env)?.as_i64("repeat")?;
                if count < 0 {
                    return Err("repeat не принимает отрицательное число".into());
                }
                for _ in 0..count {
                    self.tick()?;
                    match self.execute_block(body, env, true)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        value @ Flow::Return(_) => return Ok(value),
                    }
                }
                Ok(Flow::Normal)
            }
            StatementKind::Return(value) => Ok(Flow::Return(match value {
                Some(value) => self.evaluate(value, env)?,
                None => Value::Null,
            })),
            StatementKind::Break => Ok(Flow::Break),
            StatementKind::Continue => Ok(Flow::Continue),
        }
    }

    fn evaluate(&mut self, expression: &Expr, env: &mut Environment) -> Result<Value, String> {
        self.tick()?;
        match expression {
            Expr::Literal(value) => Ok(value.clone()),
            Expr::Variable(name) => env
                .get(name, &self.globals)
                .ok_or_else(|| format!("неизвестная переменная «{name}»")),
            Expr::Array(expressions) => {
                if expressions.len() > MAX_COLLECTION_ITEMS {
                    return Err("слишком много элементов в коллекции".into());
                }
                expressions
                    .iter()
                    .map(|value| self.evaluate(value, env))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::List)
            }
            Expr::Unary(operator, value) => {
                let value = self.evaluate(value, env)?;
                match operator {
                    Token::Not => Ok(Value::Bool(!value.as_bool()?)),
                    Token::Minus => match value {
                        Value::Int(value) => value
                            .checked_neg()
                            .map(Value::Int)
                            .ok_or_else(|| "переполнение целого числа".into()),
                        Value::Float(value) => Ok(Value::Float(-value)),
                        other => Err(format!("оператор - не поддерживает «{}»", other.display())),
                    },
                    _ => Err("неизвестный унарный оператор".into()),
                }
            }
            Expr::Binary(left, Token::And, right) => {
                let left = self.evaluate(left, env)?.as_bool()?;
                if !left {
                    Ok(Value::Bool(false))
                } else {
                    Ok(Value::Bool(self.evaluate(right, env)?.as_bool()?))
                }
            }
            Expr::Binary(left, Token::Or, right) => {
                let left = self.evaluate(left, env)?.as_bool()?;
                if left {
                    Ok(Value::Bool(true))
                } else {
                    Ok(Value::Bool(self.evaluate(right, env)?.as_bool()?))
                }
            }
            Expr::Binary(left, operator, right) => {
                let left = self.evaluate(left, env)?;
                let right = self.evaluate(right, env)?;
                binary(left, operator, right)
            }
            Expr::Conditional(condition, yes, no) => {
                if self.evaluate(condition, env)?.as_bool()? {
                    self.evaluate(yes, env)
                } else {
                    self.evaluate(no, env)
                }
            }
            Expr::Call(name, arguments) => {
                let values = arguments
                    .iter()
                    .map(|value| self.evaluate(value, env))
                    .collect::<Result<Vec<_>, _>>()?;
                self.call(name, values)
            }
            Expr::Index(target, index) => {
                let target = self.evaluate(target, env)?;
                let index = self.evaluate(index, env)?;
                index_value(target, index)
            }
            Expr::Method(target, method, arguments) => {
                let values = arguments
                    .iter()
                    .map(|value| self.evaluate(value, env))
                    .collect::<Result<Vec<_>, _>>()?;
                self.call_method(target, method, values, env)
            }
            Expr::FString(content) => self.render_fstring(content, env).map(Value::Text),
        }
    }

    fn call(&mut self, name: &str, arguments: Vec<Value>) -> Result<Value, String> {
        match name {
            "println" => {
                let text = arguments.iter().map(Value::display).collect::<Vec<_>>().join(" ");
                self.write_output(&format!("{text}\n"))?;
                return Ok(Value::Null);
            }
            "print" => {
                let text = arguments.iter().map(Value::display).collect::<Vec<_>>().join(" ");
                self.write_output(&text)?;
                return Ok(Value::Null);
            }
            "len" => {
                expect_count(name, &arguments, 1)?;
                return Ok(Value::Int(match &arguments[0] {
                    Value::Text(value) => value.chars().count() as i64,
                    Value::List(value) => value.len() as i64,
                    Value::Map(value) => value.len() as i64,
                    other => return Err(format!("len не поддерживает «{}»", other.display())),
                }));
            }
            "list" => return Ok(Value::List(arguments)),
            "set" => {
                let mut unique = Vec::new();
                for value in arguments {
                    if !unique.contains(&value) {
                        unique.push(value);
                    }
                }
                return Ok(Value::List(unique));
            }
            "map" => {
                expect_count(name, &arguments, 0)?;
                return Ok(Value::Map(BTreeMap::new()));
            }
            "toInt" => {
                expect_count(name, &arguments, 1)?;
                return match &arguments[0] {
                    Value::Int(value) => Ok(Value::Int(*value)),
                    Value::Float(value) => Ok(Value::Int(*value as i64)),
                    value => value
                        .display()
                        .trim()
                        .parse::<i64>()
                        .map(Value::Int)
                        .map_err(|_| format!("«{}» нельзя преобразовать в int", value.display())),
                };
            }
            "toDouble" => {
                expect_count(name, &arguments, 1)?;
                return match &arguments[0] {
                    Value::Int(value) => Ok(Value::Float(*value as f64)),
                    Value::Float(value) => Ok(Value::Float(*value)),
                    value => value
                        .display()
                        .trim()
                        .parse::<f64>()
                        .map(Value::Float)
                        .map_err(|_| format!("«{}» нельзя преобразовать в double", value.display())),
                };
            }
            "text" | "str" => {
                expect_count(name, &arguments, 1)?;
                return Ok(Value::Text(arguments[0].display()));
            }
            "readln" | "readInt" | "readLong" | "readDouble" | "readBool" => {
                return Err(format!(
                    "{name} пока недоступна в мобильном запуске: интерактивный ввод не подключён"
                ));
            }
            _ => {}
        }

        let function = self
            .program
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| format!("неизвестная функция «{name}»"))?;
        if arguments.len() != function.params.len() {
            return Err(format!(
                "функция {name} ожидает {} аргумент(а), получено {}",
                function.params.len(),
                arguments.len()
            ));
        }
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(format!(
                "программа остановлена: глубина вызовов превысила {MAX_CALL_DEPTH}"
            ));
        }
        self.call_depth += 1;
        let result = (|| {
            let mut env = Environment::new();
            for (parameter, value) in function.params.iter().zip(arguments) {
                env.declare(parameter, value, true);
            }
            match &function.body {
                FunctionBody::Expression(expression) => self.evaluate(expression, &mut env),
                FunctionBody::Block(body) => match self.execute_block(body, &mut env, false)? {
                    Flow::Return(value) => Ok(value),
                    Flow::Normal => Ok(Value::Null),
                    Flow::Break | Flow::Continue => {
                        Err("break/continue можно использовать только внутри цикла".into())
                    }
                },
            }
        })();
        self.call_depth -= 1;
        result
    }

    fn call_method(
        &mut self,
        target_expression: &Expr,
        method: &str,
        arguments: Vec<Value>,
        env: &mut Environment,
    ) -> Result<Value, String> {
        let target = self.evaluate(target_expression, env)?;
        let mut replacement = None;
        let result = match (target, method) {
            (Value::List(mut values), "add") => {
                expect_count(method, &arguments, 1)?;
                if values.len() >= MAX_COLLECTION_ITEMS {
                    return Err("коллекция достигла безопасного лимита размера".into());
                }
                values.push(arguments[0].clone());
                replacement = Some(Value::List(values));
                Value::Bool(true)
            }
            (Value::List(values), "get") => {
                expect_count(method, &arguments, 1)?;
                index_value(Value::List(values), arguments[0].clone())?
            }
            (Value::List(values), "contains") => {
                expect_count(method, &arguments, 1)?;
                Value::Bool(values.contains(&arguments[0]))
            }
            (Value::List(values), "size" | "length") => {
                expect_count(method, &arguments, 0)?;
                Value::Int(values.len() as i64)
            }
            (Value::List(mut values), "clear") => {
                expect_count(method, &arguments, 0)?;
                values.clear();
                replacement = Some(Value::List(values));
                Value::Null
            }
            (Value::Map(mut values), "put") => {
                expect_count(method, &arguments, 2)?;
                if values.len() >= MAX_COLLECTION_ITEMS && !values.contains_key(&arguments[0].display()) {
                    return Err("коллекция достигла безопасного лимита размера".into());
                }
                let previous = values
                    .insert(arguments[0].display(), arguments[1].clone())
                    .unwrap_or(Value::Null);
                replacement = Some(Value::Map(values));
                previous
            }
            (Value::Map(values), "get") => {
                expect_count(method, &arguments, 1)?;
                values
                    .get(&arguments[0].display())
                    .cloned()
                    .unwrap_or(Value::Null)
            }
            (Value::Map(values), "containsKey" | "contains") => {
                expect_count(method, &arguments, 1)?;
                Value::Bool(values.contains_key(&arguments[0].display()))
            }
            (Value::Map(values), "size" | "length") => {
                expect_count(method, &arguments, 0)?;
                Value::Int(values.len() as i64)
            }
            (Value::Map(mut values), "clear") => {
                expect_count(method, &arguments, 0)?;
                values.clear();
                replacement = Some(Value::Map(values));
                Value::Null
            }
            (Value::Text(value), "length" | "size") => {
                expect_count(method, &arguments, 0)?;
                Value::Int(value.chars().count() as i64)
            }
            (Value::Text(value), "contains") => {
                expect_count(method, &arguments, 1)?;
                Value::Bool(value.contains(&arguments[0].display()))
            }
            (Value::Text(value), "toUpperCase") => {
                expect_count(method, &arguments, 0)?;
                Value::Text(value.to_uppercase())
            }
            (Value::Text(value), "toLowerCase") => {
                expect_count(method, &arguments, 0)?;
                Value::Text(value.to_lowercase())
            }
            (other, _) => {
                return Err(format!(
                    "метод {method} не поддерживается для «{}»",
                    other.display()
                ))
            }
        };
        if let Some(value) = replacement {
            if let Expr::Variable(name) = target_expression {
                env.assign(name, value, &mut self.globals)?;
            } else {
                return Err("изменяющий метод можно вызвать только у переменной".into());
            }
        }
        Ok(result)
    }

    fn render_fstring(&mut self, content: &str, env: &mut Environment) -> Result<String, String> {
        let chars: Vec<char> = content.chars().collect();
        let mut result = String::new();
        let mut index = 0usize;
        while index < chars.len() {
            if chars[index] == '{' && chars.get(index + 1) == Some(&'{') {
                result.push('{');
                index += 2;
                continue;
            }
            if chars[index] == '}' && chars.get(index + 1) == Some(&'}') {
                result.push('}');
                index += 2;
                continue;
            }
            if chars[index] != '{' {
                result.push(chars[index]);
                index += 1;
                continue;
            }
            let start = index + 1;
            index = start;
            let mut depth = 1isize;
            let mut quote = None;
            let mut escaped = false;
            while index < chars.len() && depth > 0 {
                let ch = chars[index];
                if let Some(expected) = quote {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == expected {
                        quote = None;
                    }
                } else if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                } else if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                }
                index += 1;
            }
            if depth != 0 {
                return Err("в f-строке не закрыта фигурная скобка".into());
            }
            let expression: String = chars[start..index - 1].iter().collect();
            if expression.trim().is_empty() {
                return Err("в f-строке пустое выражение".into());
            }
            let expression = ExpressionParser::parse(expression.trim())?;
            result.push_str(&self.evaluate(&expression, env)?.display());
        }
        Ok(result)
    }

    fn write_output(&mut self, value: &str) -> Result<(), String> {
        if self.output.len() + value.len() > MAX_OUTPUT_BYTES {
            return Err(format!(
                "программа остановлена: вывод превысил {} КиБ",
                MAX_OUTPUT_BYTES / 1024
            ));
        }
        self.output.push_str(value);
        Ok(())
    }
}

fn expect_count(name: &str, arguments: &[Value], expected: usize) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "{name} ожидает {expected} аргумент(а), получено {}",
            arguments.len()
        ))
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Int(left), Value::Float(right)) => *left as f64 == *right,
        (Value::Float(left), Value::Int(right)) => *left == *right as f64,
        _ => left == right,
    }
}

fn numeric_pair(left: &Value, right: &Value) -> Option<(f64, f64, bool)> {
    let left_number = match left {
        Value::Int(value) => *value as f64,
        Value::Float(value) => *value,
        _ => return None,
    };
    let right_number = match right {
        Value::Int(value) => *value as f64,
        Value::Float(value) => *value,
        _ => return None,
    };
    Some((
        left_number,
        right_number,
        matches!((left, right), (Value::Int(_), Value::Int(_))),
    ))
}

fn binary(left: Value, operator: &Token, right: Value) -> Result<Value, String> {
    match operator {
        Token::EqualEqual => return Ok(Value::Bool(values_equal(&left, &right))),
        Token::NotEqual => return Ok(Value::Bool(!values_equal(&left, &right))),
        Token::Plus if matches!(&left, Value::Text(_)) || matches!(&right, Value::Text(_)) => {
            return Ok(Value::Text(format!("{}{}", left.display(), right.display())))
        }
        _ => {}
    }
    if let Some((left_number, right_number, both_int)) = numeric_pair(&left, &right) {
        return match operator {
            Token::Plus if both_int => match (left, right) {
                (Value::Int(left), Value::Int(right)) => left
                    .checked_add(right)
                    .map(Value::Int)
                    .ok_or_else(|| "переполнение целого числа".into()),
                _ => unreachable!(),
            },
            Token::Minus if both_int => match (left, right) {
                (Value::Int(left), Value::Int(right)) => left
                    .checked_sub(right)
                    .map(Value::Int)
                    .ok_or_else(|| "переполнение целого числа".into()),
                _ => unreachable!(),
            },
            Token::Star if both_int => match (left, right) {
                (Value::Int(left), Value::Int(right)) => left
                    .checked_mul(right)
                    .map(Value::Int)
                    .ok_or_else(|| "переполнение целого числа".into()),
                _ => unreachable!(),
            },
            Token::Slash if right_number == 0.0 => Err("деление на ноль".into()),
            Token::Slash if both_int => match (left, right) {
                (Value::Int(left), Value::Int(right)) => left
                    .checked_div(right)
                    .map(Value::Int)
                    .ok_or_else(|| "ошибка целочисленного деления".into()),
                _ => unreachable!(),
            },
            Token::Percent if right_number == 0.0 => Err("деление по модулю на ноль".into()),
            Token::Percent if both_int => match (left, right) {
                (Value::Int(left), Value::Int(right)) => left
                    .checked_rem(right)
                    .map(Value::Int)
                    .ok_or_else(|| "ошибка деления по модулю".into()),
                _ => unreachable!(),
            },
            Token::Plus => Ok(Value::Float(left_number + right_number)),
            Token::Minus => Ok(Value::Float(left_number - right_number)),
            Token::Star => Ok(Value::Float(left_number * right_number)),
            Token::Slash => Ok(Value::Float(left_number / right_number)),
            Token::Percent => Ok(Value::Float(left_number % right_number)),
            Token::Less => Ok(Value::Bool(left_number < right_number)),
            Token::LessEqual => Ok(Value::Bool(left_number <= right_number)),
            Token::Greater => Ok(Value::Bool(left_number > right_number)),
            Token::GreaterEqual => Ok(Value::Bool(left_number >= right_number)),
            _ => Err("неподдерживаемая числовая операция".into()),
        };
    }
    if let (Value::Text(left), Value::Text(right)) = (&left, &right) {
        return Ok(Value::Bool(match operator {
            Token::Less => left < right,
            Token::LessEqual => left <= right,
            Token::Greater => left > right,
            Token::GreaterEqual => left >= right,
            _ => return Err("для текста доступно сравнение и сложение".into()),
        }));
    }
    Err(format!(
        "операция {operator:?} не поддерживает «{}» и «{}»",
        left.display(),
        right.display()
    ))
}

fn index_value(target: Value, index: Value) -> Result<Value, String> {
    match target {
        Value::List(values) => {
            let index = index.as_i64("индекс")?;
            if index < 0 {
                return Err("индекс не может быть отрицательным".into());
            }
            values
                .get(index as usize)
                .cloned()
                .ok_or_else(|| format!("индекс {index} вне коллекции из {} элементов", values.len()))
        }
        Value::Text(value) => {
            let index = index.as_i64("индекс")?;
            if index < 0 {
                return Err("индекс не может быть отрицательным".into());
            }
            value
                .chars()
                .nth(index as usize)
                .map(|value| Value::Text(value.to_string()))
                .ok_or_else(|| format!("индекс {index} вне строки"))
        }
        Value::Map(values) => Ok(values
            .get(&index.display())
            .cloned()
            .unwrap_or(Value::Null)),
        other => Err(format!("индексирование не поддерживает «{}»", other.display())),
    }
}

fn is_minecraft_source(source: &str) -> bool {
    Regex::new(r#"(?m)^\s*(?:use\s+minecraft\.|mod\s+\")"#)
        .unwrap()
        .is_match(source)
}

/// Executes an ordinary Funo program without Java, a JDK, Gradle or subprocesses.
pub fn execute(source: &str) -> BuildResult {
    let started = Instant::now();
    if is_minecraft_source(source) {
        return BuildResult {
            success: false,
            stdout: String::new(),
            stderr: "Minecraft-моды нельзя запускать встроенным интерпретатором. На Android доступны редактирование, проверка и Java-предпросмотр; Gradle/JAR-сборка остаётся в desktop-версии.".into(),
            generated_java: String::new(),
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: None,
        };
    }

    let generated_java = match compiler::transpile(source) {
        Ok(value) => value,
        Err(diagnostics) => {
            return BuildResult {
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
            }
        }
    };

    let program = match Program::parse(source) {
        Ok(program) => program,
        Err(error) => {
            return BuildResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Ошибка разбора: {error}"),
                generated_java,
                elapsed_ms: started.elapsed().as_millis(),
                diagnostics: Vec::new(),
                artifact: None,
            }
        }
    };
    let mut runtime = Runtime::new(program);
    match runtime.run() {
        Ok(Value::Int(code)) if code != 0 && code != 200 => BuildResult {
            success: false,
            stdout: runtime.output,
            stderr: format!("Программа завершилась с кодом {code}"),
            generated_java,
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: None,
        },
        Ok(_) => BuildResult {
            success: true,
            stdout: runtime.output,
            stderr: String::new(),
            generated_java,
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: None,
        },
        Err(error) => BuildResult {
            success: false,
            stdout: runtime.output,
            stderr: format!("Ошибка выполнения: {error}"),
            generated_java,
            elapsed_ms: started.elapsed().as_millis(),
            diagnostics: Vec::new(),
            artifact: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_recursion_conditions_and_return_200() {
        let result = execute(
            r#"fun fib(n: int) -> int = if n < 2 then n else fib(n - 1) + fib(n - 2)
fun main() {
    text title = "Привет"
    int answer = fib(10)
    println(title)
    println(answer)
    if answer == 55 {
        println("готово")
    }
    return(200)
}"#,
        );
        assert!(result.success, "{}", result.stderr);
        assert_eq!(result.stdout, "Привет\n55\nготово\n");
    }

    #[test]
    fn executes_arrays_loops_and_fstrings() {
        let result = execute(
            r#"fun main() {
    int[] values = [2, 3, 5]
    int total = 0
    for i in 0..3 {
        total = total + values[i]
    }
    repeat 2 {
        println(f"Сумма: {total}; {{ok}}")
    }
}"#,
        );
        assert!(result.success, "{}", result.stderr);
        assert_eq!(result.stdout, "Сумма: 10; {ok}\nСумма: 10; {ok}\n");
    }

    #[test]
    fn executes_mutable_collections() {
        let result = execute(
            r#"fun main() {
    list<text> names = ["Alex"]
    names.add("Steve")
    map<text, int> scores = map()
    scores.put("Alex", 42)
    println(names)
    println(scores.get("Alex"))
}"#,
        );
        assert!(result.success, "{}", result.stderr);
        assert_eq!(result.stdout, "[Alex, Steve]\n42\n");
    }

    #[test]
    fn rejects_minecraft_execution() {
        let result = execute("use minecraft.fabric\nmod \"demo\" { on start { log(\"x\") } }");
        assert!(!result.success);
        assert!(result.stderr.contains("Minecraft-моды нельзя запускать"));
    }
}

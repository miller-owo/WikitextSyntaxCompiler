use crate::preprocess::{Argument, ModuleProvider};
use std::cell::RefCell;
use std::collections::HashMap;
use wikitext_lua::value::{LuaFunction, TableRef};
use wikitext_lua::{Interp, Value};
pub struct LuaModules {
    modules: RefCell<HashMap<String, String>>,
    title: RefCell<String>,
    limits: wikitext_lua::Limits,
    allow_print: bool,
}

impl std::fmt::Debug for LuaModules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaModules")
            .field("modules", &self.modules.borrow().len())
            .field("title", &self.title.borrow())
            .finish_non_exhaustive()
    }
}

impl Default for LuaModules {
    fn default() -> Self {
        LuaModules::new()
    }
}

impl LuaModules {
    pub fn new() -> Self {
        LuaModules {
            modules: RefCell::new(HashMap::new()),
            title: RefCell::new(String::new()),
            limits: wikitext_lua::Limits::default(),
            allow_print: false,
        }
    }

    pub fn insert(&self, name: &str, source: impl Into<String>) {
        self.modules
            .borrow_mut()
            .insert(normalise(name), source.into());
    }

    pub fn extend<I, N, S>(&self, items: I)
    where
        I: IntoIterator<Item = (N, S)>,
        N: AsRef<str>,
        S: Into<String>,
    {
        for (name, source) in items {
            self.insert(name.as_ref(), source);
        }
    }

    pub fn set_title(&self, title: impl Into<String>) {
        *self.title.borrow_mut() = title.into();
    }

    pub fn with_limits(mut self, limits: wikitext_lua::Limits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_print(mut self) -> Self {
        self.allow_print = true;
        self
    }

    pub fn len(&self) -> usize {
        self.modules.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(&self) -> Vec<(String, String)> {
        self.modules
            .borrow()
            .iter()
            .map(|(name, source)| (name.clone(), source.clone()))
            .collect()
    }
    pub fn get(&self, name: &str) -> Option<String> {
        self.modules.borrow().get(&normalise(name)).cloned()
    }

    pub fn run(&self, module: &str, function: &str, frame: Frame) -> Result<String, String> {
        let Some(source) = self.get(module) else {
            return Err(format!("module `{module}` not found"));
        };
        let mut interp = Interp::new().with_limits(self.limits);
        install_mw(&mut interp, &self.title.borrow(), self.allow_print);
        let exports = interp
            .eval_multi(&source)
            .map_err(|error| format!("{module}: {error}"))?;
        let exports = exports
            .first()
            .cloned()
            .ok_or_else(|| format!("module `{module}` returned nothing"))?;
        if !matches!(exports, Value::Table(_)) {
            return Err(format!(
                "module `{module}` must return a table, got {}",
                exports.type_name()
            ));
        }
        let entry = interp.index(
            &exports,
            &Value::string(function),
            wikitext_lua::lex::Span::default(),
        );
        let entry = entry.map_err(|error| format!("{module}: {error}"))?;
        if !matches!(entry, Value::Function(_)) {
            return Err(format!("module `{module}` has no function `{function}`"));
        }

        let frame_table = frame.to_frame_table();
        if let Value::Table(metatable) = interp.global("__frame_metatable") {
            frame_table.set_metatable(Some(metatable));
        }
        interp.set_global("__frame", Value::Table(frame_table.clone()));
        let results = interp
            .call_value(entry, vec![Value::Table(frame_table)])
            .map_err(|error| format!("{module}.{function}: {error}"))?;
        let first = results.first().cloned().unwrap_or(Value::Nil);
        interp
            .tostring(&first)
            .map_err(|error| format!("{module}.{function}: {error}"))
    }
}

fn normalise(name: &str) -> String {
    let trimmed = name.trim().replace('_', " ");
    let trimmed = trimmed
        .strip_prefix("Module:")
        .or_else(|| trimmed.strip_prefix("module:"))
        .unwrap_or(&trimmed)
        .trim();
    let mut chars = trimmed.chars();
    match chars.next() {
        Some(first) => {
            first.to_uppercase().collect::<String>()
                + chars
                    .next()
                    .map(|_| &trimmed[first.len_utf8()..])
                    .unwrap_or("")
        }
        None => String::new(),
    }
}

impl ModuleProvider for LuaModules {
    fn invoke(&self, module: &str, function: &str, arguments: &[Argument]) -> Option<String> {
        let mut frame = Frame::new();
        for argument in arguments {
            if argument.name.is_empty() {
                frame.push(argument.value.clone());
            } else {
                frame.set(argument.name.clone(), argument.value.clone());
            }
        }
        Some(match self.run(module, function, frame) {
            Ok(text) => text,

            Err(message) => format!(
                "<strong class=\"error\">Script error: {}</strong>",
                crate::serialize::escape_text(&message)
            ),
        })
    }

    fn module_exists(&self, module: &str) -> bool {
        self.get(module).is_some()
    }

    fn name(&self) -> &str {
        "lua"
    }
}

#[derive(Clone, Debug, Default)]
pub struct Frame {
    positional: Vec<String>,
    named: Vec<(String, String)>,
}

impl Frame {
    pub fn new() -> Self {
        Frame::default()
    }

    pub fn push(&mut self, value: impl Into<String>) {
        self.positional.push(value.into());
    }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        match self.named.iter_mut().find(|(key, _)| *key == name) {
            Some(entry) => entry.1 = value,
            None => self.named.push((name, value)),
        }
    }

    fn to_frame_table(&self) -> TableRef {
        let arguments = TableRef::new();
        for (index, value) in self.positional.iter().enumerate() {
            arguments.set(Value::from(index + 1), Value::string(value));
        }
        for (name, value) in &self.named {
            arguments.set(Value::string(name), Value::string(value));
        }
        let frame = TableRef::new();
        frame.set(Value::from("args"), Value::Table(arguments));
        frame
    }
}

// A fresh interpreter per invocation keeps two modules from sharing globals.
fn install_mw(interp: &mut Interp, title: &str, allow_print: bool) {
    let mw = TableRef::new();
    mw.set(Value::from("text"), Value::Table(text_library()));
    mw.set(Value::from("ustring"), Value::Table(ustring_library()));
    mw.set(Value::from("title"), Value::Table(title_library(title)));
    if !title.is_empty() {
        interp.set_global("__page_title", title_object(title));
    }
    mw.set(Value::from("uri"), Value::Table(uri_library()));
    mw.set(Value::from("html"), Value::Table(html_library()));
    mw.set(Value::from("language"), Value::Table(language_library()));
    mw.set(Value::from("site"), Value::Table(TableRef::new()));
    mw.set(Value::from("message"), Value::Table(TableRef::new()));
    mw.set(Value::from("loadData"), native("loadData", 1, mw_load_data));
    mw.set(
        Value::from("getCurrentFrame"),
        native("getCurrentFrame", 0, mw_current_frame),
    );
    mw.set(
        Value::from("dumpObject"),
        native("dumpObject", 1, mw_dump_object),
    );
    interp.set_global("mw", Value::Table(mw));

    let frame_methods = TableRef::new();
    frame_methods.set(
        Value::from("getTitle"),
        native("getTitle", 1, frame_get_title),
    );
    frame_methods.set(
        Value::from("getParent"),
        native("getParent", 1, frame_get_parent),
    );
    let frame_metatable = TableRef::new();
    frame_metatable.set(Value::from("__index"), Value::Table(frame_methods));
    interp.set_global("__frame_metatable", Value::Table(frame_metatable));

    if !allow_print {
        interp.set_global("print", native("print", 0, silent_print));
    }
}

fn native(name: &str, arity: usize, implementation: wikitext_lua::value::NativeFunction) -> Value {
    Value::Function(LuaFunction::native(name, arity, implementation))
}

fn library<I>(functions: I) -> TableRef
where
    I: IntoIterator<Item = (&'static str, usize, wikitext_lua::value::NativeFunction)>,
{
    let table = TableRef::new();
    for (name, arity, implementation) in functions {
        table.set(Value::from(name), native(name, arity, implementation));
    }
    table
}

fn silent_print(_interp: &mut Interp, _arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(Vec::new())
}

fn text_library() -> TableRef {
    library([
        ("trim", 1, text_trim as wikitext_lua::value::NativeFunction),
        ("lower", 1, text_lower),
        ("upper", 1, text_upper),
        ("len", 1, text_len),
        ("sub", 4, text_sub),
        ("split", 3, text_split),
        ("gsplit", 3, text_split),
        ("jsonEncode", 2, text_json_encode),
        ("unstrip", 1, identity),
        ("nowiki", 1, identity),
        ("plaintext", 1, identity),
    ])
}

fn ustring_library() -> TableRef {
    library([
        ("len", 1, text_len as wikitext_lua::value::NativeFunction),
        ("sub", 4, text_sub),
        ("upper", 1, text_upper),
        ("lower", 1, text_lower),
        ("trim", 1, text_trim),
        ("split", 3, text_split),
        ("gsub", 4, ustring_gsub),
        ("match", 3, ustring_match),
        ("find", 3, ustring_find),
        ("rep", 3, ustring_rep),
    ])
}

fn string_argument(interp: &Interp, arguments: &[Value], index: usize) -> String {
    arguments
        .get(index)
        .and_then(|value| interp.to_string_value(value))
        .unwrap_or_default()
}

fn text_trim(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(vec![Value::string(
        string_argument(interp, arguments, 0).trim(),
    )])
}

fn text_lower(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(vec![Value::string(
        string_argument(interp, arguments, 0).to_lowercase(),
    )])
}

fn text_upper(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(vec![Value::string(
        string_argument(interp, arguments, 0).to_uppercase(),
    )])
}

fn text_len(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(vec![Value::Number(
        string_argument(interp, arguments, 0).chars().count() as f64,
    )])
}

fn text_sub(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let text = string_argument(interp, arguments, 0);
    let characters: Vec<char> = text.chars().collect();
    let length = characters.len() as i64;
    let start = arguments.get(1).and_then(Value::as_number).unwrap_or(1.0) as i64;
    let stop = match arguments.get(2) {
        Some(value) if !matches!(value, Value::Nil) => {
            value.as_number().unwrap_or(length as f64) as i64
        }
        _ => length,
    };
    let begin = if start < 0 { length + start } else { start - 1 }.clamp(0, length);
    let end = if stop < 0 { length + stop + 1 } else { stop }.clamp(0, length);
    if begin >= end {
        return Ok(vec![Value::string("")]);
    }
    Ok(vec![Value::string(
        characters[begin as usize..end as usize]
            .iter()
            .collect::<String>(),
    )])
}

fn text_split(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let text = string_argument(interp, arguments, 0);
    let separator = string_argument(interp, arguments, 1);
    let limit = arguments
        .get(2)
        .and_then(Value::as_number)
        .map(|number| number as i64)
        .unwrap_or(-1);
    let mut parts: Vec<String> = if separator.is_empty() {
        text.chars().map(String::from).collect()
    } else {
        text.split(&separator).map(str::to_string).collect()
    };
    if limit > 0 && parts.len() as i64 > limit {
        let tail = parts.split_off(limit as usize - 1).join(&separator);
        parts.push(tail);
    }
    Ok(vec![Value::Table(TableRef::from_pairs(
        parts
            .into_iter()
            .enumerate()
            .map(|(index, part)| (Value::from(index + 1), Value::string(part))),
    ))])
}

fn ustring_gsub(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    delegate(interp, "gsub", arguments)
}

fn ustring_match(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    delegate(interp, "match", arguments)
}

fn ustring_find(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    delegate(interp, "find", arguments)
}

fn ustring_rep(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    delegate(interp, "rep", arguments)
}

fn delegate(
    interp: &mut Interp,
    name: &str,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    let library = interp.global("string");
    let function = interp.index(
        &library,
        &Value::string(name),
        wikitext_lua::lex::Span::default(),
    )?;
    interp.call_value(function, arguments.to_vec())
}

fn identity(_interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(arguments.to_vec())
}

fn text_json_encode(
    _interp: &mut Interp,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    Ok(vec![Value::string(encode_json(&value, 0))])
}

fn encode_json(value: &Value, depth: usize) -> String {
    if depth > 16 {
        return "\"…\"".to_string();
    }
    match value {
        Value::Nil => "null".to_string(),
        Value::Boolean(flag) => flag.to_string(),
        Value::Number(number) => Value::format_number(*number),
        Value::String(text) => {
            let mut out = String::with_capacity(text.len() + 2);
            out.push('"');
            for ch in text.chars() {
                match ch {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    other => out.push(other),
                }
            }
            out.push('"');
            out
        }
        Value::Table(table) => {
            let items = table.array_items();
            let entries = table.entries();
            let is_array = !items.is_empty() && entries.len() == items.len();
            if is_array {
                let parts: Vec<String> = items
                    .iter()
                    .map(|item| encode_json(item, depth + 1))
                    .collect();
                format!("[{}]", parts.join(","))
            } else {
                let parts: Vec<String> = entries
                    .iter()
                    .map(|(key, item)| {
                        format!(
                            "{}:{}",
                            encode_json(&Value::string(key.to_string()), depth + 1),
                            encode_json(item, depth + 1)
                        )
                    })
                    .collect();
                format!("{{{}}}", parts.join(","))
            }
        }
        other => format!("\"{}\"", other),
    }
}

fn title_library(page: &str) -> TableRef {
    let table = library([
        ("new", 2, title_new as wikitext_lua::value::NativeFunction),
        ("getCurrentTitle", 0, title_current),
        ("equals", 2, title_equals),
        ("compare", 2, title_compare),
    ]);
    table.set(Value::from("currentTitle"), Value::Nil);
    let _ = page;
    table
}

fn title_object(full: &str) -> Value {
    let (namespace, page) = match full.split_once(':') {
        Some((namespace, page)) => (namespace.trim(), page.trim()),
        None => ("", full.trim()),
    };
    let table = TableRef::new();
    table.set(Value::from("fullText"), Value::string(full.trim()));
    table.set(Value::from("prefixedText"), Value::string(full.trim()));
    table.set(Value::from("pageName"), Value::string(page));
    table.set(Value::from("text"), Value::string(page));
    table.set(Value::from("rootText"), Value::string(page));
    table.set(Value::from("subpageText"), Value::string(page));
    table.set(
        Value::from("baseText"),
        Value::string(page.split('/').next().unwrap_or(page)),
    );
    table.set(Value::from("nsText"), Value::string(namespace));
    table.set(Value::from("namespace"), Value::string(namespace));
    table.set(Value::from("id"), Value::Number(0.0));
    table.set(Value::from("exists"), Value::Boolean(false));
    table.set(Value::from("isRedirect"), Value::Boolean(false));
    let metatable = TableRef::new();
    metatable.set(
        Value::from("__tostring"),
        native("__tostring", 1, title_tostring),
    );

    metatable.set(Value::from("__concat"), native("__concat", 2, title_concat));
    table.set_metatable(Some(metatable));
    Value::Table(table)
}

// Lua 5.1 does not consult `__tostring` for `..`, so a title needs `__concat`
// of its own to survive a concatenation.
fn title_concat(_interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let left = title_or_text(arguments.first());
    let right = title_or_text(arguments.get(1));
    Ok(vec![Value::string(format!("{left}{right}"))])
}

fn title_or_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::Table(table)) => match table.get(&Value::string("fullText")) {
            Value::String(text) => text.to_string(),
            _ => String::new(),
        },
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn title_tostring(
    _interp: &mut Interp,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    let text = match arguments.first() {
        Some(Value::Table(table)) => match table.get(&Value::string("fullText")) {
            Value::String(text) => text.to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    };
    Ok(vec![Value::string(text)])
}

fn title_new(_interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let text = arguments
        .first()
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if text.is_empty() {
        return Ok(vec![Value::Nil]);
    }
    Ok(vec![title_object(&text)])
}

fn title_current(interp: &mut Interp, _arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let title = interp.global("__page_title");
    Ok(vec![title])
}

fn title_equals(_interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let left = title_text(arguments.first());
    let right = title_text(arguments.get(1));
    Ok(vec![Value::Boolean(left == right)])
}

fn title_compare(_interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let left = title_text(arguments.first());
    let right = title_text(arguments.get(1));
    Ok(vec![Value::Number(match left.cmp(&right) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    })])
}

fn title_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::Table(table)) => match table.get(&Value::string("fullText")) {
            Value::String(text) => text.to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

fn uri_library() -> TableRef {
    library([
        (
            "encode",
            2,
            uri_encode as wikitext_lua::value::NativeFunction,
        ),
        ("decode", 1, uri_decode),
        ("query", 1, uri_query),
        ("anchorEncode", 1, uri_anchor_encode),
        ("validate", 1, uri_validate),
    ])
}

fn uri_encode(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let text = string_argument(interp, arguments, 0);
    let mode = string_argument(interp, arguments, 1).to_uppercase();
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        let safe = match mode.as_str() {
            "PATH" => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'),
            "QUERY" => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'),
            "WIKI" => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'!'),
            _ => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'),
        };
        if safe {
            out.push(byte as char);
        } else if mode == "QUERY" && byte == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(vec![Value::string(out)])
}

fn uri_decode(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let text = string_argument(interp, arguments, 0).replace('+', " ");
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    Ok(vec![Value::string(String::from_utf8_lossy(&out))])
}

fn uri_query(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let Some(Value::Table(table)) = arguments.first() else {
        return Ok(vec![Value::string("")]);
    };
    let mut parts = Vec::new();
    for (key, value) in table.entries() {
        let key = interp.tostring(&key)?;
        let value = interp.tostring(&value)?;
        let encoded_key = percent_encode(&key, true);
        let encoded_value = percent_encode(&value, true);
        parts.push(format!("{encoded_key}={encoded_value}"));
    }
    Ok(vec![Value::string(parts.join("&"))])
}

fn percent_encode(text: &str, space_as_plus: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else if space_as_plus && byte == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn uri_anchor_encode(
    interp: &mut Interp,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    let text = string_argument(interp, arguments, 0);
    Ok(vec![Value::string(crate::parse::heading_anchor(&text))])
}

fn uri_validate(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let text = string_argument(interp, arguments, 0);
    let valid = !text.is_empty()
        && !text
            .chars()
            .any(|ch| ch.is_whitespace() || ch == '<' || ch == '>');
    Ok(vec![
        Value::Boolean(valid),
        if valid {
            Value::string(text)
        } else {
            Value::Nil
        },
    ])
}

fn html_library() -> TableRef {
    library([(
        "create",
        1,
        html_create as wikitext_lua::value::NativeFunction,
    )])
}

fn html_create(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let tag = string_argument(interp, arguments, 0);
    let builder = TableRef::new();
    builder.set(Value::from("__tag"), Value::string(&tag));
    builder.set(Value::from("__children"), Value::Table(TableRef::new()));
    builder.set(Value::from("__attributes"), Value::Table(TableRef::new()));
    let methods = TableRef::new();
    methods.set(
        Value::from("wikitext"),
        native("wikitext", 1, html_wikitext),
    );
    methods.set(Value::from("tag"), native("tag", 2, html_tag));
    methods.set(Value::from("attr"), native("attr", 2, html_attr));
    methods.set(
        Value::from("addClass"),
        native("addClass", 1, html_add_class),
    );
    methods.set(Value::from("css"), native("css", 2, html_attr));
    methods.set(Value::from("done"), native("done", 0, html_done));
    methods.set(Value::from("allDone"), native("allDone", 0, html_done));
    let metatable = TableRef::new();
    metatable.set(Value::from("__index"), Value::Table(methods));
    metatable.set(
        Value::from("__tostring"),
        native("__tostring", 1, html_tostring),
    );
    builder.set_metatable(Some(metatable));
    let _ = interp;
    Ok(vec![Value::Table(builder)])
}

fn html_wikitext(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let builder = builder_of(interp, arguments)?;
    let text = method_argument(interp, arguments, 0);
    push_child(&builder, Value::string(text));
    Ok(vec![Value::Table(builder)])
}

fn html_tag(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let parent = builder_of(interp, arguments)?;
    let tag = method_argument(interp, arguments, 0);
    let child = match html_create(interp, &[Value::string(&tag)])?.first() {
        Some(Value::Table(table)) => table.clone(),
        _ => return Err(interp.raise_str("mw.html: could not create a tag")),
    };
    push_child(&parent, Value::Table(child.clone()));
    Ok(vec![Value::Table(child)])
}

fn html_attr(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let builder = builder_of(interp, arguments)?;
    let name = method_argument(interp, arguments, 0);
    let value = method_argument(interp, arguments, 1);
    if let Value::Table(attributes) = builder.get(&Value::string("__attributes")) {
        attributes.set(Value::string(name), Value::string(value));
    }
    Ok(vec![Value::Table(builder)])
}

fn html_add_class(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let builder = builder_of(interp, arguments)?;
    let class = method_argument(interp, arguments, 0);
    if let Value::Table(attributes) = builder.get(&Value::string("__attributes")) {
        let existing = attributes.get(&Value::string("class"));
        let merged = match existing {
            Value::String(text) if !text.is_empty() => format!("{text} {class}"),
            _ => class,
        };
        attributes.set(Value::from("class"), Value::string(merged));
    }
    Ok(vec![Value::Table(builder)])
}

fn html_done(_interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(vec![arguments.first().cloned().unwrap_or(Value::Nil)])
}

fn html_tostring(interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let Some(Value::Table(builder)) = arguments.first() else {
        return Ok(vec![Value::string("")]);
    };
    let rendered = render_builder(interp, builder)?;
    Ok(vec![Value::string(rendered)])
}

fn builder_of(interp: &Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<TableRef> {
    match arguments.first() {
        Some(Value::Table(table)) => Ok(table.clone()),
        Some(other) => Err(interp.raise_str(format!(
            "mw.html: the builder must be a table, got {}",
            other.type_name()
        ))),
        None => Err(interp.raise_str("mw.html: no builder was passed")),
    }
}

fn method_argument(interp: &Interp, arguments: &[Value], index: usize) -> String {
    arguments
        .get(index + 1)
        .and_then(|value| interp.to_string_value(value))
        .unwrap_or_default()
}

fn push_child(builder: &TableRef, child: Value) {
    if let Value::Table(children) = builder.get(&Value::string("__children")) {
        let next = children.length() + 1;
        children.set(Value::from(next), child);
    }
}

fn render_builder(interp: &mut Interp, builder: &TableRef) -> wikitext_lua::LuaResult<String> {
    let tag = match builder.get(&Value::string("__tag")) {
        Value::String(text) => text.to_string(),
        _ => return Ok(String::new()),
    };
    let mut out = String::new();
    out.push('<');
    out.push_str(&tag);
    if let Value::Table(attributes) = builder.get(&Value::string("__attributes")) {
        for (key, value) in attributes.entries() {
            out.push(' ');
            out.push_str(&key.to_string());
            out.push_str("=\"");
            out.push_str(&crate::serialize::escape_attribute(&value.to_string()));
            out.push('"');
        }
    }
    out.push('>');
    if let Value::Table(children) = builder.get(&Value::string("__children")) {
        for (_, child) in children.entries() {
            match &child {
                Value::Table(nested)
                    if matches!(nested.get(&Value::string("__tag")), Value::String(_)) =>
                {
                    out.push_str(&render_builder(interp, nested)?);
                }
                other => out.push_str(&crate::serialize::escape_text(&interp.tostring(other)?)),
            }
        }
    }
    out.push_str("</");
    out.push_str(&tag);
    out.push('>');
    Ok(out)
}

fn language_library() -> TableRef {
    library([
        (
            "new",
            1,
            language_new as wikitext_lua::value::NativeFunction,
        ),
        ("getCode", 1, language_get_code),
        ("getFallbacks", 1, language_fallbacks),
        ("isRTL", 1, language_is_rtl),
    ])
}

fn language_new(_interp: &mut Interp, arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    let code = arguments
        .first()
        .and_then(Value::as_str)
        .unwrap_or("en")
        .to_string();
    let table = TableRef::new();
    table.set(Value::from("code"), Value::string(&code));
    let metatable = TableRef::new();
    let methods = TableRef::new();
    methods.set(
        Value::from("getCode"),
        native("getCode", 1, language_table_code),
    );
    metatable.set(Value::from("__index"), Value::Table(methods));
    table.set_metatable(Some(metatable));
    Ok(vec![Value::Table(table)])
}

fn language_table_code(
    _interp: &mut Interp,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    match arguments.first() {
        Some(Value::Table(table)) => Ok(vec![table.get(&Value::string("code"))]),
        _ => Ok(vec![Value::string("en")]),
    }
}

fn language_get_code(
    _interp: &mut Interp,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    match arguments.first() {
        Some(Value::Table(table)) => Ok(vec![table.get(&Value::string("code"))]),
        Some(other) => Ok(vec![other.clone()]),
        None => Ok(vec![Value::string("en")]),
    }
}

fn language_fallbacks(
    _interp: &mut Interp,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    let code = match arguments.first() {
        Some(Value::Table(table)) => table.get(&Value::string("code")),
        Some(other) => other.clone(),
        None => Value::string("en"),
    };
    let table = TableRef::new();
    table.set(Value::from(1), code);
    table.set(Value::from(2), Value::string("en"));
    Ok(vec![Value::Table(table)])
}

fn language_is_rtl(
    _interp: &mut Interp,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    let code = match arguments.first() {
        Some(Value::Table(table)) => table.get(&Value::string("code")),
        Some(other) => other.clone(),
        None => Value::string("en"),
    };
    let rtl = matches!(code.as_str(), Some("ar" | "he" | "fa" | "ur" | "yi" | "dv"));
    Ok(vec![Value::Boolean(rtl)])
}

fn mw_load_data(interp: &mut Interp, _arguments: &[Value]) -> wikitext_lua::LuaResult<Vec<Value>> {
    Err(interp.raise_str("mw.loadData is not supported"))
}

fn mw_current_frame(
    interp: &mut Interp,
    _arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    let frame = interp.global("__frame");
    Ok(vec![frame])
}

fn mw_dump_object(
    _interp: &mut Interp,
    arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    Ok(vec![Value::string(dump_value(&value, 0))])
}

fn dump_value(value: &Value, depth: usize) -> String {
    if depth > 4 {
        return "…".to_string();
    }
    match value {
        Value::Table(table) => {
            let entries = table.entries();
            if entries.is_empty() {
                return "{}".to_string();
            }
            let parts: Vec<String> = entries
                .iter()
                .map(|(key, item)| format!("{key} = {}", dump_value(item, depth + 1)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::String(text) => format!("{text:?}"),
        other => other.to_string(),
    }
}

fn frame_get_title(
    interp: &mut Interp,
    _arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(vec![interp.global("__page_title")])
}

fn frame_get_parent(
    _interp: &mut Interp,
    _arguments: &[Value],
) -> wikitext_lua::LuaResult<Vec<Value>> {
    Ok(vec![Value::Nil])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn modules() -> LuaModules {
        let modules = LuaModules::new();
        modules.insert(
            "Greet",
            r#"
            local p = {}
            function p.hello(frame)
                return "Hello, " .. (frame.args[1] or "world") .. "!"
            end
            return p
            "#,
        );
        modules
    }

    #[test]
    fn runs_a_module_function() {
        let modules = modules();
        let mut frame = Frame::new();
        frame.push("Rust");
        assert_eq!(
            modules.run("Greet", "hello", frame).unwrap(),
            "Hello, Rust!"
        );
    }

    #[test]
    fn module_names_are_normalised() {
        let modules = modules();
        assert!(modules.get("greet").is_some());
        assert!(modules.get("Greet").is_some());
        assert!(modules.get("Module:Greet").is_some());
        assert!(modules.get("Module:greet").is_some());
        assert!(
            modules.get("GREET").is_none(),
            "only the first letter is uppercased"
        );
    }

    #[test]
    fn missing_pieces_report_cleanly() {
        let modules = modules();
        assert!(modules
            .run("Nope", "hello", Frame::new())
            .unwrap_err()
            .contains("not found"));
        assert!(modules
            .run("Greet", "nope", Frame::new())
            .unwrap_err()
            .contains("no function"));
    }

    #[test]
    fn module_provider_renders_errors_instead_of_failing() {
        let modules = modules();
        let output = modules.invoke("Missing", "f", &[]).unwrap();
        assert!(output.contains("class=\"error\""), "{output}");
        assert!(output.contains("not found"), "{output}");
    }

    #[test]
    fn mw_text_helpers_work() {
        let modules = LuaModules::new();
        modules.insert(
            "Text",
            r#"
            local p = {}
            function p.run(frame)
                local parts = {}
                parts[#parts + 1] = mw.text.trim("  padded  ")
                parts[#parts + 1] = mw.text.upper("abc")
                parts[#parts + 1] = tostring(mw.text.len("日本語"))
                parts[#parts + 1] = mw.ustring.sub("hello", 2, 3)
                return table.concat(parts, "|")
            end
            return p
            "#,
        );
        assert_eq!(
            modules.run("Text", "run", Frame::new()).unwrap(),
            "padded|ABC|3|el"
        );
    }

    #[test]
    fn mw_title_objects_render_their_text() {
        let modules = LuaModules::new();
        modules.insert(
            "Titles",
            r#"
            local p = {}
            function p.run(frame)
                local title = mw.title.new("Template:Greet")
                return tostring(title) .. "|" .. title.pageName .. "|" .. title.nsText
            end
            return p
            "#,
        );
        assert_eq!(
            modules.run("Titles", "run", Frame::new()).unwrap(),
            "Template:Greet|Greet|Template"
        );
    }

    #[test]
    fn mw_uri_and_html_build_output() {
        let modules = LuaModules::new();
        modules.insert(
            "Build",
            r#"
            local p = {}
            function p.run(frame)
                local encoded = mw.uri.encode("a b&c")
                local query = mw.uri.query({q = "a b"})
                local html = mw.html.create("span"):addClass("x"):wikitext("hi"):allDone()
                return encoded .. "|" .. query .. "|" .. tostring(html)
            end
            return p
            "#,
        );
        let output = modules.run("Build", "run", Frame::new()).unwrap();
        assert_eq!(output, "a%20b%26c|q=a+b|<span class=\"x\">hi</span>");
    }

    #[test]
    fn frames_carry_positional_and_named_arguments() {
        let modules = LuaModules::new();
        modules.insert(
            "Args",
            r#"
            local p = {}
            function p.run(frame)
                return frame.args[1] .. "/" .. frame.args[2] .. "/" .. (frame.args.name or "-")
            end
            return p
            "#,
        );
        let mut frame = Frame::new();
        frame.push("one");
        frame.push("two");
        frame.set("name", "three");
        assert_eq!(modules.run("Args", "run", frame).unwrap(), "one/two/three");
    }

    #[test]
    fn the_page_title_reaches_modules() {
        let modules = LuaModules::new();
        modules.set_title("Ada Lovelace");
        modules.insert(
            "Where",
            r#"
            local p = {}
            function p.run(frame)
                return frame:getTitle() .. "|" .. tostring(mw.title.getCurrentTitle())
            end
            return p
            "#,
        );
        let output = modules.run("Where", "run", Frame::new()).unwrap();
        assert!(output.starts_with("Ada Lovelace|"), "{output}");
    }

    #[test]
    fn a_module_that_raises_becomes_an_error_message() {
        let modules = LuaModules::new();
        modules.insert(
            "Bad",
            r#"
            local p = {}
            function p.run(frame)
                error("kaboom")
            end
            return p
            "#,
        );
        let output = modules.invoke("Bad", "run", &[]).unwrap();
        assert!(output.contains("kaboom"), "{output}");
        assert!(output.contains("class=\"error\""), "{output}");
    }

    #[test]
    fn loops_in_a_module_are_bounded() {
        let modules = LuaModules::new();
        modules.insert(
            "Spin",
            r#"
            local p = {}
            function p.run(frame)
                while true do end
            end
            return p
            "#,
        );
        let output = modules.invoke("Spin", "run", &[]).unwrap();
        assert!(output.contains("error"), "{output}");
    }

    #[test]
    fn a_module_must_return_a_table() {
        let modules = LuaModules::new();
        modules.insert("Wrong", "return 42");
        let error = modules.run("Wrong", "run", Frame::new()).unwrap_err();
        assert!(error.contains("table"), "{error}");
    }
}

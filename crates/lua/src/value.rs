use crate::ast::FunctionBody;
use crate::lex::Span;
use core::cell::{Cell as StdCell, RefCell};
use core::fmt;
use std::collections::HashMap;
use std::rc::Rc;
#[derive(Clone)]
pub enum Value {
    Nil,

    Boolean(bool),

    Number(f64),

    String(Rc<str>),

    Table(TableRef),

    Function(Rc<LuaFunction>),

    UserData(Rc<UserData>),
}

impl Value {
    pub fn string(value: impl AsRef<str>) -> Value {
        Value::String(Rc::from(value.as_ref()))
    }

    pub fn table() -> Value {
        Value::Table(TableRef::new())
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Boolean(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Table(_) => "table",
            Value::Function(_) => "function",
            Value::UserData(_) => "userdata",
        }
    }

    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Boolean(false))
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(value) => Some(*value),
            Value::String(text) => text.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    pub fn as_strict_number(&self) -> Option<f64> {
        match self {
            Value::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_strict_str(&self) -> Option<&str> {
        self.as_str()
    }

    pub fn as_table(&self) -> Option<&TableRef> {
        match self {
            Value::Table(table) => Some(table),
            _ => None,
        }
    }

    pub fn as_function(&self) -> Option<&Rc<LuaFunction>> {
        match self {
            Value::Function(function) => Some(function),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn format_number(value: f64) -> String {
        if value.is_nan() {
            return "nan".to_string();
        }
        if value.is_infinite() {
            return if value > 0.0 { "inf" } else { "-inf" }.to_string();
        }
        if value == value.trunc() && value.abs() < 1e15 {
            return format!("{}", value as i64);
        }

        let mut text = format!("{value:.14e}");
        if let Some((mantissa, exponent)) = text.split_once('e') {
            let exponent: i32 = exponent.parse().unwrap_or(0);
            let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
            text = format!("{mantissa}e{exponent:+03}");
            if (-4..14).contains(&exponent) {
                let mut plain = format!("{value:.14}");
                if plain.contains('.') {
                    while plain.ends_with('0') {
                        plain.pop();
                    }
                    if plain.ends_with('.') {
                        plain.pop();
                    }
                }
                if plain.len() <= 16 {
                    return plain;
                }
            }
        }
        text
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => f.write_str("nil"),
            Value::Boolean(value) => f.write_str(if *value { "true" } else { "false" }),
            Value::Number(value) => f.write_str(&Value::format_number(*value)),
            Value::String(text) => f.write_str(text),
            Value::Table(table) => write!(f, "table: {:#x}", table.address()),
            Value::Function(function) => write!(f, "function: {:#x}", function.address()),
            Value::UserData(data) => write!(f, "userdata: {:#x}", data.address()),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(text) => write!(f, "{text:?}"),
            other => write!(f, "{other}"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Table(a), Value::Table(b)) => a.same_table(b),
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (Value::UserData(a), Value::UserData(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Boolean(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Number(value as f64)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Number(f64::from(value))
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Value::Number(f64::from(value))
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value::Number(value as f64)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::string(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::string(value)
    }
}

#[derive(Clone)]
pub struct TableRef(Rc<RefCell<Table>>);

impl TableRef {
    pub fn new() -> Self {
        TableRef(Rc::new(RefCell::new(Table::default())))
    }

    pub fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<Value>,
        V: Into<Value>,
    {
        let table = TableRef::new();
        for (key, value) in pairs {
            table.set(key.into(), value.into());
        }
        table
    }

    pub fn get(&self, key: &Value) -> Value {
        let table = self.0.borrow();
        match normalize_key(key) {
            Some(Key::Number(index)) => table.array.get(index).cloned().unwrap_or(Value::Nil),
            Some(Key::String(text)) => table.hash.get(text.as_ref()).cloned().unwrap_or(Value::Nil),
            Some(Key::Boolean(flag)) => table.booleans.get(&flag).cloned().unwrap_or(Value::Nil),
            None => Value::Nil,
        }
    }

    pub fn set(&self, key: Value, value: Value) {
        let mut table = self.0.borrow_mut();
        match normalize_key(&key) {
            Some(Key::Number(index)) => {
                if matches!(value, Value::Nil) {
                    if index < table.array.len() {
                        table.array[index] = Value::Nil;
                    }
                    table.compact();
                    return;
                }
                if index < table.array.len() {
                    table.array[index] = value;
                } else {
                    while table.array.len() < index {
                        table.array.push(Value::Nil);
                    }
                    table.array.push(value);
                }
            }
            Some(Key::String(text)) => {
                if matches!(value, Value::Nil) {
                    table.hash.remove(text.as_ref());
                } else {
                    table.hash.insert(text.to_string(), value);
                }
            }
            Some(Key::Boolean(flag)) => {
                if matches!(value, Value::Nil) {
                    table.booleans.remove(&flag);
                } else {
                    table.booleans.insert(flag, value);
                }
            }
            None => {}
        }
    }

    pub fn remove(&self, key: &Value) -> Value {
        let previous = self.get(key);
        self.set(key.clone(), Value::Nil);
        previous
    }

    pub fn len(&self) -> usize {
        let table = self.0.borrow();
        table
            .array
            .iter()
            .filter(|value| !matches!(value, Value::Nil))
            .count()
            + table.hash.len()
            + table.booleans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn length(&self) -> usize {
        let table = self.0.borrow();
        let mut length = 0usize;
        for (index, value) in table.array.iter().enumerate() {
            if matches!(value, Value::Nil) {
                break;
            }
            length = index + 1;
        }
        length
    }

    pub fn entries(&self) -> Vec<(Value, Value)> {
        let table = self.0.borrow();
        let mut entries: Vec<(Value, Value)> = Vec::with_capacity(self.len());
        for (index, value) in table.array.iter().enumerate() {
            if !matches!(value, Value::Nil) {
                entries.push((Value::Number((index + 1) as f64), value.clone()));
            }
        }
        for (key, value) in &table.hash {
            entries.push((Value::string(key), value.clone()));
        }
        for (flag, value) in &table.booleans {
            entries.push((Value::Boolean(*flag), value.clone()));
        }
        entries
    }

    pub fn array_items(&self) -> Vec<Value> {
        let table = self.0.borrow();
        let mut items = Vec::new();
        for value in &table.array {
            if matches!(value, Value::Nil) {
                break;
            }
            items.push(value.clone());
        }
        items
    }

    pub fn metatable(&self) -> Option<TableRef> {
        self.0.borrow().metatable.clone()
    }

    pub fn set_metatable(&self, metatable: Option<TableRef>) {
        self.0.borrow_mut().metatable = metatable;
    }

    pub fn address(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }

    pub fn replace_array(&self, items: Vec<Value>) {
        let mut table = self.0.borrow_mut();
        table.array = items;
        table.hash.retain(|_, value| !matches!(value, Value::Nil));
    }

    pub fn same_table(&self, other: &TableRef) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for TableRef {
    fn default() -> Self {
        TableRef::new()
    }
}

impl fmt::Debug for TableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "table: {:#x}", self.address())
    }
}

#[derive(Default)]
struct Table {
    array: Vec<Value>,
    hash: HashMap<String, Value>,
    booleans: HashMap<bool, Value>,
    metatable: Option<TableRef>,
}

impl Table {
    fn compact(&mut self) {
        while self
            .array
            .last()
            .is_some_and(|value| matches!(value, Value::Nil))
        {
            self.array.pop();
        }
    }
}

enum Key<'a> {
    Number(usize),
    String(std::borrow::Cow<'a, str>),
    Boolean(bool),
}

fn normalize_key(key: &Value) -> Option<Key<'_>> {
    match key {
        Value::Number(value) => {
            if *value >= 1.0 && value.fract() == 0.0 && *value < u32::MAX as f64 {
                Some(Key::Number(*value as usize - 1))
            } else if value.is_nan() {
                None
            } else {
                Some(Key::String(std::borrow::Cow::Owned(Value::format_number(
                    *value,
                ))))
            }
        }
        Value::String(text) => Some(Key::String(std::borrow::Cow::Borrowed(text))),
        Value::Boolean(flag) => Some(Key::Boolean(*flag)),
        _ => None,
    }
}

pub struct LuaFunction {
    pub kind: FunctionKind,
    pub parameters: usize,
    pub span: Span,
    pub name: StdCell<Option<String>>,
}

impl LuaFunction {
    pub fn closure(body: FunctionBody, environment: EnvRef) -> Rc<LuaFunction> {
        Rc::new(LuaFunction {
            parameters: body.parameters.len(),
            span: body.span,
            kind: FunctionKind::Closure {
                body: Box::new(body),
                environment,
            },
            name: StdCell::new(None),
        })
    }

    pub fn native(
        name: impl Into<String>,
        arity: usize,
        implementation: NativeFunction,
    ) -> Rc<LuaFunction> {
        Rc::new(LuaFunction {
            kind: FunctionKind::Native {
                name: name.into(),
                implementation,
            },
            parameters: arity,
            span: Span::default(),
            name: StdCell::new(None),
        })
    }

    pub fn name(&self) -> Option<String> {
        match &self.kind {
            FunctionKind::Native { name, .. } => Some(name.clone()),
            FunctionKind::Closure { .. } => self.declared_name(),
        }
    }

    pub fn declared_name(&self) -> Option<String> {
        match &self.kind {
            FunctionKind::Native { name, .. } => Some(name.clone()),
            FunctionKind::Closure { .. } => self.name.replace(None),
        }
    }

    pub fn address(&self) -> usize {
        self as *const LuaFunction as usize
    }
}

impl fmt::Debug for LuaFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "function: {:#x}", self.address())
    }
}

pub enum FunctionKind {
    Closure {
        body: Box<FunctionBody>,
        environment: EnvRef,
    },

    Native {
        name: String,
        implementation: NativeFunction,
    },
}

pub type NativeFunction =
    fn(&mut crate::interp::Interp, &[Value]) -> crate::error::LuaResult<Vec<Value>>;

pub struct UserData {
    pub data: Box<dyn core::any::Any>,
    pub metatable: Option<TableRef>,
}

impl UserData {
    pub fn new<T: core::any::Any>(data: T) -> Self {
        UserData {
            data: Box::new(data),
            metatable: None,
        }
    }

    pub fn address(&self) -> usize {
        self as *const UserData as usize
    }
}

impl fmt::Debug for UserData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "userdata: {:#x}", self.address())
    }
}

// A closure captures its scope by reference, so a self-referential function
// forms an Rc cycle that is never freed. Accepted because one module run drops
// the whole heap; see AGENTS.md.
#[derive(Clone)]
pub struct EnvRef(Rc<RefCell<Env>>);

impl EnvRef {
    pub fn root() -> Self {
        EnvRef(Rc::new(RefCell::new(Env {
            bindings: HashMap::new(),
            parent: None,
            varargs: Vec::new(),
        })))
    }

    pub fn child(parent: &EnvRef) -> Self {
        EnvRef(Rc::new(RefCell::new(Env {
            bindings: HashMap::new(),
            parent: Some(parent.clone()),
            varargs: Vec::new(),
        })))
    }

    pub fn define(&self, name: impl Into<String>, value: Value) {
        self.0.borrow_mut().bindings.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        let environment = self.0.borrow();
        if let Some(value) = environment.bindings.get(name) {
            return Some(value.clone());
        }
        environment
            .parent
            .as_ref()
            .and_then(|parent| parent.get(name))
    }

    pub fn set(&self, name: &str, value: Value) -> bool {
        let mut environment = self.0.borrow_mut();
        if environment.bindings.contains_key(name) {
            environment.bindings.insert(name.to_string(), value);
            return true;
        }
        let parent = environment.parent.clone();
        drop(environment);
        match parent {
            Some(parent) => parent.set(name, value),
            None => false,
        }
    }

    pub fn set_varargs(&self, values: Vec<Value>) {
        self.0.borrow_mut().varargs = values;
    }

    pub fn varargs(&self) -> Vec<Value> {
        self.0.borrow().varargs.clone()
    }

    pub fn address(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }
}

impl fmt::Debug for EnvRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "env: {:#x}", self.address())
    }
}

#[derive(Default)]
struct Env {
    bindings: HashMap<String, Value>,
    parent: Option<EnvRef>,
    varargs: Vec<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_nil_and_false_are_falsey() {
        assert!(!Value::Nil.is_truthy());
        assert!(!Value::Boolean(false).is_truthy());
        assert!(Value::Boolean(true).is_truthy());
        assert!(Value::Number(0.0).is_truthy(), "0 is true in Lua");
        assert!(
            Value::string("").is_truthy(),
            "the empty string is true in Lua"
        );
    }

    #[test]
    fn formats_numbers_like_lua() {
        assert_eq!(Value::format_number(1.0), "1");
        assert_eq!(Value::format_number(-3.0), "-3");
        assert_eq!(Value::format_number(1.5), "1.5");
        assert_eq!(Value::format_number(0.1), "0.1");
        assert_eq!(Value::format_number(f64::INFINITY), "inf");
        assert_eq!(Value::format_number(1e20), "1e+20");
    }

    #[test]
    fn table_indexing_and_length() {
        let table = TableRef::new();
        table.set(Value::from(1), Value::from("a"));
        table.set(Value::from(2), Value::from("b"));
        assert_eq!(table.length(), 2);
        assert_eq!(table.get(&Value::from(1)), Value::from("a"));

        assert_eq!(table.get(&Value::from(9)), Value::Nil);
    }

    #[test]
    fn table_length_stops_at_a_hole() {
        let table = TableRef::new();
        table.set(Value::from(1), Value::from("a"));
        table.set(Value::from(3), Value::from("c"));
        assert_eq!(table.length(), 1);
    }

    #[test]
    fn removing_an_entry_grows_no_holes() {
        let table = TableRef::new();
        for index in 1..=3 {
            table.set(Value::from(index), Value::from(index));
        }
        table.set(Value::from(3), Value::Nil);
        assert_eq!(table.length(), 2);
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn string_and_boolean_keys_work() {
        let table = TableRef::new();
        table.set(Value::from("name"), Value::from("Ada"));
        table.set(Value::Boolean(true), Value::from(1));
        assert_eq!(table.get(&Value::from("name")), Value::from("Ada"));
        assert_eq!(table.get(&Value::Boolean(true)), Value::from(1));
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn non_integer_number_keys_use_the_hash_part() {
        let table = TableRef::new();
        table.set(Value::from(1.5), Value::from("half"));
        assert_eq!(table.get(&Value::from(1.5)), Value::from("half"));
        assert_eq!(table.length(), 0);
    }

    #[test]
    fn nil_keys_are_ignored() {
        let table = TableRef::new();
        table.set(Value::Nil, Value::from("x"));
        assert_eq!(table.len(), 0);
        assert_eq!(table.get(&Value::Nil), Value::Nil);
    }

    #[test]
    fn environments_chain_and_shadow() {
        let outer = EnvRef::root();
        outer.define("x", Value::from(1));
        let inner = EnvRef::child(&outer);
        assert_eq!(inner.get("x"), Some(Value::from(1)));
        inner.define("x", Value::from(2));
        assert_eq!(inner.get("x"), Some(Value::from(2)));
        assert_eq!(outer.get("x"), Some(Value::from(1)));

        inner.define("y", Value::from(1));
        assert!(inner.set("y", Value::from(5)));
        assert_eq!(inner.get("y"), Some(Value::from(5)));

        assert!(!inner.set("z", Value::from(1)));
    }

    #[test]
    fn entries_include_the_array_part_in_order() {
        let table = TableRef::new();
        table.set(Value::from(1), Value::from("a"));
        table.set(Value::from("k"), Value::from("v"));
        let entries = table.entries();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&(Value::from(1), Value::from("a"))));
        assert!(entries.contains(&(Value::from("k"), Value::from("v"))));
    }
}

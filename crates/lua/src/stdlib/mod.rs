pub mod base;
pub mod math;
pub mod os;
pub mod pattern;
pub mod string;
pub mod table;
use crate::interp::Interp;
use crate::value::{TableRef, Value};
pub fn install(interp: &mut Interp) {
    base::install(interp);
    let string = string::library(interp);
    let table = table::library();
    let math = math::library();
    let os = os::library();
    interp.set_global("__string_library", Value::Table(string));
    interp.set_global("table", Value::Table(table));
    interp.set_global("math", Value::Table(math));
    interp.set_global("os", Value::Table(os));

    let string = interp.globals_table();
    let library = interp.global("__string_library");
    let snapshot = TableRef::new();
    for (key, value) in string.entries() {
        snapshot.set(key, value);
    }
    snapshot.set(Value::from("string"), library.clone());
    interp.set_global("_G", Value::Table(snapshot));
    interp.set_global("string", library.clone());

    let metatable = TableRef::new();
    metatable.set(Value::from("__index"), library);
    interp.set_string_metatable(metatable);
}

pub fn function_table<I>(functions: I) -> TableRef
where
    I: IntoIterator<Item = (&'static str, usize, crate::value::NativeFunction)>,
{
    let table = TableRef::new();
    for (name, arity, implementation) in functions {
        table.set(
            Value::from(name),
            Value::Function(crate::value::LuaFunction::native(
                name,
                arity,
                implementation,
            )),
        );
    }
    table
}

pub fn check_type(
    interp: &Interp,
    arguments: &[Value],
    index: usize,
    expected: &str,
) -> crate::error::LuaResult<Value> {
    let value = arguments.get(index).cloned().unwrap_or(Value::Nil);
    if value.type_name() == expected {
        return Ok(value);
    }
    Err(interp.raise_str(format!(
        "bad argument #{} ({} expected, got {})",
        index + 1,
        expected,
        value.type_name()
    )))
}

pub fn check_number(
    interp: &Interp,
    arguments: &[Value],
    index: usize,
) -> crate::error::LuaResult<f64> {
    let value = arguments.get(index).cloned().unwrap_or(Value::Nil);
    value.as_number().ok_or_else(|| {
        interp.raise_str(format!(
            "bad argument #{} (number expected, got {})",
            index + 1,
            value.type_name()
        ))
    })
}

pub fn check_string(
    interp: &Interp,
    arguments: &[Value],
    index: usize,
) -> crate::error::LuaResult<String> {
    let value = arguments.get(index).cloned().unwrap_or(Value::Nil);
    interp.to_string_value(&value).ok_or_else(|| {
        interp.raise_str(format!(
            "bad argument #{} (string expected, got {})",
            index + 1,
            value.type_name()
        ))
    })
}

pub fn optional_integer(
    interp: &Interp,
    arguments: &[Value],
    index: usize,
    default: i64,
) -> crate::error::LuaResult<i64> {
    match arguments.get(index) {
        None | Some(Value::Nil) => Ok(default),
        Some(value) => interp.to_integer(value),
    }
}

pub fn optional_number(
    interp: &Interp,
    arguments: &[Value],
    index: usize,
    default: f64,
) -> crate::error::LuaResult<f64> {
    match arguments.get(index) {
        None | Some(Value::Nil) => Ok(default),
        Some(value) => interp.to_number(value),
    }
}

pub fn varargs_from(arguments: &[Value], index: usize) -> Vec<Value> {
    arguments.iter().skip(index).cloned().collect()
}

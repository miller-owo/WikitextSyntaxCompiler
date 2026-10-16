use crate::error::{LuaError, LuaResult};
use crate::interp::Interp;
use crate::stdlib::{function_table, optional_integer};
use crate::value::{LuaFunction, TableRef, Value};
pub fn install(interp: &mut Interp) {
    let globals: [(&str, usize, crate::value::NativeFunction); 15] = [
        ("assert", 1, lua_assert),
        ("error", 1, lua_error),
        ("ipairs", 1, lua_ipairs),
        ("next", 1, lua_next),
        ("pairs", 1, lua_pairs),
        ("pcall", 1, lua_pcall),
        ("print", 0, lua_print),
        ("rawequal", 2, lua_rawequal),
        ("rawget", 2, lua_rawget),
        ("rawset", 3, lua_rawset),
        ("select", 1, lua_select),
        ("tonumber", 1, lua_tonumber),
        ("tostring", 1, lua_tostring),
        ("type", 1, lua_type),
        ("unpack", 1, lua_unpack),
    ];
    for (name, arity, implementation) in globals {
        let function = Value::Function(LuaFunction::native(name, arity, implementation));
        interp.set_global(name, function);
    }
    let metatable_functions: [(&str, usize, crate::value::NativeFunction); 2] = [
        ("getmetatable", 1, lua_getmetatable),
        ("setmetatable", 2, lua_setmetatable),
    ];
    for (name, arity, implementation) in metatable_functions {
        let function = Value::Function(LuaFunction::native(name, arity, implementation));
        interp.set_global(name, function);
    }
    interp.set_global("_VERSION", Value::string("Lua 5.1"));

    interp.set_global("_G", Value::Table(global_view(interp)));
}

fn global_view(interp: &Interp) -> TableRef {
    let table = TableRef::new();
    for name in [
        "assert",
        "error",
        "ipairs",
        "next",
        "pairs",
        "pcall",
        "print",
        "rawequal",
        "rawget",
        "rawset",
        "select",
        "tonumber",
        "tostring",
        "type",
        "unpack",
        "getmetatable",
        "setmetatable",
        "string",
        "table",
        "math",
        "os",
        "_VERSION",
    ] {
        table.set(Value::from(name), interp.global(name));
    }
    table
}

fn lua_assert(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    if value.is_truthy() {
        return Ok(arguments.to_vec());
    }
    let message = arguments
        .get(1)
        .cloned()
        .unwrap_or_else(|| Value::string("assertion failed!"));
    Err(interp.raise(message))
}

fn lua_error(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);

    let _level = optional_integer(interp, arguments, 1, 1)?;
    if matches!(value, Value::Nil) {
        return Err(interp.raise_str("nil"));
    }
    Err(interp.raise(value))
}

fn lua_pcall(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let Some(callee) = arguments.first().cloned() else {
        return Err(interp.raise_str("bad argument #1 (value expected)"));
    };
    let rest: Vec<Value> = arguments.iter().skip(1).cloned().collect();
    match interp.call_value(callee, rest) {
        Ok(mut values) => {
            let mut result = vec![Value::Boolean(true)];
            result.append(&mut values);
            Ok(result)
        }
        // A limit error escapes pcall on purpose: the interpreter gave up, so
        // letting the module continue would only trip the same limit again.
        Err(LuaError::LimitExceeded { limit }) => Err(LuaError::LimitExceeded { limit }),
        Err(error) => {
            let message = error
                .value()
                .cloned()
                .unwrap_or_else(|| Value::string(error.to_string()));
            Ok(vec![Value::Boolean(false), message])
        }
    }
}

fn lua_type(_interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    Ok(vec![Value::string(value.type_name())])
}

fn lua_tostring(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    Ok(vec![Value::string(interp.tostring(&value)?)])
}

fn lua_tonumber(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    let base = optional_integer(interp, arguments, 1, 10)?;
    match &value {
        Value::Number(number) => Ok(vec![Value::Number(*number)]),
        Value::String(text) => {
            let trimmed = text.trim();
            if base == 10 {
                return Ok(vec![match trimmed.parse::<f64>() {
                    Ok(number) => Value::Number(number),
                    Err(_) => Value::Nil,
                }]);
            }

            if !(2..=36).contains(&base) {
                return Err(interp.raise_str("bad argument #2 (base out of range)"));
            }
            let (sign, digits) = match trimmed.strip_prefix('-') {
                Some(rest) => (-1.0, rest),
                None => (1.0, trimmed.strip_prefix('+').unwrap_or(trimmed)),
            };
            match i64::from_str_radix(digits, base as u32) {
                Ok(number) => Ok(vec![Value::Number(sign * number as f64)]),
                Err(_) => Ok(vec![Value::Nil]),
            }
        }
        _ => Ok(vec![Value::Nil]),
    }
}

fn lua_print(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let mut parts = Vec::with_capacity(arguments.len());
    for argument in arguments {
        parts.push(interp.tostring(argument)?);
    }
    println!("{}", parts.join("\t"));
    Ok(Vec::new())
}

fn lua_select(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let first = arguments.first().cloned().unwrap_or(Value::Nil);
    if let Value::String(text) = &first {
        if text.as_ref() == "#" {
            return Ok(vec![
                Value::Number(arguments.len().saturating_sub(1) as f64),
            ]);
        }
    }
    let index = interp.to_integer(&first)?;
    let rest: Vec<Value> = arguments.iter().skip(1).cloned().collect();
    if index < 0 {
        let from_end = rest.len() as i64 + index;
        if from_end < 0 {
            return Err(interp.raise_str("bad argument #1 (index out of range)"));
        }
        return Ok(rest.into_iter().skip(from_end as usize).collect());
    }
    if index == 0 {
        return Err(interp.raise_str("bad argument #1 (index out of range)"));
    }
    Ok(rest.into_iter().skip(index as usize - 1).collect())
}

fn lua_rawequal(_interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let a = arguments.first().cloned().unwrap_or(Value::Nil);
    let b = arguments.get(1).cloned().unwrap_or(Value::Nil);
    Ok(vec![Value::Boolean(a == b)])
}

fn lua_rawget(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = match arguments.first() {
        Some(Value::Table(table)) => table.clone(),
        Some(other) => {
            return Err(interp.raise_str(format!(
                "bad argument #1 (table expected, got {})",
                other.type_name()
            )))
        }
        None => return Err(interp.raise_str("bad argument #1 (table expected, got no value)")),
    };
    let key = arguments.get(1).cloned().unwrap_or(Value::Nil);
    Ok(vec![table.get(&key)])
}

fn lua_rawset(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = match arguments.first() {
        Some(Value::Table(table)) => table.clone(),
        Some(other) => {
            return Err(interp.raise_str(format!(
                "bad argument #1 (table expected, got {})",
                other.type_name()
            )))
        }
        None => return Err(interp.raise_str("bad argument #1 (table expected, got no value)")),
    };
    let key = arguments.get(1).cloned().unwrap_or(Value::Nil);
    let value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    table.set(key, value);
    Ok(vec![arguments.first().cloned().unwrap_or(Value::Nil)])
}

fn lua_getmetatable(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    match interp.metatable_of(&value) {
        Some(metatable) => {
            let guard = metatable.get(&Value::string("__metatable"));
            if !matches!(guard, Value::Nil) {
                return Ok(vec![guard]);
            }
            Ok(vec![Value::Table(metatable)])
        }
        None => Ok(vec![Value::Nil]),
    }
}

fn lua_setmetatable(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    let metatable = match arguments.get(1) {
        Some(Value::Table(table)) => Some(table.clone()),
        Some(Value::Nil) | None => None,
        Some(other) => {
            return Err(interp.raise_str(format!(
                "bad argument #2 (nil or table expected, got {})",
                other.type_name()
            )))
        }
    };
    match &value {
        Value::Table(table) => {
            if let (Some(existing), Some(_)) = (table.metatable(), metatable.as_ref()) {
                if !matches!(existing.get(&Value::string("__metatable")), Value::Nil) {
                    return Err(interp.raise_str("cannot change a protected metatable"));
                }
            }
            table.set_metatable(metatable);
            Ok(vec![value])
        }
        _ => Err(interp.raise_str("bad argument #1 (table expected)")),
    }
}

fn lua_next(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = match arguments.first() {
        Some(Value::Table(table)) => table.clone(),
        _ => return Err(interp.raise_str("bad argument #1 (table expected)")),
    };
    let key = arguments.get(1).cloned().unwrap_or(Value::Nil);
    let entries = table.entries();

    let position = if matches!(key, Value::Nil) {
        None
    } else {
        entries.iter().position(|(entry_key, _)| *entry_key == key)
    };
    let start = match position {
        None => 0,
        Some(index) => index + 1,
    };
    match entries.get(start) {
        Some((key, value)) => Ok(vec![key.clone(), value.clone()]),
        None => Ok(vec![Value::Nil]),
    }
}

fn lua_pairs(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);

    if let Some(handler) = interp.metamethod(&value, "__pairs") {
        return interp.call_value(handler, vec![value]);
    }
    if !matches!(value, Value::Table(_)) {
        return Err(interp.raise_str(format!(
            "bad argument #1 (table expected, got {})",
            value.type_name()
        )));
    }
    let next = interp.global("next");
    Ok(vec![next, value, Value::Nil])
}

fn lua_ipairs(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let value = arguments.first().cloned().unwrap_or(Value::Nil);
    if !matches!(value, Value::Table(_)) {
        return Err(interp.raise_str(format!(
            "bad argument #1 (table expected, got {})",
            value.type_name()
        )));
    }

    let iterator = interp.global("__ipairs_iterator");
    let iterator = if matches!(iterator, Value::Nil) {
        let function = Value::Function(LuaFunction::native("ipairs_iter", 2, ipairs_iterator));
        interp.set_global("__ipairs_iterator", function.clone());
        function
    } else {
        iterator
    };
    Ok(vec![iterator, value, Value::Number(0.0)])
}

fn ipairs_iterator(_interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = match arguments.first() {
        Some(Value::Table(table)) => table.clone(),
        _ => return Ok(vec![Value::Nil]),
    };
    let index = arguments.get(1).and_then(Value::as_number).unwrap_or(0.0) + 1.0;
    let value = table.get(&Value::Number(index));
    if matches!(value, Value::Nil) {
        return Ok(vec![Value::Nil]);
    }
    Ok(vec![Value::Number(index), value])
}

fn lua_unpack(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = match arguments.first() {
        Some(Value::Table(table)) => table.clone(),
        _ => return Err(interp.raise_str("bad argument #1 (table expected)")),
    };
    let start = optional_integer(interp, arguments, 1, 1)?;
    let stop = match arguments.get(2) {
        None | Some(Value::Nil) => table.length() as i64,
        Some(value) => interp.to_integer(value)?,
    };
    let mut values = Vec::new();
    let mut index = start;
    while index <= stop {
        values.push(table.get(&Value::Number(index as f64)));
        index += 1;
    }
    Ok(values)
}

pub fn bad_argument(interp: &Interp, index: usize, expected: &str, got: &Value) -> LuaError {
    interp.raise_str(format!(
        "bad argument #{} ({} expected, got {})",
        index,
        expected,
        got.type_name()
    ))
}

pub fn base_function_table() -> TableRef {
    function_table([
        ("type", 1, lua_type as crate::value::NativeFunction),
        ("tostring", 1, lua_tostring),
        ("tonumber", 1, lua_tonumber),
        ("select", 1, lua_select),
    ])
}

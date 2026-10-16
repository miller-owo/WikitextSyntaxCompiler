use crate::error::LuaResult;
use crate::interp::Interp;
use crate::stdlib::{function_table, optional_integer};
use crate::value::{TableRef, Value};
pub fn library() -> TableRef {
    function_table([
        ("concat", 1, lua_concat as crate::value::NativeFunction),
        ("insert", 2, lua_insert),
        ("maxn", 1, lua_maxn),
        ("remove", 1, lua_remove),
        ("sort", 1, lua_sort),
    ])
}

fn check_table(interp: &Interp, arguments: &[Value], index: usize) -> LuaResult<TableRef> {
    match arguments.get(index) {
        Some(Value::Table(table)) => Ok(table.clone()),
        Some(other) => Err(interp.raise_str(format!(
            "bad argument #{} (table expected, got {})",
            index + 1,
            other.type_name()
        ))),
        None => Err(interp.raise_str(format!(
            "bad argument #{} (table expected, got no value)",
            index + 1
        ))),
    }
}

fn lua_concat(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = check_table(interp, arguments, 0)?;
    let separator = match arguments.get(1) {
        None | Some(Value::Nil) => String::new(),
        Some(value) => interp.to_string_value(value).ok_or_else(|| {
            interp.raise_str(format!(
                "bad argument #2 (string expected, got {})",
                value.type_name()
            ))
        })?,
    };
    let start = optional_integer(interp, arguments, 2, 1)?;
    let stop = match arguments.get(3) {
        None | Some(Value::Nil) => table.length() as i64,
        Some(value) => interp.to_integer(value)?,
    };
    let mut parts = Vec::new();
    let mut index = start;
    while index <= stop {
        let value = table.get(&Value::Number(index as f64));
        let text = interp.to_string_value(&value).ok_or_else(|| {
            interp.raise_str(format!(
                "invalid value (at index {index}) in table for 'concat'"
            ))
        })?;
        parts.push(text);
        index += 1;
    }
    Ok(vec![Value::string(parts.join(&separator))])
}

fn lua_insert(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = check_table(interp, arguments, 0)?;
    let length = table.length() as i64;
    match arguments.len() {
        2 => {
            let value = arguments[1].clone();
            table.set(Value::Number((length + 1) as f64), value);
            Ok(Vec::new())
        }
        _ => {
            let position = interp.to_integer(&arguments[1])?;
            let value = arguments[2].clone();
            if position < 1 || position > length + 1 {
                return Err(interp.raise_str("bad argument #2 (position out of bounds)"));
            }

            let mut index = length;
            while index >= position {
                let moving = table.get(&Value::Number(index as f64));
                table.set(Value::Number((index + 1) as f64), moving);
                index -= 1;
            }
            table.set(Value::Number(position as f64), value);
            Ok(Vec::new())
        }
    }
}

fn lua_remove(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = check_table(interp, arguments, 0)?;
    let length = table.length() as i64;
    let position = optional_integer(interp, arguments, 1, length)?;
    if length == 0 && position == 0 {
        return Ok(vec![Value::Nil]);
    }
    if position < 1 || position > length {
        return Ok(vec![Value::Nil]);
    }
    let removed = table.get(&Value::Number(position as f64));
    let mut index = position;
    while index < length {
        let moving = table.get(&Value::Number((index + 1) as f64));
        table.set(Value::Number(index as f64), moving);
        index += 1;
    }
    table.set(Value::Number(length as f64), Value::Nil);
    Ok(vec![removed])
}

fn lua_maxn(_interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let Some(Value::Table(table)) = arguments.first() else {
        return Ok(vec![Value::Number(0.0)]);
    };
    let mut best = 0.0f64;
    for (key, _) in table.entries() {
        if let Value::Number(number) = key {
            if number > best {
                best = number;
            }
        }
    }
    Ok(vec![Value::Number(best)])
}

fn lua_sort(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let table = check_table(interp, arguments, 0)?;
    let comparator = arguments.get(1).cloned();
    let mut items = table.array_items();
    if items.len() < 2 {
        return Ok(Vec::new());
    }

    let sorted = merge_sort(interp, &mut items, comparator.as_ref())?;
    for (index, value) in sorted.into_iter().enumerate() {
        table.set(Value::Number((index + 1) as f64), value);
    }
    Ok(Vec::new())
}

fn merge_sort(
    interp: &mut Interp,
    items: &mut Vec<Value>,
    comparator: Option<&Value>,
) -> LuaResult<Vec<Value>> {
    if items.len() <= 1 {
        return Ok(items.clone());
    }
    let right = items.split_off(items.len() / 2);
    let mut left = merge_sort(interp, items, comparator)?;
    let mut right = merge_sort(interp, &mut right.clone(), comparator)?;
    let mut merged = Vec::with_capacity(left.len() + right.len());
    let mut left_index = 0usize;
    let mut right_index = 0usize;
    while left_index < left.len() && right_index < right.len() {
        let take_left = match comparator {
            Some(comparator) => {
                let results = interp.call_value(
                    comparator.clone(),
                    vec![left[left_index].clone(), right[right_index].clone()],
                )?;
                results.first().is_some_and(Value::is_truthy)
            }
            None => less_than(interp, &left[left_index], &right[right_index])?,
        };
        if take_left {
            merged.push(left[left_index].clone());
            left_index += 1;
        } else {
            merged.push(right[right_index].clone());
            right_index += 1;
        }
    }
    merged.extend(left.drain(left_index..));
    merged.extend(right.drain(right_index..));
    Ok(merged)
}

fn less_than(interp: &mut Interp, a: &Value, b: &Value) -> LuaResult<bool> {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => Ok(a < b),
        (Value::String(a), Value::String(b)) => Ok(a < b),
        _ => Err(interp.raise_str(format!(
            "attempt to compare {} with {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concat_joins_with_a_separator() {
        let mut interp = Interp::new();
        let value = interp
            .eval("local t = {'a', 'b', 'c'} return table.concat(t, '-')")
            .unwrap();
        assert_eq!(value, Value::string("a-b-c"));
    }

    #[test]
    fn insert_appends_and_inserts_at_a_position() {
        let mut interp = Interp::new();
        let value = interp
            .eval(
                "local t = {1, 2} \
                 table.insert(t, 3) \
                 table.insert(t, 1, 0) \
                 return table.concat(t, ',')",
            )
            .unwrap();
        assert_eq!(value, Value::string("0,1,2,3"));
    }

    #[test]
    fn remove_returns_the_removed_value() {
        let mut interp = Interp::new();
        let value = interp
            .eval(
                "local t = {'a', 'b', 'c'} \
                 local removed = table.remove(t, 1) \
                 return removed .. ':' .. table.concat(t, ',')",
            )
            .unwrap();
        assert_eq!(value, Value::string("a:b,c"));
    }

    #[test]
    fn sort_orders_numbers_and_strings() {
        let mut interp = Interp::new();
        let value = interp
            .eval("local t = {3, 1, 2} table.sort(t) return table.concat(t, ',')")
            .unwrap();
        assert_eq!(value, Value::string("1,2,3"));

        let value = interp
            .eval("local t = {'c', 'a', 'b'} table.sort(t) return table.concat(t)")
            .unwrap();
        assert_eq!(value, Value::string("abc"));
    }

    #[test]
    fn sort_accepts_a_comparator() {
        let mut interp = Interp::new();
        let value = interp
            .eval(
                "local t = {1, 2, 3} \
                 table.sort(t, function(a, b) return a > b end) \
                 return table.concat(t, ',')",
            )
            .unwrap();
        assert_eq!(value, Value::string("3,2,1"));
    }

    #[test]
    fn maxn_finds_the_largest_numeric_key() {
        let mut interp = Interp::new();
        let value = interp
            .eval("local t = {1, 2, 3, [10] = 'x'} return table.maxn(t)")
            .unwrap();
        assert_eq!(value, Value::Number(10.0));
    }
}

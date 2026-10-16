use crate::error::LuaResult;
use crate::interp::Interp;
use crate::stdlib::pattern::{captures_to_values, expand_replacement, Matcher};
use crate::stdlib::{check_string, function_table};
use crate::value::{TableRef, Value};
pub fn library(_interp: &mut Interp) -> TableRef {
    function_table([
        ("byte", 1, lua_byte as crate::value::NativeFunction),
        ("char", 0, lua_char),
        ("find", 2, lua_find),
        ("format", 1, lua_format),
        ("gmatch", 2, lua_gmatch),
        ("gsub", 3, lua_gsub),
        ("len", 1, lua_len),
        ("lower", 1, lua_lower),
        ("match", 2, lua_match),
        ("rep", 2, lua_rep),
        ("reverse", 1, lua_reverse),
        ("sub", 2, lua_sub),
        ("upper", 1, lua_upper),
    ])
}

fn subject(interp: &Interp, arguments: &[Value]) -> LuaResult<String> {
    check_string(interp, arguments, 0)
}

fn start_index(
    interp: &Interp,
    arguments: &[Value],
    index: usize,
    length: usize,
) -> LuaResult<usize> {
    match arguments.get(index) {
        None | Some(Value::Nil) => Ok(0),
        Some(value) => {
            let position = interp.to_integer(value)?;
            Ok(relative_index(position, length))
        }
    }
}

fn relative_index(position: i64, length: usize) -> usize {
    if position > 0 {
        (position - 1).min(length as i64).max(0) as usize
    } else if position == 0 {
        0
    } else {
        let from_end = length as i64 + position;
        from_end.max(0) as usize
    }
}

fn char_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
    boundaries.push(text.len());
    boundaries
}

fn lua_len(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    Ok(vec![Value::Number(text.chars().count() as f64)])
}

fn lua_lower(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    Ok(vec![Value::string(
        subject(interp, arguments)?.to_lowercase(),
    )])
}

fn lua_upper(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    Ok(vec![Value::string(
        subject(interp, arguments)?.to_uppercase(),
    )])
}

fn lua_reverse(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    Ok(vec![Value::string(text.chars().rev().collect::<String>())])
}

fn lua_rep(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    let count = interp.to_integer(&arguments.get(1).cloned().unwrap_or(Value::Nil))?;
    let separator = match arguments.get(2) {
        None | Some(Value::Nil) => String::new(),
        Some(value) => interp
            .to_string_value(value)
            .ok_or_else(|| interp.raise_str("bad argument #3 (string expected)"))?,
    };
    if count <= 0 {
        return Ok(vec![Value::string("")]);
    }
    let mut parts = Vec::with_capacity(count as usize);
    for _ in 0..count {
        parts.push(text.clone());
    }
    Ok(vec![Value::string(parts.join(&separator))])
}

fn lua_sub(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    let boundaries = char_boundaries(&text);
    let length = boundaries.len() - 1;
    let start = start_index(interp, arguments, 1, length)?;
    let stop = match arguments.get(2) {
        None | Some(Value::Nil) => length,
        Some(value) => {
            let position = interp.to_integer(value)?;
            if position < 0 {
                relative_index(position, length)
                    .saturating_add(1)
                    .min(length)
            } else {
                position.min(length as i64).max(0) as usize
            }
        }
    };
    if start >= stop {
        return Ok(vec![Value::string("")]);
    }
    Ok(vec![Value::string(
        &text[boundaries[start]..boundaries[stop]],
    )])
}

fn lua_byte(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    let boundaries = char_boundaries(&text);
    let length = boundaries.len() - 1;
    let start = start_index(interp, arguments, 1, length)?;
    let stop = match arguments.get(2) {
        None | Some(Value::Nil) => start,
        Some(value) => {
            let position = interp.to_integer(value)?;
            if position < 0 {
                relative_index(position, length)
                    .saturating_add(1)
                    .min(length)
            } else {
                position.min(length as i64).max(0) as usize
            }
        }
    };
    let mut values = Vec::new();
    let mut index = start;
    while index <= stop && index < length {
        for byte in text[boundaries[index]..boundaries[index + 1]].bytes() {
            values.push(Value::Number(f64::from(byte)));
        }
        index += 1;
    }
    Ok(values)
}

fn lua_char(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let mut bytes = Vec::with_capacity(arguments.len());
    for (index, argument) in arguments.iter().enumerate() {
        let code = argument.as_number().ok_or_else(|| {
            interp.raise_str(format!(
                "bad argument #{} (number expected, got {})",
                index + 1,
                argument.type_name()
            ))
        })?;
        if !(0.0..=255.0).contains(&code) {
            return Err(
                interp.raise_str(format!("bad argument #{} (value out of range)", index + 1))
            );
        }
        bytes.push(code as u8);
    }

    Ok(vec![Value::string(String::from_utf8_lossy(&bytes))])
}

fn lua_find(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    let pattern = check_string(interp, arguments, 1)?;
    let from = start_index(interp, arguments, 2, text.len())?;
    let plain = arguments.get(3).is_some_and(Value::is_truthy);
    if plain {
        return Ok(match text[from.min(text.len())..].find(&pattern) {
            Some(offset) => {
                let start = from + offset;
                let boundaries = char_boundaries(&text);
                let start_char = boundaries.iter().position(|at| *at == start).unwrap_or(0);
                vec![
                    Value::Number((start_char + 1) as f64),
                    Value::Number((start_char + pattern.chars().count()) as f64),
                ]
            }
            None => vec![Value::Nil],
        });
    }
    let matcher = Matcher::new(&text, &pattern)?;
    let Some(found) = matcher.find(from) else {
        return Ok(vec![Value::Nil]);
    };
    let boundaries = char_boundaries(&text);
    let start_char = boundaries
        .iter()
        .position(|at| *at == found.start)
        .unwrap_or(0);
    let end_char = boundaries
        .iter()
        .position(|at| *at == found.end)
        .unwrap_or(0);
    let mut values = vec![
        Value::Number((start_char + 1) as f64),
        Value::Number(end_char as f64),
    ];
    values.extend(captures_to_values(&text, &found));
    Ok(values)
}

fn lua_match(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    let pattern = check_string(interp, arguments, 1)?;
    let from = start_index(interp, arguments, 2, text.len())?;
    let matcher = Matcher::new(&text, &pattern)?;
    match matcher.find(from) {
        Some(found) => Ok(captures_to_values(&text, &found)),
        None => Ok(vec![Value::Nil]),
    }
}

fn lua_gmatch(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    let pattern = check_string(interp, arguments, 1)?;

    let matcher = Matcher::new(&text, &pattern)?;
    let _ = matcher;
    let state = TableRef::new();
    state.set(Value::from("text"), Value::string(&text));
    state.set(Value::from("pattern"), Value::string(&pattern));
    state.set(Value::from("position"), Value::Number(0.0));
    let iterator = Value::Function(crate::value::LuaFunction::native(
        "gmatch_iterator",
        1,
        gmatch_iterator,
    ));
    let _ = interp;
    Ok(vec![iterator, Value::Table(state)])
}

fn gmatch_iterator(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let state = match arguments.first() {
        Some(Value::Table(table)) => table.clone(),
        _ => return Ok(vec![Value::Nil]),
    };
    let text = match state.get(&Value::string("text")) {
        Value::String(text) => text.to_string(),
        _ => return Ok(vec![Value::Nil]),
    };
    let pattern = match state.get(&Value::string("pattern")) {
        Value::String(pattern) => pattern.to_string(),
        _ => return Ok(vec![Value::Nil]),
    };
    let position = state
        .get(&Value::string("position"))
        .as_number()
        .unwrap_or(0.0) as usize;
    if position > text.len() {
        return Ok(vec![Value::Nil]);
    }
    let matcher = Matcher::new(&text, &pattern)?;
    let Some(found) = matcher.find(position) else {
        state.set(
            Value::from("position"),
            Value::Number(text.len() as f64 + 1.0),
        );
        return Ok(vec![Value::Nil]);
    };

    let next = if found.end == found.start {
        let step = text[found.start..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
        found.end + step
    } else {
        found.end
    };
    state.set(Value::from("position"), Value::Number(next as f64));
    let _ = interp;
    Ok(captures_to_values(&text, &found))
}

fn lua_gsub(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let text = subject(interp, arguments)?;
    let pattern = check_string(interp, arguments, 1)?;
    let replacement = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let limit = match arguments.get(3) {
        None | Some(Value::Nil) => i64::MAX,
        Some(other) => interp.to_integer(other)?,
    };
    let matcher = Matcher::new(&text, &pattern)?;
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut count = 0i64;
    let mut search_from = 0usize;
    while count < limit {
        let Some(found) = matcher.find(search_from) else {
            break;
        };
        out.push_str(&text[cursor..found.start]);
        match &replacement {
            Value::String(template) => {
                out.push_str(&expand_replacement(interp, template, &text, &found)?);
            }
            Value::Table(table) => {
                let key = first_capture_value(&text, &found);
                let value = table.get(&key);
                if matches!(value, Value::Nil) {
                    out.push_str(&text[found.start..found.end]);
                } else {
                    out.push_str(&interp.tostring(&value)?);
                }
            }
            Value::Function(_) => {
                let values = captures_to_values(&text, &found);
                let results = interp.call_value(replacement.clone(), values)?;
                match results.first() {
                    None | Some(Value::Nil) | Some(Value::Boolean(false)) => {}
                    Some(value) => out.push_str(&interp.tostring(value)?),
                }
            }
            other => {
                return Err(interp.raise_str(format!(
                    "bad argument #3 (string/function/table expected, got {})",
                    other.type_name()
                )))
            }
        }
        count += 1;
        cursor = found.end;

        search_from = if found.end == found.start {
            match text[found.end..].chars().next() {
                Some(ch) => found.end + ch.len_utf8(),
                None => text.len() + 1,
            }
        } else {
            found.end
        };
        if search_from > text.len() {
            break;
        }
    }
    out.push_str(&text[cursor.min(text.len())..]);
    Ok(vec![Value::string(out), Value::Number(count as f64)])
}

fn first_capture_value(subject: &str, found: &crate::stdlib::pattern::Match) -> Value {
    match found.captures.first() {
        Some(crate::stdlib::pattern::Capture::Range(start, end)) => {
            Value::string(&subject[*start..*end])
        }
        Some(crate::stdlib::pattern::Capture::Position(index)) => Value::Number(*index as f64),
        None => Value::string(&subject[found.start..found.end]),
    }
}

fn lua_format(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let format = match arguments.first() {
        Some(Value::String(text)) => text.to_string(),
        Some(other) => {
            return Err(interp.raise_str(format!(
                "bad argument #1 (string expected, got {})",
                other.type_name()
            )))
        }
        None => return Err(interp.raise_str("bad argument #1 (string expected, got no value)")),
    };
    let mut out = String::with_capacity(format.len());
    let mut next_argument = 1usize;
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            out.push('%');
            continue;
        }

        let mut specification = String::from("%");
        while let Some(flag) = chars.peek() {
            if "-+ #0".contains(*flag) {
                specification.push(*flag);
                chars.next();
            } else {
                break;
            }
        }
        while let Some(digit) = chars.peek() {
            if digit.is_ascii_digit() {
                specification.push(*digit);
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&'.') {
            specification.push('.');
            chars.next();
            while let Some(digit) = chars.peek() {
                if digit.is_ascii_digit() {
                    specification.push(*digit);
                    chars.next();
                } else {
                    break;
                }
            }
        }
        let Some(conversion) = chars.next() else {
            return Err(interp.raise_str("invalid format string to 'format'"));
        };
        let value = arguments.get(next_argument).cloned().unwrap_or(Value::Nil);
        next_argument += 1;
        let rendered = match conversion {
            'd' | 'i' => {
                let number = value.as_number().ok_or_else(|| {
                    interp.raise_str(format!(
                        "bad argument #{} (number expected, got {})",
                        next_argument,
                        value.type_name()
                    ))
                })?;

                format_integer(&specification, number.trunc() as i64)
            }
            'u' => {
                let number = value.as_number().unwrap_or(0.0);
                format_integer(&specification, number.trunc().max(0.0) as i64)
            }
            'c' => {
                let number = value.as_number().unwrap_or(0.0);
                char::from_u32(number as u32)
                    .map(String::from)
                    .unwrap_or_default()
            }
            'x' => format_radix(
                &specification,
                value.as_number().unwrap_or(0.0) as i64,
                16,
                false,
            ),
            'X' => format_radix(
                &specification,
                value.as_number().unwrap_or(0.0) as i64,
                16,
                true,
            ),
            'o' => format_radix(
                &specification,
                value.as_number().unwrap_or(0.0) as i64,
                8,
                false,
            ),
            'e' | 'E' | 'f' | 'F' | 'g' | 'G' => {
                let number = value.as_number().unwrap_or(0.0);
                format_float(&specification, number, conversion)
            }
            's' => {
                let text = interp.tostring(&value)?;
                apply_width(&specification, text)
            }
            'q' => {
                let text = interp.tostring(&value)?;
                format_quoted(&text)
            }
            other => return Err(interp.raise_str(format!("invalid option '%{other}' to 'format'"))),
        };
        out.push_str(&rendered);
    }
    Ok(vec![Value::string(out)])
}

fn format_integer(specification: &str, value: i64) -> String {
    let (flags, width, precision) = parse_specification(specification);
    let negative = value < 0;
    let mut digits = value.unsigned_abs().to_string();
    if let Some(precision) = precision {
        while digits.len() < precision {
            digits.insert(0, '0');
        }
    }
    if flags.contains('+') && !negative {
        digits.insert(0, '+');
    } else if flags.contains(' ') && !negative {
        digits.insert(0, ' ');
    }
    if negative {
        digits.insert(0, '-');
    }
    pad(&digits, width, &flags)
}

fn format_radix(specification: &str, value: i64, radix: u32, upper: bool) -> String {
    let (flags, width, precision) = parse_specification(specification);
    let magnitude = value.unsigned_abs();
    let mut digits = match radix {
        16 => format!("{magnitude:x}"),
        8 => format!("{magnitude:o}"),
        _ => magnitude.to_string(),
    };
    if upper {
        digits = digits.to_uppercase();
    }
    if let Some(precision) = precision {
        while digits.len() < precision {
            digits.insert(0, '0');
        }
    }
    if flags.contains('#') && magnitude != 0 {
        let prefix = match (radix, upper) {
            (16, false) => "0x",
            (16, true) => "0X",
            (8, _) => "0",
            _ => "",
        };
        digits.insert_str(0, prefix);
    }
    pad(&digits, width, &flags)
}

fn format_float(specification: &str, value: f64, conversion: char) -> String {
    let (flags, width, precision) = parse_specification(specification);
    let precision = precision.unwrap_or(6);
    let mut text = match conversion {
        'f' | 'F' => format!("{value:.precision$}"),
        'e' => format_exp(value, precision, false),
        'E' => format_exp(value, precision, true),
        'g' | 'G' => {
            let exponent = if value == 0.0 {
                0
            } else {
                value.abs().log10().floor() as i32
            };
            if exponent < -4 || exponent >= precision as i32 {
                let text = format_exp(value, precision.saturating_sub(1), conversion == 'G');
                trim_g(text, conversion == 'G', true)
            } else {
                let decimals = (precision as i32 - 1 - exponent).max(0) as usize;
                let text = format!("{value:.decimals$}");
                trim_g(text, conversion == 'G', false)
            }
        }
        _ => format!("{value}"),
    };
    if flags.contains('+') && value >= 0.0 {
        text.insert(0, '+');
    }
    pad(&text, width, &flags)
}

fn format_exp(value: f64, precision: usize, upper: bool) -> String {
    let text = format!("{value:.precision$e}");
    match text.split_once('e') {
        Some((mantissa, exponent)) => {
            let exponent: i32 = exponent.parse().unwrap_or(0);
            let marker = if upper { 'E' } else { 'e' };
            format!(
                "{mantissa}{marker}{}{exponent:02}",
                if exponent < 0 { "-" } else { "+" }
            )
        }
        None => text,
    }
}

fn trim_g(text: String, upper: bool, has_exponent: bool) -> String {
    let text = if text.contains('.') {
        let trimmed = text.trim_end_matches('0').trim_end_matches('.');
        trimmed.to_string()
    } else {
        text
    };
    let text = if upper { text.to_uppercase() } else { text };
    let _ = has_exponent;
    text
}

fn parse_specification(specification: &str) -> (String, Option<usize>, Option<usize>) {
    let body = specification.trim_start_matches('%');
    let mut flags = String::new();
    let mut rest = body;
    for ch in body.chars() {
        if "-+ #0".contains(ch) {
            flags.push(ch);
            rest = &rest[ch.len_utf8()..];
        } else {
            break;
        }
    }
    let (width, rest) = match rest.find('.') {
        Some(dot) => (&rest[..dot], &rest[dot..]),
        None => {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let remainder = &rest[digits.len()..];
            return (
                flags,
                digits.parse().ok(),
                remainder.strip_prefix('.').and_then(|text| {
                    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
                    digits.parse().ok()
                }),
            );
        }
    };
    let precision = rest.strip_prefix('.').and_then(|text| {
        let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    });
    (flags, width.parse().ok(), precision)
}

fn pad(text: &str, width: Option<usize>, flags: &str) -> String {
    let Some(width) = width else {
        return text.to_string();
    };
    if text.chars().count() >= width {
        return text.to_string();
    }
    let fill = width - text.chars().count();
    if flags.contains('-') {
        return format!("{text}{}", " ".repeat(fill));
    }
    if flags.contains('0') {
        if let Some(rest) = text.strip_prefix('-') {
            return format!("-{}{rest}", "0".repeat(fill));
        }
        if let Some(rest) = text.strip_prefix('+') {
            return format!("+{}{rest}", "0".repeat(fill));
        }
        return format!("{}{text}", "0".repeat(fill));
    }
    format!("{}{text}", " ".repeat(fill))
}

fn apply_width(specification: &str, text: String) -> String {
    let (flags, width, precision) = parse_specification(specification);
    let text = match precision {
        Some(precision) => text.chars().take(precision).collect(),
        None => text,
    };
    pad(&text, width, &flags)
}

fn format_quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn eval(source: &str) -> Value {
        let mut interp = Interp::new();
        interp
            .eval(source)
            .unwrap_or_else(|error| panic!("{source}: {error}"))
    }

    #[test]
    fn len_counts_characters() {
        assert_eq!(eval("return string.len('hello')"), Value::Number(5.0));
        assert_eq!(eval("return ('hello'):len()"), Value::Number(5.0));
        assert_eq!(eval("return string.len('日本語')"), Value::Number(3.0));
    }

    #[test]
    fn sub_handles_negative_indices() {
        assert_eq!(
            eval("return string.sub('hello', 2, 3)"),
            Value::string("el")
        );
        assert_eq!(eval("return string.sub('hello', -3)"), Value::string("llo"));
        assert_eq!(
            eval("return string.sub('hello', 2, -2)"),
            Value::string("ell")
        );
        assert_eq!(eval("return string.sub('hello', 10)"), Value::string(""));
    }

    #[test]
    fn case_conversion_and_reverse() {
        assert_eq!(eval("return string.upper('abc')"), Value::string("ABC"));
        assert_eq!(eval("return string.lower('ABC')"), Value::string("abc"));
        assert_eq!(eval("return string.reverse('abc')"), Value::string("cba"));
    }

    #[test]
    fn rep_repeats_with_a_separator() {
        assert_eq!(eval("return string.rep('ab', 3)"), Value::string("ababab"));
        assert_eq!(
            eval("return string.rep('x', 3, '-')"),
            Value::string("x-x-x")
        );
        assert_eq!(eval("return string.rep('x', 0)"), Value::string(""));
    }

    #[test]
    fn byte_and_char_round_trip() {
        assert_eq!(eval("return string.byte('A')"), Value::Number(65.0));
        assert_eq!(eval("return string.char(65, 66)"), Value::string("AB"));
        let value = eval("local a, b = string.byte('AB', 1, 2) return a + b");
        assert_eq!(value, Value::Number(131.0));
    }

    #[test]
    fn find_returns_positions_and_captures() {
        let value = eval("return string.find('hello world', 'o w')");
        assert_eq!(value, Value::Number(5.0));
        let value = eval("return select(2, string.find('hello', 'l+'))");
        assert_eq!(value, Value::Number(4.0));
        let value = eval("return (string.find('abc', 'z'))");
        assert_eq!(value, Value::Nil);
    }

    #[test]
    fn find_with_plain_skips_patterns() {
        let value = eval("return (string.find('a.b', '.', 1, true))");
        assert_eq!(value, Value::Number(2.0));
    }

    #[test]
    fn match_returns_captures() {
        assert_eq!(
            eval("return string.match('key=value', '(%w+)=(%w+)')"),
            Value::string("key")
        );
        assert_eq!(
            eval("return select(2, string.match('key=value', '(%w+)=(%w+)'))"),
            Value::string("value")
        );
    }

    #[test]
    fn gmatch_iterates_over_matches() {
        let value = eval(
            "local parts = {} \
             for word in string.gmatch('one two three', '%a+') do parts[#parts + 1] = word end \
             return table.concat(parts, '|')",
        );
        assert_eq!(value, Value::string("one|two|three"));
    }

    #[test]
    fn gsub_replaces_everything_and_counts() {
        let value = eval("return (string.gsub('hello', 'l', 'L'))");
        assert_eq!(value, Value::string("heLLo"));
        let value = eval("return select(2, string.gsub('hello', 'l', 'L'))");
        assert_eq!(value, Value::Number(2.0));
        let value = eval("return (string.gsub('hello', 'l', 'L', 1))");
        assert_eq!(value, Value::string("heLlo"));
    }

    #[test]
    fn gsub_accepts_a_function_and_a_table() {
        let value = eval("return (string.gsub('abc', '%a', function(c) return c:upper() end))");
        assert_eq!(value, Value::string("ABC"));
        let value = eval("return (string.gsub('$name', '%$(%w+)', {name = 'Ada'}))");
        assert_eq!(value, Value::string("Ada"));
    }

    #[test]
    fn gsub_handles_zero_width_matches() {
        let value = eval("return (string.gsub('abc', '', '-'))");
        assert_eq!(value, Value::string("-a-b-c-"));
    }

    #[test]
    fn format_handles_every_conversion() {
        assert_eq!(eval("return string.format('%d', 42)"), Value::string("42"));
        assert_eq!(
            eval("return string.format('%5d', 42)"),
            Value::string("   42")
        );
        assert_eq!(
            eval("return string.format('%-5d|', 42)"),
            Value::string("42   |")
        );
        assert_eq!(
            eval("return string.format('%05d', 42)"),
            Value::string("00042")
        );
        assert_eq!(eval("return string.format('%x', 255)"), Value::string("ff"));
        assert_eq!(eval("return string.format('%X', 255)"), Value::string("FF"));
        assert_eq!(
            eval("return string.format('%#x', 255)"),
            Value::string("0xff")
        );
        assert_eq!(eval("return string.format('%o', 8)"), Value::string("10"));
        assert_eq!(
            eval("return string.format('%.2f', 3.14159)"),
            Value::string("3.14")
        );
        assert_eq!(
            eval("return string.format('%s!', 'hi')"),
            Value::string("hi!")
        );
        assert_eq!(
            eval("return string.format('%10s|', 'hi')"),
            Value::string("        hi|")
        );
        assert_eq!(
            eval("return string.format('%q', 'a\"b')"),
            Value::string("\"a\\\"b\"")
        );
        assert_eq!(eval("return string.format('%%')"), Value::string("%"));
        assert_eq!(eval("return string.format('%d', 3.9)"), Value::string("3"));
    }

    #[test]
    fn format_reports_bad_specifications() {
        let mut interp = Interp::new();
        assert!(interp.eval("return string.format('%z', 1)").is_err());
    }

    #[test]
    fn string_methods_are_reachable_through_the_metatable() {
        assert_eq!(eval("return ('abc'):upper()"), Value::string("ABC"));
        assert_eq!(eval("return ('a,b'):gsub(',', ';')"), Value::string("a;b"));
    }
}

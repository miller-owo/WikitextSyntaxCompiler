use super::expand::{Argument, Expander, Frame};
use std::fmt::Write as _;
pub struct FunctionContext<'a, 'b> {
    expander: &'a mut Expander<'b>,
    frame: &'a Frame,
    depth: usize,
}

impl<'a, 'b> FunctionContext<'a, 'b> {
    pub fn expand(&mut self, value: &str) -> String {
        let mut text = value.to_string();
        self.expander
            .expand_in_place(&mut text, self.frame, self.depth + 1);
        text
    }

    pub fn expand_trimmed(&mut self, value: &str) -> String {
        self.expand(value).trim().to_string()
    }

    pub fn arg(&mut self, arguments: &[Argument], index: usize) -> String {
        match arguments.get(index) {
            Some(argument) => self.expand(&argument.value),
            None => String::new(),
        }
    }

    pub fn arg_trimmed(&mut self, arguments: &[Argument], index: usize) -> String {
        match arguments.get(index) {
            Some(argument) => self.expand_trimmed(&argument.value),
            None => String::new(),
        }
    }

    pub fn options(&self) -> &crate::options::Options {
        self.expander.options()
    }
}

pub(crate) fn call(
    expander: &mut Expander<'_>,
    name: &str,
    arguments: &[Argument],
    frame: &Frame,
    depth: usize,
) -> String {
    let mut context = FunctionContext {
        expander,
        frame,
        depth,
    };
    let lower = name.trim().to_ascii_lowercase();
    match lower.as_str() {
        "if" => fn_if(&mut context, arguments),
        "ifeq" => fn_ifeq(&mut context, arguments),
        "ifexist" => fn_ifexist(&mut context, arguments),
        "iferror" => fn_iferror(&mut context, arguments),
        "ifexpr" => fn_ifexpr(&mut context, arguments),
        "switch" => fn_switch(&mut context, arguments),
        "expr" => fn_expr(&mut context, arguments),
        "titleparts" => fn_titleparts(&mut context, arguments),
        "rel2abs" => fn_rel2abs(&mut context, arguments),
        "time" => fn_time(&mut context, arguments),
        "timel" => fn_timel(&mut context, arguments),

        "len" => fn_len(&mut context, arguments),
        "pos" => fn_pos(&mut context, arguments),
        "rpos" => fn_rpos(&mut context, arguments),
        "sub" => fn_sub(&mut context, arguments),
        "replace" => fn_replace(&mut context, arguments),
        "explode" => fn_explode(&mut context, arguments),
        "urldecode" => fn_urldecode(&mut context, arguments),
        "urlencode" => fn_urlencode(&mut context, arguments),
        "anchorencode" => fn_anchorencode(&mut context, arguments),
        "padleft" => fn_padleft(&mut context, arguments),
        "padright" => fn_padright(&mut context, arguments),
        "formatnum" => fn_formatnum(&mut context, arguments),
        "lc" => Ok(fn_lc(&mut context, arguments)),
        "uc" => Ok(fn_uc(&mut context, arguments)),
        "lcfirst" => Ok(fn_lcfirst(&mut context, arguments)),
        "ucfirst" => Ok(fn_ucfirst(&mut context, arguments)),
        "format" => Ok(fn_format(&mut context, arguments)),

        "tag" => fn_tag(&mut context, arguments),
        "invoke" => Ok(String::new()),
        _ => Err(unknown_function(name)),
    }
    .unwrap_or_else(|message| message)
}

type FunctionResult = Result<String, String>;

fn unknown_function(name: &str) -> String {
    format!(
        "<strong class=\"error\">Unrecognised parser function \u{201c}{}\u{201d}</strong>",
        crate::serialize::escape_text(name)
    )
}

fn fn_if(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let test = context.arg_trimmed(arguments, 0);
    let branch = if test.is_empty() { 2 } else { 1 };
    if arguments.len() > branch {
        Ok(context.arg(arguments, branch))
    } else {
        Ok(String::new())
    }
}

fn fn_ifeq(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let left = context.arg_trimmed(arguments, 0);
    let right = context.arg_trimmed(arguments, 1);
    let equal = match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => left == right,
    };
    let branch = if equal { 2 } else { 3 };
    if arguments.len() > branch {
        Ok(context.arg(arguments, branch))
    } else {
        Ok(String::new())
    }
}

fn fn_ifexist(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let _page = context.arg_trimmed(arguments, 0);

    let exists = context
        .options()
        .resolver
        .as_ref()
        .map(|resolver| resolver.exists(&_page))
        .unwrap_or(false);
    let branch = if exists { 1 } else { 2 };
    if arguments.len() > branch {
        Ok(context.arg(arguments, branch))
    } else {
        Ok(String::new())
    }
}

fn is_error_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("<strong class=\"error\"")
        || trimmed.starts_with("<span class=\"error\"")
        || trimmed.starts_with("<span class=\"mw-ext-cite-error\"")
}

fn fn_iferror(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let value = context.arg(arguments, 0);
    if is_error_text(&value) {
        if arguments.len() > 1 {
            Ok(context.arg(arguments, 1))
        } else {
            Ok(String::new())
        }
    } else if arguments.len() > 2 {
        Ok(context.arg(arguments, 2))
    } else {
        Ok(value)
    }
}

fn fn_ifexpr(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let expression = context.arg_trimmed(arguments, 0);
    let value = match ExprParser::new(&expression).parse() {
        Ok(value) => value,
        Err(message) => return Ok(error_box(&message)),
    };
    let truthy = value.truthy().map_err(|message| error_box(&message))?;
    let branch = if truthy { 1 } else { 2 };
    if arguments.len() > branch {
        Ok(context.arg(arguments, branch))
    } else {
        Ok(String::new())
    }
}

fn fn_switch(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let value = context.arg_trimmed(arguments, 0);

    let mut index = 1usize;
    let mut default: Option<usize> = None;
    let mut pending_labels: Vec<String> = Vec::new();
    while index < arguments.len() {
        let argument = &arguments[index];
        let key = match argument.name.is_empty() {
            true => argument.value.trim().to_string(),
            false => argument.name.trim().to_string(),
        };
        if key == "#default" {
            default = Some(index);
            index += 1;
            continue;
        }

        if argument.name.is_empty() && !argument.value.contains('=') {
            pending_labels.push(key);
            index += 1;
            continue;
        }
        let result = match argument.name.is_empty() {
            true => argument
                .value
                .split_once('=')
                .map(|(_, right)| right.to_string())
                .unwrap_or_default(),
            false => argument.value.clone(),
        };
        if key == value || pending_labels.contains(&value) {
            return Ok(context.expand(&result));
        }
        pending_labels.clear();
        index += 1;
    }
    if let Some(default) = default {
        return Ok(context.arg(arguments, default));
    }

    Ok(value)
}

fn fn_expr(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let expression = context.arg_trimmed(arguments, 0);
    match ExprParser::new(&expression).parse() {
        Ok(value) => Ok(value.render()),
        Err(message) => Ok(error_box(&message)),
    }
}

fn error_box(message: &str) -> String {
    format!(
        "<strong class=\"error\">{}</strong>",
        crate::serialize::escape_text(message)
    )
}

fn fn_len(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg_trimmed(arguments, 0);
    Ok(text.chars().count().to_string())
}

fn fn_pos(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let haystack = context.arg_trimmed(arguments, 0);
    let needle = context.arg_trimmed(arguments, 1);
    Ok(match needle {
        _ if needle.is_empty() => String::new(),
        _ => match haystack.find(&needle) {
            Some(byte) => haystack[..byte].chars().count().to_string(),
            None => String::new(),
        },
    })
}

fn fn_rpos(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let haystack = context.arg_trimmed(arguments, 0);
    let needle = context.arg_trimmed(arguments, 1);
    Ok(match needle {
        _ if needle.is_empty() => String::new(),
        _ => match haystack.rfind(&needle) {
            Some(byte) => haystack[..byte].chars().count().to_string(),
            None => String::new(),
        },
    })
}

fn fn_sub(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg_trimmed(arguments, 0);
    let start = context
        .arg_trimmed(arguments, 1)
        .parse::<i64>()
        .unwrap_or(1);
    let length = context
        .arg_trimmed(arguments, 2)
        .parse::<i64>()
        .unwrap_or(i64::MAX);
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len() as i64;

    let begin = if start < 0 { total + start } else { start - 1 };
    if begin >= total {
        return Ok(String::new());
    }
    let begin = begin.max(0) as usize;
    if length <= 0 {
        return Ok(String::new());
    }
    let end = (begin as i64 + length).min(total) as usize;
    Ok(chars[begin..end].iter().collect())
}

fn fn_replace(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg(arguments, 0);
    let from = context.arg(arguments, 1);
    let to = context.arg(arguments, 2);
    if from.is_empty() {
        return Ok(text);
    }
    Ok(text.replace(&from, &to))
}

fn fn_explode(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg(arguments, 0);
    let delimiter = context.arg(arguments, 1);
    let index = context
        .arg_trimmed(arguments, 2)
        .parse::<i64>()
        .unwrap_or(0);
    let parts: Vec<&str> = if delimiter.is_empty() {
        text.split("").filter(|part| !part.is_empty()).collect()
    } else {
        text.split(&delimiter).collect()
    };
    let total = parts.len() as i64;
    let index = if index < 0 { total + index } else { index };
    if index < 0 || index >= total {
        return Ok(String::new());
    }
    Ok(parts[index as usize].to_string())
}

fn fn_urldecode(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg(arguments, 0);
    Ok(percent_decode(text.replace('+', " ").as_str()))
}

fn fn_urlencode(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg(arguments, 0);
    let mode = context.arg_trimmed(arguments, 1).to_ascii_uppercase();
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        let safe = match mode.as_str() {
            "WIKI" => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'!'),
            "PATH" => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'),
            "QUERY" => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'),
            _ => byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'),
        };
        if safe {
            out.push(byte as char);
        } else if mode == "QUERY" && byte == b' ' {
            out.push('+');
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    Ok(out)
}

fn fn_anchorencode(
    context: &mut FunctionContext<'_, '_>,
    arguments: &[Argument],
) -> FunctionResult {
    let text = context.arg_trimmed(arguments, 0);
    Ok(crate::parse::heading_anchor(&text))
}

fn fn_padleft(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg(arguments, 0);
    let width = context
        .arg_trimmed(arguments, 1)
        .parse::<usize>()
        .unwrap_or(0);
    let pad = context.arg(arguments, 2);
    let pad = if pad.is_empty() { "0".to_string() } else { pad };
    Ok(pad_to(text, width, &pad, true))
}

fn fn_padright(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg(arguments, 0);
    let width = context
        .arg_trimmed(arguments, 1)
        .parse::<usize>()
        .unwrap_or(0);
    let pad = context.arg(arguments, 2);
    let pad = if pad.is_empty() { "0".to_string() } else { pad };
    Ok(pad_to(text, width, &pad, false))
}

fn pad_to(text: String, width: usize, pad: &str, left: bool) -> String {
    let current = text.chars().count();
    if current >= width || pad.is_empty() {
        return text;
    }
    let needed = width - current;
    let padding: String = pad.chars().cycle().take(needed).collect();
    if left {
        format!("{padding}{text}")
    } else {
        format!("{text}{padding}")
    }
}

fn fn_formatnum(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let text = context.arg_trimmed(arguments, 0);
    let mode = context.arg_trimmed(arguments, 1).to_ascii_uppercase();
    let Ok(value) = text.parse::<f64>() else {
        return Ok(text);
    };
    if mode == "R" {
        return Ok(text.replace(',', ""));
    }
    let negative = value < 0.0;
    let magnitude = value.abs();
    let rendered = format!("{magnitude}");
    let (integer, fraction) = match rendered.split_once('.') {
        Some((integer, fraction)) => (integer.to_string(), Some(fraction.to_string())),
        None => (rendered, None),
    };
    let mut grouped = String::new();
    for (index, ch) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&grouped);
    if let Some(fraction) = fraction {
        out.push('.');
        out.push_str(&fraction);
    }
    Ok(out)
}

fn fn_lc(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> String {
    context.arg_trimmed(arguments, 0).to_lowercase()
}

fn fn_uc(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> String {
    context.arg_trimmed(arguments, 0).to_uppercase()
}

fn fn_lcfirst(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> String {
    lowercase_first(&context.arg_trimmed(arguments, 0))
}

fn fn_ucfirst(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> String {
    uppercase_first(&context.arg_trimmed(arguments, 0))
}

fn fn_format(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> String {
    let text = context.arg_trimmed(arguments, 0);
    let style = context.arg_trimmed(arguments, 1).to_ascii_lowercase();
    match style.as_str() {
        "urlencode" => fn_urlencode(context, arguments).unwrap_or_default(),
        "anchorencode" => crate::parse::heading_anchor(&text),
        "lc" => text.to_lowercase(),
        "uc" => text.to_uppercase(),
        "lcfirst" => lowercase_first(&text),
        "ucfirst" => uppercase_first(&text),
        "nowiki" => format!("<nowiki>{text}</nowiki>"),
        _ => text,
    }
}

fn lowercase_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn uppercase_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &text[index + 1..index + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn fn_tag(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let name = context.arg_trimmed(arguments, 0);
    if name.is_empty() {
        return Ok(error_box("#tag: missing tag name"));
    }
    let content = context.arg(arguments, 1);
    let mut attributes = String::new();
    for argument in arguments.iter().skip(2) {
        if argument.name.is_empty() {
            continue;
        }
        let name = context.expand_trimmed(&argument.name);
        let value = context.expand_trimmed(&argument.value);
        let _ = write!(
            attributes,
            " {}=\"{}\"",
            name,
            crate::serialize::escape_attribute(&value)
        );
    }
    if content.is_empty() {
        Ok(format!("<{name}{attributes} />"))
    } else {
        Ok(format!("<{name}{attributes}>{content}</{name}>"))
    }
}

fn fn_titleparts(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let title = context.arg_trimmed(arguments, 0);
    let mut parts: Vec<&str> = title.split('/').collect();
    let count = context
        .arg_trimmed(arguments, 1)
        .parse::<i64>()
        .unwrap_or(0);
    let start = context
        .arg_trimmed(arguments, 2)
        .parse::<i64>()
        .unwrap_or(1);

    if parts.is_empty() {
        return Ok(String::new());
    }
    let total = parts.len() as i64;
    if count != 0 {
        let begin = if start < 0 { total + start } else { start - 1 };
        let begin = begin.max(0) as usize;
        let end = if count < 0 {
            (total + count).max(begin as i64) as usize
        } else {
            (begin as i64 + count).min(total) as usize
        };
        parts = parts[begin.min(parts.len())..end.min(parts.len())].to_vec();
    }
    Ok(parts.join("/"))
}

fn fn_rel2abs(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let path = context.arg_trimmed(arguments, 0);
    let base = context.arg_trimmed(arguments, 1);
    if path.starts_with('/') {
        return Ok(path);
    }
    if path.contains("://") {
        return Ok(path);
    }
    let base = if base.is_empty() {
        "/wiki/".to_string()
    } else {
        base
    };
    let base = match path.rsplit_once('/') {
        Some((prefix, _)) => prefix.to_string(),
        None => base,
    };
    Ok(format!("{}/{}", base.trim_end_matches('/'), path))
}

fn fn_time(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let format = context.arg(arguments, 0);
    let spec = context.arg_trimmed(arguments, 1);
    match format_time(&format, &spec) {
        Ok(text) => Ok(text),
        Err(message) => Ok(error_box(&message)),
    }
}

fn fn_timel(context: &mut FunctionContext<'_, '_>, arguments: &[Argument]) -> FunctionResult {
    let format = context.arg(arguments, 0);
    let spec = context.arg_trimmed(arguments, 1);
    match format_time(&format, &spec) {
        Ok(text) => Ok(text),
        Err(message) => Ok(error_box(&message)),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct CivilTime {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    epoch_days: i64,
}

impl CivilTime {
    fn epoch() -> Self {
        CivilTime {
            year: 1970,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            epoch_days: 0,
        }
    }

    fn from_unix(seconds: i64) -> Self {
        let days = seconds.div_euclid(86_400);
        let rem = seconds.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        CivilTime {
            year,
            month,
            day,
            hour: (rem / 3600) as u32,
            minute: ((rem % 3600) / 60) as u32,
            second: (rem % 60) as u32,
            epoch_days: days,
        }
    }

    fn weekday(&self) -> u32 {
        (self.epoch_days + 4).rem_euclid(7) as u32
    }

    fn day_of_year(&self) -> u32 {
        const CUMULATIVE: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        let leap = is_leap_year(self.year) && self.month > 2;
        CUMULATIVE[(self.month - 1) as usize] + self.day + u32::from(leap)
    }

    fn days_in_month(&self) -> u32 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if is_leap_year(self.year) => 29,
            2 => 28,
            _ => 30,
        }
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// Hinnant's public-domain days-to-civil algorithm; `days_from_civil` below inverts it.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn parse_timestamp(spec: &str) -> Result<CivilTime, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("now") {
        return Ok(CivilTime::epoch());
    }

    if let Ok(seconds) = spec.parse::<i64>() {
        return Ok(CivilTime::from_unix(seconds));
    }

    if let Some(rest) = spec.strip_prefix('+') {
        let mut parts = rest.split_whitespace();
        if let (Some(amount), Some(unit)) = (parts.next(), parts.next()) {
            if let Ok(amount) = amount.parse::<i64>() {
                let seconds = match unit.trim_end_matches('s') {
                    "second" => amount,
                    "minute" => amount * 60,
                    "hour" => amount * 3600,
                    "day" => amount * 86_400,
                    "week" => amount * 604_800,
                    _ => return Err(format!("unknown time unit \u{201c}{unit}\u{201d}")),
                };
                return Ok(CivilTime::from_unix(seconds));
            }
        }
        return Err(format!(
            "cannot parse time specification \u{201c}{spec}\u{201d}"
        ));
    }

    let digits: String = spec.chars().filter(|c| c.is_ascii_digit()).collect();
    let separators: String = spec
        .chars()
        .filter(|c| !c.is_ascii_digit() && !c.is_whitespace())
        .collect();
    if digits.len() == 14 && separators.is_empty() {
        let get = |from: usize, to: usize| digits[from..to].parse::<i64>().unwrap_or(0);
        let days = days_from_civil(get(0, 4), get(4, 6) as u32, get(6, 8) as u32);
        return Ok(CivilTime {
            year: get(0, 4),
            month: get(4, 6) as u32,
            day: get(6, 8) as u32,
            hour: get(8, 10) as u32,
            minute: get(10, 12) as u32,
            second: get(12, 14) as u32,
            epoch_days: days,
        });
    }
    Err(format!(
        "cannot parse time specification \u{201c}{spec}\u{201d}"
    ))
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn format_time(format: &str, spec: &str) -> Result<String, String> {
    let time = parse_timestamp(spec)?;
    let mut out = String::with_capacity(format.len());
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            '"' => {
                for next in chars.by_ref() {
                    if next == '"' {
                        break;
                    }
                    out.push(next);
                }
            }
            'Y' => {
                let _ = write!(out, "{}", time.year);
            }
            'y' => {
                let _ = write!(out, "{:02}", time.year.rem_euclid(100));
            }
            'n' => {
                let _ = write!(out, "{}", time.month);
            }
            'm' => {
                let _ = write!(out, "{:02}", time.month);
            }
            'j' => {
                let _ = write!(out, "{}", time.day);
            }
            'd' => {
                let _ = write!(out, "{:02}", time.day);
            }
            'z' => {
                let _ = write!(out, "{}", time.day_of_year() - 1);
            }
            'H' => {
                let _ = write!(out, "{:02}", time.hour);
            }
            'G' => {
                let _ = write!(out, "{}", time.hour);
            }
            'i' => {
                let _ = write!(out, "{:02}", time.minute);
            }
            's' => {
                let _ = write!(out, "{:02}", time.second);
            }
            'U' => {
                let _ = write!(
                    out,
                    "{}",
                    time.epoch_days * 86_400 + time.hour as i64 * 3600
                );
            }
            'N' => {
                let weekday = time.weekday();
                let iso = if weekday == 0 { 7 } else { weekday };
                let _ = write!(out, "{iso}");
            }
            'w' => {
                let _ = write!(out, "{}", time.weekday());
            }
            'D' => out.push_str(short_weekday(time.weekday())),
            'l' => out.push_str(long_weekday(time.weekday())),
            'M' => out.push_str(short_month(time.month)),
            'F' => out.push_str(long_month(time.month)),
            't' => {
                let _ = write!(out, "{}", time.days_in_month());
            }
            'L' => out.push(if is_leap_year(time.year) { '1' } else { '0' }),
            'a' => out.push_str(if time.hour < 12 { "am" } else { "pm" }),
            'A' => out.push_str(if time.hour < 12 { "AM" } else { "PM" }),
            'g' => {
                let hour = time.hour % 12;
                let _ = write!(out, "{}", if hour == 0 { 12 } else { hour });
            }
            'h' => {
                let hour = time.hour % 12;
                let _ = write!(out, "{:02}", if hour == 0 { 12 } else { hour });
            }
            'c' => {
                let _ = write!(
                    out,
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
                    time.year, time.month, time.day, time.hour, time.minute, time.second
                );
            }
            'r' => {
                let _ = write!(
                    out,
                    "{}, {:02} {} {:04} {:02}:{:02}:{:02} +0000",
                    short_weekday(time.weekday()),
                    time.day,
                    short_month(time.month),
                    time.year,
                    time.hour,
                    time.minute,
                    time.second
                );
            }
            'e' | 'T' | 'P' | 'O' | 'Z' => out.push_str("+00:00"),
            other => out.push(other),
        }
    }
    Ok(out)
}

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn long_weekday(index: u32) -> &'static str {
    WEEKDAYS[(index % 7) as usize]
}

fn short_weekday(index: u32) -> &'static str {
    match index % 7 {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        _ => "Sat",
    }
}

fn long_month(index: u32) -> &'static str {
    MONTHS[((index.clamp(1, 12)) - 1) as usize]
}

fn short_month(index: u32) -> &'static str {
    match index.clamp(1, 12) {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Number(f64),

    Text(String),
}

impl Value {
    pub fn render(&self) -> String {
        match self {
            Value::Text(text) => text.clone(),
            Value::Number(value) => render_number(*value),
        }
    }

    fn as_number(&self) -> Result<f64, String> {
        match self {
            Value::Number(value) => Ok(*value),
            Value::Text(text) => text
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("expected a number, got \u{201c}{text}\u{201d}")),
        }
    }

    pub fn truthy(&self) -> Result<bool, String> {
        match self {
            Value::Text(text) => Ok(!text.is_empty() && text != "0"),
            Value::Number(value) => Ok(*value != 0.0),
        }
    }
}

pub fn render_number(value: f64) -> String {
    if value.is_nan() {
        return "NAN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "INF" } else { "-INF" }.to_string();
    }
    if value == value.trunc() && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    let mut text = format!("{value:.14}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

pub struct ExprParser<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> ExprParser<'a> {
    pub fn new(input: &'a str) -> Self {
        ExprParser { input, position: 0 }
    }

    pub fn parse(mut self) -> Result<Value, String> {
        self.skip_whitespace();
        if self.input.trim().is_empty() {
            return Err("expression is empty".to_string());
        }
        let value = self.parse_or()?;
        self.skip_whitespace();
        if self.position < self.input.len() {
            return Err(format!(
                "unexpected text \u{201c}{}\u{201d}",
                self.input[self.position..].trim()
            ));
        }
        Ok(value)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.position += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn eat_word(&mut self, word: &str) -> bool {
        let rest = &self.input[self.position..];
        if rest.len() < word.len() {
            return false;
        }
        if !rest[..word.len()].eq_ignore_ascii_case(word) {
            return false;
        }

        if let Some(next) = rest[word.len()..].chars().next() {
            if next.is_alphanumeric() {
                return false;
            }
        }
        self.position += word.len();
        true
    }

    fn eat_symbol(&mut self, symbol: &str) -> bool {
        if self.input[self.position..].starts_with(symbol) {
            self.position += symbol.len();
            return true;
        }
        false
    }

    fn parse_or(&mut self) -> Result<Value, String> {
        let mut left = self.parse_and()?;
        loop {
            self.skip_whitespace();
            if !self.eat_word("or") {
                return Ok(left);
            }
            let right = self.parse_and()?;
            left = Value::Number(bool_to_number(left.truthy()? || right.truthy()?));
        }
    }

    fn parse_and(&mut self) -> Result<Value, String> {
        let mut left = self.parse_comparison()?;
        loop {
            self.skip_whitespace();
            if !self.eat_word("and") {
                return Ok(left);
            }
            let right = self.parse_comparison()?;
            left = Value::Number(bool_to_number(left.truthy()? && right.truthy()?));
        }
    }

    fn parse_comparison(&mut self) -> Result<Value, String> {
        let left = self.parse_additive()?;
        self.skip_whitespace();
        let operator = ["!=", "<>", "<=", ">=", "=", "<", ">"]
            .iter()
            .find(|op| self.input[self.position..].starts_with(**op))
            .copied();
        let Some(operator) = operator else {
            return Ok(left);
        };
        self.position += operator.len();
        let right = self.parse_additive()?;
        let a = left.as_number()?;
        let b = right.as_number()?;
        let result = match operator {
            "!=" | "<>" => a != b,
            "<=" => a <= b,
            ">=" => a >= b,
            "=" => a == b,
            "<" => a < b,
            _ => a > b,
        };
        Ok(Value::Number(bool_to_number(result)))
    }

    fn parse_additive(&mut self) -> Result<Value, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            self.skip_whitespace();
            if self.eat_symbol("+") {
                let right = self.parse_multiplicative()?;
                left = Value::Number(left.as_number()? + right.as_number()?);
            } else if self.eat_symbol("-") {
                let right = self.parse_multiplicative()?;
                left = Value::Number(left.as_number()? - right.as_number()?);
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_multiplicative(&mut self) -> Result<Value, String> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            if self.eat_symbol("*") {
                let right = self.parse_unary()?;
                left = Value::Number(left.as_number()? * right.as_number()?);
            } else if self.eat_symbol("/") {
                let right = self.parse_unary()?;
                let divisor = right.as_number()?;
                if divisor == 0.0 {
                    return Err("division by zero".to_string());
                }
                left = Value::Number(left.as_number()? / divisor);
            } else if self.eat_word("div") {
                let right = self.parse_unary()?;
                let divisor = right.as_number()?;
                if divisor == 0.0 {
                    return Err("division by zero".to_string());
                }
                left = Value::Number((left.as_number()? / divisor).floor());
            } else if self.eat_word("mod") {
                let right = self.parse_unary()?;
                let divisor = right.as_number()?;
                if divisor == 0.0 {
                    return Err("division by zero".to_string());
                }
                left = Value::Number(left.as_number()? % divisor);
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<Value, String> {
        self.skip_whitespace();
        if self.eat_symbol("-") {
            let value = self.parse_unary()?;
            return Ok(Value::Number(-value.as_number()?));
        }
        if self.eat_symbol("+") {
            return self.parse_unary();
        }
        if self.eat_word("not") {
            let value = self.parse_unary()?;
            return Ok(Value::Number(bool_to_number(!value.truthy()?)));
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<Value, String> {
        let base = self.parse_primary()?;
        self.skip_whitespace();
        if self.eat_symbol("^") {
            let exponent = self.parse_unary()?;
            return Ok(Value::Number(base.as_number()?.powf(exponent.as_number()?)));
        }
        Ok(base)
    }

    fn parse_primary(&mut self) -> Result<Value, String> {
        self.skip_whitespace();
        if self.eat_symbol("(") {
            let value = self.parse_or()?;
            self.skip_whitespace();
            if !self.eat_symbol(")") {
                return Err("unclosed parenthesis".to_string());
            }
            return Ok(value);
        }
        let rest = &self.input[self.position..];

        let number_len = scan_number(rest);
        if number_len > 0 {
            let text = &rest[..number_len];
            self.position += number_len;
            return match text.parse::<f64>() {
                Ok(value) => Ok(Value::Number(value)),
                Err(_) => Err(format!("cannot parse number \u{201c}{text}\u{201d}")),
            };
        }

        let constants = [
            ("e", std::f64::consts::E),
            ("pi", std::f64::consts::PI),
            ("\u{221e}", f64::INFINITY),
        ];
        for (name, value) in constants {
            if self.eat_word(name)
                || self.input[self.position..].starts_with(name) && {
                    self.position += name.len();
                    true
                }
            {
                return Ok(Value::Number(value));
            }
        }

        let word_len = rest
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .map(char::len_utf8)
            .sum::<usize>();
        if word_len > 0 {
            return Err(format!(
                "unknown word \u{201c}{}\u{201d}",
                &rest[..word_len]
            ));
        }
        Err("unexpected end of expression".to_string())
    }
}

fn bool_to_number(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

fn scan_number(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
    }
    if index == 0 {
        return 0;
    }
    if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
        let mut probe = index + 1;
        if probe < bytes.len() && (bytes[probe] == b'+' || bytes[probe] == b'-') {
            probe += 1;
        }
        let digits_start = probe;
        while probe < bytes.len() && bytes[probe].is_ascii_digit() {
            probe += 1;
        }
        if probe > digits_start {
            index = probe;
        }
    }
    index
}

pub(crate) fn magic_word(expander: &mut Expander<'_>, name: &str, frame: &Frame) -> Option<String> {
    let options = expander.options();
    let title = options.title.as_deref().unwrap_or(frame.title.as_str());
    let (page, namespace) = split_title(title);
    let upper = name.trim().to_ascii_uppercase();
    let value = match upper.as_str() {
        "PAGENAME" | "PAGENAMEE" => page.to_string(),
        "FULLPAGENAME" | "FULLPAGENAMEE" => title.to_string(),
        "BASEPAGENAME" => page
            .rsplit_once('/')
            .map(|(base, _)| base)
            .unwrap_or(page)
            .to_string(),
        "SUBPAGENAME" => page
            .rsplit_once('/')
            .map(|(_, sub)| sub)
            .unwrap_or(page)
            .to_string(),
        "ROOTPAGENAME" => page.split('/').next().unwrap_or(page).to_string(),
        "NAMESPACE" | "NAMESPACEE" => namespace.to_string(),
        "NAMESPACENUMBER" => namespace_number(namespace).to_string(),
        "SUBJECTSPACE" | "ARTICLESPACE" => namespace.to_string(),
        "TALKSPACE" => {
            if namespace.is_empty() {
                "Talk".to_string()
            } else {
                format!("{namespace} talk")
            }
        }
        "TALKPAGENAME" => {
            if namespace.is_empty() {
                format!("Talk:{page}")
            } else {
                title.to_string()
            }
        }
        "SUBJECTPAGENAME" | "ARTICLEPAGENAME" => title.to_string(),
        "NUMBEROFARTICLES" => "0".to_string(),
        "NUMBEROFPAGES"
        | "NUMBEROFFILES"
        | "NUMBEROFEDITS"
        | "NUMBEROFUSERS"
        | "NUMBEROFACTIVEUSERS"
        | "NUMBEROFADMINS"
        | "NUMBERINGROUP" => "0".to_string(),

        "DISPLAYTITLE" | "DEFAULTSORT" | "DEFAULTCATEGORYSORT" => String::new(),
        "SERVER" | "SERVERNAME" | "SCRIPTPATH" | "STYLEPATH" | "SITENAME" | "CURRENTVERSION"
        | "CONTENTLANGUAGE" | "DIRMARK" | "DIRECTIONMARK" => server_magic(&upper, options),

        "CURRENTYEAR"
        | "CURRENTMONTH"
        | "CURRENTMONTH1"
        | "CURRENTMONTHNAME"
        | "CURRENTMONTHNAMEGEN"
        | "CURRENTMONTHABBREV"
        | "CURRENTDAY"
        | "CURRENTDAY2"
        | "CURRENTDOW"
        | "CURRENTDAYNAME"
        | "CURRENTTIME"
        | "CURRENTHOUR"
        | "CURRENTWEEK"
        | "CURRENTTIMESTAMP"
        | "CURRENTISOYEAR" => current_date_magic(&upper),
        "LOCALYEAR" | "LOCALMONTH" | "LOCALMONTH1" | "LOCALMONTHNAME" | "LOCALMONTHNAMEGEN"
        | "LOCALMONTHABBREV" | "LOCALDAY" | "LOCALDAY2" | "LOCALDOW" | "LOCALDAYNAME"
        | "LOCALTIME" | "LOCALHOUR" | "LOCALWEEK" | "LOCALTIMESTAMP" => {
            current_date_magic(&upper.replacen("LOCAL", "CURRENT", 1))
        }

        "REVISIONID" | "REVISIONDAY" | "REVISIONDAY2" | "REVISIONMONTH" | "REVISIONMONTH1"
        | "REVISIONYEAR" | "REVISIONTIMESTAMP" | "REVISIONUSER" | "REVISIONSIZE" => "0".to_string(),
        "!" => "|".to_string(),
        "=" => "=".to_string(),
        _ => return None,
    };
    Some(value)
}

fn server_magic(name: &str, options: &crate::options::Options) -> String {
    let server = options.server_url.as_deref().unwrap_or("//localhost");
    match name {
        "SERVER" => server.to_string(),
        "SERVERNAME" => server
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("//")
            .split('/')
            .next()
            .unwrap_or("localhost")
            .to_string(),
        "SCRIPTPATH" => "/w".to_string(),
        "STYLEPATH" => "/w/skins".to_string(),
        "SITENAME" => "MediaWiki".to_string(),
        "CURRENTVERSION" => "1.43.0".to_string(),
        "CONTENTLANGUAGE" => options
            .content_language
            .clone()
            .unwrap_or_else(|| options.language.clone()),
        "DIRMARK" => "\u{200E}".to_string(),
        "DIRECTIONMARK" => "ltr".to_string(),
        _ => String::new(),
    }
}

fn split_title(title: &str) -> (&str, &str) {
    match title.split_once(':') {
        Some((namespace, page)) => (page, namespace),
        None => (title, ""),
    }
}

fn namespace_number(namespace: &str) -> i32 {
    match namespace.to_ascii_lowercase().as_str() {
        "" => 0,
        "talk" => 1,
        "user" => 2,
        "user talk" => 3,
        "project" => 4,
        "project talk" => 5,
        "file" | "image" => 6,
        "file talk" | "image talk" => 7,
        "mediawiki" => 8,
        "mediawiki talk" => 9,
        "template" => 10,
        "template talk" => 11,
        "help" => 12,
        "help talk" => 13,
        "category" => 14,
        "category talk" => 15,
        "media" => -2,
        "special" => -1,
        "module" => 828,
        "module talk" => 829,
        _ => 0,
    }
}

fn current_date_magic(name: &str) -> String {
    let time = CivilTime::epoch();
    match name {
        "CURRENTYEAR" | "CURRENTISOYEAR" => time.year.to_string(),
        "CURRENTMONTH" => format!("{:02}", time.month),
        "CURRENTMONTH1" => time.month.to_string(),
        "CURRENTMONTHNAME" | "CURRENTMONTHNAMEGEN" => long_month(time.month).to_string(),
        "CURRENTMONTHABBREV" => short_month(time.month).to_string(),
        "CURRENTDAY" => time.day.to_string(),
        "CURRENTDAY2" => format!("{:02}", time.day),
        "CURRENTDOW" => time.weekday().to_string(),
        "CURRENTDAYNAME" => long_weekday(time.weekday()).to_string(),
        "CURRENTTIME" => format!("{:02}:{:02}", time.hour, time.minute),
        "CURRENTHOUR" => format!("{:02}", time.hour),
        "CURRENTWEEK" => ((time.day_of_year() - 1) / 7 + 1).to_string(),
        "CURRENTTIMESTAMP" => format!(
            "{:04}{:02}{:02}{:02}{:02}{:02}",
            time.year, time.month, time.day, time.hour, time.minute, time.second
        ),
        _ => String::new(),
    }
}

pub(crate) fn magic_word_with_argument(
    expander: &mut Expander<'_>,
    name: &str,
    argument: &str,
    frame: &Frame,
) -> Option<String> {
    let upper = name.trim().to_ascii_uppercase();
    match upper.as_str() {
        "DEFAULTSORT" | "DEFAULTCATEGORYSORT" => {
            let mut value = argument.to_string();
            expander.expand_in_place(&mut value, frame, 1);
            expander.record_default_sort(value.trim().to_string());

            Some(String::new())
        }
        "DISPLAYTITLE" => {
            let mut value = argument.to_string();
            expander.expand_in_place(&mut value, frame, 1);
            expander.record_display_title(value.trim().to_string());
            Some(String::new())
        }
        "INT" | "MSG" | "MSGWIKI" => Some(format!("<{argument}>")),
        "FORMATNUM" => {
            let mut value = argument.to_string();
            expander.expand_in_place(&mut value, frame, 1);
            Some(value)
        }
        "PADLEFT" | "PADRIGHT" | "URLENCODE" | "ANCHORENCODE" | "LC" | "UC" | "LCFIRST"
        | "UCFIRST" | "FORMAT" | "GRAMMAR" | "GENDER" | "PLURAL" => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn evaluate(expression: &str) -> String {
        match ExprParser::new(expression).parse() {
            Ok(value) => value.render(),
            Err(message) => format!("ERR: {message}"),
        }
    }

    fn expand(source: &str) -> String {
        let options = crate::options::Options::default();
        crate::preprocess::Preprocessor::new(&options).run(source)
    }

    #[test]
    fn iferror_picks_the_branch_by_whether_the_input_errored() {
        assert_eq!(expand("{{#iferror:{{#expr:1/0}}|bad|good}}"), "bad");
        assert_eq!(expand("{{#iferror:{{#expr:1/0}}|bad}}"), "bad");
        assert_eq!(expand("{{#iferror:{{#expr:1/0}}}}"), "");
        assert_eq!(expand("{{#iferror:fine|bad|good}}"), "good");
        assert_eq!(expand("{{#iferror:fine|bad}}"), "fine");
        assert_eq!(expand("{{#iferror:fine}}"), "fine");
    }

    #[test]
    fn evaluates_arithmetic() {
        assert_eq!(evaluate("1+1"), "2");
        assert_eq!(evaluate("2*3"), "6");
        assert_eq!(evaluate("7/2"), "3.5");
        assert_eq!(evaluate("7 div 2"), "3");
        assert_eq!(evaluate("7 mod 2"), "1");
        assert_eq!(evaluate("2^10"), "1024");
    }

    #[test]
    fn respects_precedence() {
        assert_eq!(evaluate("2+3*4"), "14");
        assert_eq!(evaluate("(2+3)*4"), "20");
        assert_eq!(evaluate("-3^2"), "-9");
    }

    #[test]
    fn evaluates_comparisons_and_booleans() {
        assert_eq!(evaluate("1=1"), "1");
        assert_eq!(evaluate("1=2"), "0");
        assert_eq!(evaluate("1<2 and 3>2"), "1");
        assert_eq!(evaluate("1>2 or 3>2"), "1");
        assert_eq!(evaluate("not 0"), "1");
    }

    #[test]
    fn reports_errors() {
        assert!(evaluate("1/0").starts_with("ERR"));
        assert!(evaluate("1+").starts_with("ERR"));
        assert!(evaluate("(1+2").starts_with("ERR"));
        assert!(evaluate("foo").starts_with("ERR"));
    }

    #[test]
    fn renders_numbers_like_mediawiki() {
        assert_eq!(render_number(2.0), "2");
        assert_eq!(render_number(3.5), "3.5");
        assert_eq!(render_number(1.0 / 3.0), "0.33333333333333");
        assert_eq!(render_number(f64::INFINITY), "INF");
    }

    #[test]
    fn formats_time_from_epoch() {
        assert_eq!(format_time("Y-m-d", "").unwrap(), "1970-01-01");
        assert_eq!(format_time("Y-m-d", "0").unwrap(), "1970-01-01");
        assert_eq!(format_time("Y-m-d", "86400").unwrap(), "1970-01-02");
        assert_eq!(format_time("j F Y", "86400").unwrap(), "2 January 1970");
        assert_eq!(format_time("l", "0").unwrap(), "Thursday");
    }

    #[test]
    fn converts_between_days_and_civil_dates() {
        for days in [-1000i64, -1, 0, 1, 19_000, 100_000] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days, "days={days}");
        }
    }

    #[test]
    fn scans_numbers() {
        assert_eq!(scan_number("123"), 3);
        assert_eq!(scan_number("1.5"), 3);
        assert_eq!(scan_number("1e3"), 3);
        assert_eq!(scan_number("1e-3"), 4);
        assert_eq!(scan_number("abc"), 0);
    }

    #[test]
    fn pads_strings() {
        assert_eq!(pad_to("7".to_string(), 3, "0", true), "007");
        assert_eq!(pad_to("7".to_string(), 3, "ab", false), "7ab");
        assert_eq!(pad_to("abcdef".to_string(), 3, "0", true), "abcdef");
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("%E4%B8%AD"), "\u{4e2d}");
        assert_eq!(percent_decode("100%"), "100%");
    }
}

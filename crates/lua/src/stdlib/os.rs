use crate::error::LuaResult;
use crate::interp::Interp;
use crate::stdlib::{check_string, function_table, optional_integer};
use crate::value::{TableRef, Value};
use std::time::{SystemTime, UNIX_EPOCH};
pub fn library() -> TableRef {
    function_table([
        ("clock", 0, lua_clock as crate::value::NativeFunction),
        ("date", 1, lua_date),
        ("difftime", 2, lua_difftime),
        ("time", 1, lua_time),
    ])
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

fn lua_clock(_interp: &mut Interp, _arguments: &[Value]) -> LuaResult<Vec<Value>> {
    Ok(vec![Value::Number(now_seconds())])
}

fn lua_time(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let Some(Value::Table(table)) = arguments.first() else {
        return Ok(vec![Value::Number(now_seconds().floor())]);
    };
    let field = |name: &str, default: i64| -> i64 {
        match table.get(&Value::string(name)) {
            Value::Number(number) => number as i64,
            _ => default,
        }
    };
    let year = field("year", 1970);
    let month = field("month", 1);
    let day = field("day", 1);
    let hour = field("hour", 12);
    let minute = field("min", 0);
    let second = field("sec", 0);
    let _ = interp;
    let days = days_from_civil(year, month as u32, day as u32);
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second;
    Ok(vec![Value::Number(seconds as f64)])
}

fn lua_difftime(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let a = crate::stdlib::check_number(interp, arguments, 0)?;
    let b = crate::stdlib::check_number(interp, arguments, 1)?;
    Ok(vec![Value::Number(a - b)])
}

fn lua_date(interp: &mut Interp, arguments: &[Value]) -> LuaResult<Vec<Value>> {
    let format = match arguments.first() {
        None | Some(Value::Nil) => "%c".to_string(),
        Some(_) => check_string(interp, arguments, 0)?,
    };
    let timestamp = match arguments.get(1) {
        None | Some(Value::Nil) => now_seconds(),
        Some(_) => crate::stdlib::check_number(interp, arguments, 1)?,
    };

    let format = format.strip_prefix('!').unwrap_or(&format).to_string();
    let seconds = timestamp.floor() as i64;
    let days = seconds.div_euclid(86_400);
    let remainder = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let fields = Fields {
        year,
        month,
        day,
        hour: (remainder / 3600) as u32,
        minute: ((remainder % 3600) / 60) as u32,
        second: (remainder % 60) as u32,
        weekday: (days + 4).rem_euclid(7) as u32,
    };
    if format == "*t" || format == "!*t" {
        let table = TableRef::new();
        table.set(Value::from("year"), Value::Number(fields.year as f64));
        table.set(Value::from("month"), Value::Number(fields.month as f64));
        table.set(Value::from("day"), Value::Number(fields.day as f64));
        table.set(Value::from("hour"), Value::Number(fields.hour as f64));
        table.set(Value::from("min"), Value::Number(fields.minute as f64));
        table.set(Value::from("sec"), Value::Number(fields.second as f64));
        table.set(
            Value::from("wday"),
            Value::Number((fields.weekday + 1) as f64),
        );
        table.set(
            Value::from("yday"),
            Value::Number(fields.day_of_year() as f64),
        );
        table.set(Value::from("isdst"), Value::Boolean(false));
        return Ok(vec![Value::Table(table)]);
    }
    Ok(vec![Value::string(fields.format(&format))])
}

struct Fields {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    weekday: u32,
}

impl Fields {
    fn day_of_year(&self) -> u32 {
        const CUMULATIVE: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        let leap = is_leap(self.year) && self.month > 2;
        CUMULATIVE[(self.month - 1) as usize] + self.day + u32::from(leap)
    }

    fn format(&self, format: &str) -> String {
        let mut out = String::with_capacity(format.len() + 16);
        let mut chars = format.chars();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                out.push(ch);
                continue;
            }
            match chars.next() {
                Some('Y') => out.push_str(&self.year.to_string()),
                Some('y') => out.push_str(&format!("{:02}", self.year.rem_euclid(100))),
                Some('m') => out.push_str(&format!("{:02}", self.month)),
                Some('d') => out.push_str(&format!("{:02}", self.day)),
                Some('e') => out.push_str(&format!("{:2}", self.day)),
                Some('H') => out.push_str(&format!("{:02}", self.hour)),
                Some('I') => {
                    let hour = self.hour % 12;
                    out.push_str(&format!("{:02}", if hour == 0 { 12 } else { hour }));
                }
                Some('M') => out.push_str(&format!("{:02}", self.minute)),
                Some('S') => out.push_str(&format!("{:02}", self.second)),
                Some('p') => out.push_str(if self.hour < 12 { "AM" } else { "PM" }),
                Some('A') => out.push_str(long_weekday(self.weekday)),
                Some('a') => out.push_str(short_weekday(self.weekday)),
                Some('B') => out.push_str(long_month(self.month)),
                Some('b') | Some('h') => out.push_str(short_month(self.month)),
                Some('j') => out.push_str(&format!("{:03}", self.day_of_year())),
                Some('w') => out.push_str(&self.weekday.to_string()),
                Some('Z') => out.push_str("UTC"),
                Some('c') => out.push_str(&format!(
                    "{} {} {:2} {:02}:{:02}:{:02} {}",
                    short_weekday(self.weekday),
                    short_month(self.month),
                    self.day,
                    self.hour,
                    self.minute,
                    self.second,
                    self.year
                )),
                Some('x') => out.push_str(&format!(
                    "{:02}/{:02}/{:02}",
                    self.month,
                    self.day,
                    self.year.rem_euclid(100)
                )),
                Some('X') => out.push_str(&format!(
                    "{:02}:{:02}:{:02}",
                    self.hour, self.minute, self.second
                )),
                Some('%') => out.push('%'),
                Some(other) => {
                    out.push('%');
                    out.push(other);
                }
                None => out.push('%'),
            }
        }
        out
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { y + 1 } else { y }, month, day)
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
    MONTHS[(index.clamp(1, 12) - 1) as usize]
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

#[allow(dead_code)]
fn _optional_integer_reference(interp: &Interp, arguments: &[Value]) -> LuaResult<i64> {
    optional_integer(interp, arguments, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_a_known_timestamp() {
        let mut interp = Interp::new();
        let value = interp.eval("return os.date('%Y-%m-%d', 0)").unwrap();
        assert_eq!(value, Value::string("1970-01-01"));
        let value = interp.eval("return os.date('%H:%M:%S', 3661)").unwrap();
        assert_eq!(value, Value::string("01:01:01"));
    }

    #[test]
    fn returns_a_table_for_star_t() {
        let mut interp = Interp::new();
        let value = interp
            .eval("local t = os.date('*t', 86400) return t.year .. '-' .. t.month .. '-' .. t.day")
            .unwrap();
        assert_eq!(value, Value::string("1970-1-2"));
    }

    #[test]
    fn converts_a_table_back_to_a_timestamp() {
        let mut interp = Interp::new();
        let value = interp
            .eval("return os.time({year = 1970, month = 1, day = 2, hour = 0, min = 0, sec = 0})")
            .unwrap();
        assert_eq!(value, Value::Number(86_400.0));
    }

    #[test]
    fn difftime_subtracts() {
        let mut interp = Interp::new();
        assert_eq!(
            interp.eval("return os.difftime(100, 40)").unwrap(),
            Value::Number(60.0)
        );
    }
}

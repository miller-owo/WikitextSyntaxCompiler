use crate::error::{LuaError, LuaResult};
use crate::interp::Interp;
use crate::value::Value;
const MAX_DEPTH: usize = 200;

#[derive(Clone, PartialEq, Debug)]
pub enum Capture {
    Range(usize, usize),

    Position(usize),
}

#[derive(Clone, Debug)]
pub struct Pattern {
    source: String,
    captures: usize,
}

impl Pattern {
    pub fn new(source: &str) -> LuaResult<Pattern> {
        let mut captures = 0usize;
        let bytes = source.as_bytes();
        let mut index = 0usize;
        let mut depth = 0usize;
        while index < bytes.len() {
            match bytes[index] {
                b'%' => {
                    if source[index + 1..].starts_with('b') {
                        index += 4;
                        continue;
                    }
                    if source[index + 1..].starts_with('f') {
                        index += 3;
                        continue;
                    }
                    index += 2;
                    continue;
                }
                b'[' => {
                    index += 1;
                    if source[index..].starts_with('^') {
                        index += 1;
                    }

                    if source[index..].starts_with(']') {
                        index += 1;
                    }
                    while index < bytes.len() && bytes[index] != b']' {
                        if bytes[index] == b'%' {
                            index += 1;
                        }
                        index += 1;
                    }
                    index += 1;
                    continue;
                }
                b'(' => {
                    if !source[index..].starts_with("()") {
                        captures += 1;
                    }
                    depth += 1;
                    if depth > 32 {
                        return Err(LuaError::runtime("too many captures"));
                    }
                    index += 1;
                    continue;
                }
                _ => index += 1,
            }
        }
        Ok(Pattern {
            source: source.to_string(),
            captures,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn captures(&self) -> usize {
        self.captures
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub captures: Vec<Capture>,
}

struct State<'a> {
    subject: &'a [u8],
    pattern: &'a [u8],
    captures: Vec<Capture>,
    open_captures: Vec<(usize, usize)>,
    depth: usize,
}

impl<'a> State<'a> {
    fn at(&self, index: usize) -> Option<u8> {
        self.subject.get(index).copied()
    }

    fn is_end(&self, index: usize) -> bool {
        index >= self.subject.len()
    }

    fn matches(&mut self, mut pattern: usize, mut subject: usize) -> Option<usize> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            return None;
        }
        let result = loop {
            let Some(item) = self.pattern.get(pattern).copied() else {
                break Some(subject);
            };
            match item {
                b'(' => {
                    if self.pattern.get(pattern + 1) == Some(&b')') {
                        self.captures.push(Capture::Position(subject + 1));
                        let result = self.matches(pattern + 2, subject);
                        if result.is_none() {
                            self.captures.pop();
                        }
                        break result;
                    }

                    // A failed probe must leave no trace, or a later successful attempt
                    // reports a range from the branch that failed.
                    let saved_captures = self.captures.clone();
                    let saved_open = self.open_captures.clone();
                    self.captures.push(Capture::Range(subject, subject));
                    let index = self.captures.len() - 1;
                    self.open_captures.push((index, subject));
                    let result = self.matches(pattern + 1, subject);
                    match result {
                        Some(end) => {
                            self.open_captures.pop();
                            break Some(end);
                        }
                        None => {
                            self.captures = saved_captures;
                            self.open_captures = saved_open;
                            break None;
                        }
                    }
                }
                b')' => {
                    if let Some((index, start)) = self.open_captures.pop() {
                        if let Some(slot) = self.captures.get_mut(index) {
                            *slot = Capture::Range(start, subject);
                        }
                    }
                    pattern += 1;
                }
                b'$' => {
                    if pattern + 1 != self.pattern.len() {
                        break None;
                    }
                    break if self.is_end(subject) {
                        Some(subject)
                    } else {
                        None
                    };
                }
                b'%' if self.pattern.get(pattern + 1) == Some(&b'b') => {
                    let (Some(open), Some(close)) = (
                        self.pattern.get(pattern + 2).copied(),
                        self.pattern.get(pattern + 3).copied(),
                    ) else {
                        break None;
                    };
                    let Some(end) = self.match_balance(subject, open, close) else {
                        break None;
                    };
                    subject = end;
                    pattern += 4;
                }
                b'%' if self.pattern.get(pattern + 1) == Some(&b'f') => {
                    if self.pattern.get(pattern + 2) != Some(&b'[') {
                        break None;
                    }
                    let Some(set_end) = find_class_end(self.pattern, pattern + 2) else {
                        break None;
                    };
                    let class = &self.pattern[pattern + 3..set_end];
                    let negated = class.first() == Some(&b'^');
                    let body = if negated { &class[1..] } else { class };
                    let previous = if subject == 0 {
                        None
                    } else {
                        self.at(subject - 1)
                    };
                    let current = self.at(subject);
                    let previous_in = previous.is_some_and(|byte| class_matches(body, byte));
                    let current_in = current.is_some_and(|byte| class_matches(body, byte));
                    if previous_in || !current_in {
                        break None;
                    }
                    pattern = set_end + 1;
                }
                b'^' if pattern == 0 || pattern == 1 => {
                    pattern += 1;
                }
                _ => {
                    let Some(item_end) = item_end(self.pattern, pattern) else {
                        break None;
                    };
                    let quantifier = self.pattern.get(item_end).copied();
                    let item = &self.pattern[pattern..item_end];
                    match quantifier {
                        Some(b'?') => {
                            let saved = self.captures.clone();
                            if let Some(end) =
                                single_end_captured(item, self.subject, subject, &self.captures)
                            {
                                if let Some(result) = self.matches(item_end + 1, end) {
                                    break Some(result);
                                }
                            }
                            self.captures = saved;
                            pattern = item_end + 1;
                        }
                        Some(b'*') => {
                            break self.expand(item, subject, 0, item_end + 1, true);
                        }
                        Some(b'+') => {
                            let Some(end) =
                                single_end_captured(item, self.subject, subject, &self.captures)
                            else {
                                break None;
                            };
                            break self.expand(item, end, 1, item_end + 1, true);
                        }
                        Some(b'-') => {
                            break self.expand(item, subject, 0, item_end + 1, false);
                        }
                        _ => {
                            let Some(end) =
                                single_end_captured(item, self.subject, subject, &self.captures)
                            else {
                                break None;
                            };
                            subject = end;
                            pattern = item_end;
                        }
                    }
                }
            }
        };
        result
    }

    fn expand(
        &mut self,
        item: &[u8],
        mut subject: usize,
        minimum: usize,
        rest: usize,
        greedy: bool,
    ) -> Option<usize> {
        let mut ends = vec![subject];
        loop {
            match single_end_captured(item, self.subject, subject, &self.captures) {
                Some(end) if end != subject => {
                    subject = end;
                    ends.push(subject);
                }

                _ => break,
            }
        }
        if ends.len() < minimum {
            return None;
        }
        let attempts: Vec<usize> = if greedy {
            ends.iter().rev().copied().collect()
        } else {
            ends.clone()
        };

        for end in attempts {
            let saved_captures = self.captures.clone();
            let saved_open = self.open_captures.clone();
            if let Some(result) = self.matches(rest, end) {
                return Some(result);
            }
            self.captures = saved_captures;
            self.open_captures = saved_open;
        }
        None
    }

    fn match_balance(&self, start: usize, open: u8, close: u8) -> Option<usize> {
        if self.at(start) != Some(open) {
            return None;
        }
        let mut depth = 1i32;
        let mut index = start + 1;
        while let Some(byte) = self.at(index) {
            if byte == close {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            } else if byte == open {
                depth += 1;
            }
            index += 1;
        }
        None
    }
}

fn item_end(pattern: &[u8], start: usize) -> Option<usize> {
    match pattern.get(start).copied()? {
        b'%' => match pattern.get(start + 1).copied()? {
            b'b' => Some(start + 4),
            _ => Some(start + 2),
        },
        b'[' => find_class_end(pattern, start).map(|end| end + 1),
        _ => Some(start + 1),
    }
}

fn single_end_captured(
    item: &[u8],
    subject: &[u8],
    at: usize,
    captures: &[Capture],
) -> Option<usize> {
    let first = *item.first()?;
    match first {
        b'%' if matches!(item.get(1), Some(b'0'..=b'9')) => {
            let index = (*item.get(1)? - b'1') as usize;
            let Capture::Range(start, end) = captures.get(index)?.clone() else {
                return None;
            };
            let text = subject.get(start..end)?;
            if subject.get(at..)?.starts_with(text) {
                Some(at + text.len())
            } else {
                None
            }
        }
        b'%' if item.len() == 2 => class_item_end(item, subject, at),
        b'.' => {
            if at < subject.len() {
                Some(at + 1)
            } else {
                None
            }
        }
        b'%' => {
            let escaped = *item.get(1)?;
            let byte = *subject.get(at)?;
            if class_matches_escape(escaped, byte) {
                Some(at + 1)
            } else {
                None
            }
        }
        b'[' => class_item_end(item, subject, at),
        literal => {
            if subject.get(at) == Some(&literal) {
                Some(at + 1)
            } else {
                None
            }
        }
    }
}

fn class_item_end(item: &[u8], subject: &[u8], at: usize) -> Option<usize> {
    let byte = *subject.get(at)?;
    let matched = if item.first() == Some(&b'%') {
        class_matches_escape(*item.get(1)?, byte)
    } else {
        let body = item.get(1..item.len().checked_sub(1)?)?;
        let negated = body.first() == Some(&b'^');
        let body = if negated { &body[1..] } else { body };
        class_matches(body, byte) != negated
    };
    if matched {
        Some(at + 1)
    } else {
        None
    }
}

fn find_class_end(pattern: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    if pattern.get(index) == Some(&b'^') {
        index += 1;
    }

    if pattern.get(index) == Some(&b']') {
        index += 1;
    }
    while index < pattern.len() {
        match pattern[index] {
            b']' => return Some(index),
            b'%' => index += 2,
            _ => index += 1,
        }
    }
    None
}

fn class_matches(body: &[u8], byte: u8) -> bool {
    let mut index = 0usize;
    let mut found = false;
    while index < body.len() {
        let item = body[index];
        if item == b'%' {
            index += 1;
            let Some(escaped) = body.get(index).copied() else {
                break;
            };
            if class_matches_escape(escaped, byte) {
                found = true;
            }
            index += 1;
            continue;
        }

        if body.get(index + 1) == Some(&b'-') && index + 2 < body.len() {
            let low = item;
            let high = body[index + 2];
            if low <= byte && byte <= high {
                found = true;
            }
            index += 3;
            continue;
        }
        if item == byte {
            found = true;
        }
        index += 1;
    }
    found
}

fn class_matches_escape(escaped: u8, byte: u8) -> bool {
    match escaped {
        b'a' => byte.is_ascii_alphabetic(),
        b'A' => !byte.is_ascii_alphabetic(),
        b'd' => byte.is_ascii_digit(),
        b'D' => !byte.is_ascii_digit(),
        b'l' => byte.is_ascii_lowercase(),
        b'L' => !byte.is_ascii_lowercase(),
        b's' => byte.is_ascii_whitespace(),
        b'S' => !byte.is_ascii_whitespace(),
        b'u' => byte.is_ascii_uppercase(),
        b'U' => !byte.is_ascii_uppercase(),
        b'w' => byte.is_ascii_alphanumeric(),
        b'W' => !byte.is_ascii_alphanumeric(),
        b'x' => byte.is_ascii_hexdigit(),
        b'X' => !byte.is_ascii_hexdigit(),
        b'p' => byte.is_ascii_punctuation(),
        b'P' => !byte.is_ascii_punctuation(),
        b'c' => byte.is_ascii_control(),
        b'C' => !byte.is_ascii_control(),
        other => other == byte,
    }
}

pub struct Matcher<'a> {
    pattern: Pattern,
    subject: &'a str,
}

impl<'a> Matcher<'a> {
    pub fn new(subject: &'a str, pattern: &str) -> LuaResult<Matcher<'a>> {
        Ok(Matcher {
            pattern: Pattern::new(pattern)?,
            subject,
        })
    }

    pub fn find(&self, from: usize) -> Option<Match> {
        let anchored = self.pattern.source.starts_with('^');
        let mut start = from;
        loop {
            if start > self.subject.len() {
                return None;
            }
            let mut state = State {
                subject: self.subject.as_bytes(),
                pattern: self.pattern.source.as_bytes(),
                captures: Vec::new(),
                open_captures: Vec::new(),
                depth: 0,
            };
            let pattern_start = if anchored { 1 } else { 0 };

            if anchored && start != from {
                return None;
            }
            if let Some(end) = state.matches(pattern_start, start) {
                return Some(Match {
                    start,
                    end,
                    captures: state.captures,
                });
            }
            if anchored {
                return None;
            }

            match self.subject[start..].chars().next() {
                Some(ch) => start += ch.len_utf8(),
                None => return None,
            }
        }
    }

    pub fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn subject(&self) -> &str {
        self.subject
    }
}

pub fn captures_to_values(subject: &str, found: &Match) -> Vec<Value> {
    if found.captures.is_empty() {
        return vec![Value::string(&subject[found.start..found.end])];
    }
    found
        .captures
        .iter()
        .map(|capture| match capture {
            Capture::Range(start, end) => Value::string(&subject[*start..*end]),
            Capture::Position(index) => Value::Number(*index as f64),
        })
        .collect()
}

pub fn expand_replacement(
    interp: &mut Interp,
    replacement: &str,
    subject: &str,
    found: &Match,
) -> LuaResult<String> {
    let mut out = String::with_capacity(replacement.len());
    let mut chars = replacement.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some(digit @ '0'..='9') => {
                let index = digit as usize - '0' as usize;
                if index == 0 {
                    out.push_str(&subject[found.start..found.end]);
                    continue;
                }
                match found.captures.get(index - 1) {
                    Some(Capture::Range(start, end)) => out.push_str(&subject[*start..*end]),
                    Some(Capture::Position(position)) => out.push_str(&position.to_string()),
                    None => {
                        return Err(interp.raise_str(format!(
                            "invalid capture index %{digit} in replacement string"
                        )))
                    }
                }
            }
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn find(subject: &str, pattern: &str) -> Option<Match> {
        Matcher::new(subject, pattern).unwrap().find(0)
    }

    fn matched(subject: &str, pattern: &str) -> Option<String> {
        find(subject, pattern).map(|found| subject[found.start..found.end].to_string())
    }

    #[test]
    fn matches_literals() {
        assert_eq!(matched("hello world", "world"), Some("world".to_string()));
        assert_eq!(matched("hello", "xyz"), None);
    }

    #[test]
    fn dot_matches_any_byte() {
        assert_eq!(matched("abc", "a.c"), Some("abc".to_string()));
        assert_eq!(matched("ac", "a.c"), None);
    }

    #[test]
    fn classes_match_character_kinds() {
        assert_eq!(matched("abc123", "%d+"), Some("123".to_string()));
        assert_eq!(matched("abc123", "%a+"), Some("abc".to_string()));
        assert_eq!(matched("  x", "%s+"), Some("  ".to_string()));
        assert_eq!(matched("a-b", "%p"), Some("-".to_string()));
        assert_eq!(matched("a1", "%w+"), Some("a1".to_string()));
        assert_eq!(matched("ab", "%W"), None);
    }

    #[test]
    fn sets_and_ranges_work() {
        assert_eq!(matched("hello", "[aeiou]"), Some("e".to_string()));
        assert_eq!(matched("hello", "[^aeiou]"), Some("h".to_string()));
        assert_eq!(matched("a1b2", "[0-9]+"), Some("1".to_string()));
        assert_eq!(matched("abc", "[%a]+"), Some("abc".to_string()));
    }

    #[test]
    fn quantifiers_greedy_and_lazy() {
        assert_eq!(matched("<a><b>", "<.->"), Some("<a>".to_string()));
        assert_eq!(matched("<a><b>", "<.*>"), Some("<a><b>".to_string()));
        assert_eq!(matched("aaa", "a*"), Some("aaa".to_string()));
        assert_eq!(matched("aaa", "a?"), Some("a".to_string()));
        assert_eq!(matched("bbb", "a*"), Some(String::new()));
    }

    #[test]
    fn anchors_work() {
        assert_eq!(matched("hello", "^h"), Some("h".to_string()));
        assert_eq!(matched("hello", "^e"), None);
        assert_eq!(matched("hello", "o$"), Some("o".to_string()));
        assert_eq!(matched("hello", "l$"), None);
    }

    #[test]
    fn captures_are_reported() {
        let found = find("key=value", "(%w+)=(%w+)").unwrap();
        assert_eq!(found.captures.len(), 2);
        assert_eq!(
            captures_to_values("key=value", &found),
            vec![Value::string("key"), Value::string("value")]
        );
    }

    #[test]
    fn position_captures_are_numbers() {
        let found = find("abc", "()b()").unwrap();
        assert_eq!(
            captures_to_values("abc", &found),
            vec![Value::Number(2.0), Value::Number(3.0)]
        );
    }

    #[test]
    fn balanced_matches_work() {
        assert_eq!(matched("f(a(b)c)d", "%b()"), Some("(a(b)c)".to_string()));
        assert_eq!(matched("no brackets", "%b()"), None);
    }

    #[test]
    fn frontier_matches_work() {
        assert_eq!(matched("the cat", "%f[%a]cat"), Some("cat".to_string()));
        assert_eq!(matched("scatter", "%f[%a]cat"), None);
    }

    #[test]
    fn back_references_work() {
        assert_eq!(matched("abcabc", "(abc)%1"), Some("abcabc".to_string()));
        assert_eq!(matched("abcdef", "(abc)%1"), None);
    }

    #[test]
    fn escaped_magic_characters_are_literal() {
        assert_eq!(matched("a.b", "a%.b"), Some("a.b".to_string()));
        assert_eq!(matched("axb", "a%.b"), None);
    }

    #[test]
    fn replacement_expansion_handles_captures() {
        let mut interp = Interp::new();
        let found = find("key=value", "(%w+)=(%w+)").unwrap();
        let text = expand_replacement(&mut interp, "%2=%1", "key=value", &found).unwrap();
        assert_eq!(text, "value=key");
        let text = expand_replacement(&mut interp, "[%0]", "key=value", &found).unwrap();
        assert_eq!(text, "[key=value]");
    }

    #[test]
    fn multi_byte_text_is_not_split() {
        assert_eq!(matched("héllo", "llo"), Some("llo".to_string()));
        assert_eq!(matched("日本語", "本"), Some("本".to_string()));
    }
}

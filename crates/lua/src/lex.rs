use core::fmt;
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Span {
    pub line: u32,
    pub column: u32,
}

impl Span {
    pub const fn new(line: u32, column: u32) -> Self {
        Span { line, column }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Keyword {
    And,
    Break,
    Do,
    Else,
    Elseif,
    End,
    False,
    For,
    Function,
    If,
    In,
    Local,
    Nil,
    Not,
    Or,
    Repeat,
    Return,
    Then,
    True,
    Until,
    While,
}

impl Keyword {
    pub fn lookup(name: &str) -> Option<Keyword> {
        Some(match name {
            "and" => Keyword::And,
            "break" => Keyword::Break,
            "do" => Keyword::Do,
            "else" => Keyword::Else,
            "elseif" => Keyword::Elseif,
            "end" => Keyword::End,
            "false" => Keyword::False,
            "for" => Keyword::For,
            "function" => Keyword::Function,
            "if" => Keyword::If,
            "in" => Keyword::In,
            "local" => Keyword::Local,
            "nil" => Keyword::Nil,
            "not" => Keyword::Not,
            "or" => Keyword::Or,
            "repeat" => Keyword::Repeat,
            "return" => Keyword::Return,
            "then" => Keyword::Then,
            "true" => Keyword::True,
            "until" => Keyword::Until,
            "while" => Keyword::While,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Keyword::And => "and",
            Keyword::Break => "break",
            Keyword::Do => "do",
            Keyword::Else => "else",
            Keyword::Elseif => "elseif",
            Keyword::End => "end",
            Keyword::False => "false",
            Keyword::For => "for",
            Keyword::Function => "function",
            Keyword::If => "if",
            Keyword::In => "in",
            Keyword::Local => "local",
            Keyword::Nil => "nil",
            Keyword::Not => "not",
            Keyword::Or => "or",
            Keyword::Repeat => "repeat",
            Keyword::Return => "return",
            Keyword::Then => "then",
            Keyword::True => "true",
            Keyword::Until => "until",
            Keyword::While => "while",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    Name(String),

    Number(f64),

    String(String),

    Keyword(Keyword),

    Symbol(&'static str),
    Eof,
}

impl Token {
    pub fn describe(&self) -> String {
        match self {
            Token::Name(name) => format!("name `{name}`"),
            Token::Number(value) => format!("number `{value}`"),
            Token::String(_) => "string".to_string(),
            Token::Keyword(keyword) => format!("`{}`", keyword.as_str()),
            Token::Symbol(symbol) => format!("`{symbol}`"),
            Token::Eof => "end of input".to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Spanned {
    pub token: Token,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.span, self.message)
    }
}

impl std::error::Error for LexError {}

pub fn tokenize(source: &str) -> Result<Vec<Spanned>, LexError> {
    Lexer::new(source).run()
}

struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    offset: usize,
    line: u32,
    column: u32,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        let source = source
            .strip_prefix("#!")
            .map_or(source, |rest| match rest.find('\n') {
                Some(newline) => &rest[newline..],
                None => "",
            });
        Lexer {
            source,
            bytes: source.as_bytes(),
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    fn span(&self) -> Span {
        Span::new(self.line, self.column)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.offset + ahead).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.offset += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else if byte & 0xC0 != 0x80 {
            self.column += 1;
        }
        Some(byte)
    }

    fn run(mut self) -> Result<Vec<Spanned>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia()?;
            let span = self.span();
            let Some(byte) = self.peek() else {
                tokens.push(Spanned {
                    token: Token::Eof,
                    span,
                });
                return Ok(tokens);
            };
            let token = match byte {
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.read_name(),
                b'0'..=b'9' => self.read_number()?,
                b'.' if self.peek_at(1).is_some_and(|next| next.is_ascii_digit()) => {
                    self.read_number()?
                }
                b'"' | b'\'' => self.read_short_string(byte)?,
                b'[' if matches!(self.peek_at(1), Some(b'[') | Some(b'=')) => {
                    match self.try_read_long_string()? {
                        Some(text) => Token::String(text),
                        None => self.read_symbol()?,
                    }
                }
                _ => self.read_symbol()?,
            };
            tokens.push(Spanned { token, span });
        }
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(byte) if byte.is_ascii_whitespace() => {
                    self.bump();
                }
                Some(b'-') if self.peek_at(1) == Some(b'-') => {
                    self.bump();
                    self.bump();

                    if matches!(self.peek(), Some(b'['))
                        && matches!(self.peek_at(1), Some(b'[') | Some(b'='))
                    {
                        let before = self.offset;
                        if self.try_read_long_string()?.is_none() {
                            self.offset = before;
                            self.skip_line_comment();
                        }
                    } else {
                        self.skip_line_comment();
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(byte) = self.peek() {
            if byte == b'\n' {
                return;
            }
            self.bump();
        }
    }

    fn read_name(&mut self) -> Token {
        let start = self.offset;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                self.bump();
            } else {
                break;
            }
        }
        let name = &self.source[start..self.offset];
        match Keyword::lookup(name) {
            Some(keyword) => Token::Keyword(keyword),
            None => Token::Name(name.to_string()),
        }
    }

    fn read_number(&mut self) -> Result<Token, LexError> {
        let start = self.offset;
        let span = self.span();
        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'x') | Some(b'X')) {
            self.bump();
            self.bump();
            while self.peek().is_some_and(|byte| byte.is_ascii_hexdigit()) {
                self.bump();
            }

            if self.peek() == Some(b'.') {
                self.bump();
                while self.peek().is_some_and(|byte| byte.is_ascii_hexdigit()) {
                    self.bump();
                }
            }
            let text = &self.source[start..self.offset];
            let digits = text.trim_start_matches(['0', 'x', 'X']);
            if digits.is_empty() {
                return Err(LexError {
                    message: "malformed number".to_string(),
                    span,
                });
            }
            let (integer, fraction) = match digits.split_once('.') {
                Some((integer, fraction)) => (integer, fraction),
                None => (digits, ""),
            };
            let mut value = u64::from_str_radix(integer, 16).unwrap_or(0) as f64;
            if !fraction.is_empty() {
                let scale = 16f64.powi(-(fraction.len() as i32));
                value += u64::from_str_radix(fraction, 16).unwrap_or(0) as f64 * scale;
            }
            return Ok(Token::Number(value));
        }
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.bump();
        }
        if self.peek() == Some(b'.') {
            self.bump();
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            let mut probe = self.offset + 1;
            if matches!(self.bytes.get(probe), Some(b'+') | Some(b'-')) {
                probe += 1;
            }
            if self
                .bytes
                .get(probe)
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                self.bump();
                if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                    self.bump();
                }
                while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                    self.bump();
                }
            }
        }
        let text = &self.source[start..self.offset];
        text.parse::<f64>()
            .map(Token::Number)
            .map_err(|_| LexError {
                message: format!("malformed number `{text}`"),
                span,
            })
    }

    fn read_short_string(&mut self, quote: u8) -> Result<Token, LexError> {
        let span = self.span();
        self.bump();
        let mut out = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(LexError {
                    message: "unfinished string".to_string(),
                    span,
                });
            };
            if byte == quote {
                self.bump();
                return Ok(Token::String(out));
            }
            if byte == b'\n' {
                return Err(LexError {
                    message: "unfinished string".to_string(),
                    span,
                });
            }
            if byte != b'\\' {
                let start = self.offset;
                while let Some(next) = self.peek() {
                    if next == quote || next == b'\\' || next == b'\n' {
                        break;
                    }
                    self.bump();
                }
                out.push_str(&self.source[start..self.offset]);
                continue;
            }
            self.bump();
            let Some(escape) = self.bump() else {
                return Err(LexError {
                    message: "unfinished string".to_string(),
                    span,
                });
            };
            match escape {
                b'a' => out.push('\u{7}'),
                b'b' => out.push('\u{8}'),
                b'f' => out.push('\u{c}'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'v' => out.push('\u{b}'),
                b'\\' => out.push('\\'),
                b'"' => out.push('"'),
                b'\'' => out.push('\''),
                b'\n' => out.push('\n'),
                b'x' => {
                    let mut value = 0u32;
                    let mut digits = 0;
                    while digits < 2 {
                        match self.peek() {
                            Some(byte) if byte.is_ascii_hexdigit() => {
                                value = value * 16 + hex_value(byte);
                                self.bump();
                                digits += 1;
                            }
                            _ => break,
                        }
                    }
                    if digits == 0 {
                        return Err(LexError {
                            message: "hexadecimal digit expected".to_string(),
                            span,
                        });
                    }
                    out.push(char::from_u32(value).unwrap_or('\u{FFFD}'));
                }
                b'0'..=b'9' => {
                    let mut value = u32::from(escape - b'0');
                    let mut digits = 1;
                    while digits < 3 {
                        match self.peek() {
                            Some(byte) if byte.is_ascii_digit() => {
                                value = value * 10 + u32::from(byte - b'0');
                                self.bump();
                                digits += 1;
                            }
                            _ => break,
                        }
                    }
                    if value > 255 {
                        return Err(LexError {
                            message: "decimal escape too large".to_string(),
                            span,
                        });
                    }
                    out.push(char::from_u32(value).unwrap_or('\u{FFFD}'));
                }
                b'z' => {
                    while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
                        self.bump();
                    }
                }
                other => {
                    return Err(LexError {
                        message: format!("invalid escape sequence `\\{}`", other as char),
                        span,
                    })
                }
            }
        }
    }

    fn try_read_long_string(&mut self) -> Result<Option<String>, LexError> {
        let span = self.span();
        if self.peek() != Some(b'[') {
            return Ok(None);
        }
        let mut probe = self.offset + 1;
        let mut level = 0usize;
        while self.bytes.get(probe) == Some(&b'=') {
            level += 1;
            probe += 1;
        }
        if self.bytes.get(probe) != Some(&b'[') {
            return Ok(None);
        }

        while self.offset <= probe {
            self.bump();
        }

        if self.peek() == Some(b'\r') {
            self.bump();
        }
        if self.peek() == Some(b'\n') {
            self.bump();
        }
        let mut out = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(LexError {
                    message: "unfinished long string".to_string(),
                    span,
                });
            };
            if byte == b']' {
                let mut probe = self.offset + 1;
                let mut closes = 0usize;
                while self.bytes.get(probe) == Some(&b'=') {
                    closes += 1;
                    probe += 1;
                }
                if closes == level && self.bytes.get(probe) == Some(&b']') {
                    while self.offset <= probe {
                        self.bump();
                    }
                    return Ok(Some(out));
                }
            }
            let start = self.offset;
            self.bump();
            out.push_str(&self.source[start..self.offset]);
        }
    }

    fn read_symbol(&mut self) -> Result<Token, LexError> {
        let span = self.span();
        let byte = self.peek().unwrap_or(b'\0');

        if self.source[self.offset..].starts_with("...") {
            for _ in 0..3 {
                self.bump();
            }
            return Ok(Token::Symbol("..."));
        }

        let two = &self.source[self.offset..self.source.len().min(self.offset + 2)];
        for symbol in ["==", "~=", "<=", ">=", ".."] {
            if two == symbol {
                self.bump();
                self.bump();
                return Ok(Token::Symbol(symbol));
            }
        }
        let single = match byte {
            b'+' => "+",
            b'-' => "-",
            b'*' => "*",
            b'/' => "/",
            b'%' => "%",
            b'^' => "^",
            b'#' => "#",
            b'=' => "=",
            b'<' => "<",
            b'>' => ">",
            b'(' => "(",
            b')' => ")",
            b'{' => "{",
            b'}' => "}",
            b'[' => "[",
            b']' => "]",
            b';' => ";",
            b':' => ":",
            b',' => ",",
            b'.' => ".",
            _ => {
                return Err(LexError {
                    message: format!("unexpected character `{}`", byte as char),
                    span,
                })
            }
        };
        self.bump();
        Ok(Token::Symbol(single))
    }
}

fn hex_value(byte: u8) -> u32 {
    match byte {
        b'0'..=b'9' => u32::from(byte - b'0'),
        b'a'..=b'f' => u32::from(byte - b'a') + 10,
        _ => u32::from(byte - b'A') + 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tokens(source: &str) -> Vec<Token> {
        tokenize(source)
            .expect("lexes")
            .into_iter()
            .map(|spanned| spanned.token)
            .filter(|token| *token != Token::Eof)
            .collect()
    }

    #[test]
    fn tokenizes_names_keywords_and_symbols() {
        assert_eq!(
            tokens("local x = 1 + 2"),
            vec![
                Token::Keyword(Keyword::Local),
                Token::Name("x".to_string()),
                Token::Symbol("="),
                Token::Number(1.0),
                Token::Symbol("+"),
                Token::Number(2.0),
            ]
        );
    }

    #[test]
    fn reads_numbers_in_every_form() {
        assert_eq!(tokens("42"), vec![Token::Number(42.0)]);
        assert_eq!(tokens("3.5"), vec![Token::Number(3.5)]);
        assert_eq!(tokens(".5"), vec![Token::Number(0.5)]);
        assert_eq!(tokens("1e3"), vec![Token::Number(1000.0)]);
        assert_eq!(tokens("1E-2"), vec![Token::Number(0.01)]);
        assert_eq!(tokens("0xff"), vec![Token::Number(255.0)]);
        assert_eq!(tokens("0x10"), vec![Token::Number(16.0)]);
    }

    #[test]
    fn reads_short_strings_with_escapes() {
        assert_eq!(tokens(r#""a\nb""#), vec![Token::String("a\nb".to_string())]);
        assert_eq!(
            tokens(r#"'it\'s'"#),
            vec![Token::String("it's".to_string())]
        );
        assert_eq!(tokens(r#""\65""#), vec![Token::String("A".to_string())]);
        assert_eq!(tokens(r#""\x41""#), vec![Token::String("A".to_string())]);
        assert_eq!(tokens("\"a\\z  b\""), vec![Token::String("ab".to_string())]);
    }

    #[test]
    fn reads_long_strings_with_levels() {
        assert_eq!(tokens("[[a\nb]]"), vec![Token::String("a\nb".to_string())]);
        assert_eq!(
            tokens("[==[a]]b]==]"),
            vec![Token::String("a]]b".to_string())]
        );

        assert_eq!(tokens("[[\nx]]"), vec![Token::String("x".to_string())]);
    }

    #[test]
    fn skips_comments() {
        assert_eq!(
            tokens("a -- comment\nb"),
            vec![Token::Name("a".to_string()), Token::Name("b".to_string())]
        );
        assert_eq!(
            tokens("a --[[ long\ncomment ]] b"),
            vec![Token::Name("a".to_string()), Token::Name("b".to_string())]
        );
        assert_eq!(
            tokens("a --[==[ x ]==] b"),
            vec![Token::Name("a".to_string()), Token::Name("b".to_string())]
        );
    }

    #[test]
    fn skips_a_shebang() {
        let tokens = tokens("#!/usr/bin/lua\nreturn 1");
        assert_eq!(
            tokens,
            vec![Token::Keyword(Keyword::Return), Token::Number(1.0)]
        );
    }

    #[test]
    fn reads_multi_character_symbols() {
        assert_eq!(tokens("..."), vec![Token::Symbol("...")]);
        assert_eq!(
            tokens("a..b"),
            vec![
                Token::Name("a".to_string()),
                Token::Symbol(".."),
                Token::Name("b".to_string())
            ]
        );
        assert_eq!(
            tokens("a ~= b"),
            vec![
                Token::Name("a".to_string()),
                Token::Symbol("~="),
                Token::Name("b".to_string())
            ]
        );
    }

    #[test]
    fn reports_line_numbers() {
        let spanned = tokenize("a\nbb\n  c").unwrap();
        assert_eq!(spanned[0].span, Span::new(1, 1));
        assert_eq!(spanned[1].span, Span::new(2, 1));
        assert_eq!(spanned[2].span, Span::new(3, 3));
    }

    #[test]
    fn rejects_unterminated_strings() {
        assert!(tokenize("\"abc").is_err());
        assert!(tokenize("'abc\ndef'").is_err());
        assert!(tokenize("[[abc").is_err());
    }

    #[test]
    fn rejects_unknown_characters() {
        assert!(tokenize("a ? b").is_err());
    }
}

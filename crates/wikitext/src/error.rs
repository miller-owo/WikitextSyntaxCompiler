use core::fmt;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Severity {
    Note,
    Warning,
    Error,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Note => "note",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[non_exhaustive]
pub enum DiagnosticCode {
    UnclosedTag,
    StrayEndTag,
    NestedTable,
    UnclosedBracket,
    UnclosedTable,
    UnclosedHeading,
    BadArgumentCount,
    UnknownParserFunction,
    MissingTemplate,
    ExpansionLimit,
    UnbalancedQuotes,
    BrokenList,
    MisplacedRedirect,
    EmptyReference,
    Unsupported,
    InvalidValue,
    UnknownEntity,
    Truncated,
}

impl DiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            DiagnosticCode::UnclosedTag => "unclosed-tag",
            DiagnosticCode::StrayEndTag => "stray-end-tag",
            DiagnosticCode::NestedTable => "nested-table",
            DiagnosticCode::UnclosedBracket => "unclosed-bracket",
            DiagnosticCode::UnclosedTable => "unclosed-table",
            DiagnosticCode::UnclosedHeading => "unclosed-heading",
            DiagnosticCode::BadArgumentCount => "bad-argument-count",
            DiagnosticCode::UnknownParserFunction => "unknown-parser-function",
            DiagnosticCode::MissingTemplate => "missing-template",
            DiagnosticCode::ExpansionLimit => "expansion-limit",
            DiagnosticCode::UnbalancedQuotes => "unbalanced-quotes",
            DiagnosticCode::BrokenList => "broken-list",
            DiagnosticCode::MisplacedRedirect => "misplaced-redirect",
            DiagnosticCode::EmptyReference => "empty-reference",
            DiagnosticCode::Unsupported => "unsupported",
            DiagnosticCode::InvalidValue => "invalid-value",
            DiagnosticCode::UnknownEntity => "unknown-entity",
            DiagnosticCode::Truncated => "truncated",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Position {
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

impl Position {
    pub const fn new(offset: usize, line: u32, column: u32) -> Self {
        Position {
            offset,
            line,
            column,
        }
    }

    pub fn of(source: &str, offset: usize) -> Self {
        let offset = offset.min(source.len());
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|b| *b == b'\n').count() as u32 + 1;
        let column = prefix
            .rsplit_once('\n')
            .map(|(_, tail)| tail.chars().count())
            .unwrap_or_else(|| prefix.chars().count()) as u32
            + 1;
        Position {
            offset,
            line,
            column,
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub position: Position,
}

impl Diagnostic {
    pub fn new(
        severity: Severity,
        code: DiagnosticCode,
        message: impl Into<String>,
        position: Position,
    ) -> Self {
        Diagnostic {
            severity,
            code,
            message: message.into(),
            position,
        }
    }

    pub fn warning(code: DiagnosticCode, message: impl Into<String>, position: Position) -> Self {
        Diagnostic::new(Severity::Warning, code, message, position)
    }

    pub fn note(code: DiagnosticCode, message: impl Into<String>, position: Position) -> Self {
        Diagnostic::new(Severity::Note, code, message, position)
    }

    pub fn error(code: DiagnosticCode, message: impl Into<String>, position: Position) -> Self {
        Diagnostic::new(Severity::Error, code, message, position)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}] at {}: {}",
            self.severity, self.code, self.position, self.message
        )
    }
}

impl std::error::Error for Diagnostic {}

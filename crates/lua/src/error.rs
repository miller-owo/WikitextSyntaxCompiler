use crate::lex::Span;
use crate::value::Value;
use core::fmt;
#[derive(Clone, Debug)]
pub enum LuaError {
    Syntax {
        message: String,
        span: Span,
        chunk: String,
    },

    Runtime {
        value: Value,
        span: Option<Span>,
    },

    LimitExceeded {
        limit: &'static str,
    },

    Host {
        message: String,
    },
}

impl LuaError {
    pub fn runtime(message: impl Into<String>) -> LuaError {
        LuaError::Runtime {
            value: Value::string(message.into()),
            span: None,
        }
    }

    pub fn at(value: Value, span: Span) -> LuaError {
        LuaError::Runtime {
            value,
            span: Some(span),
        }
    }

    pub fn with_span(self, span: Span) -> LuaError {
        match self {
            LuaError::Runtime { value, span: None } => LuaError::Runtime {
                value,
                span: Some(span),
            },
            other => other,
        }
    }

    pub fn value(&self) -> Option<&Value> {
        match self {
            LuaError::Runtime { value, .. } => Some(value),
            _ => None,
        }
    }

    pub fn span(&self) -> Option<Span> {
        match self {
            LuaError::Syntax { span, .. } => Some(*span),
            LuaError::Runtime { span, .. } => *span,
            _ => None,
        }
    }
}

impl fmt::Display for LuaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LuaError::Syntax {
                message,
                span,
                chunk,
            } => write!(f, "{chunk}:{span}: {message}"),
            LuaError::Runtime { value, span } => match span {
                Some(span) => write!(f, "{span}: {}", render_value(value)),
                None => f.write_str(&render_value(value)),
            },
            LuaError::LimitExceeded { limit } => write!(f, "{limit} exceeded"),
            LuaError::Host { message } => f.write_str(message),
        }
    }
}

impl std::error::Error for LuaError {}

impl From<crate::parse::ParseError> for LuaError {
    fn from(error: crate::parse::ParseError) -> Self {
        LuaError::Syntax {
            message: error.message,
            span: error.span,
            chunk: "?".to_string(),
        }
    }
}

fn render_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_string(),
        Value::Nil => "nil".to_string(),
        other => other.to_string(),
    }
}

pub type LuaResult<T> = Result<T, LuaError>;

#[derive(Debug)]
pub enum Flow {
    Normal,
    Break,

    Return(Vec<Value>),
}

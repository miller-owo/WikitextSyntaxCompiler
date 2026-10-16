#![forbid(unsafe_code)]
#![warn(clippy::all)]

pub mod ast;
pub mod error;
pub mod interp;
pub mod lex;
pub mod parse;
pub mod stdlib;
pub mod value;
pub use crate::error::{LuaError, LuaResult};
pub use crate::interp::{Interp, Limits};
pub use crate::value::Value;

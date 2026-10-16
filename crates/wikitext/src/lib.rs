#![forbid(unsafe_code)]
#![warn(clippy::all)]

pub mod dom;
pub mod entity;
pub mod error;
pub mod extensions;
pub mod lex;
#[cfg(feature = "lua")]
pub mod lua_modules;
pub mod options;
pub mod parse;
pub mod preprocess;
pub mod references;
pub mod scan;
pub mod serialize;
pub use crate::dom::{Category, Document, Element, ElementKind, Node, NodeList, Tag};
pub use crate::error::{Diagnostic, DiagnosticCode, Position, Severity};
pub use crate::extensions::{is_extension_tag, scan_ext_tag, ExtTagSpan, Rendered};
pub use crate::lex::{lex_inline, Token};
pub use crate::options::{Options, OutputMode};
pub use crate::parse::{parse, parse_with_context, ParseContext};
pub use crate::preprocess::{
    decode_entities, Directives, Frame, MemoryTemplates, ModuleProvider, Preprocessor,
    TemplateResolver, Transclusion,
};
pub use crate::references::{Reference, References};
pub use crate::serialize::{serialize, serialize_with, SerializeOptions};
pub fn compile(source: &str, options: &Options) -> String {
    compile_with_diagnostics(source, options).0
}

pub fn compile_with_diagnostics(source: &str, options: &Options) -> (String, Document) {
    let (preprocessed, directives) = Preprocessor::new(options).run_collecting(source);
    let mut document = parse(&preprocessed, options);
    document.default_sort = directives.default_sort;
    document.display_title = directives.display_title;
    let html = serialize_with(
        &document,
        &SerializeOptions::default()
            .with_mode(options.output_mode)
            .with_pretty(options.pretty),
    );
    (html, document)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiles_a_simple_page() {
        let html = compile("Hello '''world'''", &Options::default());
        assert_eq!(html, "<p>Hello <b>world</b></p>\n");
    }

    #[test]
    fn compiles_with_templates() {
        let templates = MemoryTemplates::new();
        templates.insert("t", "value: {{{1|x}}}");
        let options = Options::default().with_resolver(std::sync::Arc::new(templates));
        assert_eq!(compile("{{t|y}}", &options), "<p>value: y</p>\n");
    }

    #[test]
    fn compiles_parser_functions() {
        let options = Options::default();
        assert_eq!(compile("{{#if:yes|Y|N}}", &options), "<p>Y</p>\n");
        assert_eq!(compile("{{#expr:2*3}}", &options), "<p>6</p>\n");
        assert_eq!(compile("{{#len:abcd}}", &options), "<p>4</p>\n");
    }
}

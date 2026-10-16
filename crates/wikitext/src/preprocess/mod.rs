mod entities;
mod expand;
pub mod functions;
pub mod protect;
use crate::options::Options;
use crate::scan::is_protected_content_tag;
pub use entities::{decode_entities, decode_entities_into};
pub use expand::{
    normalise_title, Argument, Frame, MemoryTemplates, ModuleProvider, TemplateResolver,
    Transclusion,
};
pub use protect::{Protect, ProtectedSpan};
fn protect_block_tags(source: &str, protection: &mut Protect) -> String {
    if !source.contains('<') {
        return source.to_string();
    }
    let mut out = String::with_capacity(source.len());
    let mut index = 0usize;
    let mut copied = 0usize;
    while index < source.len() {
        if source.as_bytes()[index] != b'<' {
            index += 1;
            continue;
        }
        let Some(tag) = crate::extensions::scan_ext_tag(&source[index..]) else {
            index += 1;
            continue;
        };
        if !crate::extensions::is_block_tag(&tag.name) {
            index += 1;
            continue;
        }
        out.push_str(&source[copied..index]);
        let id = protection.protect_span(
            index,
            index + tag.end,
            source[index..index + tag.end].to_string(),
        );
        out.push(protect::SENTINEL_OPEN);
        out.push_str(&id.to_string());
        out.push(protect::SENTINEL_CLOSE);
        index += tag.end;
        copied = index;
    }
    out.push_str(&source[copied..]);
    out
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Directives {
    pub default_sort: Option<String>,
    pub display_title: Option<String>,
}

pub struct Preprocessor<'a> {
    options: &'a Options,
}

impl<'a> Preprocessor<'a> {
    pub fn new(options: &'a Options) -> Self {
        Preprocessor { options }
    }

    pub fn run(&self, source: &str) -> String {
        self.run_collecting(source).0
    }

    pub fn run_protected(&self, source: &str) -> (String, Protect) {
        let stripped = strip_comments(&normalize_line_endings(source));
        let mut protection = Protect::new();
        let protected = protection.protect(&stripped);

        let protected = protect_block_tags(&protected, &mut protection);
        let mut expander = expand::Expander::new(self.options);
        let expanded = expander.run(&protected);
        let restored = protection.restore(&expanded);
        (fold_line_endings(restored), protection)
    }

    pub fn run_collecting(&self, source: &str) -> (String, Directives) {
        let stripped = strip_comments(&normalize_line_endings(source));
        let mut protection = Protect::new();
        let protected = protection.protect(&stripped);
        let mut expander = expand::Expander::new(self.options);
        let expanded = expander.run(&protected);
        let directives = Directives {
            default_sort: expander.default_sort().map(str::to_string),
            display_title: expander.display_title().map(str::to_string),
        };
        let restored = protection.restore(&expanded);
        (fold_line_endings(restored), directives)
    }

    pub fn expand(&self, source: &str) -> String {
        expand::Expander::new(self.options).run(source)
    }

    pub fn resolver(&self) -> Option<&dyn TemplateResolver> {
        expand::resolver_of(self.options)
    }
}

// A carriage return reaches the DOM otherwise and lands in the output as a stray
// byte, which a terminal reads as "return to column zero". CRLF is the default on
// Windows, so it has to be folded away before anything else looks at the text.
pub fn normalize_line_endings(source: &str) -> std::borrow::Cow<'_, str> {
    if !source.contains('\r') {
        return std::borrow::Cow::Borrowed(source);
    }
    std::borrow::Cow::Owned(source.replace("\r\n", "\n").replace('\r', "\n"))
}

fn fold_line_endings(source: String) -> String {
    if !source.contains('\r') {
        return source;
    }
    source.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn strip_comments(source: &str) -> String {
    if !source.contains("<!--") {
        return source.to_string();
    }
    let mut out = String::with_capacity(source.len());
    let mut index = 0usize;
    let bytes = source.as_bytes();
    while index < bytes.len() {
        if bytes[index] == b'<' {
            if let Some(tag) = crate::scan::scan_tag(&source[index..]) {
                if !tag.closing && !tag.self_closing && is_protected_content_tag(tag.name) {
                    let open_end = index + tag.end;
                    let (content_end, tag_end) =
                        match crate::scan::find_raw_text_end(&source[open_end..], tag.name) {
                            Some(offset) => {
                                let close = open_end + offset;
                                let close_end =
                                    close + source[close..].find('>').map(|i| i + 1).unwrap_or(0);
                                (close, close_end)
                            }
                            None => (source.len(), source.len()),
                        };
                    out.push_str(&source[index..tag_end]);
                    index = tag_end;
                    let _ = content_end;
                    continue;
                }
            }
            if let Some(comment) = crate::scan::scan_comment(&source[index..]) {
                index += comment.end;
                continue;
            }
        }
        let start = index;
        while index < bytes.len() && bytes[index] != b'<' {
            index += 1;
        }
        if index == start {
            index += 1;
        }
        out.push_str(&source[start..index.min(source.len())]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_comments() {
        assert_eq!(strip_comments("a<!-- x -->b"), "ab");
        assert_eq!(strip_comments("a<!-- x -->"), "a");
        assert_eq!(strip_comments("a<!-- unterminated"), "a");
    }

    #[test]
    fn keeps_comments_inside_raw_text() {
        assert_eq!(
            strip_comments("<nowiki><!-- x --></nowiki>"),
            "<nowiki><!-- x --></nowiki>"
        );
        assert_eq!(
            strip_comments("<pre><!-- x --></pre>"),
            "<pre><!-- x --></pre>"
        );
    }

    #[test]
    fn preprocessor_passes_plain_text_through() {
        let options = Options::default();
        let preprocessor = Preprocessor::new(&options);
        assert_eq!(preprocessor.run("hello"), "hello");
        assert_eq!(preprocessor.run("<!-- x -->hello"), "hello");
    }

    #[test]
    fn nowiki_content_is_protected() {
        let options = Options::default();
        let preprocessor = Preprocessor::new(&options);
        assert_eq!(
            preprocessor.run("<nowiki>{{x}}</nowiki>"),
            "<nowiki>{{x}}</nowiki>"
        );
    }
}

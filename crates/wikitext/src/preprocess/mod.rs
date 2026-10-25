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

/// Which side of a transclusion the text belongs to.
///
/// `<noinclude>` and `<includeonly>` are opposites: a page shows its
/// `<noinclude>` text and hides its `<includeonly>` text, while the page that
/// transcludes it sees exactly the other way round.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum IncludeContext {
    /// The page is being read directly.
    Page,
    /// The page's body is being pulled into another page.
    Transclusion,
}

fn keeps_content(name: &str, context: IncludeContext) -> bool {
    match (name, context) {
        ("noinclude", IncludeContext::Page) => true,
        ("noinclude", IncludeContext::Transclusion) => false,
        ("includeonly", IncludeContext::Page) => false,
        ("includeonly", IncludeContext::Transclusion) => true,
        // `<onlyinclude>` selects what is transcluded but adds nothing to the
        // text itself, so its content is kept in both directions.
        _ => true,
    }
}

/// Behaviour switches that MediaWiki consumes instead of rendering.
const BEHAVIOUR_SWITCHES: &[&str] = &[
    "NOTOC",
    "FORCETOC",
    "TOC",
    "NOEDITSECTION",
    "NEWSECTIONLINK",
    "NONEWSECTIONLINK",
    "NOCONTENTCONVERT",
    "NOCC",
    "NOTITLECONVERT",
    "NOTC",
    "INDEX",
    "NOINDEX",
    "NOGALLERY",
    "HIDDENCAT",
    "STATICREDIRECT",
    "NOGLOBAL",
    "NOCACHE",
    "DISAMBIG",
    "DISAMBIGUATION",
    "EXPECTED_UNCONNECTED_PAGE",
    "ARCHIVEDTALK",
    "EXPENSIVEPARSERFUNCTIONSUSED",
];

fn is_behaviour_switch(name: &str) -> bool {
    BEHAVIOUR_SWITCHES
        .iter()
        .any(|switch| switch.eq_ignore_ascii_case(name))
}

/// Remove `__NOTOC__` and friends from the expanded text.
///
/// A template usually carries these in its `<includeonly>` half, so they arrive
/// with the transclusion rather than with the page source. Left in place they
/// would be literal text in the article.
fn strip_behaviour_switches(source: &str) -> String {
    if !source.contains("__") {
        return source.to_string();
    }
    let mut out = String::with_capacity(source.len());
    let mut copied = 0usize;
    let mut index = 0usize;
    while let Some(offset) = source[index..].find("__") {
        let start = index + offset;
        let rest = &source[start + 2..];
        let Some(end) = rest.find("__") else { break };
        let name = &rest[..end];
        if !name.is_empty() && is_behaviour_switch(name) {
            out.push_str(&source[copied..start]);
            index = start + 2 + end + 2;
            copied = index;
        } else {
            index = start + 2;
        }
    }
    out.push_str(&source[copied..]);
    out
}

/// Remove the transclusion markers, keeping the content that belongs to the
/// context.
///
/// This runs before expansion because the two directions disagree about what is
/// even present: a template's `<includeonly>` text must survive transclusion and
/// must not appear when the template page is read, and the reverse for
/// `<noinclude>`. Doing it as a scan over the whole text also makes a marker that
/// spans lines work, which a per-line tag would not.
/// Whether the text at `source` (which starts with `<`) opens or closes one of
/// the transclusion markers, and whether it is a closing tag.
///
/// This is a byte comparison rather than a call to `scan_tag` on every `<`: a
/// page of unterminated tags would otherwise be walked up to the tag-length
/// ceiling once per `<`.
fn include_marker_at(source: &str) -> Option<bool> {
    let bytes = source.as_bytes();
    let mut cursor = 1usize;
    let closing = bytes.get(cursor) == Some(&b'/');
    if closing {
        cursor += 1;
    }
    for name in ["noinclude", "includeonly", "onlyinclude"] {
        let end = cursor + name.len();
        if bytes.len() < end || !bytes[cursor..end].eq_ignore_ascii_case(name.as_bytes()) {
            continue;
        }
        let after = bytes.get(end);
        if matches!(
            after,
            None | Some(b'>') | Some(b'/') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')
        ) {
            return Some(closing);
        }
    }
    None
}

pub(crate) fn resolve_include_tags(source: &str, context: IncludeContext) -> String {
    if !source.contains('<') {
        return source.to_string();
    }
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut open: Vec<bool> = Vec::new();
    // How many open markers suppress their content. Testing the stack itself
    // would be linear per marker, which makes deep nesting quadratic; this counter
    // keeps the keep/emit decision O(1).
    let mut suppressed = 0usize;
    let mut index = 0usize;
    let mut copied = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        let Some(closing) = include_marker_at(&source[index..]) else {
            index += 1;
            continue;
        };
        let Some(tag) = crate::scan::scan_tag(&source[index..]) else {
            index += 1;
            continue;
        };
        if suppressed == 0 {
            out.push_str(&source[copied..index]);
        }
        index += tag.end;
        copied = index;
        if closing {
            if let Some(keep) = open.pop() {
                if !keep {
                    suppressed -= 1;
                }
            }
        } else if !tag.self_closing {
            let keep = keeps_content(tag.name, context);
            if !keep {
                suppressed += 1;
            }
            open.push(keep);
        }
    }
    if suppressed == 0 {
        out.push_str(&source[copied..]);
    }
    out
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
        let protected = resolve_include_tags(&protected, IncludeContext::Page);
        let mut expander = expand::Expander::new(self.options);
        let expanded = expander.run(&protected);
        // A template may bring comments and behaviour switches in with its body;
        // MediaWiki handles both after expansion too. Everything whose markers
        // must survive (`<nowiki>`, `<pre>`, raw extension tags) is a sentinel.
        let expanded = strip_comments(&expanded);
        let expanded = strip_behaviour_switches(&expanded);
        let restored = protection.restore(&expanded);
        (fold_line_endings(restored), protection)
    }

    pub fn run_collecting(&self, source: &str) -> (String, Directives) {
        let stripped = strip_comments(&normalize_line_endings(source));
        let mut protection = Protect::new();
        let protected = protection.protect(&stripped);
        let protected = resolve_include_tags(&protected, IncludeContext::Page);
        let mut expander = expand::Expander::new(self.options);
        let expanded = expander.run(&protected);
        let directives = Directives {
            default_sort: expander.default_sort().map(str::to_string),
            display_title: expander.display_title().map(str::to_string),
        };
        let expanded = strip_comments(&expanded);
        let expanded = strip_behaviour_switches(&expanded);
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

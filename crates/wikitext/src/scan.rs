use std::borrow::Cow;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EntitySpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CommentSpan {
    pub start: usize,
    pub end: usize,
    pub body_start: usize,
    pub body_end: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TagSpan {
    pub start: usize,
    pub end: usize,
    pub name: &'static str,
    pub closing: bool,
    pub self_closing: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentModel {
    Void,
    RawText,
    Wikitext,
}

pub const KNOWN_TAGS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "base",
    "bdi",
    "bdo",
    "big",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "caption",
    "center",
    "ce",
    "chem",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "dd",
    "del",
    "details",
    "dfn",
    "dialog",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "font",
    "footer",
    "form",
    "gallery",
    "graph",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hiero",
    "hr",
    "html",
    "i",
    "img",
    "includeonly",
    "indicator",
    "input",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "map",
    "mark",
    "math",
    "menu",
    "meta",
    "meter",
    "nav",
    "nobr",
    "noinclude",
    "noscript",
    "nowiki",
    "object",
    "ol",
    "onlyinclude",
    "optgroup",
    "option",
    "output",
    "p",
    "pagelist",
    "param",
    "picture",
    "poem",
    "pre",
    "progress",
    "q",
    "rb",
    "ref",
    "references",
    "rp",
    "rt",
    "rtc",
    "ruby",
    "s",
    "samp",
    "score",
    "script",
    "section",
    "select",
    "small",
    "source",
    "span",
    "strike",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "svg",
    "table",
    "tbody",
    "td",
    "templatedata",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "timeline",
    "title",
    "tr",
    "track",
    "tt",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
    "categorytree",
    "charinsert",
    "dynamicpagelist",
    "inputbox",
    "mapframe",
    "maplink",
    "syntaxhighlight",
];

pub fn canonical_tag_name(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    KNOWN_TAGS.iter().copied().find(|k| *k == lower)
}

pub fn content_model(name: &str) -> ContentModel {
    match name {
        "br" | "hr" | "img" | "wbr" | "area" | "base" | "col" | "embed" | "input" | "link"
        | "meta" | "param" | "source" | "track" | "references" => ContentModel::Void,
        "nowiki" | "pre" | "script" | "style" | "textarea" | "title" => ContentModel::RawText,
        _ => ContentModel::Wikitext,
    }
}

pub fn is_protected_content_tag(name: &str) -> bool {
    matches!(
        name,
        "nowiki" | "pre" | "script" | "style" | "textarea" | "math" | "syntaxhighlight" | "source"
    )
}

pub fn strips_content_newlines(name: &str) -> bool {
    matches!(
        name,
        "div"
            | "p"
            | "blockquote"
            | "center"
            | "table"
            | "caption"
            | "ul"
            | "ol"
            | "li"
            | "dl"
            | "dt"
            | "dd"
            | "section"
            | "article"
            | "aside"
            | "header"
            | "footer"
            | "nav"
            | "main"
            | "figure"
            | "figcaption"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "tr"
            | "td"
            | "th"
            | "thead"
            | "tbody"
            | "tfoot"
            | "form"
            | "fieldset"
            | "legend"
            | "details"
            | "summary"
    )
}

pub fn is_block_tag(name: &str) -> bool {
    crate::serialize::is_block(name) || matches!(name, "ref" | "references" | "gallery" | "poem")
}

pub fn scan_comment(source: &str) -> Option<CommentSpan> {
    let rest = source.strip_prefix("<!--")?;
    let body_start = 4;
    match rest.find("-->") {
        Some(offset) => {
            let body_end = body_start + offset;
            Some(CommentSpan {
                start: 0,
                end: body_end + 3,
                body_start,
                body_end,
            })
        }
        None => Some(CommentSpan {
            start: 0,
            end: source.len(),
            body_start,
            body_end: source.len(),
        }),
    }
}

// A tag has to close within this many bytes to count as one. Real attributes can
// be long — a data URL, say — so the ceiling is generous; its purpose is to stop
// a `<` with no `>` after it from scanning the rest of the page, which made a
// page of unterminated tags quadratic.
const MAX_TAG_LEN: usize = 4096;

pub fn scan_tag(source: &str) -> Option<TagSpan> {
    if !source.starts_with('<') {
        return None;
    }
    let bytes = source.as_bytes();
    let mut index = 1usize;
    let closing = if bytes.get(index) == Some(&b'/') {
        index += 1;
        true
    } else {
        false
    };
    let name_start = index;
    while let Some(byte) = bytes.get(index) {
        if byte.is_ascii_alphanumeric() {
            index += 1;
        } else {
            break;
        }
    }
    if index == name_start {
        return None;
    }
    let raw_name = &source[name_start..index];

    let mut quote: Option<u8> = None;
    let mut self_closing = false;
    let mut closed = false;
    let limit = index + MAX_TAG_LEN;
    while let Some(byte) = bytes.get(index) {
        if index > limit {
            return None;
        }
        match quote {
            Some(q) => {
                if *byte == q {
                    quote = None;
                }
            }
            None => match byte {
                b'"' | b'\'' => quote = Some(*byte),
                b'>' => {
                    closed = true;
                    index += 1;
                    break;
                }
                _ => {}
            },
        }
        index += 1;
    }
    if !closed {
        return None;
    }

    let mut probe = index - 2;
    while probe > name_start && bytes[probe].is_ascii_whitespace() {
        probe -= 1;
    }
    if bytes.get(probe) == Some(&b'/') {
        self_closing = true;
    }
    let name = canonical_tag_name(raw_name)?;
    Some(TagSpan {
        start: 0,
        end: index,
        name,
        closing,
        self_closing,
    })
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RawAttribute {
    pub name: Cow<'static, str>,
    pub value: String,
}

pub fn parse_attributes(tag_source: &str) -> Vec<RawAttribute> {
    let mut attributes = Vec::new();
    let bytes = tag_source.as_bytes();
    let mut index = 1usize;
    if bytes.get(index) == Some(&b'/') {
        index += 1;
    }

    while let Some(byte) = bytes.get(index) {
        if byte.is_ascii_alphanumeric() {
            index += 1;
        } else {
            break;
        }
    }
    loop {
        while let Some(byte) = bytes.get(index) {
            if byte.is_ascii_whitespace() || *byte == b'/' {
                index += 1;
            } else {
                break;
            }
        }
        if index >= bytes.len() || bytes[index] == b'>' {
            break;
        }
        let name_start = index;
        while let Some(byte) = bytes.get(index) {
            if byte.is_ascii_whitespace() || *byte == b'=' || *byte == b'/' || *byte == b'>' {
                break;
            }
            index += 1;
        }
        if index == name_start {
            index += 1;
            continue;
        }
        let name = tag_source[name_start..index].to_ascii_lowercase();

        let mut probe = index;
        while let Some(byte) = bytes.get(probe) {
            if byte.is_ascii_whitespace() {
                probe += 1;
            } else {
                break;
            }
        }
        let value = if bytes.get(probe) == Some(&b'=') {
            probe += 1;
            while let Some(byte) = bytes.get(probe) {
                if byte.is_ascii_whitespace() {
                    probe += 1;
                } else {
                    break;
                }
            }
            match bytes.get(probe) {
                Some(&quote @ (b'"' | b'\'')) => {
                    let value_start = probe + 1;
                    let mut end = value_start;
                    while let Some(byte) = bytes.get(end) {
                        if *byte == quote {
                            break;
                        }
                        end += 1;
                    }
                    index = (end + 1).min(bytes.len());
                    tag_source[value_start..end.min(bytes.len())].to_string()
                }
                _ => {
                    let value_start = probe;
                    let mut end = value_start;
                    while let Some(byte) = bytes.get(end) {
                        if byte.is_ascii_whitespace() || *byte == b'>' {
                            break;
                        }
                        end += 1;
                    }
                    index = end;
                    tag_source[value_start..end].to_string()
                }
            }
        } else {
            index = probe;
            String::new()
        };
        attributes.push(RawAttribute {
            name: Cow::Owned(name),
            value,
        });
    }
    attributes
}

pub fn find_raw_text_end(source: &str, name: &str) -> Option<usize> {
    let needle = format!("</{name}");
    let needle = needle.as_bytes();
    let bytes = source.as_bytes();
    let first = *needle.first()?;
    let mut from = 0usize;
    // Matching case-insensitively by hand avoids copying the remaining source on
    // every call, which on a page of unclosed tags was quadratic.
    while let Some(offset) = bytes[from..].iter().position(|byte| *byte == first) {
        let at = from + offset;
        let matches = bytes.len() >= at + needle.len()
            && bytes[at..at + needle.len()]
                .iter()
                .zip(needle)
                .all(|(left, right)| left.eq_ignore_ascii_case(right));
        if !matches {
            from = at + 1;
            continue;
        }
        let after = at + needle.len();
        match bytes.get(after) {
            Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'/') => {
                return Some(at)
            }
            None => return Some(at),
            _ => from = at + 1,
        }
    }
    None
}

pub fn scan_entity(source: &str) -> Option<EntitySpan> {
    if !source.starts_with('&') {
        return None;
    }

    let limit = source.len().min(34);
    let window = &source[..limit];
    let semicolon = window.find(';')?;
    if semicolon < 2 {
        return None;
    }
    let candidate = &window[..=semicolon];
    let (_, consumed) = crate::entity::decode_at(candidate)?;
    if consumed != candidate.len() {
        return None;
    }
    Some(EntitySpan {
        start: 0,
        end: semicolon + 1,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuoteSpan {
    pub start: usize,
    pub end: usize,
    pub count: usize,
}

pub fn scan_quotes(source: &str) -> Option<QuoteSpan> {
    if !source.starts_with('\'') {
        return None;
    }
    let count = source.bytes().take_while(|b| *b == b'\'').count();
    Some(QuoteSpan {
        start: 0,
        end: count,
        count,
    })
}

// A two-apostrophe run counts as markup only when a word starts right after it,
// which is what keeps the apostrophe in `it's` literal.
pub fn quotes_are_markup(source: &str, span: QuoteSpan) -> bool {
    if span.count < 2 {
        return false;
    }
    if span.count >= 3 {
        return true;
    }

    match source[span.end..].chars().next() {
        Some(ch) => is_word_start(ch),
        None => false,
    }
}

pub fn is_word_start(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch >= '\u{80}'
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EqualsSpan {
    pub start: usize,
    pub end: usize,
    pub count: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LinkSpan {
    pub start: usize,
    pub end: usize,
    pub target_start: usize,
    pub target_end: usize,
    pub parts: Vec<LinkPartSpan>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkPartSpan {
    pub delimiter: usize,
    pub content_start: usize,
    pub content_end: usize,
}

pub fn scan_wikilink(source: &str) -> Option<LinkSpan> {
    if !source.starts_with("[[") {
        return None;
    }
    let bytes = source.as_bytes();
    let mut index = 2usize;
    let target_start = index;
    let mut parts: Vec<LinkPartSpan> = Vec::new();

    let mut link_depth = 0usize;
    let mut ext_link_depth = 0usize;
    let mut template_depth = 0usize;
    let mut current_delimiter = 0usize;
    let mut current_start = index;

    while index < bytes.len() {
        match bytes[index] {
            b'|' if link_depth == 0 && ext_link_depth == 0 && template_depth == 0 => {
                parts.push(LinkPartSpan {
                    delimiter: index,
                    content_start: current_start,
                    content_end: index,
                });
                current_delimiter = index;
                current_start = index + 1;
                index += 1;
            }
            b'[' => {
                if source[index..].starts_with("[[") {
                    link_depth += 1;
                    index += 2;
                } else {
                    ext_link_depth += 1;
                    index += 1;
                }
            }
            b']' => {
                if source[index..].starts_with("]]") {
                    if link_depth > 0 {
                        link_depth -= 1;
                        index += 2;
                        continue;
                    }
                    if ext_link_depth > 0 {
                        ext_link_depth -= 1;
                        index += 2;
                        continue;
                    }
                    if template_depth > 0 {
                        index += 2;
                        continue;
                    }

                    let end = index + 2;
                    let target_end = match parts.first() {
                        Some(first) => first.delimiter,
                        None => index,
                    };
                    let mut parts = parts;
                    if !parts.is_empty() {
                        parts.push(LinkPartSpan {
                            delimiter: current_delimiter,
                            content_start: current_start,
                            content_end: index,
                        });
                    }
                    return Some(LinkSpan {
                        start: 0,
                        end,
                        target_start,
                        target_end,
                        parts,
                    });
                }
                ext_link_depth = ext_link_depth.saturating_sub(1);
                index += 1;
            }
            b'{' => {
                if source[index..].starts_with("{{{") {
                    template_depth += 1;
                    index += 3;
                } else if source[index..].starts_with("{{") {
                    template_depth += 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            b'}' => {
                if source[index..].starts_with("}}}") {
                    template_depth = template_depth.saturating_sub(1);
                    index += 3;
                } else if source[index..].starts_with("}}") {
                    template_depth = template_depth.saturating_sub(1);
                    index += 2;
                } else {
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    None
}

pub fn scan_external_link(source: &str) -> Option<(usize, usize, usize)> {
    if source.starts_with("[[") {
        return None;
    }
    let bytes = source.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }
    let mut index = 1usize;
    while bytes.get(index).is_some_and(|b| b.is_ascii_whitespace()) {
        index += 1;
    }
    let url_start = index;
    while let Some(byte) = bytes.get(index) {
        if byte.is_ascii_whitespace() || *byte == b']' {
            break;
        }
        index += 1;
    }
    let url_end = index;
    if url_end == url_start {
        return None;
    }
    let close = source[url_end..].find(']')? + url_end + 1;
    if !is_link_scheme(&source[url_start..url_end]) {
        return None;
    }
    Some((close, url_start, url_end))
}

pub fn is_link_scheme(url: &str) -> bool {
    const SCHEMES: &[&str] = &[
        "http://",
        "https://",
        "ftp://",
        "ftps://",
        "ssh://",
        "sftp://",
        "irc://",
        "ircs://",
        "gopher://",
        "telnet://",
        "nntp://",
        "worldwind://",
        "mailto:",
        "news:",
        "svn://",
        "git://",
        "mms://",
        "bitcoin:",
        "magnet:",
        "urn:",
        "geo:",
        "tel:",
        "sms:",
        "xmpp:",
        "//",
    ];
    let lower = url.to_ascii_lowercase();
    SCHEMES.iter().any(|scheme| lower.starts_with(scheme))
}

pub fn scan_equals(source: &str) -> Option<EqualsSpan> {
    if !source.starts_with('=') {
        return None;
    }
    let count = source.bytes().take_while(|b| *b == b'=').count();
    Some(EqualsSpan {
        start: 0,
        end: count,
        count,
    })
}

pub fn leading_indent(line: &str) -> usize {
    line.bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count()
}

pub fn trim_line_end(line: &str) -> &str {
    line.trim_end_matches([' ', '\t', '\r'])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scans_comments() {
        let span = scan_comment("<!-- hi -->tail").unwrap();
        assert_eq!(span.end, 11);
        assert_eq!(span.body_start, 4);
        assert_eq!(span.body_end, 8);
    }

    #[test]
    fn unterminated_comment_runs_to_end() {
        let span = scan_comment("<!-- open").unwrap();
        assert_eq!(span.end, 9);
    }

    #[test]
    fn scans_tags() {
        let span = scan_tag("<div class=\"a\">x").unwrap();
        assert_eq!(span.name, "div");
        assert_eq!(span.end, 15);
        assert!(!span.closing);
        let span = scan_tag("</div>").unwrap();
        assert!(span.closing);
        assert_eq!(span.name, "div");
        let span = scan_tag("<br/>").unwrap();
        assert!(span.self_closing);
        let span = scan_tag("<br />").unwrap();
        assert!(span.self_closing);
    }

    #[test]
    fn tag_scanning_respects_quoted_angle_brackets() {
        let source = "<span title=\"a>b\">";
        let span = scan_tag(source).unwrap();
        assert_eq!(span.end, source.len());
        assert_eq!(&source[..span.end], source);
    }

    #[test]
    fn unknown_tags_are_rejected() {
        assert!(scan_tag("<blink>").is_none());
        assert!(scan_tag("<>").is_none());
        assert!(scan_tag("< unclosed").is_none());
    }

    #[test]
    fn parses_attributes() {
        let attributes = parse_attributes("<div class=\"a\" id='b' data-x=1 hidden>");
        assert_eq!(attributes.len(), 4);
        assert_eq!(attributes[0].name, "class");
        assert_eq!(attributes[0].value, "a");
        assert_eq!(attributes[1].value, "b");
        assert_eq!(attributes[2].name, "data-x");
        assert_eq!(attributes[2].value, "1");
        assert_eq!(attributes[3].name, "hidden");
        assert_eq!(attributes[3].value, "");
    }

    #[test]
    fn finds_raw_text_end() {
        assert_eq!(find_raw_text_end("hello</nowiki>x", "nowiki"), Some(5));
        assert_eq!(find_raw_text_end("hello</nowikiX>", "nowiki"), None);
        assert_eq!(find_raw_text_end("hello", "nowiki"), None);
    }

    #[test]
    fn scans_entities() {
        assert_eq!(scan_entity("&amp;x").map(|e| e.end), Some(5));
        assert_eq!(scan_entity("&#x26;").map(|e| e.end), Some(6));
        assert!(scan_entity("&notanentity;").is_none());
        assert!(scan_entity("&x").is_none());
    }

    #[test]
    fn classifies_quote_runs() {
        assert!(!quotes_are_markup("it's", scan_quotes("'s").unwrap()));
        assert!(quotes_are_markup(
            "''word''",
            scan_quotes("''word").unwrap()
        ));
        assert!(quotes_are_markup(
            "'''word'''",
            scan_quotes("'''word").unwrap()
        ));
        assert!(!quotes_are_markup(
            "'' word",
            scan_quotes("'' word").unwrap()
        ));
    }
}

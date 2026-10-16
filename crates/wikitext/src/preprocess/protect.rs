use crate::scan::{self, is_protected_content_tag};
// Private-use code points need no escaping and cannot collide with expansion
// output; a source that contains them decodes slightly wrong, which is the
// documented trade for the simpler mechanism.
pub const SENTINEL_OPEN: char = '\u{E000}';

pub const SENTINEL_CLOSE: char = '\u{E001}';

#[derive(Clone, PartialEq, Eq, Debug)]
enum Payload {
    Text(String),

    Comment(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProtectedSpan {
    pub start: usize,
    pub end: usize,
    pub tag: Option<&'static str>,
    payload: Payload,
}

impl ProtectedSpan {
    pub fn text(&self) -> &str {
        match &self.payload {
            Payload::Text(text) | Payload::Comment(text) => text,
        }
    }

    pub fn is_comment(&self) -> bool {
        matches!(self.payload, Payload::Comment(_))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Protect {
    spans: Vec<ProtectedSpan>,
    keep_comments: bool,
}

impl Protect {
    pub fn new() -> Self {
        Protect {
            spans: Vec::new(),
            keep_comments: false,
        }
    }

    pub fn keeping_comments(keep: bool) -> Self {
        Protect {
            spans: Vec::new(),
            keep_comments: keep,
        }
    }

    pub fn spans(&self) -> &[ProtectedSpan] {
        &self.spans
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    pub fn protect(&mut self, source: &str) -> String {
        if !source.contains("<!--") && !source.contains('<') {
            return source.to_string();
        }
        let bytes = source.as_bytes();
        let mut out = String::with_capacity(source.len());
        let mut index = 0usize;
        let mut copied = 0usize;
        while index < bytes.len() {
            if bytes[index] != b'<' {
                index += 1;
                continue;
            }

            if let Some(comment) = scan::scan_comment(&source[index..]) {
                out.push_str(&source[copied..index]);
                let body = &source[index + comment.body_start..index + comment.body_end];
                let id = self.spans.len();
                self.spans.push(ProtectedSpan {
                    start: index,
                    end: index + comment.end,
                    tag: None,
                    payload: Payload::Comment(body.to_string()),
                });
                push_sentinel(&mut out, id);
                index += comment.end;
                copied = index;
                continue;
            }

            if let Some(tag) = scan::scan_tag(&source[index..]) {
                if !tag.closing && !tag.self_closing && is_protected_content_tag(tag.name) {
                    let open_end = index + tag.end;
                    let (content_end, tag_end) =
                        match scan::find_raw_text_end(&source[open_end..], tag.name) {
                            Some(offset) => {
                                let close = open_end + offset;
                                let close_end =
                                    close + source[close..].find('>').map(|i| i + 1).unwrap_or(0);
                                (close, close_end)
                            }
                            None => (source.len(), source.len()),
                        };
                    let mut body = source[open_end..content_end].to_string();
                    if tag.name == "pre" {
                        if let Some(rest) = body.strip_prefix('\n') {
                            body = rest.to_string();
                        }
                        if let Some(rest) = body.strip_suffix('\n') {
                            body = rest.to_string();
                        }
                    }
                    out.push_str(&source[copied..index]);
                    out.push_str(&source[index..open_end]);
                    let id = self.spans.len();
                    self.spans.push(ProtectedSpan {
                        start: open_end,
                        end: content_end,
                        tag: Some(tag.name),
                        payload: Payload::Text(body),
                    });
                    push_sentinel(&mut out, id);
                    out.push_str(&source[content_end..tag_end]);
                    index = tag_end;
                    copied = index;
                    continue;
                }
            }
            index += 1;
        }
        out.push_str(&source[copied..]);
        out
    }

    pub fn protect_span(&mut self, start: usize, end: usize, text: String) -> usize {
        let id = self.spans.len();
        self.spans.push(ProtectedSpan {
            start,
            end,
            tag: None,
            payload: Payload::Text(text),
        });
        id
    }

    pub fn restore(&self, expanded: &str) -> String {
        if self.spans.is_empty() {
            return expanded.to_string();
        }
        let mut out = String::with_capacity(expanded.len());
        let mut chars = expanded.char_indices().peekable();
        while let Some((index, ch)) = chars.next() {
            if ch != SENTINEL_OPEN {
                out.push(ch);
                continue;
            }

            let start = index + ch.len_utf8();
            let mut digits_end = start;
            let bytes = expanded.as_bytes();
            while digits_end < bytes.len() && bytes[digits_end].is_ascii_digit() {
                digits_end += 1;
            }
            let id: Option<usize> = expanded[start..digits_end].parse().ok();
            let closed = expanded[digits_end..].starts_with(SENTINEL_CLOSE);
            match (id, closed) {
                (Some(id), true) if id < self.spans.len() => {
                    let span = &self.spans[id];
                    match &span.payload {
                        Payload::Text(text) => out.push_str(text),
                        Payload::Comment(text) => {
                            if self.keep_comments {
                                out.push_str("<!--");
                                out.push_str(text);
                                out.push_str("-->");
                            }
                        }
                    }

                    let skip_to = digits_end + SENTINEL_CLOSE.len_utf8();
                    while let Some((next, _)) = chars.peek() {
                        if *next < skip_to {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                _ => out.push(ch),
            }
        }
        out
    }
}

fn push_sentinel(out: &mut String, id: usize) {
    out.push(SENTINEL_OPEN);
    out.push_str(&id.to_string());
    out.push(SENTINEL_CLOSE);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protects_and_restores_nowiki() {
        let mut protect = Protect::new();
        let protected = protect.protect("a<nowiki>{{x}}</nowiki>b");
        assert!(!protected.contains("{{x}}"));
        assert_eq!(protect.restore(&protected), "a<nowiki>{{x}}</nowiki>b");
    }

    #[test]
    fn protects_pre_content_and_trims_newlines() {
        let mut protect = Protect::new();
        let protected = protect.protect("<pre>\nline\n</pre>");
        assert_eq!(protect.restore(&protected), "<pre>line</pre>");
    }

    #[test]
    fn drops_comments_by_default() {
        let mut protect = Protect::new();
        let protected = protect.protect("a<!-- x -->b");
        assert_eq!(protect.restore(&protected), "ab");
    }

    #[test]
    fn keeps_comments_when_asked() {
        let mut protect = Protect::keeping_comments(true);
        let protected = protect.protect("a<!-- x -->b");
        assert_eq!(protect.restore(&protected), "a<!-- x -->b");
    }

    #[test]
    fn handles_multiple_regions() {
        let mut protect = Protect::new();
        let source = "<nowiki>a</nowiki>mid<nowiki>b</nowiki>";
        let protected = protect.protect(source);
        assert_eq!(protect.restore(&protected), source);
    }

    #[test]
    fn leaves_unterminated_nowiki_alone() {
        let mut protect = Protect::new();
        let source = "<nowiki>a";
        let protected = protect.protect(source);
        assert_eq!(protect.restore(&protected), source);
    }

    #[test]
    fn does_nothing_without_markup() {
        let mut protect = Protect::new();
        let protected = protect.protect("plain text");
        assert_eq!(protected, "plain text");
        assert!(protect.is_empty());
    }
}

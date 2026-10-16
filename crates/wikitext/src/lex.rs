use crate::scan;
use crate::scan::TagSpan;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineKind {
    Blank,
    Heading,
    List,
    TableStart,
    TableEnd,
    TableRowOrCell,
    Preformatted,
    HorizontalRule,
    Text,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Line<'a> {
    pub index: usize,
    pub start: usize,
    pub content: &'a str,
}

impl<'a> Line<'a> {
    pub fn is_blank(&self) -> bool {
        self.content
            .bytes()
            .all(|b| b == b' ' || b == b'\t' || b == b'\r')
    }

    pub fn trimmed_end(&self) -> &'a str {
        scan::trim_line_end(self.content)
    }

    pub fn kind(&self) -> LineKind {
        let content = self.content;
        if self.is_blank() {
            return LineKind::Blank;
        }
        if content.starts_with("----") {
            return LineKind::HorizontalRule;
        }
        let first = content.as_bytes()[0];
        match first {
            b'=' => LineKind::Heading,
            b'*' | b'#' | b';' | b':' => LineKind::List,
            b'{' => {
                if content.starts_with("{|") {
                    LineKind::TableStart
                } else {
                    LineKind::Text
                }
            }
            b'|' => {
                if content.starts_with("|}") {
                    LineKind::TableEnd
                } else {
                    LineKind::TableRowOrCell
                }
            }
            b'!' => LineKind::TableRowOrCell,
            b' ' | b'\t' => LineKind::Preformatted,
            _ => LineKind::Text,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LineScanner<'a> {
    source: &'a str,
    offset: usize,
    index: usize,
}

impl<'a> LineScanner<'a> {
    pub fn new(source: &'a str) -> Self {
        LineScanner {
            source,
            offset: 0,
            index: 0,
        }
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn line_index(&self) -> usize {
        self.index
    }

    pub fn set_remaining(&mut self, remaining: &'a str) {
        self.source = remaining;
        self.offset = 0;
    }

    pub fn remaining(&self) -> &'a str {
        &self.source[self.offset..]
    }

    pub fn split_next_line(&mut self) -> Option<(&'a str, usize)> {
        if self.offset >= self.source.len() {
            return None;
        }
        let rest = &self.source[self.offset..];
        let (line, consumed) = match rest.find('\n') {
            Some(newline) => (&rest[..newline], newline + 1),
            None => (rest, rest.len()),
        };
        let start = self.offset;
        self.offset += consumed;
        self.index += 1;
        Some((line, start))
    }

    pub fn skip_to_offset(&mut self, offset: usize) -> bool {
        if offset < self.offset || offset > self.source.len() {
            return false;
        }
        let skipped = &self.source[self.offset..offset];
        self.index += skipped.bytes().filter(|byte| *byte == b'\n').count();
        self.offset = offset;
        true
    }

    pub fn next_line(&mut self) -> Option<Line<'a>> {
        let (content, start) = self.split_next_line()?;
        let index = self.index - 1;
        Some(Line {
            index,
            start,
            content,
        })
    }

    pub fn peek_line(&self) -> Option<Line<'a>> {
        if self.offset >= self.source.len() {
            return None;
        }
        let rest = &self.source[self.offset..];
        let content = match rest.find('\n') {
            Some(newline) => &rest[..newline],
            None => rest,
        };
        Some(Line {
            index: self.index,
            start: self.offset,
            content,
        })
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Token {
    Text(String),

    Entity(String),

    Heading {
        level: u8,
        content: Vec<Token>,
        trailing: String,
    },
    HorizontalRule,

    Preformatted(String),

    ListItem {
        markers: String,
        content: Vec<Token>,
    },

    Table(TableSyntax),

    TagStart(TagToken),

    TagEnd {
        name: &'static str,
        source: String,
    },

    TagSelfClosing(TagToken),

    ExtTag(ExtTagToken),

    Comment(String),

    Raw(String),

    InternalLink(LinkToken),

    ExternalLink(LinkToken),
    Newline,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExtTagToken {
    pub name: String,
    pub source: String,
    pub content: Option<String>,
    pub attributes: Vec<scan::RawAttribute>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TagToken {
    pub name: &'static str,
    pub source: String,
    pub attributes: Vec<scan::RawAttribute>,
    pub explicit_self_closing: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LinkToken {
    pub raw: String,
    pub target: String,
    pub parts: Vec<LinkPart>,
    pub leading_colon: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LinkPart {
    pub name: Option<String>,
    pub value: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableSyntax {
    pub attributes: String,
    pub lines: Vec<TableLine>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableLine {
    pub text: String,
    pub offset: usize,
}

pub fn split_link_part(part: &str) -> LinkPart {
    let mut depth_bracket = 0i32;
    let mut depth_brace = 0i32;
    let bytes = part.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'[' => depth_bracket += 1,
            b']' => depth_bracket -= 1,
            b'{' => depth_brace += 1,
            b'}' => depth_brace -= 1,
            b'=' if depth_bracket <= 0 && depth_brace <= 0 => {
                let name = part[..index].trim();
                let value = &part[index + 1..];
                return LinkPart {
                    name: Some(name.to_string()),
                    value: value.to_string(),
                };
            }
            _ => {}
        }
        index += 1;
    }
    LinkPart {
        name: None,
        value: part.to_string(),
    }
}

// A quote run is markup only at length two or an odd length of three or more;
// even longer runs stay in the text, which is what keeps `it's` literal.
pub fn lex_inline(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    lex_inline_into(source, &mut tokens);
    tokens
}

pub fn lex_inline_into(source: &str, out: &mut Vec<Token>) {
    let mut index = 0usize;
    let bytes = source.as_bytes();
    let mut text_start = 0usize;
    // A tag cannot end without a `>`, so past the last one no tag needs trying.
    // Without this every unmatched `<div` rescans the whole tail, which is
    // quadratic on a page that never closes its tags.
    let last_close = source.rfind('>').map(|at| at + 1).unwrap_or(0);
    macro_rules! flush {
        () => {
            if index > text_start {
                out.push(Token::Text(source[text_start..index].to_string()));
            }
        };
    }
    while index < bytes.len() {
        match bytes[index] {
            b'&' => {
                if let Some(span) = scan::scan_entity(&source[index..]) {
                    let (decoded, _) = crate::entity::decode_at(&source[index..index + span.end])
                        .expect("scan_entity only succeeds on decodable references");
                    flush!();
                    out.push(Token::Entity(decoded));
                    index += span.end;
                    text_start = index;
                    continue;
                }
                index += 1;
            }
            b'<' if index >= last_close => {
                // No `>` remains, so nothing here can open a tag; skip to the
                // next `<` that could start one.
                index += 1 + source[index + 1..]
                    .find('<')
                    .unwrap_or(source.len() - index - 1);
            }
            b'<' => {
                if let Some(comment) = scan::scan_comment(&source[index..]) {
                    flush!();
                    let body = &source[index + comment.body_start..index + comment.body_end];
                    out.push(Token::Comment(body.to_string()));
                    index += comment.end;
                    text_start = index;
                    continue;
                }

                if let Some(ext) = crate::extensions::scan_ext_tag(&source[index..]) {
                    flush!();
                    let attributes = ext
                        .attributes
                        .iter()
                        .map(|attribute| scan::RawAttribute {
                            name: attribute.name.clone(),
                            value: attribute.value.clone(),
                        })
                        .collect();
                    out.push(Token::ExtTag(ExtTagToken {
                        name: ext.name,
                        source: source[index..index + ext.end].to_string(),
                        content: ext.content,
                        attributes,
                    }));
                    index += ext.end;
                    text_start = index;
                    continue;
                }
                if let Some(tag) = scan::scan_tag(&source[index..]) {
                    flush!();
                    let tag_source = &source[index..index + tag.end];
                    if tag.name == "nowiki" && !tag.closing && !tag.self_closing {
                        let after = index + tag.end;
                        match scan::find_raw_text_end(&source[after..], "nowiki") {
                            Some(offset) => {
                                let payload = &source[after..after + offset];
                                out.push(Token::Raw(payload.to_string()));

                                let close_end = after
                                    + offset
                                    + source[after + offset..]
                                        .find('>')
                                        .map(|i| i + 1)
                                        .unwrap_or(0);
                                index = close_end;
                            }
                            None => {
                                let payload = &source[after..];
                                out.push(Token::Raw(payload.to_string()));
                                index = source.len();
                            }
                        }
                    } else {
                        push_tag_token(out, tag, tag_source);
                        index += tag.end;
                    }
                    text_start = index;
                    continue;
                }
                index += 1;
            }
            b'\'' => {
                let span = scan::scan_quotes(&source[index..]).expect("byte is an apostrophe");
                if span.count == 2 || (span.count >= 3 && span.count % 2 == 1) {
                    flush!();
                    out.push(Token::Text(source[index..index + span.count].to_string()));
                    index += span.count;
                    text_start = index;
                    continue;
                }

                index += span.count;
            }
            b'[' => {
                if source[index..].starts_with("[[") {
                    match scan::scan_wikilink(&source[index..]) {
                        Some(span) => {
                            flush!();
                            let raw = &source[index..index + span.end];
                            let target = source[index + span.target_start..index + span.target_end]
                                .to_string();

                            let parts = span
                                .parts
                                .iter()
                                .skip(1)
                                .map(|part| {
                                    split_link_part(
                                        &source
                                            [index + part.content_start..index + part.content_end],
                                    )
                                })
                                .collect();

                            let leading_colon = text_start < index
                                && source[..index].ends_with(':')
                                && !source[..index - 1].ends_with(':');
                            if leading_colon {
                                out.pop();
                                let trimmed = source[text_start..index - 1].to_string();
                                if !trimmed.is_empty() {
                                    out.push(Token::Text(trimmed));
                                }
                            }
                            out.push(Token::InternalLink(LinkToken {
                                raw: raw.to_string(),
                                target,
                                parts,
                                leading_colon,
                            }));
                            index += span.end;
                            text_start = index;
                            continue;
                        }
                        None => {
                            // A wikilink needs a `]]` to end, so with none left
                            // ahead every remaining opener fails as well. Skipping
                            // straight past them keeps a page of unclosed links
                            // linear instead of rescanning the tail once per `[[`.
                            // The text is still emitted, so unmatched markup
                            // degrades to text rather than disappearing.
                            let rest = &source[index..];
                            let opener_run = rest.bytes().take_while(|byte| *byte == b'[').count();
                            if !rest[opener_run..].contains("]]") {
                                index = source.len();
                                break;
                            }
                            index += opener_run.max(2);
                            continue;
                        }
                    }
                }
                if let Some((end, url_start, url_end)) = scan::scan_external_link(&source[index..])
                {
                    flush!();
                    let raw = &source[index..index + end];
                    let url = source[index + url_start..index + url_end].to_string();
                    let parts = vec![LinkPart {
                        name: None,
                        value: source[index + url_end..index + end - 1].to_string(),
                    }];
                    out.push(Token::ExternalLink(LinkToken {
                        raw: raw.to_string(),
                        target: url,
                        parts,
                        leading_colon: false,
                    }));
                    index += end;
                    text_start = index;
                    continue;
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    if index > text_start {
        out.push(Token::Text(source[text_start..index].to_string()));
    }
}

fn push_tag_token(out: &mut Vec<Token>, tag: TagSpan, tag_source: &str) {
    let attributes = scan::parse_attributes(tag_source);
    if tag.closing {
        out.push(Token::TagEnd {
            name: tag.name,
            source: tag_source.to_string(),
        });
    } else if tag.self_closing || scan::content_model(tag.name) == scan::ContentModel::Void {
        out.push(Token::TagSelfClosing(TagToken {
            name: tag.name,
            source: tag_source.to_string(),
            attributes,
            explicit_self_closing: tag.self_closing,
        }));
    } else {
        out.push(Token::TagStart(TagToken {
            name: tag.name,
            source: tag_source.to_string(),
            attributes,
            explicit_self_closing: false,
        }));
    }
}

pub fn split_list_markers(line: &str) -> (String, usize) {
    let end = line
        .bytes()
        .take_while(|b| matches!(b, b'*' | b'#' | b';' | b':'))
        .count();
    (line[..end].to_string(), end)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_lines() {
        let make = |content: &'static str| Line {
            index: 0,
            start: 0,
            content,
        };
        assert_eq!(make("").kind(), LineKind::Blank);
        assert_eq!(make("   ").kind(), LineKind::Blank);
        assert_eq!(make("== A ==").kind(), LineKind::Heading);
        assert_eq!(make("** item").kind(), LineKind::List);
        assert_eq!(make("; term : def").kind(), LineKind::List);
        assert_eq!(make("{| class=x").kind(), LineKind::TableStart);
        assert_eq!(make("|}").kind(), LineKind::TableEnd);
        assert_eq!(make("|-").kind(), LineKind::TableRowOrCell);
        assert_eq!(make("| cell").kind(), LineKind::TableRowOrCell);
        assert_eq!(make("! header").kind(), LineKind::TableRowOrCell);
        assert_eq!(make("----").kind(), LineKind::HorizontalRule);
        assert_eq!(make("----- x").kind(), LineKind::HorizontalRule);
        assert_eq!(make(" pre").kind(), LineKind::Preformatted);
        assert_eq!(make("plain").kind(), LineKind::Text);
    }

    #[test]
    fn scanner_yields_lines_with_offsets() {
        let source = "a\nbb\nccc";
        let mut scanner = LineScanner::new(source);
        let first = scanner.next_line().unwrap();
        assert_eq!(first.content, "a");
        assert_eq!(first.start, 0);
        let second = scanner.next_line().unwrap();
        assert_eq!(second.content, "bb");
        assert_eq!(second.start, 2);
        let third = scanner.next_line().unwrap();
        assert_eq!(third.content, "ccc");
        assert_eq!(third.start, 5);
        assert!(scanner.next_line().is_none());
    }

    #[test]
    fn splits_list_markers() {
        assert_eq!(split_list_markers("** item"), ("**".to_string(), 2));
        assert_eq!(split_list_markers(";term"), (";".to_string(), 1));
        assert_eq!(split_list_markers("text"), (String::new(), 0));
    }
}

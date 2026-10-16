use crate::markup::{Block, Cell, Inline, List, Row, Table};
use std::collections::HashMap;

pub fn parse(source: &str) -> Vec<Block> {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut parser = Parser {
        lines,
        index: 0,
        references: HashMap::new(),
        footnotes: Vec::new(),
    };
    parser.collect_references();
    parser.index = 0;
    parser.blocks(0)
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    index: usize,
    references: HashMap<String, String>,
    footnotes: Vec<(String, String)>,
}

impl<'a> Parser<'a> {
    fn collect_references(&mut self) {
        let mut kept = Vec::new();
        let mut index = 0;
        while index < self.lines.len() {
            let line = self.lines[index];
            if let Some(rest) = line.strip_prefix('[') {
                if let Some(close) = rest.find("]:") {
                    let label = rest[..close].trim().to_lowercase();
                    let target = rest[close + 2..].trim().to_string();
                    if !label.is_empty() && !target.is_empty() {
                        self.references.insert(label, target);
                        index += 1;
                        continue;
                    }
                }
            }
            kept.push(line);
            index += 1;
        }
        self.lines = kept;
    }

    fn peek(&self) -> Option<&'a str> {
        self.lines.get(self.index).copied()
    }

    fn blocks(&mut self, depth: usize) -> Vec<Block> {
        let mut blocks = Vec::new();
        while let Some(line) = self.peek() {
            if line.trim().is_empty() {
                self.index += 1;
                continue;
            }

            if let Some(fence) = fence_of(line) {
                blocks.push(self.code_block(fence));
                continue;
            }

            if is_rule(line) {
                self.index += 1;
                blocks.push(Block::Rule);
                continue;
            }

            if let Some(level) = atx_level(line) {
                self.index += 1;
                let content = line[level + 1..].trim();
                let content = content.trim_end_matches('#').trim_end();
                blocks.push(Block::Heading {
                    level: level as u8,
                    content: self.inlines(content),
                });
                continue;
            }

            if let Some(level) = setext_level(self.peek(), self.lines.get(self.index + 1).copied())
            {
                self.index += 2;
                let content = line.trim();
                blocks.push(Block::Heading {
                    level,
                    content: self.inlines(content),
                });
                continue;
            }

            if line.trim_start().starts_with('>') {
                blocks.push(self.quote(depth));
                continue;
            }

            if let Some(list) = self.list(depth) {
                blocks.push(Block::List(list));
                continue;
            }

            if let Some(table) = self.table() {
                blocks.push(Block::Table(table));
                continue;
            }

            if let Some(html) = html_block(line) {
                blocks.push(html);
                continue;
            }

            if let Some(block) = self.indented_code() {
                blocks.push(block);
                continue;
            }

            blocks.push(self.paragraph());
        }
        blocks
    }

    fn paragraph(&mut self) -> Block {
        let mut collected = Vec::new();
        while let Some(line) = self.peek() {
            if line.trim().is_empty() {
                break;
            }
            if !collected.is_empty() && starts_block(self, line) {
                break;
            }
            if line.trim_start().starts_with("<!--") {
                break;
            }
            self.index += 1;
            if line.trim_end().ends_with("  ") || line.trim_end().ends_with('\\') {
                collected.push(line.trim_end().trim_end_matches('\\').to_string());
                collected.push("\u{0}".to_string());
            } else {
                collected.push(line.trim().to_string());
            }
        }
        let joined = collected.join("\n");
        Block::Paragraph(self.inlines(&joined))
    }

    fn code_block(&mut self, fence: Fence<'_>) -> Block {
        self.index += 1;
        let mut text = String::new();
        while let Some(line) = self.peek() {
            if line.trim_start().starts_with(fence.marker) && line.trim().len() >= fence.length {
                self.index += 1;
                break;
            }
            text.push_str(line);
            text.push('\n');
            self.index += 1;
        }
        Block::Pre {
            language: fence.language.map(str::to_string),
            text: text.trim_end_matches('\n').to_string(),
        }
    }

    fn indented_code(&mut self) -> Option<Block> {
        let line = self.peek()?;
        if !line.starts_with("    ") && !line.starts_with('\t') {
            return None;
        }
        let mut text = String::new();
        while let Some(line) = self.peek() {
            if line.trim().is_empty() {
                text.push('\n');
                self.index += 1;
                continue;
            }
            let stripped = line
                .strip_prefix("    ")
                .or_else(|| line.strip_prefix('\t'))
                .unwrap_or(line);
            text.push_str(stripped);
            text.push('\n');
            self.index += 1;
        }
        Some(Block::Pre {
            language: None,
            text: text.trim_end_matches('\n').to_string(),
        })
    }

    fn quote(&mut self, depth: usize) -> Block {
        let mut inner = Vec::new();
        while let Some(line) = self.peek() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix('>') {
                inner.push(rest.strip_prefix(' ').unwrap_or(rest));
                self.index += 1;
            } else if trimmed.is_empty() {
                inner.push("");
                self.index += 1;
            } else {
                break;
            }
        }
        let source = inner.join("\n");
        let mut nested = Parser {
            lines: source.lines().collect(),
            index: 0,
            references: self.references.clone(),
            footnotes: Vec::new(),
        };
        let mut blocks = nested.blocks(depth + 1);
        self.footnotes.append(&mut nested.footnotes);
        if blocks.is_empty() {
            blocks.push(Block::Paragraph(Vec::new()));
        }
        Block::Quote(blocks)
    }

    fn list(&mut self, depth: usize) -> Option<List> {
        let first = self.peek()?;
        let marker = list_marker(first)?;
        if depth > 8 {
            return None;
        }
        let ordered = marker.ordered;
        let mut items = Vec::new();
        while let Some(line) = self.peek() {
            let Some(current) = list_marker(line) else {
                if line.trim().is_empty() {
                    let next_is_item = self
                        .lines
                        .get(self.index + 1)
                        .and_then(|next| list_marker(next))
                        .is_some_and(|next| next.ordered == ordered);
                    if next_is_item {
                        self.index += 1;
                        continue;
                    }
                }
                break;
            };
            if current.ordered != ordered {
                break;
            }
            let width = current.width;
            let mut content = vec![line[width..].to_string()];
            self.index += 1;
            while let Some(line) = self.peek() {
                if line.trim().is_empty() {
                    let following = self.lines.get(self.index + 1).copied();
                    match following {
                        Some(next) if list_marker(next).is_some() => {
                            content.push(String::new());
                            self.index += 1;
                        }
                        Some(next) if next.starts_with("  ") || next.starts_with('\t') => {
                            content.push(String::new());
                            self.index += 1;
                        }
                        _ => break,
                    }
                    continue;
                }
                // Indentation is checked first: a marker two columns in is a
                // nested list, which belongs to this item, not a sibling.
                let indent = line.len() - line.trim_start().len();
                if indent == 0
                    && line.starts_with(|ch: char| !ch.is_whitespace())
                    && list_marker(line).is_some()
                {
                    break;
                }
                if indent >= 2 || line.starts_with('\t') {
                    let stripped = line
                        .strip_prefix("  ")
                        .or_else(|| line.strip_prefix('\t'))
                        .unwrap_or(line);
                    content.push(stripped.to_string());
                    self.index += 1;
                } else {
                    break;
                }
            }
            let source = content.join("\n");
            let mut nested = Parser {
                lines: source.lines().collect(),
                index: 0,
                references: self.references.clone(),
                footnotes: Vec::new(),
            };
            let mut blocks = nested.blocks(depth + 1);
            self.footnotes.append(&mut nested.footnotes);
            if blocks.is_empty() {
                blocks.push(Block::Paragraph(Vec::new()));
            }
            items.push(blocks);
        }
        if items.is_empty() {
            return None;
        }
        Some(List { ordered, items })
    }

    fn table(&mut self) -> Option<Table> {
        let header = self.peek()?;
        if !header.contains('|') {
            return None;
        }
        let delimiter = self.lines.get(self.index + 1).copied()?;
        let alignments = delimiter_alignments(delimiter)?;
        let header_cells = split_row(header);
        if header_cells.len() != alignments.len() {
            return None;
        }
        let mut rows = vec![Row {
            cells: header_cells
                .iter()
                .map(|cell| Cell {
                    header: true,
                    content: vec![Block::Paragraph(self.inlines(cell))],
                })
                .collect(),
        }];
        self.index += 2;
        while let Some(line) = self.peek() {
            if line.trim().is_empty() || !line.contains('|') {
                break;
            }
            if is_rule(line) {
                break;
            }
            let cells = split_row(line);
            if cells.is_empty() {
                break;
            }
            rows.push(Row {
                cells: cells
                    .iter()
                    .map(|cell| Cell {
                        header: false,
                        content: vec![Block::Paragraph(self.inlines(cell))],
                    })
                    .collect(),
            });
            self.index += 1;
        }
        Some(Table { rows })
    }

    fn inlines(&self, source: &str) -> Vec<Inline> {
        let mut parser = InlineParser {
            chars: source.chars().collect(),
            index: 0,
            references: &self.references,
            depth: 0,
        };
        parser.parse()
    }
}

struct Fence<'a> {
    marker: &'a str,
    length: usize,
    language: Option<&'a str>,
}

fn fence_of(line: &str) -> Option<Fence<'_>> {
    let trimmed = line.trim_start();
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    for marker in ["```", "~~~"] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            let fence = marker;
            let length = fence.len()
                + rest
                    .chars()
                    .take_while(|ch| *ch == fence.chars().next().unwrap())
                    .count();
            let language = rest.trim();
            return Some(Fence {
                marker: fence,
                length,
                language: if language.is_empty() {
                    None
                } else {
                    Some(language.split_whitespace().next().unwrap_or(language))
                },
            });
        }
    }
    None
}

fn is_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    for marker in ['-', '*', '_'] {
        let count = trimmed.chars().filter(|ch| *ch == marker).count();
        if count >= 3 && count == trimmed.chars().count() {
            return true;
        }
    }
    false
}

fn atx_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if rest.is_empty() || rest.starts_with(' ') {
        Some(level)
    } else {
        None
    }
}

fn setext_level(current: Option<&str>, next: Option<&str>) -> Option<u8> {
    let current = current?;
    if current.trim().is_empty() || atx_level(current).is_some() {
        return None;
    }
    let next = next?.trim();
    if next.is_empty() {
        return None;
    }
    if next.chars().all(|ch| ch == '=') {
        return Some(1);
    }
    if next.chars().all(|ch| ch == '-') {
        return Some(2);
    }
    None
}

struct Marker {
    ordered: bool,
    width: usize,
}

fn list_marker(line: &str) -> Option<Marker> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if indent > 3 {
        return None;
    }
    let first = trimmed.chars().next()?;
    if first == '-' || first == '*' || first == '+' {
        let rest = &trimmed[1..];
        if rest.is_empty() || rest.starts_with(' ') {
            return Some(Marker {
                ordered: false,
                width: line.len() - rest.len(),
            });
        }
        return None;
    }
    if first.is_ascii_digit() {
        let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
        if digits > 9 {
            return None;
        }
        let rest = &trimmed[digits..];
        if let Some(after) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            let _ = after;
            let width = line.len() - rest.len() + 2;
            return Some(Marker {
                ordered: true,
                width,
            });
        }
    }
    None
}

fn starts_block(parser: &Parser<'_>, line: &str) -> bool {
    fence_of(line).is_some()
        || is_rule(line)
        || atx_level(line).is_some()
        || line.trim_start().starts_with('>')
        || list_marker(line).is_some()
        || setext_level(Some(line), parser.lines.get(parser.index + 1).copied()).is_some()
}

fn html_block(line: &str) -> Option<Block> {
    let trimmed = line.trim();
    const BLOCKS: &[&str] = &[
        "div",
        "table",
        "section",
        "article",
        "aside",
        "header",
        "footer",
        "nav",
        "figure",
        "form",
        "fieldset",
        "details",
        "summary",
        "main",
        "blockquote",
        "dl",
        "dd",
        "dt",
        "pre",
        "script",
        "style",
    ];
    for tag in BLOCKS {
        if trimmed.starts_with(&format!("<{tag}")) || trimmed.starts_with(&format!("</{tag}")) {
            return Some(Block::Raw(trimmed.to_string()));
        }
    }
    None
}

fn delimiter_alignments(line: &str) -> Option<Vec<Alignment>> {
    let cells = split_row(line);
    if cells.is_empty() {
        return None;
    }
    let mut alignments = Vec::new();
    for cell in cells {
        let trimmed = cell.trim();
        let left = trimmed.starts_with(':');
        let right = trimmed.ends_with(':');
        let core = trimmed.trim_matches(':').trim();
        if core.is_empty() || !core.chars().all(|ch| ch == '-') {
            return None;
        }
        alignments.push(match (left, right) {
            (true, true) => Alignment::Center,
            (true, false) => Alignment::Left,
            (false, true) => Alignment::Right,
            (false, false) => Alignment::None,
        });
    }
    Some(alignments)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Alignment {
    None,
    Left,
    Right,
    Center,
}

fn split_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in trimmed.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                current.push(ch);
                escaped = true;
            }
            '|' => {
                cells.push(current.trim().to_string());
                current.clear();
            }
            other => current.push(other),
        }
    }
    cells.push(current.trim().to_string());
    cells
}

struct InlineParser<'a> {
    chars: Vec<char>,
    index: usize,
    references: &'a HashMap<String, String>,
    depth: usize,
}

impl<'a> InlineParser<'a> {
    fn parse(&mut self) -> Vec<Inline> {
        let mut out = Vec::new();
        let mut text = String::new();
        while self.index < self.chars.len() {
            let ch = self.chars[self.index];
            match ch {
                '\\' => {
                    if let Some(next) = self.chars.get(self.index + 1).copied() {
                        if next.is_ascii_punctuation() {
                            text.push(next);
                            self.index += 2;
                            continue;
                        }
                    }
                    text.push(ch);
                    self.index += 1;
                }
                '`' => {
                    let count = self.run('`');
                    if let Some(code) = self.code_span(count) {
                        flush_text(&mut out, &mut text);
                        out.push(Inline::Code(code));
                    } else {
                        for _ in 0..count {
                            text.push('`');
                        }
                    }
                }
                '*' | '_' => {
                    if let Some(inline) = self.emphasis(ch) {
                        flush_text(&mut out, &mut text);
                        out.push(inline);
                    } else {
                        text.push(ch);
                        self.index += 1;
                    }
                }
                '~' => {
                    if self.chars.get(self.index + 1) == Some(&'~') {
                        if let Some(content) = self.delimited("~~") {
                            flush_text(&mut out, &mut text);
                            out.push(Inline::Struck(content));
                            continue;
                        }
                    }
                    text.push(ch);
                    self.index += 1;
                }
                '!' => {
                    if self.chars.get(self.index + 1) == Some(&'[') {
                        if let Some(inline) = self.image() {
                            flush_text(&mut out, &mut text);
                            out.push(inline);
                            continue;
                        }
                    }
                    text.push(ch);
                    self.index += 1;
                }
                '[' => {
                    if let Some(inline) = self.link() {
                        flush_text(&mut out, &mut text);
                        out.push(inline);
                    } else {
                        text.push(ch);
                        self.index += 1;
                    }
                }
                '<' => {
                    if let Some(inline) = self.autolink_or_html() {
                        flush_text(&mut out, &mut text);
                        out.push(inline);
                    } else {
                        text.push(ch);
                        self.index += 1;
                    }
                }
                '&' => {
                    let entity = self.entity();
                    match entity {
                        Some(decoded) => text.push_str(&decoded),
                        None => text.push('&'),
                    }
                }
                '\n' => {
                    text.push('\n');
                    self.index += 1;
                }
                other => {
                    text.push(other);
                    self.index += 1;
                }
            }
        }
        flush_text(&mut out, &mut text);
        out
    }

    fn run(&mut self, mark: char) -> usize {
        let mut count = 0;
        while self.chars.get(self.index + count) == Some(&mark) {
            count += 1;
        }
        count
    }

    fn code_span(&mut self, count: usize) -> Option<String> {
        let start = self.index + count;
        let mut end = start;
        while end < self.chars.len() {
            if self.chars[end] == '`' {
                let closing = self.chars[end..]
                    .iter()
                    .take_while(|ch| **ch == '`')
                    .count();
                if closing == count {
                    let code: String = self.chars[start..end].iter().collect();
                    self.index = end + count;
                    let code = code.replace('\n', " ");
                    let code =
                        if code.starts_with(' ') && code.ends_with(' ') && code.trim().len() > 1 {
                            code[1..code.len() - 1].to_string()
                        } else {
                            code
                        };
                    return Some(code);
                }
                end += closing;
                continue;
            }
            end += 1;
        }
        None
    }

    fn emphasis(&mut self, mark: char) -> Option<Inline> {
        if self.depth > 8 {
            return None;
        }
        // A run of three or more is bold around italic; only matching the same
        // width on both sides leaves a stray marker behind.
        let opening = self.run(mark);
        if opening == 0 {
            return None;
        }
        let mut width = opening.min(3);
        if width == 3 {
            if let Some(inline) = self.emphasis_span(mark, 3) {
                return Some(inline);
            }
            width = 2;
        }
        if opening < width {
            return None;
        }
        if self.index + width >= self.chars.len() {
            return None;
        }
        if self.chars[self.index + width].is_whitespace() {
            return None;
        }
        let mut end = self.index + width;
        while end < self.chars.len() {
            if self.chars[end] == '\n' {
                return None;
            }
            if self.chars[end] == mark {
                let closing = self.chars[end..]
                    .iter()
                    .take_while(|ch| **ch == mark)
                    .count();
                if closing >= width && !self.chars[end - 1].is_whitespace() {
                    let content = self.slice(self.index + width, end);
                    let parsed = self.nested(&content);
                    self.index = end + closing;
                    return Some(self.wrap(width, parsed));
                }
                end += closing;
                continue;
            }
            end += 1;
        }
        None
    }

    fn emphasis_span(&mut self, mark: char, width: usize) -> Option<Inline> {
        let start = self.index + width;
        if start >= self.chars.len() || self.chars[start].is_whitespace() {
            return None;
        }
        let mut end = start;
        while end < self.chars.len() {
            if self.chars[end] == '\n' {
                return None;
            }
            if self.chars[end] == mark {
                let closing = self.chars[end..]
                    .iter()
                    .take_while(|ch| **ch == mark)
                    .count();
                if closing >= width && end > start && !self.chars[end - 1].is_whitespace() {
                    let content = self.slice(start, end);
                    let mut nested = InlineParser {
                        chars: content.chars().collect(),
                        index: 0,
                        references: self.references,
                        depth: self.depth + 1,
                    };
                    let parsed = nested.parse();
                    let italic = if closing >= 3 {
                        Inline::Italic(vec![Inline::Bold(parsed)])
                    } else {
                        Inline::Italic(parsed)
                    };
                    self.index = end + closing;
                    return Some(italic);
                }
                end += closing;
                continue;
            }
            end += 1;
        }
        None
    }

    fn wrap(&self, width: usize, content: Vec<Inline>) -> Inline {
        if width >= 2 {
            Inline::Bold(content)
        } else {
            Inline::Italic(content)
        }
    }

    fn slice(&self, start: usize, end: usize) -> String {
        self.chars[start..end].iter().collect()
    }

    fn nested(&self, content: &str) -> Vec<Inline> {
        let mut parser = InlineParser {
            chars: content.chars().collect(),
            index: 0,
            references: self.references,
            depth: self.depth + 1,
        };
        parser.parse()
    }
    fn delimited(&mut self, marks: &str) -> Option<Vec<Inline>> {
        let width = marks.chars().count();
        let opener_end = self.index + width;
        let mut end = opener_end;
        while end < self.chars.len() {
            if self.chars[end] == marks.chars().next().unwrap()
                && self.chars[end..].iter().take(width).collect::<String>() == marks
                && end > opener_end
            {
                let content: String = self.chars[opener_end..end].iter().collect();
                let mut nested = InlineParser {
                    chars: content.chars().collect(),
                    index: 0,
                    references: self.references,
                    depth: self.depth + 1,
                };
                let parsed = nested.parse();
                self.index = end + width;
                return Some(parsed);
            }
            if self.chars[end] == '\n' {
                return None;
            }
            end += 1;
        }
        None
    }

    fn link(&mut self) -> Option<Inline> {
        let start = self.index;
        self.index += 1;
        let mut close = None;
        let mut depth = 1;
        let mut scan = self.index;
        while scan < self.chars.len() {
            match self.chars[scan] {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(scan);
                        break;
                    }
                }
                _ => {}
            }
            scan += 1;
        }
        let close = close?;
        let label: String = self.chars[self.index..close].iter().collect();

        if self.chars.get(close + 1) == Some(&'(') {
            let mut end = close + 2;
            let mut parens = 1;
            while end < self.chars.len() {
                match self.chars[end] {
                    '(' => parens += 1,
                    ')' => {
                        parens -= 1;
                        if parens == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                end += 1;
            }
            if end >= self.chars.len() {
                self.index = start;
                return None;
            }
            let destination: String = self.chars[close + 2..end].iter().collect();
            let (url, title) = split_destination(&destination);
            self.index = end + 1;
            return Some(self.make_link(&label, &url, title.as_deref()));
        }

        if self.chars.get(close + 1) == Some(&'[') {
            if let Some(end) = self.chars[close + 2..].iter().position(|ch| *ch == ']') {
                let reference: String = self.chars[close + 2..close + 2 + end].iter().collect();
                self.index = close + 3 + end;
                let key = if reference.trim().is_empty() {
                    label.to_lowercase()
                } else {
                    reference.trim().to_lowercase()
                };
                let url = self.references.get(&key)?.clone();
                return Some(self.make_link(&label, &url, None));
            }
            return None;
        }

        let url = self.references.get(&label.to_lowercase())?.clone();
        self.index = close + 1;
        Some(self.make_link(&label, &url, None))
    }

    fn make_link(&self, label: &str, url: &str, title: Option<&str>) -> Inline {
        let mut nested = InlineParser {
            chars: label.chars().collect(),
            index: 0,
            references: self.references,
            depth: self.depth + 1,
        };
        let label = nested.parse();
        let _ = title;
        if let Some(target) = wiki_target(url) {
            let label = if label.is_empty() {
                vec![Inline::Text(target.clone())]
            } else {
                label
            };
            Inline::Link { target, label }
        } else {
            Inline::External {
                url: url.to_string(),
                label,
            }
        }
    }

    fn image(&mut self) -> Option<Inline> {
        self.index += 1;
        let start = self.index;
        let inline = self.link()?;
        let _ = start;
        match inline {
            Inline::Link { target, label } | Inline::External { url: target, label } => {
                let target = if target.contains('.') && !target.starts_with("File:") {
                    format!("File:{target}")
                } else {
                    target
                };
                Some(Inline::Image { target, alt: label })
            }
            other => Some(other),
        }
    }

    fn autolink_or_html(&mut self) -> Option<Inline> {
        let rest: String = self.chars[self.index..].iter().take(2048).collect();
        if let Some(close) = rest.find('>') {
            let inner = &rest[1..close];
            if let Some(url) = inner
                .strip_prefix("http://")
                .or_else(|| inner.strip_prefix("https://"))
            {
                let url = format!("{}://{}", &inner[..inner.len() - url.len() - 3], url);
                self.index += close + 1;
                return Some(Inline::External {
                    url: url.clone(),
                    label: vec![Inline::Text(url)],
                });
            }
            if inner.contains('@') && !inner.contains(' ') {
                let url = format!("mailto:{inner}");
                self.index += close + 1;
                return Some(Inline::External {
                    url,
                    label: vec![Inline::Text(inner.to_string())],
                });
            }
            if inner.starts_with('/') || inner.starts_with(|ch: char| ch.is_ascii_alphabetic()) {
                let raw = &rest[..close + 1];
                self.index += close + 1;
                return Some(Inline::Raw(raw.to_string()));
            }
            if inner.starts_with("--") || inner.starts_with('!') {
                self.index += close + 1;
                return None;
            }
        }
        None
    }

    fn entity(&mut self) -> Option<String> {
        let rest: String = self.chars[self.index..].iter().take(32).collect();
        let end = rest.find(';')?;
        let body = &rest[1..end];
        if body.is_empty() || body.len() > 10 {
            return None;
        }
        let decoded = decode_entity(body)?;
        self.index += end + 1;
        Some(decoded)
    }
}

fn flush_text(out: &mut Vec<Inline>, text: &mut String) {
    if text.is_empty() {
        return;
    }
    let taken = std::mem::take(text);
    match out.last_mut() {
        Some(Inline::Text(existing)) => existing.push_str(&taken),
        _ => out.push(Inline::Text(taken)),
    }
}

fn split_destination(destination: &str) -> (String, Option<String>) {
    let trimmed = destination.trim();
    match trimmed.find(char::is_whitespace) {
        Some(index) => {
            let url = trimmed[..index].to_string();
            let title = trimmed[index..]
                .trim()
                .trim_matches(|ch| ch == '"' || ch == '\'' || ch == '(' || ch == ')')
                .to_string();
            (url, if title.is_empty() { None } else { Some(title) })
        }
        None => (trimmed.to_string(), None),
    }
}

fn wiki_target(url: &str) -> Option<String> {
    let url = url.trim();
    if url.starts_with("./") || url.starts_with("/wiki/") {
        let path = url
            .strip_prefix("./")
            .or_else(|| url.strip_prefix("/wiki/"))
            .unwrap_or(url);
        let name = percent_decode(path).replace('_', " ");
        let name = if path.contains('.') && !name.contains(':') && is_media(path) {
            format!("File:{name}")
        } else {
            name
        };
        return Some(name);
    }
    None
}

fn is_media(path: &str) -> bool {
    let extension = path.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(
        extension.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "svg"
            | "webp"
            | "tiff"
            | "tif"
            | "xcf"
            | "pdf"
            | "ogg"
            | "ogv"
            | "oga"
            | "mp3"
            | "wav"
            | "flac"
            | "mp4"
            | "webm"
    )
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn decode_entity(body: &str) -> Option<String> {
    if let Some(digits) = body.strip_prefix('#') {
        let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            digits.parse::<u32>().ok()?
        };
        return char::from_u32(code).map(String::from);
    }
    let named = match body {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "hellip" => '…',
        "mdash" => '—',
        "ndash" => '–',
        "lsquo" => '‘',
        "rsquo" => '’',
        "ldquo" => '“',
        "rdquo" => '”',
        "times" => '×',
        "divide" => '÷',
        "deg" => '°',
        "plusmn" => '±',
        "frac12" => '½',
        "sup2" => '²',
        "sup3" => '³',
        "micro" => 'µ',
        "para" => '¶',
        "sect" => '§',
        "middot" => '·',
        "laquo" => '«',
        "raquo" => '»',
        "larr" => '←',
        "rarr" => '→',
        "harr" => '↔',
        "infin" => '∞',
        "ne" => '≠',
        "le" => '≤',
        "ge" => '≥',
        "alpha" => 'α',
        "beta" => 'β',
        "gamma" => 'γ',
        "delta" => 'δ',
        "pi" => 'π',
        "sigma" => 'σ',
        "omega" => 'ω',
        _ => return None,
    };
    Some(named.to_string())
}

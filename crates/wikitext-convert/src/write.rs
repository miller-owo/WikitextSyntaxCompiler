use crate::markup::{Block, Cell, Inline, List, Table};
use std::fmt::Write as _;

pub fn write(blocks: &[Block]) -> String {
    render_blocks(blocks)
}

pub(crate) fn write_inlines(inlines: &[Inline]) -> String {
    let mut writer = Writer {
        out: String::new(),
        separator: false,
    };
    writer.inlines(inlines);
    writer.out
}

struct Writer {
    out: String,
    separator: bool,
}

impl Writer {
    fn paragraph_break(&mut self) {
        if self.separator {
            self.out.push_str("\n\n");
        }
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.block(block);
        }
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Paragraph(inlines) => {
                self.paragraph_break();
                self.inlines(inlines);
                self.separator = true;
            }
            Block::Heading { level, content } => {
                self.paragraph_break();
                // MediaWiki needs two equals or more, so an h1 is written as
                // level 2: a single equals pair would not compile back.
                let marks = "=".repeat((*level).clamp(2, 7) as usize);
                let _ = write!(self.out, "{marks} ");
                self.inlines(content);
                let _ = write!(self.out, " {marks}");
                self.separator = true;
            }
            Block::List(list) => {
                self.paragraph_break();
                self.list(list, 0);
                self.separator = true;
            }
            Block::Pre { language, text } => {
                self.paragraph_break();
                let closing = match language {
                    Some(language) => {
                        let _ = writeln!(self.out, "<syntaxhighlight lang=\"{language}\">");
                        "</syntaxhighlight>"
                    }
                    None => {
                        self.out.push_str("<pre>");
                        "</pre>"
                    }
                };
                self.out.push_str(&escape_raw(text));
                self.out.push('\n');
                self.out.push_str(closing);
                self.separator = true;
            }
            Block::Quote(blocks) => {
                self.paragraph_break();
                let inner = render_blocks(blocks);
                for (index, line) in inner.lines().enumerate() {
                    if index > 0 {
                        self.out.push('\n');
                    }
                    let _ = write!(self.out, "<blockquote>{line}</blockquote>");
                }
                self.separator = true;
            }
            Block::Rule => {
                self.paragraph_break();
                self.out.push_str("----");
                self.separator = true;
            }
            Block::Table(table) => {
                self.paragraph_break();
                self.table(table);
                self.separator = true;
            }
            Block::Raw(text) => {
                self.paragraph_break();
                self.out.push_str(text.trim());
                self.separator = true;
            }
        }
    }

    fn list(&mut self, list: &List, _depth: usize) {
        let root = list.items.iter();
        let mut first = true;
        for item in root {
            let prepared = item_of(item, list.ordered);
            let mut rendered = String::new();
            prepared.render("", &mut rendered, &mut first);
            let _ = self.out.write_str(&rendered);
        }
    }
    fn table(&mut self, table: &Table) {
        self.out.push_str("{| class=\"wikitable\"");
        let columns = table
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0);
        for row in &table.rows {
            self.out.push_str("\n|-");
            for cell in &row.cells {
                self.out.push('\n');
                self.cell(cell);
            }
            for _ in row.cells.len()..columns {
                self.out.push_str("\n|");
            }
        }
        self.out.push_str("\n|}");
    }

    fn cell(&mut self, cell: &Cell) {
        let marker = if cell.header { '!' } else { '|' };
        self.out.push(marker);
        self.out.push(' ');
        let inner = render_blocks(&cell.content);
        let _ = self.out.write_str(&inner.replace('\n', " "));
    }

    fn inlines(&mut self, inlines: &[Inline]) {
        for inline in inlines {
            self.inline(inline);
        }
    }

    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Text(text) => self.out.push_str(&escape_text(text)),
            // Bold inside italic is five apostrophes, not three then two: the
            // apostrophe marking is positional, so nesting has to be fused.
            Inline::Bold(content) if is_italic(content) => {
                let Inline::Italic(inner) = &content[0] else {
                    unreachable!()
                };
                self.out.push_str("'''''");
                self.inlines(inner);
                self.out.push_str("'''''");
            }
            Inline::Italic(content) if is_bold(content) => {
                let Inline::Bold(inner) = &content[0] else {
                    unreachable!()
                };
                self.out.push_str("'''''");
                self.inlines(inner);
                self.out.push_str("'''''");
            }
            Inline::Bold(content) => {
                self.out.push_str("'''");
                self.inlines(content);
                self.out.push_str("'''");
            }
            Inline::Italic(content) => {
                self.out.push_str("''");
                self.inlines(content);
                self.out.push_str("''");
            }
            Inline::Under(content) => self.tag("u", content),
            Inline::Struck(content) => self.tag("s", content),
            Inline::Sub(content) => self.tag("sub", content),
            Inline::Sup(content) => self.tag("sup", content),
            Inline::Code(text) => {
                let _ = write!(self.out, "<code>{}</code>", escape_raw(text));
            }
            Inline::Link { target, label } => {
                self.out.push_str("[[");
                self.out.push_str(&escape_target(target));
                if !is_redundant_label(label, target) {
                    self.out.push('|');
                    self.inlines(label);
                }
                self.out.push_str("]]");
            }
            Inline::External { url, label } => {
                let _ = write!(self.out, "[{} {}]", escape_url(url), write_inlines(label));
            }
            Inline::Image { target, alt } => {
                self.out.push_str("[[");
                self.out.push_str(&escape_target(target));
                if !alt.is_empty() {
                    self.out.push_str("|alt=");
                    self.inlines(alt);
                }
                self.out.push_str("]]");
            }
            Inline::LineBreak => self.out.push_str("<br>"),
            Inline::Raw(text) => self.out.push_str(text),
        }
    }

    fn tag(&mut self, name: &str, content: &[Inline]) {
        let _ = write!(self.out, "<{name}>");
        self.inlines(content);
        let _ = write!(self.out, "</{name}>");
    }
}

pub(crate) fn render_blocks(blocks: &[Block]) -> String {
    let mut writer = Writer {
        out: String::new(),
        separator: false,
    };
    writer.blocks(blocks);
    writer.out.trim_end().to_string()
}

struct Item {
    text: Option<String>,
    nested: Vec<Item>,
    ordered: bool,
}

impl Item {
    // Wikitext marks nesting positionally: every ancestor's list kind comes
    // first and the item's own kind last, so a bullet inside a bullet inside an
    // ordered list is `*#*`.
    fn render(&self, prefix: &str, out: &mut String, first: &mut bool) {
        let marker = format!("{prefix}{}", self.kind());
        let mut started = false;
        if let Some(text) = &self.text {
            if !*first {
                out.push('\n');
            }
            *first = false;
            started = true;
            out.push_str(&marker);
            out.push(' ');
            out.push_str(text);
        }
        for child in &self.nested {
            // The first nested item carries this item's marker so it continues
            // the same line as its parent; the rest start their own.
            if started {
                child.render(&marker, out, &mut false);
                started = false;
                continue;
            }
            child.render(&marker, out, first);
        }
    }

    fn kind(&self) -> char {
        if self.ordered {
            '#'
        } else {
            '*'
        }
    }
}

fn item_of(blocks: &[Block], ordered: bool) -> Item {
    let mut item = Item {
        text: None,
        nested: Vec::new(),
        ordered,
    };
    for block in blocks {
        match block {
            Block::List(nested) => {
                for inner in &nested.items {
                    item.nested.push(item_of(inner, nested.ordered));
                }
            }
            other => {
                let text = render_blocks(std::slice::from_ref(other));
                for line in text.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match &mut item.text {
                        Some(existing) => {
                            existing.push('\n');
                            existing.push_str(line);
                        }
                        None => item.text = Some(line.to_string()),
                    }
                }
            }
        }
    }
    item
}
fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '[' => out.push_str("&#91;"),
            ']' => out.push_str("&#93;"),
            '{' => out.push_str("&#123;"),
            '}' => out.push_str("&#125;"),
            other => out.push(other),
        }
    }
    out
}

pub(crate) fn escape_raw(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_target(target: &str) -> String {
    let mut out = String::with_capacity(target.len());
    for ch in target.chars() {
        match ch {
            '|' => out.push_str("{{!}}"),
            '[' => out.push_str("&#91;"),
            ']' => out.push_str("&#93;"),
            '{' => out.push_str("&#123;"),
            '}' => out.push_str("&#125;"),
            '\n' | '\r' | '\t' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

fn escape_url(url: &str) -> String {
    url.chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .replace(']', "%5D")
        .replace('[', "%5B")
}

fn is_italic(content: &[Inline]) -> bool {
    matches!(content, [Inline::Italic(_)])
}

fn is_bold(content: &[Inline]) -> bool {
    matches!(content, [Inline::Bold(_)])
}

fn is_redundant_label(label: &[Inline], target: &str) -> bool {
    match label {
        [Inline::Text(text)] => text.trim() == target.trim(),
        _ => false,
    }
}

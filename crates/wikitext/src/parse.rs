use crate::dom::{Attribute, Document, Element, ElementKind, Node, Tag};
use crate::error::{Diagnostic, DiagnosticCode, Position};
use crate::lex::{
    self, split_list_markers, ExtTagToken, Line, LineKind, LineScanner, LinkPart, LinkToken,
    TableLine, TableSyntax, TagToken, Token,
};
use crate::options::Options;
use crate::references;
use crate::scan;
use std::collections::HashMap;
pub struct ParseContext {
    pub options: Options,
    pub document: Document,
    heading_ids: HashMap<String, usize>,
    source_len: usize,
    source: String,
    nesting: usize,
    // Block-level HTML tags opened as raw markup on an earlier line. Their
    // closing tag may sit after text on a later line, where the line-local
    // builder cannot see the opener.
    html_blocks: Vec<&'static str>,
    // How many open entries each name has. A closer with no entry is ignored in
    // O(1), instead of scanning the whole stack for every unmatched `</div>`.
    html_block_counts: HashMap<&'static str, usize>,
}

// Nesting beyond this is markup that has run away rather than markup anyone
// wrote, so it is left as literal text. Without a ceiling the recursion needed to
// re-parse a label overflows the stack, and a page of 800 nested links is only
// 1.6 KB of source.
const MAX_NESTING: usize = 40;

impl ParseContext {
    pub fn new(options: Options) -> Self {
        ParseContext {
            options,
            document: Document::new(),
            heading_ids: HashMap::new(),
            nesting: 0,
            source_len: 0,
            source: String::new(),
            html_blocks: Vec::new(),
            html_block_counts: HashMap::new(),
        }
    }

    fn push_html_block(&mut self, name: &'static str) {
        if !crate::dom::is_void(name) {
            self.html_blocks.push(name);
            *self.html_block_counts.entry(name).or_insert(0) += 1;
        }
    }

    // Returns whether a matching block was open. Closing one implicitly closes
    // any still-open block above it, which is how the browser reads the markup.
    fn close_html_block(&mut self, name: &str) -> bool {
        if self.html_block_counts.get(name).copied().unwrap_or(0) == 0 {
            return false;
        }
        let Some(position) = self.html_blocks.iter().rposition(|open| *open == name) else {
            return false;
        };
        // Every entry removed by the truncation leaves the count, so a later
        // unmatched closer never scans for a name that is not there.
        for index in position..self.html_blocks.len() {
            let open = self.html_blocks[index];
            if let Some(count) = self.html_block_counts.get_mut(open) {
                *count -= 1;
                if *count == 0 {
                    self.html_block_counts.remove(open);
                }
            }
        }
        self.html_blocks.truncate(position);
        true
    }

    fn warn(&mut self, code: DiagnosticCode, message: impl Into<String>, offset: usize) {
        if !self.options.diagnostics {
            return;
        }
        let (line, column) = self.line_column(offset);
        let position = Position {
            offset,
            line,
            column,
        };
        self.document
            .warn(Diagnostic::warning(code, message, position));
    }

    // Diagnostics carry byte offsets, but a caller reads line and column. Both are
    // counted from one, and the column counts characters rather than bytes so it
    // lines up with what an editor shows.
    fn line_column(&self, offset: usize) -> (u32, u32) {
        let offset = offset.min(self.source.len());
        let before = &self.source[..offset];
        let line = before.matches('\n').count() as u32 + 1;
        let line_start = before.rfind('\n').map(|at| at + 1).unwrap_or(0);
        let column = self.source[line_start..offset].chars().count() as u32 + 1;
        (line, column)
    }
}

pub fn parse(source: &str, options: &Options) -> Document {
    let mut context = ParseContext::new(options.clone());
    parse_with_context(source, &mut context);
    context.document.trailing_newline = source.ends_with('\n');
    context.document
}

pub fn parse_with_context(source: &str, context: &mut ParseContext) {
    context.source_len = source.len();
    context.html_blocks.clear();
    context.html_block_counts.clear();

    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    context.source = source.to_string();
    let mut parser = BlockParser {
        scanner: LineScanner::new(source),
        context,
    };
    parser.run();
}

struct BlockParser<'a, 'c> {
    scanner: LineScanner<'a>,
    context: &'c mut ParseContext,
}

impl BlockParser<'_, '_> {
    fn run(&mut self) {
        let mut paragraph = Paragraph::default();
        let mut list: Option<ListState> = None;

        while let Some(line) = self.scanner.peek_line() {
            let offset = line.start;
            let kind = line.kind();

            let extends_list = matches!(kind, LineKind::Blank | LineKind::List);
            if !extends_list {
                self.flush_list(&mut list);
            }
            if kind == LineKind::Blank {
                self.scanner.next_line();
                paragraph.flush(self.context);
                continue;
            }

            match kind {
                LineKind::Heading => {
                    self.scanner.next_line();
                    paragraph.flush(self.context);
                    self.heading(line, offset);
                }
                LineKind::HorizontalRule => {
                    self.scanner.next_line();
                    paragraph.flush(self.context);
                    if line.content.trim_start_matches('-').trim().is_empty() {
                        self.context.document.push(Node::element(
                            Element::new("hr").with_kind(ElementKind::HorizontalRule),
                        ));
                    } else {
                        self.context.document.push(Node::element(
                            Element::new("hr").with_kind(ElementKind::HorizontalRule),
                        ));
                        let rest = line.content.trim_start_matches('-');
                        let tokens = lex::lex_inline(rest);
                        let nodes = self.parse_inline(&tokens);
                        paragraph.push_nodes(nodes);
                        paragraph.flush(self.context);
                    }
                }
                LineKind::Preformatted => {
                    paragraph.flush(self.context);
                    let mut buffer = String::new();
                    while let Some(next) = self.scanner.peek_line() {
                        if next.kind() != LineKind::Preformatted {
                            break;
                        }
                        let consumed = self.scanner.next_line().expect("peeked");
                        if !buffer.is_empty() {
                            buffer.push('\n');
                        }
                        buffer.push_str(
                            consumed
                                .content
                                .strip_prefix(' ')
                                .unwrap_or(consumed.content),
                        );
                    }
                    self.context.document.push(Node::element(
                        Element::new("pre")
                            .with_kind(ElementKind::Preformatted)
                            .with_child(Node::text(buffer)),
                    ));
                }
                LineKind::TableStart => {
                    paragraph.flush(self.context);
                    let table = self.table(offset);
                    self.context.document.push(Node::element(table));
                }
                LineKind::Text if starts_block_ext_tag(line.content) => {
                    paragraph.flush(self.context);
                    self.ext_tag_block(offset);
                }
                LineKind::Text if starts_html_block(line.content) => {
                    paragraph.flush(self.context);
                    self.html_block();
                }
                LineKind::List => {
                    paragraph.flush(self.context);
                    let consumed = self.scanner.next_line().expect("peeked");
                    self.list_item(consumed, &mut list);
                }
                LineKind::TableRowOrCell => {
                    paragraph.flush(self.context);

                    let consumed = self.scanner.next_line().expect("peeked");
                    let tokens = lex::lex_inline(consumed.content);
                    let nodes = self.parse_inline(&tokens);
                    paragraph.push_nodes(nodes);
                    paragraph.flush(self.context);
                }
                _ => {
                    let consumed = self.scanner.next_line().expect("peeked");
                    let tokens = lex::lex_inline(consumed.content);
                    let nodes = self.parse_inline(&tokens);
                    self.push_inline_nodes(&mut paragraph, nodes);
                }
            }
        }

        self.flush_list(&mut list);
        paragraph.flush(self.context);
    }

    fn push_inline_nodes(&mut self, paragraph: &mut Paragraph, nodes: Vec<Node>) {
        let mut pending: Vec<Node> = Vec::new();
        for node in nodes {
            if is_raw_block_close(&node) {
                if !pending.is_empty() {
                    paragraph.push_nodes(core::mem::take(&mut pending));
                }
                paragraph.flush(self.context);
                self.context.document.push(node);
            } else {
                pending.push(node);
            }
        }
        // A picture alone on a line is a block in MediaWiki, not a paragraph:
        // wrapping it in `<p>` moves it out of the place the template expects.
        if paragraph.is_empty() && !pending.is_empty() && pending.iter().all(is_media_node) {
            paragraph.flush(self.context);
            for node in pending {
                self.context.document.push(node);
            }
            return;
        }
        paragraph.push_nodes(pending);
    }

    fn flush_list(&mut self, list: &mut Option<ListState>) {
        let Some(mut state) = list.take() else {
            return;
        };
        // An item keeps its source text as its only child until the list ends,
        // but an item is attached below the outermost list as soon as the next
        // one starts. Walking just the still-open frames therefore missed every
        // finished item, leaving its inline markup (`'''bold'''`, links, …) as
        // literal text. Parse each open frame's subtree instead, which covers
        // both the attached items and the one that is still open.
        for index in 0..state.frames.len() {
            let element = state.frames[index].element_mut();
            self.parse_item_text(element);
        }
        state.finish(self.context);
    }

    // Replace an item's raw text child with parsed inline nodes. Nested children
    // (deeper lists) are kept and visited too.
    fn parse_item_text(&mut self, element: &mut Element) {
        if matches!(
            element.kind,
            ElementKind::UnorderedItem
                | ElementKind::OrderedItem
                | ElementKind::TermItem
                | ElementKind::DefinitionItem
        ) {
            if let Some(Node::Text(text)) = element.children.first().cloned() {
                element.children.remove(0);
                let nodes = self.parse_inline_text(&text);
                element.children.splice(0..0, nodes);
            }
        }
        for child in &mut element.children {
            if let Node::Element(child_element) = child {
                self.parse_item_text(child_element);
            }
        }
    }

    fn heading(&mut self, line: Line<'_>, offset: usize) {
        let trimmed = line.trimmed_end();
        let content = match parse_heading(trimmed) {
            Some(parsed) => parsed,
            None => {
                let tokens = lex::lex_inline(trimmed);
                let nodes = self.parse_inline(&tokens);
                let mut paragraph = Paragraph::default();
                paragraph.push_nodes(nodes);
                paragraph.flush(self.context);
                return;
            }
        };
        let level = content.level.min(self.context.options.max_heading_level);
        let tag: Tag = Tag::from(format!("h{level}"));
        let tokens = lex::lex_inline(content.text);
        let children = self.parse_inline(&tokens);
        let mut element = Element::new(tag).with_kind(ElementKind::Heading);
        let title = self.context.document.text_content_of(&children);
        element.children = children;
        let id = self.next_heading_id(content.text);
        element.set_attr("id", id.clone());
        self.context
            .document
            .headings
            .push(crate::dom::Heading { level, id, title });
        self.context.document.push(Node::element(element));

        if !content.trailing.is_empty() {
            let tokens = lex::lex_inline(content.trailing);
            let nodes = self.parse_inline(&tokens);
            let mut paragraph = Paragraph::default();
            paragraph.push_nodes(nodes);
            paragraph.flush(self.context);
        }
        let _ = offset;
    }

    fn next_heading_id(&mut self, text: &str) -> String {
        let mut base = heading_anchor(text);
        if base.is_empty() {
            base = "section".to_string();
        }
        let counter = self
            .context
            .heading_ids
            .entry(base.clone())
            .and_modify(|count| *count += 1)
            .or_insert(0);
        if *counter == 0 {
            base
        } else {
            format!("{base}_{}", *counter + 1)
        }
    }

    fn list_item(&mut self, line: Line<'_>, list: &mut Option<ListState>) {
        let (markers, content_start) = split_list_markers(line.content);
        let content = &line.content[content_start..];
        let markers = markers_to_frames(&markers);
        let state = list.get_or_insert_with(ListState::new);
        state.apply(&markers, content, self.context);
    }

    fn html_block(&mut self) {
        let line = self.scanner.next_line().expect("peeked");
        let tokens = lex::lex_inline(line.content);
        let nodes = self.parse_inline_html_block(&tokens);
        for node in nodes {
            self.context.document.push(node);
        }
    }

    fn ext_tag_block(&mut self, offset: usize) {
        let remaining = self.scanner.remaining();
        let Some(tag) = crate::extensions::scan_ext_tag(remaining) else {
            let line = self.scanner.next_line().expect("peeked");
            let tokens = lex::lex_inline(line.content);
            let nodes = self.parse_inline(&tokens);
            let mut paragraph = Paragraph::default();
            paragraph.push_nodes(nodes);
            paragraph.flush(self.context);
            return;
        };
        let tag_source = remaining[..tag.end].to_string();
        self.scanner.skip_to_offset(offset + tag.end);
        let open_tag = &tag_source[..tag_source
            .find('>')
            .map(|i| i + 1)
            .unwrap_or(tag_source.len())];
        let attributes = decode_attributes(attribute_source(open_tag));
        let content = tag.content.clone().unwrap_or_default();
        let nodes = match crate::extensions::render(&tag.name, &attributes, &content) {
            crate::extensions::Rendered::Nodes(nodes) => nodes,
            crate::extensions::Rendered::References { group } => self.build_references_list(&group),
        };
        for node in nodes {
            self.context.document.push(node);
        }
    }

    fn build_references_list(&mut self, group: &str) -> Vec<Node> {
        let mut nodes = self.context.document.references.render(group);

        for node in &mut nodes {
            self.parse_reference_text(node);
        }
        nodes
    }

    fn parse_reference_text(&mut self, node: &mut Node) {
        let mut parsed: Vec<Node> = match node {
            Node::Text(text) => {
                let nodes = self.parse_inline_text(text);
                if nodes.is_empty() {
                    return;
                }
                nodes
            }
            Node::Element(element) => {
                for child in &mut element.children {
                    self.parse_reference_text(child);
                }
                return;
            }
            _ => return,
        };
        match parsed.len() {
            1 => *node = parsed.pop().expect("one node"),
            _ => {
                let mut wrapper = Element::new("span");
                wrapper.children = parsed;
                *node = Node::element(wrapper);
            }
        }
    }

    fn table(&mut self, offset: usize) -> Element {
        let header = self.scanner.next_line().expect("peeked");
        let attributes_source = header
            .content
            .strip_prefix("{|")
            .unwrap_or(header.content)
            .to_string();
        let mut lines = Vec::new();
        let mut depth = 1usize;
        while let Some(line) = self.scanner.peek_line() {
            if line.kind() == LineKind::TableStart {
                depth += 1;
            } else if line.kind() == LineKind::TableEnd {
                depth -= 1;
                if depth == 0 {
                    self.scanner.next_line();
                    break;
                }
            }
            let consumed = self.scanner.next_line().expect("peeked");
            lines.push(TableLine {
                text: consumed.trimmed_end().to_string(),
                offset: consumed.start,
            });
        }
        if depth != 0 {
            self.context.warn(
                DiagnosticCode::UnclosedTable,
                "table was never closed with `|}`",
                offset,
            );
        }
        let syntax = TableSyntax {
            attributes: attributes_source,
            lines,
        };
        self.build_table(&syntax)
    }

    fn build_table(&mut self, syntax: &TableSyntax) -> Element {
        let mut table = Element::new("table");
        table.attributes = decode_attributes(&syntax.attributes);

        let mut index = 0usize;
        if let Some(line) = syntax.lines.first() {
            if let Some(caption) = line.text.strip_prefix("|+") {
                let tokens = lex::lex_inline(caption.trim());
                let children = self.parse_inline(&tokens);
                let mut element = Element::new("caption");
                element.children = children;
                table.children.push(Node::element(element));
                index = 1;
            }
        }

        let mut row: Option<Element> = None;
        let mut section = String::new();
        while index < syntax.lines.len() {
            let line = &syntax.lines[index];
            let text = line.text.as_str();
            if text.starts_with("|-") {
                if let Some(previous) = row.take() {
                    table.children.push(Node::element(previous));
                }
                let mut new_row = Element::new("tr");
                let attributes = text.strip_prefix("|-").unwrap_or(text).trim();
                if !attributes.is_empty() {
                    new_row.attributes = decode_attributes(attributes);
                }
                row = Some(new_row);
            } else if text.starts_with('|') || text.starts_with('!') {
                if row.is_none() {
                    row = Some(Element::new("tr"));
                }
                let cells = split_cells(text);
                for cell in cells {
                    let tag = if cell.header { "th" } else { "td" };
                    let mut element = Element::new(tag);
                    let (attributes, content) = split_cell_attributes(&cell.content);
                    element.attributes = decode_attributes(attributes);
                    let content = content.replace('\n', " ");
                    let tokens = lex::lex_inline(content.trim());
                    element.children = self.parse_inline(&tokens);
                    if let Some(open_row) = row.as_mut() {
                        open_row.children.push(Node::element(element));
                    }
                }
            } else if let Some(open_row) = row.as_mut() {
                if let Some(last) = open_row.children.last_mut().and_then(Node::as_element_mut) {
                    if !last.children.is_empty() {
                        last.children.push(Node::Text("\n".to_string()));
                    }
                    let tokens = lex::lex_inline(text);
                    let extra = self.parse_inline(&tokens);
                    last.children.extend(extra);
                }
            }
            index += 1;
        }
        if let Some(last) = row.take() {
            table.children.push(Node::element(last));
        }
        let _ = &mut section;
        table
    }

    fn parse_inline(&mut self, tokens: &[Token]) -> Vec<Node> {
        self.parse_inline_in(tokens, false)
    }

    // A line that begins with a block-level HTML tag is raw HTML to the browser:
    // `<div style="…">` opens an element that later lines fill, and `</div>` on a
    // line of its own closes it. The tag cannot be balanced against its closer
    // here, so it is emitted verbatim instead of becoming literal text.
    fn parse_inline_html_block(&mut self, tokens: &[Token]) -> Vec<Node> {
        self.parse_inline_in(tokens, true)
    }

    fn parse_inline_in(&mut self, tokens: &[Token], html_block: bool) -> Vec<Node> {
        let mut builder = InlineBuilder {
            tokens,
            index: 0,
            context: self.context,
            open_tags: HashMap::new(),
            closers_ahead: InlineBuilder::closer_counts(tokens),
            html_block,
        };
        builder.run()
    }

    fn parse_inline_text(&mut self, text: &str) -> Vec<Node> {
        // A label is markup in its own right, so it is parsed again — which
        // recurses once per nesting level. Past the ceiling the text is kept as
        // it stands rather than followed any deeper.
        if self.context.nesting >= MAX_NESTING {
            return vec![Node::text(text.to_string())];
        }
        self.context.nesting += 1;
        let tokens = lex::lex_inline(text);
        let nodes = self.parse_inline(&tokens);
        self.context.nesting -= 1;
        nodes
    }
}

#[derive(Default)]
struct Paragraph {
    nodes: Vec<Node>,
    needs_break: bool,
}

impl Paragraph {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn push_nodes(&mut self, nodes: Vec<Node>) {
        if nodes.is_empty() {
            return;
        }
        if self.needs_break && !self.nodes.is_empty() {
            self.nodes.push(Node::Raw("\n".to_string()));
        }
        self.nodes.extend(nodes);
        self.needs_break = true;
    }

    fn flush(&mut self, context: &mut ParseContext) {
        if self.nodes.is_empty() {
            self.needs_break = false;
            return;
        }
        let nodes = core::mem::take(&mut self.nodes);
        self.needs_break = false;
        context.document.push(Node::element(
            Element::new("p")
                .with_kind(ElementKind::Paragraph)
                .with_children(nodes),
        ));
    }
}

fn is_raw_block_close(node: &Node) -> bool {
    let Node::Raw(raw) = node else {
        return false;
    };
    let raw = raw.trim_start();
    if !raw.starts_with("</") {
        return false;
    }
    scan::scan_tag(raw).is_some_and(|tag| scan::is_block_tag(tag.name))
}

fn is_media_node(node: &Node) -> bool {
    matches!(node, Node::Element(element) if element.kind == ElementKind::Image)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ListKind {
    Bullet,
    Number,
    Definition,
}

impl ListKind {
    fn tag(self) -> &'static str {
        match self {
            ListKind::Bullet => "ul",
            ListKind::Number => "ol",
            ListKind::Definition => "dl",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ItemKind {
    Bullet,
    Number,
    Term,
    Definition,
}

impl ItemKind {
    fn tag(self) -> &'static str {
        match self {
            ItemKind::Bullet | ItemKind::Number => "li",
            ItemKind::Term => "dt",
            ItemKind::Definition => "dd",
        }
    }

    fn element_kind(self) -> ElementKind {
        match self {
            ItemKind::Bullet => ElementKind::UnorderedItem,
            ItemKind::Number => ElementKind::OrderedItem,
            ItemKind::Term => ElementKind::TermItem,
            ItemKind::Definition => ElementKind::DefinitionItem,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Marker {
    Bullet,
    Number,
    Semicolon,
    Colon,
}

impl Marker {
    fn list(self) -> ListKind {
        match self {
            Marker::Bullet => ListKind::Bullet,
            Marker::Number => ListKind::Number,
            Marker::Semicolon | Marker::Colon => ListKind::Definition,
        }
    }

    fn item(self) -> ItemKind {
        match self {
            Marker::Bullet => ItemKind::Bullet,
            Marker::Number => ItemKind::Number,
            Marker::Semicolon => ItemKind::Term,
            Marker::Colon => ItemKind::Definition,
        }
    }

    fn container_item(self) -> ItemKind {
        self.item()
    }
}

enum ListFrame {
    List(Element, ListKind),

    Item(Element),
}

impl ListFrame {
    fn into_element(self) -> Element {
        match self {
            ListFrame::List(element, _) | ListFrame::Item(element) => element,
        }
    }

    fn element_mut(&mut self) -> &mut Element {
        match self {
            ListFrame::List(element, _) | ListFrame::Item(element) => element,
        }
    }
}

struct ListState {
    frames: Vec<ListFrame>,
}

impl ListState {
    fn new() -> Self {
        ListState { frames: Vec::new() }
    }

    fn apply(&mut self, markers: &[Marker], content: &str, context: &mut ParseContext) {
        let text = content.trim_start();

        if markers[0] == Marker::Semicolon {
            if let Some((term, definition)) = text.split_once(':') {
                self.apply_definition_line(&markers[1..], term.trim(), definition.trim(), context);
                return;
            }
        }
        let target = markers.len() * 2;
        while self.frames.len() > target {
            self.pop_frame(context);
        }
        for (index, marker) in markers.iter().enumerate() {
            let list_slot = index * 2;
            let item_slot = list_slot + 1;
            let matches_list = matches!(
                self.frames.get(list_slot),
                Some(ListFrame::List(_, open)) if *open == marker.list()
            );
            if !matches_list {
                while self.frames.len() > list_slot {
                    self.pop_frame(context);
                }
                if index > 0 && !matches!(self.frames.get(list_slot - 1), Some(ListFrame::Item(..)))
                {
                    self.push_item(markers[index - 1].container_item(), String::new());
                }
                self.push_list(marker.list());
            }
            if index + 1 != markers.len() {
                continue;
            }

            while self.frames.len() > item_slot {
                self.pop_frame(context);
            }
            if *marker == Marker::Semicolon {
                self.push_item(ItemKind::Term, text.trim().to_string());
            } else {
                self.push_item(marker.item(), text.to_string());
            }
        }
    }

    fn apply_definition_line(
        &mut self,
        rest: &[Marker],
        term: &str,
        definition: &str,
        context: &mut ParseContext,
    ) {
        while self.frames.len() > 1 {
            self.pop_frame(context);
        }
        if !matches!(self.frames.first(), Some(ListFrame::List(..))) {
            self.push_list(ListKind::Definition);
        }
        self.push_item(ItemKind::Term, term.to_string());

        while self.frames.len() > 1 {
            self.pop_frame(context);
        }
        if rest.is_empty() {
            self.push_item(ItemKind::Definition, definition.to_string());
            return;
        }
        self.push_item(ItemKind::Definition, String::new());
        self.apply_nested(rest, definition, context);
    }

    fn apply_nested(&mut self, markers: &[Marker], text: &str, context: &mut ParseContext) {
        for (index, marker) in markers.iter().enumerate() {
            let list_slot = self.frames.len();
            if !matches!(
                self.frames.get(list_slot),
                Some(ListFrame::List(_, open)) if *open == marker.list()
            ) {
                self.push_list(marker.list());
            }
            let is_last = index + 1 == markers.len();
            if is_last {
                while self.frames.len() > list_slot + 1 {
                    self.pop_frame(context);
                }
                self.push_item(marker.item(), text.to_string());
            } else {
                self.push_item(marker.container_item(), String::new());
            }
        }
    }
    fn push_list(&mut self, kind: ListKind) {
        self.frames
            .push(ListFrame::List(Element::new(kind.tag()), kind));
    }

    fn push_item(&mut self, item: ItemKind, content: String) {
        let mut element = Element::new(item.tag()).with_kind(item.element_kind());
        if !content.is_empty() {
            element.children.push(Node::Text(content));
        }
        self.frames.push(ListFrame::Item(element));
    }

    // An item is attached to its list, a list to the item that precedes it, and
    // the outermost list to the document: that is what makes sibling markers
    // siblings rather than children.
    fn pop_frame(&mut self, context: &mut ParseContext) {
        let Some(frame) = self.frames.pop() else {
            return;
        };
        let element = frame.into_element();
        match self.frames.last_mut() {
            Some(ListFrame::List(parent, _)) => parent.children.push(Node::element(element)),

            Some(ListFrame::Item(parent)) => parent.children.push(Node::element(element)),

            None => context.document.push(Node::element(element)),
        }
    }

    fn finish(&mut self, context: &mut ParseContext) {
        while !self.frames.is_empty() {
            self.pop_frame(context);
        }
    }
}

fn markers_to_frames(markers: &str) -> Vec<Marker> {
    let parsed: Vec<Marker> = markers
        .bytes()
        .map(|byte| match byte {
            b'*' => Marker::Bullet,
            b'#' => Marker::Number,
            b';' => Marker::Semicolon,
            _ => Marker::Colon,
        })
        .collect();
    if parsed.is_empty() {
        vec![Marker::Bullet]
    } else {
        parsed
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum DelimiterKind {
    Italic,
    Bold,

    Tag(&'static str),
}

struct Block {
    element: Element,
    opened_by: DelimiterKind,
    open_index: usize,
    children: Vec<Node>,
}

struct InlineBuilder<'t, 'c> {
    tokens: &'t [Token],
    index: usize,
    context: &'c mut ParseContext,
    // Where each open tag sits in the block stack. Searching the stack by name
    // for every closing tag is linear per tag, which makes a page with many tags
    // quadratic; this turns the lookup into a hash.
    open_tags: HashMap<&'static str, Vec<usize>>,
    // Names that still have a closing tag ahead of the cursor. Scanning the
    // remaining tokens per opener is linear per tag, and a page whose tags are
    // never closed has one opener for every token.
    closers_ahead: HashMap<&'static str, usize>,
    // The line starts with a block-level HTML tag, so block tags belong to the
    // surrounding document rather than to this line.
    html_block: bool,
}

impl<'t, 'c> InlineBuilder<'t, 'c> {
    fn run(&mut self) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        let mut stack: Vec<Block> = Vec::new();
        self.run_into_stack(&mut stack, &mut out);

        while let Some(block) = stack.pop() {
            // An HTML-block line may leave a block tag open, e.g. a wrapper that
            // starts and ends inside a line that also opens its parent. That is
            // markup the following lines will fill, not a broken tag.
            if self.html_block {
                if let DelimiterKind::Tag(name) = block.opened_by {
                    if scan::is_block_tag(name) {
                        let source = self.tag_source_at(block.open_index);
                        self.context.push_html_block(name);
                        out.push(Node::Raw(source));
                        out.extend(block.children);
                        continue;
                    }
                }
            }
            append_literalised(block, &mut out);
        }
        out
    }

    fn run_into(&mut self, out: &mut Vec<Node>) {
        let mut stack: Vec<Block> = Vec::new();
        self.run_into_stack(&mut stack, out);
        while let Some(block) = stack.pop() {
            append_literalised(block, out);
        }
    }

    fn run_into_stack(&mut self, stack: &mut Vec<Block>, out: &mut Vec<Node>) {
        while self.index < self.tokens.len() {
            // The cursor is about to pass this token, so a closer it holds stops
            // counting as ahead.
            self.retire_closer(self.index);
            let token = &self.tokens[self.index];
            match token {
                Token::Text(text) if text.bytes().all(|byte| byte == b'\'') => {
                    let count = text.len();
                    self.handle_quotes(count, stack, out);
                }
                Token::Text(text) => {
                    let text = text.clone();
                    push_text_node(self.top_children(stack, out), &text);
                    self.index += 1;
                }
                Token::Entity(value) => {
                    let value = value.clone();
                    push_text_node(self.top_children(stack, out), &value);
                    self.index += 1;
                }
                Token::Raw(value) => {
                    let value = value.clone();
                    push_text_node(self.top_children(stack, out), &value);
                    self.index += 1;
                }
                Token::Comment(value) => {
                    if self.context.options.keep_comments {
                        let comment = Node::comment(value.clone());
                        self.top_children(stack, out).push(comment);
                    }
                    self.index += 1;
                }
                Token::Newline => {
                    self.top_children(stack, out)
                        .push(Node::Raw("\n".to_string()));
                    self.index += 1;
                }
                Token::TagSelfClosing(tag) => {
                    let node = self.self_closing_tag(tag);
                    self.top_children(stack, out).push(node);
                    self.index += 1;
                }
                Token::TagStart(tag) => {
                    let name = tag.name;
                    self.handle_tag_start(name, stack, out);
                }
                Token::ExtTag(tag) => {
                    let tag = tag.clone();
                    let nodes = self.handle_ext_tag(&tag);
                    self.top_children(stack, out).extend(nodes);
                    self.index += 1;
                }
                Token::TagEnd { name, source } => {
                    let name = *name;
                    let source = source.clone();
                    self.handle_tag_end(name, &source, stack, out);
                }
                Token::InternalLink(link) => {
                    let link = link.clone();
                    let nodes = self.internal_link(&link);
                    self.top_children(stack, out).extend(nodes);
                    self.index += 1;
                }
                Token::ExternalLink(link) => {
                    let link = link.clone();
                    let nodes = self.external_link(&link);
                    self.top_children(stack, out).extend(nodes);
                    self.index += 1;
                }
                Token::Heading { .. }
                | Token::Table(_)
                | Token::Preformatted(_)
                | Token::ListItem { .. }
                | Token::HorizontalRule => {
                    self.index += 1;
                }
            }
        }
    }

    fn top_children<'s>(
        &self,
        stack: &'s mut [Block],
        out: &'s mut Vec<Node>,
    ) -> &'s mut Vec<Node> {
        match stack.last_mut() {
            Some(block) => &mut block.children,
            None => out,
        }
    }

    fn handle_quotes(&mut self, count: usize, stack: &mut Vec<Block>, out: &mut Vec<Node>) {
        let mut left = count;
        if left == 5 {
            let bold = stack.iter().rposition(|block| {
                block.opened_by == DelimiterKind::Bold && block.open_index <= self.index
            });
            let italic = stack.iter().rposition(|block| {
                block.opened_by == DelimiterKind::Italic && block.open_index <= self.index
            });
            match (bold, italic) {
                (Some(bold), Some(italic)) if italic > bold => {
                    self.close_block(italic, stack, out);
                    self.close_block(bold, stack, out);
                    left = 0;
                }
                (None, None) => {
                    self.open_block(DelimiterKind::Bold, stack);
                    self.open_block(DelimiterKind::Italic, stack);
                    left = 0;
                }
                _ => {}
            }
        }
        while left >= 3 {
            let position = stack.iter().rposition(|block| {
                block.opened_by == DelimiterKind::Bold && block.open_index <= self.index
            });
            let Some(position) = position else { break };
            self.close_block(position, stack, out);
            left -= 3;
        }
        while left >= 2 {
            let position = stack.iter().rposition(|block| {
                block.opened_by == DelimiterKind::Italic && block.open_index <= self.index
            });
            let Some(position) = position else { break };
            self.close_block(position, stack, out);
            left -= 2;
        }
        if left >= 3 {
            self.open_block(DelimiterKind::Bold, stack);
            left -= 3;
        }
        if left >= 2 {
            self.open_block(DelimiterKind::Italic, stack);
            left -= 2;
        }
        if left > 0 {
            let literal = "'".repeat(left);
            push_text_node(self.top_children(stack, out), &literal);
        }
        self.index += 1;
    }

    fn handle_tag_start(
        &mut self,
        name: &'static str,
        stack: &mut Vec<Block>,
        out: &mut Vec<Node>,
    ) {
        if !self.has_tag_closer_ahead(name) {
            let source = self.tag_source_at(self.index);
            // In an HTML block the tag is part of the page's own markup, so it
            // is written through. Anywhere else an unmatched opener would be a
            // broken document, and keeping it as text is the recovery.
            if self.html_block && scan::is_block_tag(name) {
                self.context.push_html_block(name);
                self.top_children(stack, out).push(Node::Raw(source));
            } else {
                push_text_node(self.top_children(stack, out), &source);
            }
            self.index += 1;
            return;
        }
        let kind = DelimiterKind::Tag(name);
        let attributes = match self.tokens.get(self.index) {
            Some(Token::TagStart(tag)) => decode_attributes_from(&tag.attributes),
            _ => Vec::new(),
        };
        let mut element = Element::new(name);
        element.attributes = attributes;
        self.open_tags.entry(name).or_default().push(stack.len());
        stack.push(Block {
            element,
            opened_by: kind,
            open_index: self.index,
            children: Vec::new(),
        });
        self.index += 1;
    }

    fn handle_tag_end(
        &mut self,
        name: &'static str,
        source: &str,
        stack: &mut Vec<Block>,
        out: &mut Vec<Node>,
    ) {
        let position = self
            .open_tags
            .get_mut(name)
            .and_then(|positions| positions.pop());
        match position {
            Some(position) => {
                self.close_block(position, stack, out);
            }
            None => {
                // A block closer may belong to a wrapper opened on an earlier
                // line. That is valid HTML wherever it appears, even when it
                // follows text (`caption</div>`), so write it through instead
                // of escaping it as text.
                if scan::is_block_tag(name) && self.context.close_html_block(name) {
                    self.top_children(stack, out)
                        .push(Node::Raw(source.to_string()));
                } else if self.html_block && scan::is_block_tag(name) {
                    // A stray closer on an HTML-block line stays markup, as the
                    // browser ignores a close with nothing open.
                    self.top_children(stack, out)
                        .push(Node::Raw(source.to_string()));
                } else {
                    let literal = source.to_string();
                    push_text_node(self.top_children(stack, out), &literal);
                }
            }
        }
        self.index += 1;
    }

    fn has_tag_closer_ahead(&self, name: &str) -> bool {
        self.closers_ahead.get(name).is_some_and(|count| *count > 0)
    }

    // Built once by walking backwards: a closing tag is ahead of the cursor
    // while the cursor has not passed it.
    fn closer_counts(tokens: &[Token]) -> HashMap<&'static str, usize> {
        let mut counts: HashMap<&'static str, usize> = HashMap::new();
        for token in tokens.iter().rev() {
            if let Token::TagEnd { name, .. } = token {
                *counts.entry(name).or_insert(0) += 1;
            }
        }
        counts
    }

    // The cursor moved past one token, so a closer it held is no longer ahead.
    fn retire_closer(&mut self, index: usize) {
        if let Some(Token::TagEnd { name, .. }) = self.tokens.get(index) {
            if let Some(count) = self.closers_ahead.get_mut(name) {
                *count = count.saturating_sub(1);
            }
        }
    }

    fn tag_source_at(&self, index: usize) -> String {
        match self.tokens.get(index) {
            Some(Token::TagStart(tag)) => tag.source.clone(),
            _ => String::new(),
        }
    }

    fn open_block(&mut self, kind: DelimiterKind, stack: &mut Vec<Block>) {
        let element = match kind {
            DelimiterKind::Italic => Element::new("i"),
            DelimiterKind::Bold => Element::new("b"),
            DelimiterKind::Tag(name) => Element::new(name),
        };
        stack.push(Block {
            element,
            opened_by: kind,
            open_index: self.index,
            children: Vec::new(),
        });
    }

    fn close_block(&mut self, position: usize, stack: &mut Vec<Block>, out: &mut Vec<Node>) {
        let mut interior: Vec<Node> = Vec::new();
        while stack.len() > position + 1 {
            let block = stack.pop().expect("stack is deeper than position");
            // The blocks closed this way become literal text, so they are no
            // longer open and their slots have to leave the index with them.
            self.forget_open_tag(stack.len(), &block);
            append_literalised(block, &mut interior);
        }
        let block = stack.pop().expect("block at position exists");
        let mut element = block.element;
        let mut children = block.children;
        children.extend(interior);
        element.children = children;
        match stack.last_mut() {
            Some(parent) => parent.children.push(Node::element(element)),
            None => out.push(Node::element(element)),
        }
    }

    fn forget_open_tag(&mut self, position: usize, block: &Block) {
        let DelimiterKind::Tag(name) = block.opened_by else {
            return;
        };
        if let Some(positions) = self.open_tags.get_mut(name) {
            if positions.last() == Some(&position) {
                positions.pop();
            }
        }
    }

    fn internal_link(&mut self, link: &LinkToken) -> Vec<Node> {
        let target = link.target.trim();
        let namespace = namespace_of(target, &self.context.options);
        match namespace {
            Namespace::Category if !link.leading_colon => {
                let name = target
                    .split_once(':')
                    .map(|(_, rest)| rest)
                    .unwrap_or(target)
                    .trim()
                    .to_string();
                let sort_key = link
                    .parts
                    .iter()
                    .find(|part| part.name.is_none())
                    .map(|part| part.value.trim().to_string())
                    .filter(|key| !key.is_empty());
                self.context.document.categories.push(crate::dom::Category {
                    name: name.clone(),
                    sort_key,
                });
                if self.context.options.show_categories {
                    let mut element = Element::new("a");
                    element.set_attr("href", self.href_for(&format!("Category:{name}")));
                    element.add_class("mw-category-link");
                    element.children = vec![Node::text(name)];
                    return vec![Node::element(element)];
                }
                Vec::new()
            }
            Namespace::File if !link.leading_colon => {
                let name = target
                    .split_once(':')
                    .map(|(_, rest)| rest)
                    .unwrap_or(target)
                    .trim()
                    .to_string();
                let options = parse_image_options(&link.parts, &self.context.options);
                if !self.context.options.show_images {
                    let mut element = Element::new("a");
                    element.set_attr("href", self.href_for(&format!("File:{name}")));
                    element.children = vec![Node::text(name)];
                    return vec![Node::element(element)];
                }
                vec![self.image(&name, &options)]
            }
            _ => {
                let label = link
                    .parts
                    .iter()
                    .find(|part| part.name.is_none())
                    .map(|part| part.value.clone());

                let label = match label {
                    Some(label) if !label.trim().is_empty() => label,
                    _ => apply_pipe_trick(target, ""),
                };
                let mut element = Element::new("a");
                element.set_attr("href", self.href_for(target));
                element.add_class("mw-wikilink");
                element.kind = crate::dom::ElementKind::Normal;
                element.children = self.parse_inline_text(&label);
                vec![Node::element(element)]
            }
        }
    }

    fn external_link(&mut self, link: &LinkToken) -> Vec<Node> {
        let url = link.target.trim();
        let label = link
            .parts
            .first()
            .map(|part| part.value.trim().to_string())
            .unwrap_or_default();
        let mut element = Element::new("a");
        element.set_attr("href", url);
        element.add_class("external");
        element.set_attr("rel", "nofollow");
        element.kind = crate::dom::ElementKind::ExternalLink;
        if label.is_empty() {
            self.context.document.external_link_count += 1;
            element.children = vec![Node::text(format!(
                "[{}]",
                self.context.document.external_link_count
            ))];
        } else {
            element.children = self.parse_inline_text(&label);
        }
        vec![Node::element(element)]
    }

    fn href_for(&self, target: &str) -> String {
        let options = &self.context.options;
        // A `#` in a link target is a fragment separator, not part of the page
        // title. Encoding it turned `[[Page#Section]]` into a link to a page
        // literally named "Page#Section", so section links could never work.
        let (page, fragment) = match target.split_once('#') {
            Some((page, fragment)) => (page, Some(fragment)),
            None => (target, None),
        };
        let encoded_page = encode_title(page);
        let encoded = match fragment {
            Some(fragment) => {
                let fragment = encode_title(fragment);
                if encoded_page.is_empty() {
                    // `[[#Section]]` is an anchor in the current document.
                    return format!("#{fragment}");
                }
                format!("{encoded_page}#{fragment}")
            }
            None => encoded_page,
        };
        options.article_path.replace("$1", &encoded)
    }

    fn image(&mut self, name: &str, options: &ImageOptions) -> Node {
        let mut img = Element::new("img");
        img.set_attr("src", self.image_url(name));
        img.set_attr("alt", options.alt.clone().unwrap_or_default());
        if let Some(width) = options.width {
            img.set_attr("width", width.to_string());
        }
        if let Some(height) = options.height {
            img.set_attr("height", height.to_string());
        }
        img.kind = crate::dom::ElementKind::Image;

        // `link=` names the page the image links to; written with no value it
        // means the image is not a link at all, which is how a template puts a
        // picture inside its own clickable layout without adding a second one.
        let mut media = match &options.link {
            Some(link) if link.is_empty() => Node::element(img),
            Some(link) => {
                // A named target is a page title unless it is already a URL or
                // an anchor of its own.
                let href = if link.contains("://") || link.starts_with('/') || link.starts_with('#')
                {
                    link.clone()
                } else {
                    self.href_for(link)
                };
                let mut anchor = Element::new("a");
                anchor.add_class("image");
                anchor.set_attr("href", href);
                anchor.children.push(Node::element(img));
                Node::element(anchor)
            }
            None => {
                let mut anchor = Element::new("a");
                anchor.add_class("image");
                anchor.set_attr("href", self.href_for(&format!("File:{name}")));
                anchor.children.push(Node::element(img));
                Node::element(anchor)
            }
        };

        let mut node = match &options.caption {
            Some(caption) => {
                let mut figure = Element::new("figure");
                if let Some(class) = &options.class {
                    figure.add_class(class);
                }
                figure.children.push(media);
                let mut figcaption = Element::new("figcaption");
                figcaption.children = self.parse_inline_text(caption);
                figure.children.push(Node::element(figcaption));
                Node::element(figure)
            }
            None => {
                if let (Some(class), Node::Element(element)) = (&options.class, &mut media) {
                    element.add_class(class);
                }
                media
            }
        };

        // `right` and `left` are layout options, not captions: a template puts
        // a picture in a div corner with them. The preview stylesheet supplies
        // MediaWiki's own float classes.
        if let Some(align) = options.align {
            if let Node::Element(element) = &mut node {
                element.add_class(match align {
                    ImageAlign::Right => "floatright",
                    ImageAlign::Left => "floatleft",
                });
            }
        }

        // The outermost node identifies a block picture, so a line that holds
        // only this picture is not wrapped in a paragraph.
        if let Node::Element(element) = &mut node {
            element.kind = ElementKind::Image;
        }
        node
    }

    fn image_url(&self, name: &str) -> String {
        let encoded = encode_title(&name.replace(' ', "_"));
        match &self.context.options.image_base_url {
            Some(base) => format!("{}/{}", base.trim_end_matches('/'), encoded),
            None => format!("/images/{}", encoded),
        }
    }

    fn handle_ext_tag(&mut self, tag: &ExtTagToken) -> Vec<Node> {
        if tag.name == "ref" {
            return self.handle_ref(tag);
        }
        if tag.name == "references" {
            if tag.content.is_some() {
                self.skip_to_after_tag(&tag.name);
            }
            return self.build_references_list(&references::group_of(&self.ext_attributes(tag)));
        }

        if crate::extensions::renders_nothing(&tag.name)
            || crate::extensions::is_transclusion_marker(&tag.name)
        {
            if tag.content.is_some() {
                self.skip_to_after_tag(&tag.name);
            }
            return Vec::new();
        }
        let attributes = self.ext_attributes(tag);
        let content = match (&tag.content, self.take_ext_content(&tag.name)) {
            (Some(_), Some(nodes)) => nodes,

            (None, _) => Vec::new(),

            (Some(raw), None) => vec![Node::text(raw.clone())],
        };
        match crate::extensions::render(&tag.name, &attributes, "") {
            crate::extensions::Rendered::Nodes(mut nodes) => {
                if content.is_empty() {
                    return nodes;
                }
                if let Some(element) = nodes.first_mut().and_then(Node::as_element_mut) {
                    if element.children.is_empty() {
                        element.children = content;
                    }
                    nodes
                } else {
                    content
                }
            }
            crate::extensions::Rendered::References { group } => self.build_references_list(&group),
        }
    }

    fn handle_ref(&mut self, tag: &ExtTagToken) -> Vec<Node> {
        let attributes = self.ext_attributes(tag);
        let group = references::group_of(&attributes);
        let name = references::name_of(&attributes);
        let content = match (&tag.content, self.take_ext_content("ref")) {
            (Some(_), Some(nodes)) => nodes,
            (None, _) => Vec::new(),
            (Some(raw), None) => vec![Node::text(raw.clone())],
        };
        if content.is_empty() && name.is_none() {
            self.context.warn(
                DiagnosticCode::EmptyReference,
                "a <ref> needs either content or a name",
                self.context.source_len,
            );
        }
        self.context
            .document
            .references
            .cite(&group, name.as_deref(), content)
    }

    fn take_ext_content(&mut self, name: &str) -> Option<Vec<Node>> {
        if !self.has_tag_closer_ahead(name) {
            return None;
        }
        let mut children = Vec::new();
        self.index += 1;
        while self.index < self.tokens.len() {
            if let Token::TagEnd { name: end, .. } = &self.tokens[self.index] {
                if *end == name {
                    self.index += 1;
                    return Some(children);
                }
            }
            let before = self.index;
            self.run_into(&mut children);
            if self.index == before {
                self.index += 1;
            }
        }
        Some(children)
    }

    fn skip_to_after_tag(&mut self, name: &str) {
        if !self.has_tag_closer_ahead(name) {
            return;
        }
        let mut depth = 0usize;
        self.index += 1;
        while self.index < self.tokens.len() {
            match &self.tokens[self.index] {
                Token::TagStart(tag) if tag.name == name => depth += 1,
                Token::TagEnd { name: end, .. } if *end == name => {
                    if depth == 0 {
                        self.index += 1;
                        return;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            self.index += 1;
        }
    }

    fn ext_attributes(&self, tag: &ExtTagToken) -> Vec<Attribute> {
        tag.attributes
            .iter()
            .map(|attribute| Attribute {
                name: attribute.name.clone(),
                value: crate::preprocess::decode_entities(&attribute.value),
            })
            .collect()
    }

    fn build_references_list(&mut self, group: &str) -> Vec<Node> {
        let mut nodes = self.context.document.references.render(group);

        for node in &mut nodes {
            self.parse_reference_text(node);
        }
        nodes
    }

    fn parse_reference_text(&mut self, node: &mut Node) {
        match node {
            Node::Text(text) => {
                let parsed = self.parse_inline_text(text);
                if let Some(first) = parsed.first().cloned() {
                    if parsed.len() == 1 {
                        *node = first;
                        return;
                    }
                }
                if parsed.is_empty() {
                    return;
                }
                let mut wrapper = Element::new("span");
                wrapper.children = parsed;
                *node = Node::element(wrapper);
            }
            Node::Element(element) => {
                for child in &mut element.children {
                    self.parse_reference_text(child);
                }
            }
            _ => {}
        }
    }

    fn self_closing_tag(&mut self, tag: &TagToken) -> Node {
        let mut element = Element::new(tag.name);
        element.attributes = decode_attributes_from(&tag.attributes);
        element.self_closing = tag.explicit_self_closing;
        Node::element(element)
    }

    fn parse_inline_text(&mut self, text: &str) -> Vec<Node> {
        if self.context.nesting >= MAX_NESTING {
            return vec![Node::text(text.to_string())];
        }
        self.context.nesting += 1;
        let tokens = lex::lex_inline(text);
        let mut builder = InlineBuilder {
            tokens: &tokens,
            index: 0,
            context: self.context,
            open_tags: HashMap::new(),
            closers_ahead: InlineBuilder::closer_counts(&tokens),
            html_block: false,
        };
        let nodes = builder.run();
        self.context.nesting -= 1;
        nodes
    }
}

fn append_literalised(block: Block, out: &mut Vec<Node>) {
    let mut text = String::new();
    let children = block.children;
    match block.opened_by {
        DelimiterKind::Italic => text.push_str("''"),
        DelimiterKind::Bold => text.push_str("'''"),
        DelimiterKind::Tag(name) => {
            text.push('<');
            text.push_str(name);
            for attribute in &block.element.attributes {
                text.push(' ');
                text.push_str(&attribute.name);
                text.push_str("=\"");
                text.push_str(&attribute.value);
                text.push('"');
            }
            text.push('>');
        }
    }
    out.push(Node::Text(text));
    out.extend(children);
}

fn push_text_node(out: &mut Vec<Node>, text: &str) {
    if text.is_empty() {
        return;
    }
    let decoded = if text.contains('&') {
        crate::preprocess::decode_entities(text)
    } else {
        text.to_string()
    };
    match out.last_mut() {
        Some(Node::Text(previous)) => previous.push_str(&decoded),
        _ => out.push(Node::Text(decoded)),
    }
}

struct HeadingParts<'a> {
    level: u8,
    text: &'a str,
    trailing: &'a str,
}

fn parse_heading(line: &str) -> Option<HeadingParts<'_>> {
    let opening = scan::scan_equals(line)?;
    if opening.count < 2 {
        return None;
    }
    let level = opening.count.min(6) as u8;
    let rest = &line[opening.end..];

    let trimmed = rest.trim_end();
    let trailing_equals = trimmed.bytes().rev().take_while(|b| *b == b'=').count();
    if trailing_equals < opening.count {
        return None;
    }
    let text_end = trimmed.len() - trailing_equals;
    let text = &rest[..text_end];

    let closing_end = opening.end + trimmed.len();
    let trailing = &line[closing_end..];
    Some(HeadingParts {
        level,
        text: text.trim(),
        trailing,
    })
}

pub fn heading_anchor(text: &str) -> String {
    let decoded = crate::preprocess::decode_entities(text);
    let mut out = String::with_capacity(decoded.len());
    for ch in decoded.chars() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => out.push('_'),
            '[' | ']' | '{' | '}' | '|' | '<' | '>' | '"' | '&' | '\'' => {}
            '!' | '#' | '$' | '%' | '(' | ')' | '*' | '+' | ',' | '-' | '.' | '/' | ':' | ';'
            | '=' | '?' | '@' | '\\' | '^' | '_' | '`' | '~' => out.push(ch),
            c if c.is_alphanumeric() => out.push(c),
            c if c >= '\u{80}' => out.push(c),
            _ => {}
        }
    }
    out.trim_matches('_').to_string()
}

fn attribute_source(open_tag: &str) -> &str {
    let body = open_tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end_matches('/');
    match body.find(|ch: char| ch.is_ascii_whitespace()) {
        Some(index) => &body[index..],
        None => "",
    }
}

// A block-level HTML tag on its own line starts a block: parsed inline it would
// end up wrapped in a paragraph.
fn starts_html_block(content: &str) -> bool {
    crate::scan::scan_tag(content).is_some_and(|tag| crate::scan::is_block_tag(tag.name))
}

fn starts_block_ext_tag(content: &str) -> bool {
    let trimmed = content.trim_start();
    if !trimmed.starts_with('<') {
        return false;
    }
    match crate::extensions::scan_ext_tag(trimmed) {
        Some(tag) => crate::extensions::is_block_tag(&tag.name),
        None => false,
    }
}

fn decode_attributes(source: &str) -> Vec<Attribute> {
    let raw = scan::parse_attributes(&format!("<x {source}>"));
    raw.into_iter()
        .map(|attribute| Attribute {
            name: attribute.name,
            value: crate::preprocess::decode_entities(&attribute.value),
        })
        .collect()
}

fn decode_attributes_from(raw: &[scan::RawAttribute]) -> Vec<Attribute> {
    raw.iter()
        .map(|attribute| Attribute {
            name: attribute.name.clone(),
            value: crate::preprocess::decode_entities(&attribute.value),
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Namespace {
    Category,
    File,
    Other,
}

fn namespace_of(target: &str, options: &Options) -> Namespace {
    let Some((prefix, _)) = target.split_once(':') else {
        return Namespace::Other;
    };
    let prefix = prefix.trim();
    if options
        .category_namespace_aliases
        .iter()
        .any(|alias| alias.eq_ignore_ascii_case(prefix))
    {
        return Namespace::Category;
    }
    if options
        .file_namespace_aliases
        .iter()
        .any(|alias| alias.eq_ignore_ascii_case(prefix))
    {
        return Namespace::File;
    }
    Namespace::Other
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ImageAlign {
    Right,
    Left,
}

#[derive(Default, Clone, Debug)]
struct ImageOptions {
    alt: Option<String>,
    caption: Option<String>,
    link: Option<String>,
    class: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    align: Option<ImageAlign>,
}

fn parse_image_options(parts: &[LinkPart], _options: &Options) -> ImageOptions {
    let mut result = ImageOptions::default();
    for part in parts {
        match &part.name {
            Some(name) => {
                let name = name.trim().to_ascii_lowercase();
                let value = part.value.trim().to_string();
                match name.as_str() {
                    "alt" => result.alt = Some(value),
                    "link" => result.link = Some(value),
                    "class" => result.class = Some(value),
                    "upright" | "thumbnail" | "thumb" | "frame" | "frameless" | "border" => {}
                    _ => {
                        if let Some(dimension) = parse_dimension(&name, &value) {
                            match dimension {
                                Dimension::Width(width) => result.width = Some(width),
                                Dimension::Height(height) => result.height = Some(height),
                            }
                        }
                    }
                }
            }
            None => {
                let value = part.value.trim();
                if value.is_empty() {
                    continue;
                }
                let lower = value.to_ascii_lowercase();
                match lower.as_str() {
                    "right" => result.align = Some(ImageAlign::Right),
                    "left" => result.align = Some(ImageAlign::Left),
                    "thumb" | "thumbnail" | "frame" | "frameless" | "border" | "center"
                    | "centre" | "none" | "baseline" | "sub" | "super" | "top" | "text-top"
                    | "middle" | "bottom" | "text-bottom" | "upright" => {}
                    _ => {
                        if let Some((width, height)) = parse_pixel_size(value) {
                            result.width = Some(width);
                            if let Some(height) = height {
                                result.height = Some(height);
                            }
                        } else {
                            result.caption = Some(value.to_string());
                        }
                    }
                }
            }
        }
    }
    result
}

enum Dimension {
    Width(u32),
    Height(u32),
}

fn parse_dimension(name: &str, value: &str) -> Option<Dimension> {
    match name {
        "width" => value.parse().ok().map(Dimension::Width),
        "height" => value.parse().ok().map(Dimension::Height),
        _ => None,
    }
}

fn parse_pixel_size(value: &str) -> Option<(u32, Option<u32>)> {
    let lower = value.to_ascii_lowercase();
    let body = lower.strip_suffix("px")?;
    match body.split_once('x') {
        Some((width, height)) => Some((width.parse().ok()?, Some(height.parse().ok()?))),
        None => Some((body.parse().ok()?, None)),
    }
}

fn apply_pipe_trick(target: &str, label: &str) -> String {
    if !label.is_empty() {
        return label.to_string();
    }
    let mut text = target.to_string();
    if let Some(index) = text.find('(') {
        text.truncate(index);
    }
    if let Some((_, rest)) = text.split_once(':') {
        text = rest.to_string();
    }
    text.trim().to_string()
}

pub fn encode_title(title: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let trimmed = title.trim();
    let mut out = String::with_capacity(trimmed.len());
    for byte in trimmed.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'!'
            | b'*'
            | b'('
            | b')'
            | b'\'' => out.push(byte as char),
            // A space becomes an underscore rather than an escape, which is what
            // makes a title readable in a URL.
            b' ' => out.push('_'),
            other => {
                out.push('%');
                out.push(HEX[(other >> 4) as usize] as char);
                out.push(HEX[(other & 0x0f) as usize] as char);
            }
        }
    }
    out
}

struct RawCell {
    header: bool,
    content: String,
}

fn split_cells(line: &str) -> Vec<RawCell> {
    let header = line.starts_with('!');
    let body = &line[1..];
    let separator: &str = if header { "!!" } else { "||" };
    let parts = split_top_level(body, separator);
    parts
        .into_iter()
        .map(|content| RawCell {
            header,
            content: content.trim().to_string(),
        })
        .collect()
}

fn split_top_level(text: &str, separator: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let separator_bytes = separator.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut brackets = 0i32;
    let mut braces = 0i32;
    while index < bytes.len() {
        if brackets == 0 && braces == 0 && bytes[index..].starts_with(separator_bytes) {
            parts.push(text[start..index].to_string());
            index += separator_bytes.len();
            start = index;
            continue;
        }
        match bytes[index] {
            b'[' => brackets += 1,
            b']' => brackets -= 1,
            b'{' => braces += 1,
            b'}' => braces -= 1,
            _ => {}
        }
        index += 1;
    }
    parts.push(text[start..].to_string());
    parts
}

fn split_cell_attributes(content: &str) -> (&str, &str) {
    let trimmed = content.trim();
    let Some(pipe) = find_attribute_pipe(trimmed) else {
        return ("", trimmed);
    };
    (trimmed[..pipe].trim(), trimmed[pipe + 1..].trim())
}

fn find_attribute_pipe(body: &str) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut index = 0usize;
    let mut saw_equals = false;
    let mut quote: Option<u8> = None;
    let mut brackets = 0i32;
    let mut braces = 0i32;
    while index < bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(mark) => {
                if byte == mark {
                    quote = None;
                }
                index += 1;
                continue;
            }
            None => match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'[' => brackets += 1,
                b']' => brackets -= 1,
                b'{' => braces += 1,
                b'}' => braces -= 1,
                b'=' if brackets <= 0 && braces <= 0 => saw_equals = true,
                b'|' if brackets > 0 || braces > 0 => {}
                b'|' if saw_equals => return Some(index),

                b'|' => return None,
                b'\n' => return None,
                _ => {}
            },
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse_html(source: &str) -> String {
        crate::serialize::serialize(&parse(source, &Options::default()))
    }

    #[test]
    fn parses_paragraphs() {
        assert_eq!(parse_html("Hello"), "<p>Hello</p>\n");
        assert_eq!(parse_html("a\n\nb"), "<p>a</p>\n<p>b</p>\n");
        assert_eq!(parse_html("a\nb"), "<p>a\nb</p>\n");
    }

    #[test]
    fn parses_bold_and_italic() {
        assert_eq!(parse_html("''a''"), "<p><i>a</i></p>\n");
        assert_eq!(parse_html("'''a'''"), "<p><b>a</b></p>\n");
        assert_eq!(parse_html("'''''a'''''"), "<p><b><i>a</i></b></p>\n");
    }

    #[test]
    fn apostrophes_in_words_stay_literal() {
        assert_eq!(parse_html("it's fine"), "<p>it's fine</p>\n");
    }

    #[test]
    fn parses_headings() {
        assert_eq!(parse_html("== A =="), "<h2 id=\"A\">A</h2>\n");
        assert_eq!(parse_html("=== A ==="), "<h3 id=\"A\">A</h3>\n");
        assert_eq!(parse_html("= A ="), "<p>= A =</p>\n");
    }

    #[test]
    fn heading_ids_are_deduplicated() {
        assert_eq!(
            parse_html("== A ==\n== A =="),
            "<h2 id=\"A\">A</h2>\n<h2 id=\"A_2\">A</h2>\n"
        );
    }

    #[test]
    fn parses_horizontal_rules() {
        assert_eq!(parse_html("----"), "<hr>\n");
    }

    #[test]
    fn parses_preformatted_blocks() {
        assert_eq!(parse_html(" a\n b"), "<pre>a\nb</pre>\n");
    }

    #[test]
    fn parses_entities() {
        assert_eq!(parse_html("&amp;"), "<p>&amp;</p>\n");
        assert_eq!(parse_html("&#65;"), "<p>A</p>\n");
        assert_eq!(parse_html("&notreal;"), "<p>&amp;notreal;</p>\n");
    }

    #[test]
    fn parses_nowiki() {
        assert_eq!(parse_html("<nowiki>'''x'''</nowiki>"), "<p>'''x'''</p>\n");
    }

    #[test]
    fn parses_simple_tags() {
        assert_eq!(parse_html("<b>x</b>"), "<p><b>x</b></p>\n");
        assert_eq!(
            parse_html("<span class=\"a\">x</span>"),
            "<p><span class=\"a\">x</span></p>\n"
        );
        assert_eq!(parse_html("<br>"), "<p><br></p>\n");
    }

    #[test]
    fn unmatched_tag_is_literal() {
        assert_eq!(parse_html("<b>x"), "<p>&lt;b&gt;x</p>\n");
    }

    #[test]
    fn parses_wikilinks() {
        assert_eq!(
            parse_html("[[Foo]]"),
            "<p><a href=\"/wiki/Foo\" class=\"mw-wikilink\">Foo</a></p>\n"
        );
        assert_eq!(
            parse_html("[[Foo|bar]]"),
            "<p><a href=\"/wiki/Foo\" class=\"mw-wikilink\">bar</a></p>\n"
        );
        assert_eq!(
            parse_html("[[Foo (bar)|]]"),
            "<p><a href=\"/wiki/Foo_(bar)\" class=\"mw-wikilink\">Foo</a></p>\n"
        );
    }

    #[test]
    fn collects_categories() {
        let document = parse("[[Category:Things]]", &Options::default());
        assert_eq!(document.categories.len(), 1);
        assert_eq!(document.categories[0].name, "Things");
    }

    #[test]
    fn parses_external_links() {
        let html = parse_html("[https://example.org label]");
        assert!(html.contains("href=\"https://example.org\""));
        assert!(html.contains(">label</a>"));
    }

    #[test]
    fn parses_a_simple_table() {
        let html = parse_html("{| class=\"wikitable\"\n|-\n! A !! B\n|-\n| 1 || 2\n|}");
        assert!(html.contains("<table class=\"wikitable\">"));
        assert!(html.contains("<tr><th>A</th><th>B</th></tr>"), "{html}");
        assert!(html.contains("<tr><td>1</td><td>2</td></tr>"), "{html}");
    }

    #[test]
    fn parses_table_caption_and_attributes() {
        let html = parse_html("{| class=\"x\"\n|+ Cap\n|-\n| style=\"color:red\" | cell\n|}");
        assert!(html.contains("<caption>Cap</caption>"), "{html}");
        assert!(html.contains("<td style=\"color:red\">cell</td>"), "{html}");
    }

    #[test]
    fn parses_lists() {
        assert_eq!(
            squash(&parse_html("* a\n* b")),
            "<ul><li>a</li><li>b</li></ul>"
        );
        assert_eq!(
            squash(&parse_html("# a\n# b")),
            "<ol><li>a</li><li>b</li></ol>"
        );

        assert_eq!(
            squash(&parse_html("* a\n** b")),
            "<ul><li>a<ul><li>b</li></ul></li></ul>"
        );
        assert_eq!(
            squash(&parse_html("* a\n* b\n** c")),
            "<ul><li>a</li><li>b<ul><li>c</li></ul></li></ul>"
        );

        assert_eq!(
            squash(&parse_html("* a\n** b\n* c")),
            "<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul>"
        );
        assert_eq!(squash(&parse_html("; t")), "<dl><dt>t</dt></dl>");
        assert_eq!(squash(&parse_html(": d")), "<dl><dd>d</dd></dl>");

        assert_eq!(
            squash(&parse_html("; t : d")),
            "<dl><dt>t</dt><dd>d</dd></dl>"
        );
    }

    fn squash(html: &str) -> String {
        html.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace("> ", ">")
            .replace(" <", "<")
    }

    #[test]
    fn parses_images() {
        let html = parse_html("[[File:X.png|thumb|alt=Alt text|Caption]]");
        assert!(html.contains("<img"), "{html}");
        assert!(html.contains("alt=\"Alt text\""), "{html}");
        assert!(html.contains("<figcaption>Caption</figcaption>"), "{html}");
    }

    #[test]
    fn parses_headings_inside_paragraph_boundaries() {
        let html = parse_html("text\n== A ==\nmore");
        assert_eq!(html, "<p>text</p>\n<h2 id=\"A\">A</h2>\n<p>more</p>\n");
    }
}

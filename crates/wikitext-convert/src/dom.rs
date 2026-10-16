use crate::markup::{Block, Cell, Inline, List, Row, Table};
use wikitext::dom::{Element, ElementKind, Node};
use wikitext::Document;

pub fn blocks(document: &Document) -> Vec<Block> {
    children(&document.children)
}

fn children(nodes: &[Node]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut pending: Vec<Inline> = Vec::new();

    for node in nodes {
        match node {
            Node::Text(text) | Node::Raw(text) => push_text(&mut pending, text),
            Node::Comment(_) => {}
            Node::Element(element) => {
                if is_block(element) {
                    flush(&mut blocks, &mut pending);
                    if let Some(block) = block_of(element) {
                        blocks.push(block);
                    }
                    continue;
                }
                pending.extend(inline(element));
            }
            _ => {}
        }
    }

    flush(&mut blocks, &mut pending);
    blocks
}

fn flush(blocks: &mut Vec<Block>, pending: &mut Vec<Inline>) {
    let taken = std::mem::take(pending);
    if taken.iter().any(|inline| !is_blank(inline)) {
        blocks.push(Block::Paragraph(taken));
    }
}

fn is_blank(inline: &Inline) -> bool {
    match inline {
        Inline::Text(text) => text.trim().is_empty(),
        Inline::LineBreak => true,
        _ => false,
    }
}

fn push_text(pending: &mut Vec<Inline>, text: &str) {
    if text.is_empty() {
        return;
    }
    match pending.last_mut() {
        Some(Inline::Text(existing)) => existing.push_str(text),
        _ => pending.push(Inline::Text(text.to_string())),
    }
}

fn is_block(element: &Element) -> bool {
    element.kind == ElementKind::Heading
        || element.kind == ElementKind::HorizontalRule
        || element.kind == ElementKind::Preformatted
        || matches!(
            element.tag.as_str(),
            "p" | "pre"
                | "ul"
                | "ol"
                | "li"
                | "dl"
                | "dt"
                | "dd"
                | "blockquote"
                | "table"
                | "caption"
                | "figure"
                | "figcaption"
                | "section"
                | "article"
                | "aside"
                | "header"
                | "footer"
                | "nav"
                | "main"
                | "div"
        )
}

fn block_of(element: &Element) -> Option<Block> {
    let tag = element.tag.as_str();
    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Some(Block::Heading {
            level: tag[1..].parse::<u8>().unwrap_or(2),
            content: inline_children(element),
        }),
        "p" => {
            let content = inline_children(element);
            if content.is_empty() {
                None
            } else if content.len() == 1 {
                // A paragraph holding one block has no text of its own.
                if let Inline::Raw(markup) = &content[0] {
                    return Some(Block::Raw(markup.clone()));
                }
                Some(Block::Paragraph(content))
            } else {
                Some(Block::Paragraph(content))
            }
        }
        "ul" | "ol" => Some(Block::List(list_of(element, tag == "ol"))),
        "pre" => Some(pre_of(element)),
        "blockquote" => {
            let inner = children(&element.children);
            Some(Block::Quote(inner))
        }
        "hr" => Some(Block::Rule),
        "table" => table_of(element),
        // Markdown has no spelling for these, and embedded HTML is what it
        // offers instead, so the markup itself is the faithful rendering.
        _ => Some(Block::Raw(serialize(element))),
    }
}

fn inline_children(element: &Element) -> Vec<Inline> {
    let mut out = Vec::new();
    collect_inline(&element.children, &mut out);
    trim(&mut out);
    out
}

fn collect_inline(nodes: &[Node], out: &mut Vec<Inline>) {
    for node in nodes {
        match node {
            Node::Text(text) | Node::Raw(text) => push_text(out, text),
            Node::Comment(_) => {}
            Node::Element(element) => out.extend(inline(element)),
            _ => {}
        }
    }
}

fn inline(element: &Element) -> Vec<Inline> {
    if element.kind == ElementKind::ExternalLink {
        return vec![external(element)];
    }
    // A wikilink is an ordinary anchor carrying the wikilink class, so the class
    // is what identifies it — not the element kind.
    if has_class(element, "mw-wikilink") {
        return vec![link(element)];
    }
    if let Some(markup) = extension_markup(element) {
        return vec![Inline::Raw(markup)];
    }
    inline_by_tag(element)
}

fn has_class(element: &Element, class: &str) -> bool {
    element
        .attr("class")
        .is_some_and(|value| value.split_whitespace().any(|name| name == class))
}

// An extension renders to a span tagged with the original name, so the wikitext
// spelling is rebuilt from that plus the text content.
fn extension_markup(element: &Element) -> Option<String> {
    let name = element.attr("data-mw-ext")?;
    if name == "references" {
        return Some("<references/>".to_string());
    }
    Some(format!("<{name}>{}</{name}>", text_of(element)))
}

fn inline_by_tag(element: &Element) -> Vec<Inline> {
    match element.tag.as_str() {
        "b" | "strong" => vec![Inline::Bold(inline_children(element))],
        "i" | "em" | "cite" | "var" => vec![Inline::Italic(inline_children(element))],
        "u" | "ins" => vec![Inline::Under(inline_children(element))],
        "s" | "strike" | "del" => vec![Inline::Struck(inline_children(element))],
        "sub" => vec![Inline::Sub(inline_children(element))],
        "sup" => vec![Inline::Sup(inline_children(element))],
        "code" | "kbd" | "samp" | "tt" => vec![Inline::Code(text_of(element))],
        "br" => vec![Inline::LineBreak],
        // A block reached from inside an inline run stays whole rather than
        // being spliced into the middle of a sentence.
        "figure" => find_image(element)
            .map(|image| vec![image])
            .unwrap_or_else(|| vec![Inline::Raw(serialize(element))]),
        "div" | "section" | "table" | "ul" | "ol" | "pre" | "blockquote" => {
            vec![Inline::Raw(serialize(element))]
        }
        _ => inline_of_children(element),
    }
}

fn inline_of_children(element: &Element) -> Vec<Inline> {
    let mut out = Vec::new();
    collect_inline(&element.children, &mut out);
    out
}

fn find_image(element: &Element) -> Option<Inline> {
    for child in &element.children {
        let Node::Element(child) = child else {
            continue;
        };
        if child.tag.as_str() == "img" {
            return Some(image(child));
        }
        if let Some(found) = find_image(child) {
            return Some(found);
        }
    }
    None
}

fn link(element: &Element) -> Inline {
    let label = inline_children(element);
    let target = element
        .attr("title")
        .map(str::to_string)
        .or_else(|| element.attr("href").and_then(article_target))
        .unwrap_or_default();
    let label = if label.is_empty() {
        vec![Inline::Text(target.clone())]
    } else {
        label
    };
    Inline::Link { target, label }
}

fn article_target(href: &str) -> Option<String> {
    href.strip_prefix("/wiki/")
        .map(|path| percent_decode(path).replace('_', " "))
}

fn external(element: &Element) -> Inline {
    let url = element.attr("href").unwrap_or_default().to_string();
    Inline::External {
        url,
        label: inline_children(element),
    }
}

fn image(element: &Element) -> Inline {
    let source = element.attr("src").unwrap_or_default();
    let target = source
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(|name| format!("File:{}", percent_decode(name)))
        .unwrap_or_else(|| "File:".to_string());
    Inline::Image {
        target,
        alt: inline_children(element),
    }
}

fn list_of(element: &Element, ordered: bool) -> List {
    let mut items = Vec::new();
    for child in &element.children {
        let Node::Element(item) = child else {
            continue;
        };
        if !matches!(item.tag.as_str(), "li" | "dt" | "dd") {
            continue;
        }
        let mut blocks = children(&item.children);
        if blocks.is_empty() {
            blocks.push(Block::Paragraph(Vec::new()));
        }
        items.push(blocks);
    }
    List { ordered, items }
}

fn text_of(element: &Element) -> String {
    let mut out = String::new();
    collect_text(&element.children, &mut out);
    out
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(text) | Node::Raw(text) => out.push_str(text),
            Node::Element(element) => collect_text(&element.children, out),
            _ => {}
        }
    }
}

fn pre_of(element: &Element) -> Block {
    let language = element
        .attr("class")
        .and_then(|class| {
            class
                .split_whitespace()
                .find_map(|name| name.strip_prefix("mw-highlight-lang-").map(str::to_string))
        })
        .or_else(|| {
            element.attr("lang").map(str::to_string).or_else(|| {
                element.children.iter().find_map(|node| match node {
                    Node::Element(code) if code.tag.as_str() == "code" => {
                        code.attr("class").and_then(|class| {
                            class
                                .split_whitespace()
                                .find_map(|name| name.strip_prefix("language-").map(str::to_string))
                        })
                    }
                    _ => None,
                })
            })
        });
    Block::Pre {
        language,
        text: text_of(element).trim_end_matches('\n').to_string(),
    }
}

fn table_of(element: &Element) -> Option<Block> {
    let mut rows = Vec::new();
    collect_rows(element, &mut rows);
    if rows.is_empty() {
        return None;
    }
    Some(Block::Table(Table { rows }))
}

fn collect_rows(element: &Element, rows: &mut Vec<Row>) {
    for child in &element.children {
        let Node::Element(child) = child else {
            continue;
        };
        match child.tag.as_str() {
            "tr" => rows.push(row_of(child)),
            "thead" | "tbody" | "tfoot" => collect_rows(child, rows),
            _ => {}
        }
    }
}

fn row_of(element: &Element) -> Row {
    let mut cells = Vec::new();
    for child in &element.children {
        let Node::Element(child) = child else {
            continue;
        };
        let tag = child.tag.as_str();
        if tag != "td" && tag != "th" {
            continue;
        }
        let content = children(&child.children);
        let content = if content.is_empty() {
            vec![Block::Paragraph(Vec::new())]
        } else {
            content
        };
        cells.push(Cell {
            header: tag == "th",
            content,
        });
    }
    Row { cells }
}

fn trim(inlines: &mut Vec<Inline>) {
    if let Some(Inline::Text(first)) = inlines.first_mut() {
        *first = first.trim_start().to_string();
    }
    if let Some(Inline::Text(last)) = inlines.last_mut() {
        *last = last.trim_end().to_string();
    }
    inlines.retain(|inline| match inline {
        Inline::Text(text) => !text.is_empty(),
        _ => true,
    });
}

pub fn serialize(element: &Element) -> String {
    let mut out = String::new();
    write_element(element, &mut out);
    out
}

fn write_element(element: &Element, out: &mut String) {
    let tag = element.tag.as_str();
    out.push('<');
    out.push_str(tag);
    for attribute in &element.attributes {
        out.push(' ');
        out.push_str(&attribute.name);
        out.push_str("=\"");
        out.push_str(&escape_attribute(&attribute.value));
        out.push('"');
    }
    if wikitext::dom::is_void(tag) {
        out.push('>');
        return;
    }
    out.push('>');
    for child in &element.children {
        match child {
            Node::Text(text) | Node::Raw(text) => out.push_str(text),
            Node::Element(child) => write_element(child, out),
            Node::Comment(text) => {
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
            }
            _ => {}
        }
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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

use crate::markup::{Block, Cell, Inline, List, Row, Table};
use html5ever::tendril::TendrilSink;
use html5ever::{parse_document, ParseOpts};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

pub fn parse(source: &str) -> Vec<Block> {
    let dom = parse_document(RcDom::default(), ParseOpts::default()).one(source);
    let body = find_body(&dom.document).unwrap_or_else(|| dom.document.clone());
    let mut blocks = Blocks::default();
    for child in body.children.borrow().iter() {
        collect_blocks(child, &mut blocks);
    }
    let Blocks { ready, pending } = blocks;
    let mut out = ready;
    push_paragraph(&mut out, pending);
    out
}

fn find_body(handle: &Handle) -> Option<Handle> {
    for child in handle.children.borrow().iter() {
        if let NodeData::Element { name, .. } = &child.data {
            if &*name.local == "body" {
                return Some(child.clone());
            }
        }
        if let Some(found) = find_body(child) {
            return Some(found);
        }
    }
    None
}

#[derive(Default)]
struct Blocks {
    ready: Vec<Block>,
    pending: Vec<Inline>,
}

fn push_paragraph(blocks: &mut Vec<Block>, pending: Vec<Inline>) {
    if pending.iter().any(|inline| !is_blank(inline)) {
        blocks.push(Block::Paragraph(pending));
    }
}

fn is_blank(inline: &Inline) -> bool {
    match inline {
        Inline::Text(text) => text.trim().is_empty(),
        Inline::LineBreak => true,
        _ => false,
    }
}

fn collect_blocks(handle: &Handle, blocks: &mut Blocks) {
    match &handle.data {
        NodeData::Text { contents } => {
            let text = contents.borrow();
            let collapsed = collapse_whitespace(&text);
            if !collapsed.is_empty() {
                blocks.pending.push(Inline::Text(collapsed));
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.to_string();
            let attributes = attributes(&attrs.borrow());
            match tag.as_str() {
                "script" | "style" | "head" | "title" | "meta" | "link" | "template" => {}
                "p" => {
                    // A paragraph holding only inline content is the common case;
                    // one holding a block (an image caption, say) has to be split.
                    let mut collected = Blocks::default();
                    collect_blocks_into(handle, &mut collected);
                    let Blocks { ready, pending } = collected;
                    if ready.is_empty() {
                        push_paragraph(&mut blocks.ready, inline_content(handle, true));
                    } else {
                        flush(blocks);
                        blocks.ready.extend(ready);
                        push_paragraph(&mut blocks.ready, pending);
                    }
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    flush(blocks);
                    let level = tag[1..].parse::<u8>().unwrap_or(2);
                    blocks.ready.push(Block::Heading {
                        level,
                        content: inline_content(handle, true),
                    });
                }
                "ul" | "ol" => {
                    flush(blocks);
                    blocks.ready.push(Block::List(list_of(handle, tag == "ol")));
                }
                "pre" => {
                    flush(blocks);
                    blocks.ready.push(pre_of(handle));
                }
                "blockquote" => {
                    flush(blocks);
                    let mut inner = Blocks::default();
                    collect_blocks_into(handle, &mut inner);
                    let Blocks { ready, pending } = inner;
                    let mut content = ready;
                    push_paragraph(&mut content, pending);
                    blocks.ready.push(Block::Quote(content));
                }
                "hr" => {
                    flush(blocks);
                    blocks.ready.push(Block::Rule);
                }
                "table" => {
                    flush(blocks);
                    if let Some(table) = table_of(handle) {
                        blocks.ready.push(Block::Table(table));
                    }
                }
                "div" | "section" | "article" | "main" | "header" | "footer" | "aside" | "nav"
                | "figure" | "figcaption" | "details" | "summary" | "form" | "fieldset" => {
                    flush(blocks);
                    collect_blocks_into(handle, blocks);
                }
                "br" => blocks.pending.push(Inline::LineBreak),
                _ => {
                    blocks.pending.extend(inline_of(handle, &attributes));
                }
            }
        }
        _ => {
            for child in handle.children.borrow().iter() {
                collect_blocks(child, blocks);
            }
        }
    }
}

fn collect_blocks_into(handle: &Handle, blocks: &mut Blocks) {
    for child in handle.children.borrow().iter() {
        collect_blocks(child, blocks);
    }
}

fn flush(blocks: &mut Blocks) {
    let pending = std::mem::take(&mut blocks.pending);
    push_paragraph(&mut blocks.ready, pending);
}

fn inline_content(handle: &Handle, trim_edges: bool) -> Vec<Inline> {
    let mut out = Vec::new();
    for child in handle.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => {
                let text = contents.borrow();
                let collapsed = collapse_whitespace(&text);
                if !collapsed.is_empty() {
                    out.push(Inline::Text(collapsed));
                }
            }
            NodeData::Element { name, attrs, .. } => {
                let tag = name.local.to_string();
                if matches!(tag.as_str(), "script" | "style") {
                    continue;
                }
                if tag == "br" {
                    out.push(Inline::LineBreak);
                    continue;
                }
                if tag == "p" {
                    if !out.is_empty() && !ends_with_space(&out) {
                        out.push(Inline::Text(" ".to_string()));
                    }
                    out.extend(inline_content(child, true));
                    continue;
                }
                let attributes = attributes(&attrs.borrow());
                out.extend(inline_of(child, &attributes));
            }
            _ => {}
        }
    }
    if trim_edges {
        trim(&mut out);
    }
    out
}

fn ends_with_space(inlines: &[Inline]) -> bool {
    match inlines.last() {
        Some(Inline::Text(text)) => text.ends_with(' '),
        _ => false,
    }
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

fn inline_of(handle: &Handle, attributes: &[(String, String)]) -> Vec<Inline> {
    let Some(tag) = element_tag(handle) else {
        return Vec::new();
    };
    match tag.as_str() {
        "b" | "strong" => vec![Inline::Bold(inline_content(handle, false))],
        "i" | "em" | "cite" | "var" => vec![Inline::Italic(inline_content(handle, false))],
        "u" | "ins" => vec![Inline::Under(inline_content(handle, false))],
        "s" | "strike" | "del" => vec![Inline::Struck(inline_content(handle, false))],
        "sub" => vec![Inline::Sub(inline_content(handle, false))],
        "sup" => vec![Inline::Sup(inline_content(handle, false))],
        "code" | "kbd" | "samp" | "tt" => vec![Inline::Code(text_of(handle))],
        "a" => vec![anchor(handle, attributes)],
        "img" => vec![image(attributes)],
        "span" | "small" | "big" | "mark" | "abbr" | "dfn" | "q" | "time" | "data" | "bdi"
        | "bdo" | "ruby" | "rt" | "rp" | "wbr" | "font" | "center" => {
            let style = attribute(attributes, "style").unwrap_or_default();
            let class = attribute(attributes, "class").unwrap_or_default();
            if style.is_empty() && class.is_empty() {
                inline_content(handle, false)
            } else {
                vec![Inline::Raw(serialize(handle))]
            }
        }
        _ => vec![Inline::Raw(serialize(handle))],
    }
}

fn anchor(handle: &Handle, attributes: &[(String, String)]) -> Inline {
    let href = attribute(attributes, "href").unwrap_or_default();
    let label = inline_content(handle, false);
    if let Some(target) = href.strip_prefix("/wiki/") {
        let target = percent_decode(target.replace('_', " "));
        let label = if label.is_empty() {
            vec![Inline::Text(target.clone())]
        } else {
            label
        };
        return Inline::Link { target, label };
    }
    if href.is_empty() || href.starts_with('#') {
        return if label.is_empty() {
            Inline::Text(href)
        } else {
            Inline::Raw(render_inlines(&label))
        };
    }
    let url = percent_decode(href.clone());
    let label = if label.is_empty() {
        vec![Inline::Text(url.clone())]
    } else if let [Inline::Text(text)] = label.as_slice() {
        if text.trim() == url {
            Vec::new()
        } else {
            label
        }
    } else {
        label
    };
    if label.is_empty() {
        return Inline::External {
            url,
            label: vec![Inline::Text(String::new())],
        };
    }
    Inline::External { url, label }
}

fn image(attributes: &[(String, String)]) -> Inline {
    let src = attribute(attributes, "src").unwrap_or_default();
    let alt = attribute(attributes, "alt").unwrap_or_default();
    let target = src
        .strip_prefix("/wiki/")
        .map(|path| percent_decode(path.replace('_', " ")))
        .or_else(|| {
            src.strip_prefix("./").map(|path| {
                let mut name = percent_decode(path.replace('_', " "));
                if !name.to_lowercase().starts_with("file:") {
                    name = format!("File:{name}");
                }
                name
            })
        })
        .unwrap_or_else(|| src.clone());
    let alt = if alt.is_empty() {
        Vec::new()
    } else {
        vec![Inline::Text(alt)]
    };
    Inline::Image { target, alt }
}

fn list_of(handle: &Handle, ordered: bool) -> List {
    let mut items = Vec::new();
    for child in handle.children.borrow().iter() {
        if element_tag(child).as_deref() != Some("li") {
            continue;
        }
        // Wikitext list items are single lines, so a loose item becomes one
        // paragraph.
        let mut blocks = Blocks::default();
        collect_blocks_into(child, &mut blocks);
        let Blocks { ready, pending } = blocks;
        let mut content = ready;
        push_paragraph(&mut content, pending);
        items.push(content);
    }
    List { ordered, items }
}

fn pre_of(handle: &Handle) -> Block {
    let (text, language) = pre_text(handle);
    Block::Pre { language, text }
}

fn pre_text(handle: &Handle) -> (String, Option<String>) {
    let mut text = String::new();
    let mut language = None;
    for child in handle.children.borrow().iter() {
        if let NodeData::Element { name, attrs, .. } = &child.data {
            if &*name.local == "code" {
                if let Some(class) = attribute(&attributes(&attrs.borrow()), "class") {
                    for name in class.split_whitespace() {
                        if let Some(lang) = name.strip_prefix("language-") {
                            language = Some(lang.to_string());
                        }
                    }
                }
                text.push_str(&text_of(child));
                continue;
            }
        }
        text.push_str(&raw_text(child));
    }
    let text = text.trim_matches('\n').to_string();
    (text, language)
}

fn table_of(handle: &Handle) -> Option<Table> {
    let mut rows = Vec::new();
    collect_rows(handle, &mut rows);
    if rows.is_empty() {
        return None;
    }
    Some(Table { rows })
}

fn collect_rows(handle: &Handle, rows: &mut Vec<Row>) {
    for child in handle.children.borrow().iter() {
        match element_tag(child).as_deref() {
            Some("tr") => rows.push(row_of(child)),
            Some("thead") | Some("tbody") | Some("tfoot") => collect_rows(child, rows),
            _ => {}
        }
    }
}

fn row_of(handle: &Handle) -> Row {
    let mut cells = Vec::new();
    for child in handle.children.borrow().iter() {
        let Some(tag) = element_tag(child) else {
            continue;
        };
        if tag != "td" && tag != "th" {
            continue;
        }
        let mut blocks = Blocks::default();
        collect_blocks_into(child, &mut blocks);
        let Blocks { ready, pending } = blocks;
        let mut content = ready;
        push_paragraph(&mut content, pending);
        cells.push(Cell {
            header: tag == "th",
            content,
        });
    }
    Row { cells }
}

fn element_tag(handle: &Handle) -> Option<String> {
    match &handle.data {
        NodeData::Element { name, .. } => Some(name.local.to_string()),
        _ => None,
    }
}

fn attributes(attrs: &[html5ever::Attribute]) -> Vec<(String, String)> {
    attrs
        .iter()
        .map(|attribute| {
            (
                attribute.name.local.to_string(),
                attribute.value.to_string(),
            )
        })
        .collect()
}

fn attribute(attributes: &[(String, String)], name: &str) -> Option<String> {
    attributes
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

fn text_of(handle: &Handle) -> String {
    let mut out = String::new();
    for child in handle.children.borrow().iter() {
        out.push_str(&raw_text(child));
    }
    out
}

fn raw_text(handle: &Handle) -> String {
    match &handle.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        NodeData::Element { .. } => text_of(handle),
        _ => String::new(),
    }
}

fn serialize(handle: &Handle) -> String {
    match &handle.data {
        NodeData::Text { contents } => crate::write::escape_raw(&contents.borrow()),
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.to_string();
            let mut out = format!("<{tag}");
            for attribute in attrs.borrow().iter() {
                let value = attribute.value.to_string();
                out.push_str(&format!(
                    " {}=\"{}\"",
                    attribute.name.local,
                    crate::write::escape_raw(&value)
                ));
            }
            out.push('>');
            if !crate::html::is_void(&tag) {
                for child in handle.children.borrow().iter() {
                    out.push_str(&serialize(child));
                }
                out.push_str(&format!("</{tag}>"));
            }
            out
        }
        _ => String::new(),
    }
}

fn render_inlines(inlines: &[Inline]) -> String {
    crate::write::write_inlines(inlines)
}

pub fn is_void(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !space {
                out.push(' ');
                space = true;
            }
        } else {
            out.push(ch);
            space = false;
        }
    }
    out
}

fn percent_decode(text: String) -> String {
    wikitext::decode_entities(&percent_decode_bytes(&text))
}

fn percent_decode_bytes(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &text[index + 1..index + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
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

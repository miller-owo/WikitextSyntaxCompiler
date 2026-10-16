use crate::dom::{Attribute, Element, Node};
pub fn is_extension_tag(name: &str) -> bool {
    matches!(
        name,
        "ref"
            | "references"
            | "poem"
            | "gallery"
            | "math"
            | "chem"
            | "ce"
            | "syntaxhighlight"
            | "source"
            | "indicator"
            | "noinclude"
            | "includeonly"
            | "onlyinclude"
            | "templatedata"
            | "categorytree"
            | "charinsert"
            | "hiero"
            | "score"
            | "timeline"
            | "graph"
            | "mapframe"
            | "maplink"
    )
}

// `<nowiki>` and `<pre>` are absent on purpose: the tokenizer already treats
// them as raw text before this module sees them.
pub fn is_raw_content_tag(name: &str) -> bool {
    matches!(
        name,
        "math"
            | "chem"
            | "ce"
            | "syntaxhighlight"
            | "source"
            | "score"
            | "timeline"
            | "graph"
            | "hiero"
    )
}

pub fn is_transclusion_marker(name: &str) -> bool {
    matches!(name, "noinclude" | "includeonly")
}

pub fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "references"
            | "poem"
            | "gallery"
            | "math"
            | "chem"
            | "ce"
            | "syntaxhighlight"
            | "source"
            | "score"
            | "timeline"
            | "graph"
            | "hiero"
            | "templatedata"
            | "categorytree"
            | "mapframe"
    )
}

pub fn renders_nothing(name: &str) -> bool {
    matches!(
        name,
        "noinclude" | "includeonly" | "templatedata" | "indicator" | "categorytree" | "charinsert"
    )
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExtTagSpan {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub attributes: Vec<crate::scan::RawAttribute>,
    pub content: Option<String>,
}

pub fn scan_ext_tag(source: &str) -> Option<ExtTagSpan> {
    if !source.starts_with('<') || source.starts_with("</") {
        return None;
    }
    let open = crate::scan::scan_tag(source)?;
    if open.closing || !is_extension_tag(open.name) {
        return None;
    }
    let attributes = crate::scan::parse_attributes(&source[..open.end]);
    let name = open.name.to_string();

    if open.self_closing {
        return Some(ExtTagSpan {
            name,
            start: 0,
            end: open.end,
            attributes,
            content: None,
        });
    }
    let content_start = open.end;
    let body = &source[content_start..];
    if is_raw_content_tag(open.name) {
        let offset = match crate::scan::find_raw_text_end(body, open.name) {
            Some(offset) => offset,
            None => {
                return Some(ExtTagSpan {
                    name,
                    start: 0,
                    end: source.len(),
                    attributes,
                    content: Some(body.to_string()),
                })
            }
        };
        let close = content_start + offset;
        let close_end = close + body[offset..].find('>').map(|i| i + 1).unwrap_or(0);
        return Some(ExtTagSpan {
            name,
            start: 0,
            end: close_end,
            attributes,
            content: Some(source[content_start..close].to_string()),
        });
    }

    match find_matching_end(body, &name) {
        Some((content_end, tag_end)) => Some(ExtTagSpan {
            name,
            start: 0,
            end: content_start + tag_end,
            attributes,
            content: Some(source[content_start..content_start + content_end].to_string()),
        }),
        None => Some(ExtTagSpan {
            name,
            start: 0,
            end: source.len(),
            attributes,
            content: Some(body.to_string()),
        }),
    }
}

fn find_matching_end(body: &str, name: &str) -> Option<(usize, usize)> {
    let lower = body.to_ascii_lowercase();
    let open_needle = format!("<{name}");
    let close_needle = format!("</{name}");
    let mut depth = 1usize;
    let mut cursor = 0usize;
    while cursor < lower.len() {
        let next_open = lower[cursor..].find(&open_needle).map(|at| cursor + at);
        let next_close = lower[cursor..].find(&close_needle).map(|at| cursor + at);
        match (next_open, next_close) {
            (_, None) => return None,
            (Some(open_at), Some(close_at)) if open_at < close_at => {
                let after = open_at + open_needle.len();
                let is_tag = matches!(
                    body.as_bytes().get(after),
                    Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'/')
                );
                if is_tag {
                    depth += 1;
                }
                cursor = after;
            }
            (_, Some(close_at)) => {
                let after = close_at + close_needle.len();
                let is_tag = matches!(
                    body.as_bytes().get(after),
                    Some(b'>')
                        | Some(b' ')
                        | Some(b'\t')
                        | Some(b'\n')
                        | Some(b'\r')
                        | Some(b'/')
                        | None
                );
                if !is_tag {
                    cursor = after;
                    continue;
                }
                depth -= 1;
                if depth == 0 {
                    let tag_end = close_at + body[close_at..].find('>').map(|i| i + 1).unwrap_or(0);
                    return Some((close_at, tag_end));
                }
                cursor = after;
            }
        }
    }
    None
}

pub enum Rendered {
    Nodes(Vec<Node>),

    References { group: String },
}

pub fn render(name: &str, attributes: &[Attribute], content: &str) -> Rendered {
    match name {
        "poem" => Rendered::Nodes(vec![render_poem(attributes, content)]),
        "gallery" => Rendered::Nodes(vec![render_gallery(attributes, content)]),
        "math" | "chem" | "ce" => Rendered::Nodes(vec![render_math(name, attributes, content)]),
        "syntaxhighlight" | "source" => {
            Rendered::Nodes(vec![render_syntax_highlight(attributes, content)])
        }
        "references" => Rendered::References {
            group: attribute(attributes, "group")
                .unwrap_or_default()
                .to_string(),
        },

        _ if renders_nothing(name) => Rendered::Nodes(Vec::new()),
        _ => Rendered::Nodes(vec![render_fallback(name, attributes, content)]),
    }
}

fn attribute<'a>(attributes: &'a [Attribute], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
        .map(|attribute| attribute.value.as_str())
}

fn render_poem(attributes: &[Attribute], content: &str) -> Node {
    let mut element = Element::new("div");
    element.attributes = clone_attributes(attributes);
    element.add_class("poem");
    let mut children: Vec<Node> = Vec::new();
    let normalised = content.replace("\r\n", "\n");

    let body = normalised.strip_suffix('\n').unwrap_or(&normalised);
    for (index, line) in body.split('\n').enumerate() {
        if index > 0 {
            children.push(Node::element(Element::new("br")));
        }
        children.push(Node::text(line.to_string()));
    }
    if !children.is_empty() {
        children.push(Node::element(Element::new("br")));
    }
    element.children = children;
    Node::element(element)
}

fn render_gallery(attributes: &[Attribute], content: &str) -> Node {
    let mut gallery = Element::new("ul");
    gallery.attributes = clone_attributes(attributes);
    gallery.add_class("gallery");
    let normalised = content.replace("\r\n", "\n");
    for line in normalised.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('|');
        let Some(file) = parts.next() else { continue };
        let name = file
            .split_once(':')
            .map(|(_, rest)| rest)
            .unwrap_or(file)
            .trim();
        if name.is_empty() {
            continue;
        }
        let caption = parts.next().map(str::trim).unwrap_or("");
        let mut item = Element::new("li");
        item.add_class("gallerybox");
        let mut image = Element::new("img");
        image.set_attr("src", image_url(name));
        image.set_attr("alt", caption);
        item.children.push(Node::element(image));
        if !caption.is_empty() {
            let mut caption_element = Element::new("div");
            caption_element.add_class("gallerytext");
            caption_element
                .children
                .push(Node::text(caption.to_string()));
            item.children.push(Node::element(caption_element));
        }
        gallery.children.push(Node::element(item));
    }
    Node::element(gallery)
}

fn render_math(name: &str, attributes: &[Attribute], content: &str) -> Node {
    let mut span = Element::new("span");
    span.attributes = clone_attributes(attributes);
    span.add_class("mw-ext");
    span.add_class(name);
    span.set_attr("data-mw-ext", name);
    let mut code = Element::new("code");
    code.children.push(Node::text(content.to_string()));
    span.children.push(Node::element(code));
    Node::element(span)
}

fn render_syntax_highlight(attributes: &[Attribute], content: &str) -> Node {
    let mut pre = Element::new("pre");

    let language = attribute(attributes, "lang")
        .or_else(|| attribute(attributes, "language"))
        .unwrap_or("")
        .trim();
    pre.attributes = clone_attributes(attributes);
    pre.remove_attr("lang");
    pre.remove_attr("language");
    pre.add_class("mw-highlight");
    if !language.is_empty() {
        pre.add_class(&format!("mw-highlight-lang-{language}"));
    }
    if attribute(attributes, "line").is_some() {
        pre.add_class("mw-highlight-lines");
    }
    let body = content.strip_prefix('\n').unwrap_or(content);
    pre.children.push(Node::text(body.to_string()));
    Node::element(pre)
}

fn render_fallback(name: &str, attributes: &[Attribute], content: &str) -> Node {
    let mut element = Element::new(name);
    element.attributes = clone_attributes(attributes);
    element.children.push(Node::text(content.to_string()));
    Node::element(element)
}

fn clone_attributes(attributes: &[Attribute]) -> Vec<Attribute> {
    attributes.to_vec()
}

fn image_url(name: &str) -> String {
    format!("/images/{}", name.replace(' ', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn attributes(source: &str) -> Vec<Attribute> {
        crate::scan::parse_attributes(source)
            .into_iter()
            .map(|raw| Attribute::new(raw.name, crate::preprocess::decode_entities(&raw.value)))
            .collect()
    }

    #[test]
    fn only_known_tags_are_extensions() {
        assert!(is_extension_tag("ref"));
        assert!(is_extension_tag("poem"));
        assert!(!is_extension_tag("div"));
        assert!(!is_extension_tag("span"));
    }

    #[test]
    fn scans_a_paired_tag() {
        let source = "<ref>a '''b'''</ref>";
        let span = scan_ext_tag(source).unwrap();
        assert_eq!(span.name, "ref");
        assert_eq!(span.content.as_deref(), Some("a '''b'''"));
        assert_eq!(span.end, source.len(), "the span covers the whole element");
        assert_eq!(&source[..span.end], source);
    }

    #[test]
    fn scans_a_self_closing_tag() {
        let span = scan_ext_tag("<references/>").unwrap();
        assert_eq!(span.name, "references");
        assert!(span.content.is_none());
        let span = scan_ext_tag("<references responsive />").unwrap();
        assert!(span.content.is_none());
    }

    #[test]
    fn scans_nested_same_name_tags() {
        let source = "<ref>outer <ref>inner</ref> end</ref>tail";
        let span = scan_ext_tag(source).unwrap();
        assert_eq!(span.content.as_deref(), Some("outer <ref>inner</ref> end"));
        assert_eq!(&source[..span.end], "<ref>outer <ref>inner</ref> end</ref>");
    }

    #[test]
    fn raw_content_tags_stop_at_the_first_end_tag() {
        let span = scan_ext_tag("<math>x <math> y</math>z").unwrap();
        assert_eq!(span.content.as_deref(), Some("x <math> y"));
    }

    #[test]
    fn rejects_html_and_unclosed_tags() {
        assert!(scan_ext_tag("<div>x</div>").is_none());
        assert!(scan_ext_tag("</ref>").is_none());
        assert!(scan_ext_tag("<ref>unclosed").unwrap().content.is_some());
    }

    #[test]
    fn renders_poem_with_line_breaks() {
        let Rendered::Nodes(nodes) = render("poem", &[], "one\ntwo\n") else {
            panic!("poem renders nodes")
        };
        let Node::Element(element) = &nodes[0] else {
            panic!("poem is an element")
        };
        assert_eq!(element.tag.as_str(), "div");
        assert_eq!(element.attr("class"), Some("poem"));
        assert_eq!(element.children.len(), 4, "{:?}", element.children);
    }

    #[test]
    fn renders_syntax_highlight_with_language_class() {
        let attrs = attributes("<syntaxhighlight lang=\"rust\">");
        let Rendered::Nodes(nodes) = render("syntaxhighlight", &attrs, "fn main() {}") else {
            panic!("expected nodes")
        };
        let Node::Element(element) = &nodes[0] else {
            panic!("expected an element")
        };
        assert_eq!(element.tag.as_str(), "pre");
        assert!(element
            .attr("class")
            .unwrap()
            .contains("mw-highlight-lang-rust"));
    }

    #[test]
    fn references_asks_the_parser_to_expand_it() {
        let Rendered::References { group } = render("references", &[], "") else {
            panic!("expected the references variant")
        };
        assert_eq!(group, "");
    }

    #[test]
    fn noinclude_renders_nothing() {
        let Rendered::Nodes(nodes) = render("noinclude", &[], "docs") else {
            panic!("expected nodes")
        };
        assert!(nodes.is_empty());
    }
}

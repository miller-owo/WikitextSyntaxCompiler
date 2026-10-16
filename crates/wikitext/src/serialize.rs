use crate::dom::{is_void, Attribute, Document, Element, Node};
use crate::options::OutputMode;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SerializeOptions {
    pub pretty: bool,
    pub mode: OutputMode,
    pub escape_nbsp: bool,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        SerializeOptions {
            pretty: true,
            mode: OutputMode::Fragment,
            escape_nbsp: false,
        }
    }
}

impl SerializeOptions {
    pub fn new() -> Self {
        SerializeOptions::default()
    }

    pub fn with_pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }

    pub fn with_mode(mut self, mode: OutputMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_escape_nbsp(mut self, escape: bool) -> Self {
        self.escape_nbsp = escape;
        self
    }
}

pub fn serialize(document: &Document) -> String {
    serialize_with(document, &SerializeOptions::default())
}

pub fn serialize_with(document: &Document, options: &SerializeOptions) -> String {
    let mut writer = Serializer::new(options);
    match options.mode {
        OutputMode::Fragment => {
            writer.write_children(&document.children);
        }
        OutputMode::ParserOutput => {
            let mut wrapper = Element::new("div");
            wrapper.add_class("mw-parser-output");
            writer.write_children(&document.children);
            let body = core::mem::take(&mut writer.out);
            writer.out.push_str("<div class=\"mw-parser-output\">");
            writer.out.push_str(&body);
            writer.out.push_str("</div>");
        }
        OutputMode::Document => {
            writer.write_children(&document.children);
            let body = core::mem::take(&mut writer.out);
            writer.out.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
            writer.out.push_str(
                "<meta charset=\"UTF-8\">\n<meta name=\"generator\" content=\"wikitext\">\n",
            );
            writer.out.push_str("</head>\n<body>\n");
            writer.out.push_str(&body);
            writer.out.push_str("\n</body>\n</html>\n");
        }
    }
    writer.finish()
}

pub fn serialize_nodes(nodes: &[Node]) -> String {
    let options = SerializeOptions::default();
    let mut writer = Serializer::new(&options);
    writer.write_children(nodes);
    writer.finish()
}

enum Step<'a> {
    Node(&'a Node),
    Close(&'a str),
}

struct Serializer<'a> {
    out: String,
    options: &'a SerializeOptions,
}

impl<'a> Serializer<'a> {
    fn new(options: &'a SerializeOptions) -> Self {
        Serializer {
            out: String::with_capacity(1024),
            options,
        }
    }

    fn finish(self) -> String {
        self.out
    }

    fn newline(&mut self) {
        if self.options.pretty && !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    fn block_break(&mut self) {
        if self.options.pretty {
            self.newline();
        }
    }

    fn write_children(&mut self, nodes: &[Node]) {
        // An explicit work stack rather than recursion: a page of deeply nested
        // tags would otherwise overflow the stack while being written out.
        let mut work: Vec<Step<'_>> = nodes.iter().rev().map(Step::Node).collect();
        while let Some(step) = work.pop() {
            match step {
                Step::Node(node) => self.write_node(node, &mut work),
                Step::Close(tag) => {
                    self.out.push_str("</");
                    self.out.push_str(tag);
                    self.out.push('>');
                    if is_block(tag) {
                        self.newline();
                    }
                }
            }
        }
    }

    fn write_node<'n>(&mut self, node: &'n Node, work: &mut Vec<Step<'n>>) {
        match node {
            Node::Text(text) => self.write_text(text),
            Node::Raw(raw) => self.out.push_str(raw),
            Node::Comment(comment) => {
                self.out.push_str("<!--");
                self.out.push_str(comment);
                self.out.push_str("-->");
            }
            Node::Element(element) => self.write_element(element, work),
        }
    }

    fn write_element<'n>(&mut self, element: &'n Element, work: &mut Vec<Step<'n>>) {
        let tag = element.tag.as_str();
        let block = is_block(tag);

        if block && !element.children.is_empty() {
            self.block_break();
        }

        self.out.push('<');
        self.out.push_str(tag);
        self.write_attributes(&element.attributes);

        if is_void(tag) {
            if element.self_closing {
                self.out.push_str(" />");
            } else {
                self.out.push('>');
            }
            if block {
                self.newline();
            }
            return;
        }

        self.out.push('>');

        if is_raw_text(tag) {
            for child in &element.children {
                match child {
                    Node::Text(t) | Node::Raw(t) => self.out.push_str(t),
                    Node::Comment(c) => {
                        self.out.push_str("<!--");
                        self.out.push_str(c);
                        self.out.push_str("-->");
                    }
                    Node::Element(_) => {}
                }
            }
            self.out.push_str("</");
            self.out.push_str(tag);
            self.out.push('>');
            if block {
                self.newline();
            }
            return;
        }

        // The closing tag goes on the stack before the children, so it is popped
        // after the whole subtree has been written.
        work.push(Step::Close(tag));
        work.extend(element.children.iter().rev().map(Step::Node));
    }

    fn write_attributes(&mut self, attributes: &[Attribute]) {
        for attribute in attributes {
            if attribute.name.is_empty() {
                continue;
            }
            self.out.push(' ');

            self.out.push_str(&attribute.name);
            self.out.push_str("=\"");
            escape_attribute_into(&attribute.value, self.options.escape_nbsp, &mut self.out);
            self.out.push('"');
        }
    }

    fn write_text(&mut self, text: &str) {
        escape_text_into(text, self.options.escape_nbsp, &mut self.out);
    }
}

pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_text_into(text, false, &mut out);
    out
}

pub fn escape_attribute(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_attribute_into(text, false, &mut out);
    out
}

fn escape_text_into(text: &str, escape_nbsp: bool, out: &mut String) {
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        let replacement = match ch {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '\u{a0}' if escape_nbsp => "&nbsp;",
            _ => continue,
        };
        out.push_str(&text[start..index]);
        out.push_str(replacement);
        start = index + ch.len_utf8();
    }
    out.push_str(&text[start..]);
}

fn escape_attribute_into(text: &str, escape_nbsp: bool, out: &mut String) {
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        let replacement = match ch {
            '&' => "&amp;",
            '"' => "&quot;",
            '<' => "&lt;",
            '>' => "&gt;",
            '\u{a0}' if escape_nbsp => "&nbsp;",
            _ => continue,
        };
        out.push_str(&text[start..index]);
        out.push_str(replacement);
        start = index + ch.len_utf8();
    }
    out.push_str(&text[start..]);
}

pub fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "body"
            | "caption"
            | "center"
            | "dd"
            | "details"
            | "dialog"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "legend"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "summary"
            | "table"
            | "tbody"
            | "tfoot"
            | "thead"
            | "tr"
            | "ul"
    )
}

fn is_raw_text(tag: &str) -> bool {
    matches!(tag, "script" | "style" | "nowiki")
}

pub fn push_escaped(out: &mut String, text: &str) {
    escape_text_into(text, false, out);
}

pub fn write_escaped(out: &mut String, text: &str) -> core::fmt::Result {
    escape_text_into(text, false, out);
    Ok(())
}

impl core::fmt::Display for crate::dom::Document {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&serialize(self))
    }
}

impl core::fmt::Display for crate::dom::NodeList {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let _ = write!(f, "{}", serialize_nodes(&self.0));
        Ok(())
    }
}

impl core::fmt::Display for Element {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let options = SerializeOptions::default();
        let mut writer = Serializer::new(&options);
        let mut work = Vec::new();
        writer.write_element(self, &mut work);
        f.write_str(&writer.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;
    #[test]
    fn escapes_text() {
        assert_eq!(escape_text("a & b < c > d"), "a &amp; b &lt; c &gt; d");
        assert_eq!(escape_text("quotes \" stay"), "quotes \" stay");
    }

    #[test]
    fn escapes_attributes() {
        assert_eq!(escape_attribute("a\"b&c"), "a&quot;b&amp;c");
    }

    #[test]
    fn void_elements_get_no_end_tag() {
        let mut img = Element::new("img");
        img.set_attr("src", "a.png");
        let options = SerializeOptions::default();
        let mut writer = Serializer::new(&options);
        let mut work = Vec::new();
        writer.write_node(&Node::element(img), &mut work);
        assert_eq!(writer.finish(), "<img src=\"a.png\">");
    }

    #[test]
    fn paragraphs_are_newline_separated() {
        let mut doc = Document::new();
        doc.push(Node::element(
            Element::new("p").with_child(Node::text("one")),
        ));
        doc.push(Node::element(
            Element::new("p").with_child(Node::text("two")),
        ));
        assert_eq!(serialize(&doc), "<p>one</p>\n<p>two</p>\n");
    }

    #[test]
    fn parser_output_mode_wraps_in_div() {
        let mut doc = Document::new();
        doc.push(Node::element(Element::new("p").with_child(Node::text("x"))));
        let html = serialize_with(
            &doc,
            &SerializeOptions::default().with_mode(OutputMode::ParserOutput),
        );
        assert!(html.starts_with("<div class=\"mw-parser-output\">"));
        assert!(html.ends_with("</div>"));
    }
}

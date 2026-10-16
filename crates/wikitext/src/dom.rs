use std::borrow::Cow;
use std::fmt;
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag(Cow<'static, str>);

impl Tag {
    pub const fn new(name: &'static str) -> Self {
        Tag(Cow::Borrowed(name))
    }

    pub fn owned(name: impl Into<String>) -> Self {
        let name = name.into();
        if name.bytes().any(|byte| byte.is_ascii_uppercase()) {
            Tag(Cow::Owned(name.to_ascii_lowercase()))
        } else {
            Tag(Cow::Owned(name))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.as_str())
    }
}

impl PartialEq<str> for Tag {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl From<&str> for Tag {
    fn from(name: &str) -> Self {
        Tag::owned(name)
    }
}

impl From<String> for Tag {
    fn from(name: String) -> Self {
        Tag::owned(name)
    }
}

impl From<TagName> for Tag {
    fn from(name: TagName) -> Self {
        Tag::new(name.as_str())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[non_exhaustive]
pub enum TagName {
    Html,
    Head,
    Body,
    Title,
    Meta,
    Link,
    Style,
    Script,
    Base,
    Div,
    Span,
    Section,
    Article,
    Aside,
    Header,
    Footer,
    Nav,
    Main,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    P,
    Blockquote,
    Q,
    Address,
    Figure,
    Figcaption,
    A,
    Em,
    Strong,
    B,
    I,
    U,
    S,
    Strike,
    Del,
    Ins,
    Mark,
    Small,
    Big,
    Sub,
    Sup,
    Code,
    Kbd,
    Samp,
    Var,
    Tt,
    Abbr,
    Bdi,
    Bdo,
    Cite,
    Data,
    Dfn,
    Time,
    Ruby,
    Rb,
    Rt,
    Rtc,
    Rp,
    Wbr,
    Br,
    Hr,
    Pre,
    Center,
    Font,
    Nobr,
    Ul,
    Ol,
    Li,
    Dl,
    Dt,
    Dd,
    Table,
    Caption,
    Thead,
    Tbody,
    Tfoot,
    Tr,
    Th,
    Td,
    Colgroup,
    Col,
    Img,
    Audio,
    Video,
    Source,
    Track,
    Map,
    Area,
    Embed,
    Object,
    Param,
    Picture,
    Canvas,
    Svg,
    MathMl,
    Form,
    Input,
    Button,
    Select,
    Option,
    Optgroup,
    Textarea,
    Label,
    Fieldset,
    Legend,
    Ref,
    References,
    Gallery,
    Poem,
    SyntaxHighlight,
    Nowiki,
    Indicator,
    TemplateData,
    CategoryTree,
    CharInsert,
    Hiero,
    Score,
    Timeline,
    Graph,
    Chem,
    Ce,
    MapFrame,
    MapLink,
    DynamicPageList,
    InputBox,
    PageList,
    Noinclude,
    IncludeOnly,
    OnlyInclude,
}

const TAG_NAMES: &[(TagName, &str)] = {
    use TagName::*;
    &[
        (Html, "html"),
        (Head, "head"),
        (Body, "body"),
        (Title, "title"),
        (Meta, "meta"),
        (Link, "link"),
        (Style, "style"),
        (Script, "script"),
        (Base, "base"),
        (Div, "div"),
        (Span, "span"),
        (Section, "section"),
        (Article, "article"),
        (Aside, "aside"),
        (Header, "header"),
        (Footer, "footer"),
        (Nav, "nav"),
        (Main, "main"),
        (H1, "h1"),
        (H2, "h2"),
        (H3, "h3"),
        (H4, "h4"),
        (H5, "h5"),
        (H6, "h6"),
        (P, "p"),
        (Blockquote, "blockquote"),
        (Q, "q"),
        (Address, "address"),
        (Figure, "figure"),
        (Figcaption, "figcaption"),
        (A, "a"),
        (Em, "em"),
        (Strong, "strong"),
        (B, "b"),
        (I, "i"),
        (U, "u"),
        (S, "s"),
        (Strike, "strike"),
        (Del, "del"),
        (Ins, "ins"),
        (Mark, "mark"),
        (Small, "small"),
        (Big, "big"),
        (Sub, "sub"),
        (Sup, "sup"),
        (Code, "code"),
        (Kbd, "kbd"),
        (Samp, "samp"),
        (Var, "var"),
        (Tt, "tt"),
        (Abbr, "abbr"),
        (Bdi, "bdi"),
        (Bdo, "bdo"),
        (Cite, "cite"),
        (Data, "data"),
        (Dfn, "dfn"),
        (Time, "time"),
        (Ruby, "ruby"),
        (Rb, "rb"),
        (Rt, "rt"),
        (Rtc, "rtc"),
        (Rp, "rp"),
        (Wbr, "wbr"),
        (Br, "br"),
        (Hr, "hr"),
        (Pre, "pre"),
        (Center, "center"),
        (Font, "font"),
        (Nobr, "nobr"),
        (Ul, "ul"),
        (Ol, "ol"),
        (Li, "li"),
        (Dl, "dl"),
        (Dt, "dt"),
        (Dd, "dd"),
        (Table, "table"),
        (Caption, "caption"),
        (Thead, "thead"),
        (Tbody, "tbody"),
        (Tfoot, "tfoot"),
        (Tr, "tr"),
        (Th, "th"),
        (Td, "td"),
        (Colgroup, "colgroup"),
        (Col, "col"),
        (Img, "img"),
        (Audio, "audio"),
        (Video, "video"),
        (Source, "source"),
        (Track, "track"),
        (Map, "map"),
        (Area, "area"),
        (Embed, "embed"),
        (Object, "object"),
        (Param, "param"),
        (Picture, "picture"),
        (Canvas, "canvas"),
        (Svg, "svg"),
        (MathMl, "math"),
        (Form, "form"),
        (Input, "input"),
        (Button, "button"),
        (Select, "select"),
        (Option, "option"),
        (Optgroup, "optgroup"),
        (Textarea, "textarea"),
        (Label, "label"),
        (Fieldset, "fieldset"),
        (Legend, "legend"),
        (Ref, "ref"),
        (References, "references"),
        (Gallery, "gallery"),
        (Poem, "poem"),
        (SyntaxHighlight, "syntaxhighlight"),
        (Nowiki, "nowiki"),
        (Indicator, "indicator"),
        (TemplateData, "templatedata"),
        (CategoryTree, "categorytree"),
        (CharInsert, "charinsert"),
        (Hiero, "hiero"),
        (Score, "score"),
        (Timeline, "timeline"),
        (Graph, "graph"),
        (Chem, "chem"),
        (Ce, "ce"),
        (MapFrame, "mapframe"),
        (MapLink, "maplink"),
        (DynamicPageList, "dynamicpagelist"),
        (InputBox, "inputbox"),
        (PageList, "pagelist"),
        (Noinclude, "noinclude"),
        (IncludeOnly, "includeonly"),
        (OnlyInclude, "onlyinclude"),
    ]
};

impl TagName {
    pub fn as_str(self) -> &'static str {
        TAG_NAMES
            .iter()
            .find(|(name, _)| *name == self)
            .map(|(_, text)| *text)
            .unwrap_or("span")
    }

    pub fn from_name(name: &str) -> Option<TagName> {
        let lower = name.to_ascii_lowercase();
        TAG_NAMES
            .iter()
            .find(|(_, text)| *text == lower)
            .map(|(tag, _)| *tag)
    }

    pub fn tag(self) -> Tag {
        Tag::new(self.as_str())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Attribute {
    // Names written as literals — which is most of them, since the compiler
    // generates the markup — are stored borrowed and cost no allocation.
    pub name: Cow<'static, str>,
    pub value: String,
}

impl Attribute {
    pub fn new(name: impl Into<Cow<'static, str>>, value: impl Into<String>) -> Self {
        Attribute {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum ElementKind {
    #[default]
    Normal,
    Paragraph,
    Heading,
    UnorderedItem,
    OrderedItem,
    TermItem,
    DefinitionItem,
    Preformatted,
    HorizontalRule,
    Image,
    Category,
    Reference,
    RedLink,
    ExternalLink,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Element {
    pub tag: Tag,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Node>,
    pub kind: ElementKind,
    pub self_closing: bool,
    pub void: bool,
}

impl Drop for Element {
    fn drop(&mut self) {
        // Dropping the tree naively recurses once per nesting level, and a page
        // of deeply nested tags then overflows the stack while merely being
        // freed. Taking each child's subtree into a worklist keeps it iterative.
        let mut pending = std::mem::take(&mut self.children);
        while let Some(node) = pending.pop() {
            if let Node::Element(mut element) = node {
                pending.append(&mut element.children);
            }
        }
    }
}

impl Element {
    pub fn new(tag: impl Into<Tag>) -> Self {
        let tag = tag.into();
        let void = is_void(tag.as_str());
        Element {
            tag,
            attributes: Vec::new(),
            children: Vec::new(),
            kind: ElementKind::Normal,
            self_closing: false,
            void,
        }
    }

    pub fn with_kind(mut self, kind: ElementKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(children);
        self
    }

    pub fn with_attr(
        mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<String>,
    ) -> Self {
        let name = name.into();
        self.attributes.push(Attribute {
            name,
            value: value.into(),
        });
        self
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
            .map(|attribute| attribute.value.as_str())
    }

    pub fn set_attr(&mut self, name: impl Into<Cow<'static, str>>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        match self
            .attributes
            .iter_mut()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(&name))
        {
            Some(existing) => existing.value = value,
            None => self.attributes.push(Attribute { name, value }),
        }
    }

    pub fn remove_attr(&mut self, name: &str) -> Option<String> {
        let index = self
            .attributes
            .iter()
            .position(|attribute| attribute.name.eq_ignore_ascii_case(name))?;
        Some(self.attributes.remove(index).value)
    }

    pub fn add_class(&mut self, class: &str) {
        match self.attr("class") {
            Some(existing) if !existing.is_empty() => {
                let merged = format!("{existing} {class}");
                self.set_attr("class", merged);
            }
            _ => self.set_attr("class", class),
        }
    }

    pub fn is_void(&self) -> bool {
        self.void
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Node {
    Element(Element),

    Text(String),

    Comment(String),

    Raw(String),
}

impl Node {
    pub fn text(value: impl Into<String>) -> Self {
        Node::Text(value.into())
    }

    pub fn element(element: Element) -> Self {
        Node::Element(element)
    }

    pub fn comment(value: impl Into<String>) -> Self {
        Node::Comment(value.into())
    }

    pub fn as_element(&self) -> Option<&Element> {
        match self {
            Node::Element(element) => Some(element),
            _ => None,
        }
    }

    pub fn as_element_mut(&mut self) -> Option<&mut Element> {
        match self {
            Node::Element(element) => Some(element),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Node::Text(text) | Node::Raw(text) => Some(text),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct NodeList(pub Vec<Node>);

impl NodeList {
    pub fn new() -> Self {
        NodeList(Vec::new())
    }

    pub fn push(&mut self, node: Node) {
        self.0.push(node);
    }

    pub fn push_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !text.is_empty() {
            self.0.push(Node::Text(text));
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Node> {
        self.0.iter()
    }

    pub fn into_vec(self) -> Vec<Node> {
        self.0
    }

    pub fn text_content(&self) -> String {
        let mut out = String::new();
        collect_text(&self.0, &mut out);
        out
    }
}

impl From<Vec<Node>> for NodeList {
    fn from(nodes: Vec<Node>) -> Self {
        NodeList(nodes)
    }
}

impl IntoIterator for NodeList {
    type Item = Node;
    type IntoIter = std::vec::IntoIter<Node>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a NodeList {
    type Item = &'a Node;
    type IntoIter = core::slice::Iter<'a, Node>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(text) | Node::Raw(text) => out.push_str(text),
            Node::Comment(_) => {}
            Node::Element(element) => collect_text(&element.children, out),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Document {
    pub children: Vec<Node>,
    pub diagnostics: Vec<crate::error::Diagnostic>,
    pub categories: Vec<Category>,
    pub redirect: Option<String>,
    pub languages: Vec<String>,
    pub display_title: Option<String>,
    pub default_sort: Option<String>,
    pub toc: TocBehaviour,
    pub external_link_count: usize,
    pub references: crate::references::References,
    pub headings: Vec<Heading>,
    pub trailing_newline: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub title: String,
}

impl Document {
    pub fn new() -> Self {
        Document::default()
    }

    pub fn push(&mut self, node: Node) {
        self.children.push(node);
    }

    pub fn warn(&mut self, diagnostic: crate::error::Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn text_content(&self) -> String {
        let mut out = String::new();
        collect_text(&self.children, &mut out);
        out
    }

    pub fn text_content_of(&self, nodes: &[Node]) -> String {
        let mut out = String::new();
        collect_text(nodes, &mut out);
        out
    }

    pub fn descendants(&self) -> Descendants<'_> {
        Descendants::new(&self.children)
    }
}

pub struct Descendants<'a> {
    stack: Vec<core::slice::Iter<'a, Node>>,
}

impl<'a> Descendants<'a> {
    fn new(nodes: &'a [Node]) -> Self {
        Descendants {
            stack: vec![nodes.iter()],
        }
    }
}

impl<'a> Iterator for Descendants<'a> {
    type Item = &'a Element;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let top = self.stack.last_mut()?;
            match top.next() {
                Some(Node::Element(element)) => {
                    self.stack.push(element.children.iter());
                    return Some(element);
                }
                Some(_) => continue,
                None => {
                    self.stack.pop();
                    if self.stack.is_empty() {
                        return None;
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Category {
    pub name: String,
    pub sort_key: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TocBehaviour {
    pub none: bool,
    pub force: bool,
    pub inline: bool,
    pub no_edit_section: bool,
    pub new_section_link: bool,
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

pub fn is_raw_text_element(tag: &str) -> bool {
    matches!(
        tag,
        "script" | "style" | "textarea" | "title" | "nowiki" | "pre"
    )
}

pub fn is_inline_element(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "abbr"
            | "b"
            | "bdi"
            | "bdo"
            | "big"
            | "br"
            | "cite"
            | "code"
            | "data"
            | "del"
            | "dfn"
            | "em"
            | "font"
            | "i"
            | "ins"
            | "kbd"
            | "label"
            | "mark"
            | "nobr"
            | "q"
            | "rb"
            | "rp"
            | "rt"
            | "rtc"
            | "ruby"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strike"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "tt"
            | "u"
            | "var"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tag_names_round_trip() {
        for (variant, name) in TAG_NAMES {
            assert_eq!(variant.as_str(), *name);
            assert_eq!(TagName::from_name(name), Some(*variant));
        }
    }

    #[test]
    fn tag_lookup_is_case_insensitive() {
        assert_eq!(TagName::from_name("DIV"), Some(TagName::Div));
        assert_eq!(TagName::from_name("nope"), None);
    }

    #[test]
    fn tags_lower_case_owned_names() {
        assert_eq!(Tag::owned("DIV").as_str(), "div");
        assert_eq!(Tag::from("SPAN").as_str(), "span");
    }

    #[test]
    fn void_elements_are_known() {
        assert!(is_void("br"));
        assert!(is_void("img"));
        assert!(!is_void("div"));
        assert!(Element::new("br").is_void());
    }

    #[test]
    fn text_content_strips_markup() {
        let mut element = Element::new("p");
        element.children.push(Node::text("a"));
        element
            .children
            .push(Node::element(Element::new("b").with_child(Node::text("c"))));
        let list = NodeList::from(vec![Node::element(element)]);
        assert_eq!(list.text_content(), "ac");
    }

    #[test]
    fn descendants_walk_depth_first() {
        let mut outer = Element::new("div");
        outer.children.push(Node::element(Element::new("span")));
        let mut document = Document::new();
        document.push(Node::element(outer));
        let tags: Vec<&str> = document
            .descendants()
            .map(|element| element.tag.as_str())
            .collect();
        assert_eq!(tags, vec!["div", "span"]);
    }
}

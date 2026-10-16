#[derive(Clone, PartialEq, Debug)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    List(List),
    Pre {
        language: Option<String>,
        text: String,
    },
    Quote(Vec<Block>),
    Rule,
    Table(Table),
    Raw(String),
}

#[derive(Clone, PartialEq, Debug)]
pub struct List {
    pub ordered: bool,
    pub items: Vec<Vec<Block>>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Table {
    pub rows: Vec<Row>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Row {
    pub cells: Vec<Cell>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Cell {
    pub header: bool,
    pub content: Vec<Block>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Struck(Vec<Inline>),
    Under(Vec<Inline>),
    Sub(Vec<Inline>),
    Sup(Vec<Inline>),
    Code(String),
    Link { target: String, label: Vec<Inline> },
    External { url: String, label: Vec<Inline> },
    Image { target: String, alt: Vec<Inline> },
    LineBreak,
    Raw(String),
}

impl Block {
    pub fn paragraph(text: impl Into<String>) -> Block {
        Block::Paragraph(vec![Inline::Text(text.into())])
    }

    pub fn raw(text: impl Into<String>) -> Block {
        Block::Raw(text.into())
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Block::Paragraph(content) => content.is_empty(),
            Block::Heading { content, .. } => content.is_empty(),
            Block::List(list) => list.items.is_empty(),
            Block::Quote(blocks) => blocks.is_empty(),
            Block::Table(table) => table.rows.is_empty(),
            Block::Pre { text, .. } => text.is_empty(),
            Block::Rule | Block::Raw(_) => false,
        }
    }
}

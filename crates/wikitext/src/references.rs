use crate::dom::{Attribute, Element, ElementKind, Node};
use std::collections::HashMap;
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reference {
    pub group: String,
    pub name: Option<String>,
    pub content: Vec<Node>,
    pub id: String,
    pub count: usize,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct References {
    entries: Vec<Reference>,
    by_name: HashMap<(String, String), usize>,
}

impl References {
    pub fn new() -> Self {
        References::default()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &[Reference] {
        &self.entries
    }

    pub fn cite(&mut self, group: &str, name: Option<&str>, content: Vec<Node>) -> Vec<Node> {
        let group = group.trim().to_string();
        if let Some(name) = name {
            let key = (group.clone(), name.to_string());
            if let Some(index) = self.by_name.get(&key).copied() {
                let entry = &mut self.entries[index];
                entry.count += 1;
                let count = entry.count;
                let id = entry.id.clone();
                return vec![backlink(&id, count)];
            }
            let index = self.entries.len();
            let id = footnote_id(&group, Some(name), index);
            self.entries.push(Reference {
                group,
                name: Some(name.to_string()),
                content,
                id,
                count: 1,
            });
            self.by_name.insert(key, index);
            let id = self.entries[index].id.clone();
            return vec![backlink(&id, 1)];
        }
        let index = self.entries.len();
        let id = footnote_id(&group, None, index);
        self.entries.push(Reference {
            group,
            name: None,
            content,
            id,
            count: 1,
        });
        self.entries
            .last()
            .map(|entry| vec![backlink(&entry.id, 1)])
            .unwrap_or_default()
    }

    pub fn render(&self, group: &str) -> Vec<Node> {
        let group = group.trim();
        let selected: Vec<&Reference> = self
            .entries
            .iter()
            .filter(|entry| group.is_empty() || entry.group == group)
            .collect();
        if selected.is_empty() {
            return Vec::new();
        }

        let grouped = selected.iter().any(|entry| !entry.group.is_empty())
            || (group.is_empty() && self.has_multiple_groups());
        if !grouped {
            return vec![Node::element(list_for(&selected))];
        }

        let mut groups: Vec<String> = Vec::new();
        for entry in &selected {
            if !groups.contains(&entry.group) {
                groups.push(entry.group.clone());
            }
        }
        let mut out = Vec::new();
        for group_name in groups {
            let members: Vec<&Reference> = selected
                .iter()
                .copied()
                .filter(|entry| entry.group == group_name)
                .collect();
            let mut wrapper = Element::new("div");
            wrapper.add_class("mw-references-wrap");
            if !group_name.is_empty() {
                wrapper.add_class(&format!("mw-references-columns-{}", group_name));
            }
            wrapper.children.push(Node::element(list_for(&members)));
            out.push(Node::element(wrapper));
        }
        out
    }

    fn has_multiple_groups(&self) -> bool {
        let mut seen: Vec<&str> = Vec::new();
        for entry in &self.entries {
            if !seen.contains(&entry.group.as_str()) {
                seen.push(entry.group.as_str());
            }
        }
        seen.len() > 1
    }
}

fn list_for(entries: &[&Reference]) -> Element {
    let mut list = Element::new("ol");
    list.add_class("references");
    for entry in entries {
        let mut item = Element::new("li");
        item.set_attr("id", entry.id.clone());
        item.kind = ElementKind::Reference;

        let mut backlinks = Element::new("span");
        backlinks.add_class("mw-cite-backlink");
        for citation in 1..=entry.count {
            if citation > 1 {
                backlinks.children.push(Node::text(" ".to_string()));
            }
            backlinks.children.push(backlink(&entry.id, citation));
        }
        item.children.push(Node::element(backlinks));

        let mut text = Element::new("span");
        text.add_class("reference-text");
        text.children = entry.content.clone();
        item.children.push(Node::element(text));
        list.children.push(Node::element(item));
    }
    list
}

fn backlink(id: &str, index: usize) -> Node {
    let mut anchor = Element::new("a");
    anchor.set_attr("href", format!("#{id}"));

    if index == 1 {
        anchor.add_class("mw-cite-backlink");
    }
    let arrow = if index == 1 {
        "\u{2191}".to_string()
    } else {
        format!("\u{2191}{index}")
    };
    anchor.children.push(Node::text(arrow));
    Node::element(anchor)
}

fn footnote_id(group: &str, name: Option<&str>, index: usize) -> String {
    let suffix = match name {
        Some(name) => sanitise_id(name),
        None => format!("{}", index + 1),
    };
    if group.is_empty() {
        format!("cite_note-{suffix}")
    } else {
        format!("cite_note-{}-{suffix}", sanitise_id(group))
    }
}

fn sanitise_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            ' ' | '\t' | '\n' => out.push('_'),
            '"' | '\'' | '<' | '>' | '&' | '#' | '[' | ']' | '{' | '}' | '|' => {}
            _ => out.push(ch),
        }
    }
    if out.is_empty() {
        "1".to_string()
    } else {
        out
    }
}

pub fn group_of(attributes: &[Attribute]) -> String {
    attributes
        .iter()
        .find(|attribute| attribute.name.eq_ignore_ascii_case("group"))
        .map(|attribute| attribute.value.trim().to_string())
        .unwrap_or_default()
}

pub fn name_of(attributes: &[Attribute]) -> Option<String> {
    attributes
        .iter()
        .find(|attribute| attribute.name.eq_ignore_ascii_case("name"))
        .map(|attribute| attribute.value.trim().to_string())
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(value: &str) -> Vec<Node> {
        vec![Node::text(value)]
    }

    #[test]
    fn numbers_anonymous_references() {
        let mut references = References::new();
        let first = references.cite("", None, text("a"));
        let second = references.cite("", None, text("b"));
        assert_eq!(references.len(), 2);
        assert!(format!("{first:?}").contains("cite_note-1"));
        assert!(format!("{second:?}").contains("cite_note-2"));
    }

    #[test]
    fn reuses_a_named_reference() {
        let mut references = References::new();
        references.cite("", Some("a"), text("first"));
        references.cite("", Some("a"), text("ignored"));
        assert_eq!(references.len(), 1, "the name identifies one footnote");
        assert_eq!(references.entries()[0].count, 2);
        let html = format!("{:?}", references.render(""));
        assert!(html.contains("cite_note-a"), "{html}");
    }

    #[test]
    fn renders_a_single_list_for_one_group() {
        let mut references = References::new();
        references.cite("", None, text("a"));
        let nodes = references.render("");
        assert_eq!(nodes.len(), 1);
        let Node::Element(list) = &nodes[0] else {
            panic!("expected a list")
        };
        assert_eq!(list.tag.as_str(), "ol");
        assert_eq!(list.attr("class"), Some("references"));
    }

    #[test]
    fn renders_a_wrapper_per_group() {
        let mut references = References::new();
        references.cite("", None, text("a"));
        references.cite("note", None, text("b"));
        let nodes = references.render("");
        assert_eq!(nodes.len(), 2, "two groups produce two lists");
    }

    #[test]
    fn a_group_attribute_selects_one_group() {
        let mut references = References::new();
        references.cite("", None, text("a"));
        references.cite("note", None, text("b"));
        assert_eq!(references.render("note").len(), 1);
        assert_eq!(references.render("").len(), 2);
    }

    #[test]
    fn empty_store_renders_nothing() {
        let references = References::new();
        assert!(references.render("").is_empty());
        assert!(references.is_empty());
    }

    #[test]
    fn sanitises_ids() {
        assert_eq!(sanitise_id("a b"), "a_b");
        assert_eq!(sanitise_id("a<b>c"), "abc");
        assert_eq!(sanitise_id(""), "1");
    }

    #[test]
    fn reads_attributes() {
        let attributes = vec![
            Attribute::new("name", " a "),
            Attribute::new("group", "note"),
        ];
        assert_eq!(name_of(&attributes).as_deref(), Some("a"));
        assert_eq!(group_of(&attributes), "note");
        assert_eq!(name_of(&[]), None);
    }
}

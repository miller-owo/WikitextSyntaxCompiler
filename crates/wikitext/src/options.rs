use crate::preprocess::{ModuleProvider, TemplateResolver};
use std::rc::Rc;
use std::sync::Arc;
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum OutputMode {
    #[default]
    Fragment,
    ParserOutput,
    Document,
}

#[derive(Clone)]
pub struct Options {
    pub title: Option<String>,
    pub namespace: Option<String>,
    pub namespaces: Vec<String>,
    pub media_namespaces: Vec<String>,
    pub file_namespace_aliases: Vec<String>,
    pub category_namespace_aliases: Vec<String>,
    pub language: String,
    pub content_language: Option<String>,
    pub resolver: Option<Arc<dyn TemplateResolver>>,
    pub module_provider: Option<Rc<dyn ModuleProvider>>,
    pub max_expansion_depth: usize,
    pub max_expansion_size: usize,
    pub max_expansions: usize,
    pub heading_anchors: bool,
    pub edit_section_links: bool,
    pub show_categories: bool,
    pub show_images: bool,
    pub diagnostics: bool,
    pub output_mode: OutputMode,
    pub pretty: bool,
    pub image_base_url: Option<String>,
    pub article_path: String,
    pub server_url: Option<String>,
    pub max_heading_level: u8,
    pub in_transclusion: bool,
    pub keep_comments: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            title: None,
            namespace: None,
            namespaces: default_namespaces(),
            media_namespaces: vec!["media".to_string()],
            file_namespace_aliases: vec!["file".to_string(), "image".to_string()],
            category_namespace_aliases: vec!["category".to_string()],
            language: "en".to_string(),
            content_language: None,
            resolver: None,
            module_provider: None,
            max_expansion_depth: 40,
            max_expansion_size: 2 * 1024 * 1024,
            max_expansions: 10_000,
            heading_anchors: false,
            edit_section_links: false,
            show_categories: false,
            show_images: true,
            diagnostics: true,
            output_mode: OutputMode::Fragment,
            pretty: true,
            image_base_url: None,
            article_path: "/wiki/$1".to_string(),
            server_url: None,
            max_heading_level: 6,
            in_transclusion: false,
            keep_comments: false,
        }
    }
}

impl Options {
    pub fn new() -> Self {
        Options::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_resolver(mut self, resolver: Arc<dyn TemplateResolver>) -> Self {
        self.resolver = Some(resolver);
        self
    }

    pub fn with_module_provider(mut self, provider: Rc<dyn ModuleProvider>) -> Self {
        self.module_provider = Some(provider);
        self
    }

    pub fn with_output_mode(mut self, mode: OutputMode) -> Self {
        self.output_mode = mode;
        self
    }

    pub fn title_or_empty(&self) -> &str {
        self.title.as_deref().unwrap_or("")
    }
}

impl core::fmt::Debug for Options {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Options")
            .field("title", &self.title)
            .field("namespace", &self.namespace)
            .field("language", &self.language)
            .field("resolver", &self.resolver.as_ref().map(|_| "<resolver>"))
            .field("output_mode", &self.output_mode)
            .finish_non_exhaustive()
    }
}

fn default_namespaces() -> Vec<String> {
    [
        "media",
        "special",
        "talk",
        "user",
        "user talk",
        "project",
        "project talk",
        "file",
        "file talk",
        "image",
        "image talk",
        "mediawiki",
        "mediawiki talk",
        "template",
        "template talk",
        "help",
        "help talk",
        "category",
        "category talk",
        "module",
        "module talk",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_options_are_populated() {
        let options = Options::default();
        assert!(options.namespaces.contains(&"template".to_string()));
        assert!(options.file_namespace_aliases.contains(&"file".to_string()));
        assert_eq!(options.max_expansion_depth, 40);
    }

    #[test]
    fn builder_sets_title() {
        let options = Options::new().with_title("Foo");
        assert_eq!(options.title_or_empty(), "Foo");
    }
}

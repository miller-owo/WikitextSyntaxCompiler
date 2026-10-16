//! Synchronous by design: a page compiles in a bounded amount of work, so there
//! is nothing to await. A caller that needs concurrency should run this on a
//! worker thread.

use napi::bindgen_prelude::Result;
use napi_derive::napi;
use std::cell::RefCell;
use std::rc::Rc;
use wikitext::lua_modules::LuaModules;
use wikitext::preprocess::MemoryTemplates;
use wikitext::{Options, OutputMode, Severity};

#[napi(object)]
pub struct Format {
    pub html: i32,
    pub markdown: i32,
    pub wikitext: i32,
}

#[napi]
pub fn formats() -> Format {
    Format {
        html: 0,
        markdown: 1,
        wikitext: 2,
    }
}

fn convert_options(from: i32) -> Result<wikitext_convert::Input> {
    match from {
        0 => Ok(wikitext_convert::Input::Html),
        1 => Ok(wikitext_convert::Input::Markdown),
        2 => Ok(wikitext_convert::Input::Wikitext),
        other => Err(napi::Error::from_reason(format!(
            "unknown source format {other}; expected 0 (html), 1 (markdown) or 2 (wikitext)"
        ))),
    }
}

fn convert_target(to: i32) -> Result<wikitext_convert::Output> {
    match to {
        0 => Ok(wikitext_convert::Output::Html),
        1 => Ok(wikitext_convert::Output::Markdown),
        2 => Ok(wikitext_convert::Output::Wikitext),
        other => Err(napi::Error::from_reason(format!(
            "unknown target format {other}; expected 0 (html), 1 (markdown) or 2 (wikitext)"
        ))),
    }
}

#[derive(Default)]
struct Settings {
    title: Option<String>,
    language: Option<String>,
    mode: u8,
    pretty: bool,
    templates: MemoryTemplates,
    modules: LuaModules,
}

impl Settings {
    fn new() -> Settings {
        Settings {
            pretty: true,
            ..Settings::default()
        }
    }

    fn options(&self) -> Options {
        let mut options = Options {
            title: self.title.clone(),
            language: self.language.clone().unwrap_or_else(|| "en".to_string()),
            output_mode: match self.mode {
                1 => OutputMode::ParserOutput,
                2 => OutputMode::Document,
                _ => OutputMode::Fragment,
            },
            pretty: self.pretty,
            ..Options::default()
        };
        if !self.templates.is_empty() {
            let store = MemoryTemplates::new();
            for (name, body) in self.templates.entries() {
                store.insert(&name, body);
            }
            options.resolver = Some(std::sync::Arc::new(store));
        }
        if !self.modules.is_empty() {
            let modules = LuaModules::new();
            if let Some(title) = &self.title {
                modules.set_title(title.clone());
            }
            for (name, source) in self.modules.entries() {
                modules.insert(&name, source);
            }
            options.module_provider = Some(Rc::new(modules));
        }
        options
    }
}

#[napi]
pub struct WikitextOptions {
    inner: Rc<RefCell<Settings>>,
}

// napi wants a constructor, and clippy wants `Default` beside it; they do the
// same thing, so one calls the other.
impl Default for WikitextOptions {
    fn default() -> Self {
        WikitextOptions::new()
    }
}

#[napi]
impl WikitextOptions {
    #[napi(constructor)]
    pub fn new() -> WikitextOptions {
        WikitextOptions {
            inner: Rc::new(RefCell::new(Settings::new())),
        }
    }

    #[napi]
    pub fn title(&self, title: String) {
        self.inner.borrow_mut().title = Some(title);
    }

    #[napi]
    pub fn language(&self, language: String) {
        self.inner.borrow_mut().language = Some(language);
    }

    /// 0 fragment, 1 parser output, 2 full document.
    #[napi]
    pub fn output_mode(&self, mode: u8) {
        self.inner.borrow_mut().mode = mode;
    }

    #[napi]
    pub fn pretty(&self, pretty: bool) {
        self.inner.borrow_mut().pretty = pretty;
    }

    #[napi]
    pub fn add_template(&self, name: String, body: String) {
        self.inner.borrow().templates.insert(&name, body);
    }

    #[napi]
    pub fn add_module(&self, name: String, source: String) {
        self.inner.borrow().modules.insert(&name, source);
    }

    #[napi(getter)]
    pub fn template_count(&self) -> u32 {
        self.inner.borrow().templates.len() as u32
    }

    #[napi(getter)]
    pub fn module_count(&self) -> u32 {
        self.inner.borrow().modules.len() as u32
    }
}

#[napi(object)]
pub struct Heading {
    pub level: u32,
    pub id: String,
    pub title: String,
}

#[napi(object)]
pub struct Category {
    pub name: String,
    pub sort_key: String,
}

#[napi(object)]
pub struct Diagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub line: u32,
    pub column: u32,
}

#[napi(object)]
pub struct Page {
    pub html: String,
    pub headings: Vec<Heading>,
    pub categories: Vec<Category>,
    pub diagnostics: Vec<Diagnostic>,
    pub plain_text: String,
    pub references: u32,
    pub external_links: u32,
    pub redirect: String,
}

#[napi]
pub fn wikitext_to_html(source: String) -> String {
    wikitext::compile(&source, &Options::default())
}

#[napi]
pub fn wikitext_to_document(source: String, title: String) -> String {
    let options = Options {
        title: Some(title),
        output_mode: OutputMode::Document,
        ..Options::default()
    };
    wikitext::compile(&source, &options)
}

#[napi]
pub fn compile(source: String, options: &WikitextOptions) -> String {
    let settings = options.inner.borrow().options();
    wikitext::compile(&source, &settings)
}

#[napi]
pub fn convert(source: String, from: i32, to: i32) -> Result<String> {
    let input = convert_options(from)?;
    let output = convert_target(to)?;
    if input == wikitext_convert::Input::Wikitext && output == wikitext_convert::Output::Html {
        return Ok(wikitext::compile(&source, &Options::default()));
    }
    let settings = wikitext_convert::Options::default().with_input(input);
    Ok(wikitext_convert::convert(&source, &settings, output))
}

#[napi]
pub fn describe(source: String, options: Option<&WikitextOptions>) -> Page {
    let settings = match options {
        Some(options) => options.inner.borrow().options(),
        None => Options::default(),
    };
    let (html, document) = wikitext::compile_with_diagnostics(&source, &settings);
    Page {
        html,
        headings: document
            .headings
            .iter()
            .map(|heading| Heading {
                level: heading.level as u32,
                id: heading.id.clone(),
                title: heading.title.clone(),
            })
            .collect(),
        categories: document
            .categories
            .iter()
            .map(|category| Category {
                name: category.name.clone(),
                sort_key: category.sort_key.clone().unwrap_or_default(),
            })
            .collect(),
        diagnostics: document
            .diagnostics
            .iter()
            .map(|diagnostic| Diagnostic {
                severity: severity_name(diagnostic.severity).to_string(),
                code: diagnostic.code.as_str().to_string(),
                message: diagnostic.message.clone(),
                line: diagnostic.position.line,
                column: diagnostic.position.column,
            })
            .collect(),
        plain_text: document.text_content(),
        references: document.references.len() as u32,
        external_links: document.external_link_count as u32,
        redirect: document.redirect.clone().unwrap_or_default(),
    }
}

#[napi]
pub fn expand(source: String, options: Option<&WikitextOptions>) -> String {
    let settings = match options {
        Some(options) => options.inner.borrow().options(),
        None => Options::default(),
    };
    wikitext::Preprocessor::new(&settings).run(&source)
}

#[napi]
pub fn decode_entities(source: String) -> String {
    wikitext::decode_entities(&source)
}

#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Note => "note",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

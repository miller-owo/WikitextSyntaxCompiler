#![forbid(unsafe_code)]

#[cfg(feature = "wasm")]
use std::cell::RefCell;
use std::rc::Rc;
use wikitext::lua_modules::LuaModules;
use wikitext::preprocess::MemoryTemplates;
use wikitext::{Options, OutputMode, Severity};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Format {
    #[default]
    Html,
    Markdown,
    Wikitext,
}

impl Format {
    #[cfg_attr(not(feature = "wasm"), allow(dead_code))]
    fn from_code(code: i32) -> Option<Format> {
        match code {
            0 => Some(Format::Html),
            1 => Some(Format::Markdown),
            2 => Some(Format::Wikitext),
            _ => None,
        }
    }

    fn input(self) -> wikitext_convert::Input {
        match self {
            Format::Html => wikitext_convert::Input::Html,
            Format::Markdown => wikitext_convert::Input::Markdown,
            Format::Wikitext => wikitext_convert::Input::Wikitext,
        }
    }

    fn output(self) -> wikitext_convert::Output {
        match self {
            Format::Html => wikitext_convert::Output::Html,
            Format::Markdown => wikitext_convert::Output::Markdown,
            Format::Wikitext => wikitext_convert::Output::Wikitext,
        }
    }
}

#[derive(Default)]
pub struct Settings {
    pub title: Option<String>,
    pub language: Option<String>,
    pub mode: u8,
    pub pretty: bool,
    pub templates: MemoryTemplates,
    pub modules: LuaModules,
}

impl Settings {
    pub fn new() -> Settings {
        Settings {
            pretty: true,
            ..Settings::default()
        }
    }

    pub fn options(&self) -> Options {
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

pub fn compile_to_html(source: &str, title: Option<&str>, mode: OutputMode) -> String {
    let options = Options {
        title: title.map(|title| title.to_string()),
        output_mode: mode,
        ..Options::default()
    };
    wikitext::compile(source, &options)
}

pub fn compile_with_settings(source: &str, settings: &Settings) -> String {
    wikitext::compile(source, &settings.options())
}

pub fn convert(source: &str, from: Format, to: Format, settings: Option<&Settings>) -> String {
    if from == Format::Wikitext && to == Format::Html {
        return match settings {
            Some(settings) => compile_with_settings(source, settings),
            None => compile_to_html(source, None, OutputMode::Fragment),
        };
    }
    let options = wikitext_convert::Options::default().with_input(from.input());
    wikitext_convert::convert(source, &options, to.output())
}

// Hand-rolled so the crate keeps a single dependency: a JSON writer is less
// code than pulling in a serialiser.

fn json_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Control characters are not legal raw inside a JSON string.
            other if (other as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}
fn json_field(out: &mut String, first: &mut bool, name: &str, value: &str) {
    if !*first {
        out.push(',');
    }
    *first = false;
    json_string(out, name);
    out.push(':');
    json_string(out, value);
}

// Numbers must not go through json_field, which quotes everything it is given.
fn json_number(out: &mut String, first: &mut bool, name: &str, value: u32) {
    if !*first {
        out.push(',');
    }
    *first = false;
    json_string(out, name);
    out.push(':');
    out.push_str(&value.to_string());
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Note => "note",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

pub fn describe(source: &str, settings: &Settings) -> String {
    let options = settings.options();
    let (html, document) = wikitext::compile_with_diagnostics(source, &options);

    let mut out = String::with_capacity(512);
    out.push('{');
    let mut first = true;
    json_field(&mut out, &mut first, "html", &html);

    if !first {
        out.push(',');
    }
    out.push_str("\"headings\":[");
    for (index, heading) in document.headings.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        let mut head_first = true;
        json_number(&mut out, &mut head_first, "level", heading.level as u32);
        json_field(&mut out, &mut head_first, "id", &heading.id);
        json_field(&mut out, &mut head_first, "title", &heading.title);
        out.push('}');
    }
    out.push(']');

    out.push_str(",\"categories\":[");
    for (index, category) in document.categories.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        let mut cat_first = true;
        json_field(&mut out, &mut cat_first, "name", &category.name);
        json_field(
            &mut out,
            &mut cat_first,
            "sortKey",
            category.sort_key.as_deref().unwrap_or(""),
        );
        out.push('}');
    }
    out.push(']');

    out.push_str(",\"diagnostics\":[");
    for (index, diagnostic) in document.diagnostics.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        let mut diag_first = true;
        json_field(
            &mut out,
            &mut diag_first,
            "severity",
            severity_name(diagnostic.severity),
        );
        json_field(&mut out, &mut diag_first, "code", diagnostic.code.as_str());
        json_field(&mut out, &mut diag_first, "message", &diagnostic.message);
        json_number(&mut out, &mut diag_first, "line", diagnostic.position.line);
        json_number(
            &mut out,
            &mut diag_first,
            "column",
            diagnostic.position.column,
        );
        out.push('}');
    }
    out.push(']');

    out.push_str(",\"plainText\":");
    json_string(&mut out, &document.text_content());
    out.push_str(",\"references\":");
    out.push_str(&document.references.len().to_string());
    out.push_str(",\"externalLinks\":");
    out.push_str(&document.external_link_count.to_string());
    out.push_str(",\"redirect\":");
    json_string(&mut out, document.redirect.as_deref().unwrap_or(""));
    out.push('}');
    out
}

pub fn diagnostics_json(source: &str) -> String {
    let document = wikitext::parse(source, &Options::default());
    let mut out = String::from("[");
    for (index, diagnostic) in document.diagnostics.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        let mut first = true;
        json_field(
            &mut out,
            &mut first,
            "severity",
            severity_name(diagnostic.severity),
        );
        json_field(&mut out, &mut first, "code", diagnostic.code.as_str());
        json_field(&mut out, &mut first, "message", &diagnostic.message);
        json_number(&mut out, &mut first, "line", diagnostic.position.line);
        json_number(&mut out, &mut first, "column", diagnostic.position.column);
        out.push('}');
    }
    out.push(']');
    out
}

#[cfg(feature = "wasm")]
mod bindings {
    use super::*;
    use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen(js_name = WikitextOptions)]
    pub struct WasmOptions {
        inner: Rc<RefCell<Settings>>,
    }

    #[wasm_bindgen(js_class = WikitextOptions)]
    impl WasmOptions {
        #[wasm_bindgen(constructor)]
        pub fn new() -> WasmOptions {
            WasmOptions {
                inner: Rc::new(RefCell::new(Settings::new())),
            }
        }

        pub fn title(&self, title: &str) {
            self.inner.borrow_mut().title = Some(title.to_string());
        }

        pub fn language(&self, language: &str) {
            self.inner.borrow_mut().language = Some(language.to_string());
        }

        #[wasm_bindgen(js_name = outputMode)]
        pub fn output_mode(&self, mode: u8) {
            self.inner.borrow_mut().mode = mode;
        }

        pub fn pretty(&self, pretty: bool) {
            self.inner.borrow_mut().pretty = pretty;
        }

        #[wasm_bindgen(js_name = addTemplate)]
        pub fn add_template(&self, name: &str, body: &str) {
            self.inner.borrow().templates.insert(name, body);
        }

        #[wasm_bindgen(js_name = addModule)]
        pub fn add_module(&self, name: &str, source: &str) {
            self.inner.borrow().modules.insert(name, source);
        }

        #[wasm_bindgen(js_name = templateCount)]
        pub fn template_count(&self) -> usize {
            self.inner.borrow().templates.len()
        }

        #[wasm_bindgen(js_name = moduleCount)]
        pub fn module_count(&self) -> usize {
            self.inner.borrow().modules.len()
        }
    }

    #[wasm_bindgen(js_name = wikitextToHtml)]
    pub fn wikitext_to_html(source: &str) -> String {
        compile_to_html(source, None, OutputMode::Fragment)
    }

    #[wasm_bindgen(js_name = wikitextToDocument)]
    pub fn wikitext_to_document(source: &str, title: &str) -> String {
        compile_to_html(source, Some(title), OutputMode::Document)
    }

    #[wasm_bindgen(js_name = wikitextCompile)]
    pub fn wikitext_compile(source: &str, options: &WasmOptions) -> String {
        compile_with_settings(source, &options.inner.borrow())
    }

    #[wasm_bindgen(js_name = wikitextConvert)]
    pub fn wikitext_convert(source: &str, from: i32, to: i32) -> Result<String, String> {
        let from = Format::from_code(from).ok_or_else(|| format!("unknown format {from}"))?;
        let to = Format::from_code(to).ok_or_else(|| format!("unknown format {to}"))?;
        Ok(convert(source, from, to, None))
    }

    #[wasm_bindgen(js_name = wikitextDescribe)]
    pub fn wikitext_describe(source: &str, options: Option<WasmOptions>) -> String {
        match options {
            Some(options) => describe(source, &options.inner.borrow()),
            None => describe(source, &Settings::new()),
        }
    }

    #[wasm_bindgen(js_name = wikitextDiagnostics)]
    pub fn wikitext_diagnostics(source: &str) -> String {
        diagnostics_json(source)
    }

    #[wasm_bindgen(js_name = wikitextExpand)]
    pub fn wikitext_expand(source: &str, options: Option<WasmOptions>) -> String {
        let settings = match options {
            Some(options) => options.inner.borrow().options(),
            None => Options::default(),
        };
        wikitext::Preprocessor::new(&settings).run(source)
    }

    #[wasm_bindgen(js_name = wikitextVersion)]
    pub fn wikitext_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_to_fragment() {
        let html = compile_to_html("'''x'''", None, OutputMode::Fragment);
        assert_eq!(html, "<p><b>x</b></p>\n");
    }

    #[test]
    fn compiles_to_document() {
        let html = compile_to_html("x", Some("T"), OutputMode::Document);
        assert!(html.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn settings_carry_templates() {
        let settings = Settings::new();
        settings.templates.insert("greet", "Hello, {{{1|world}}}!");
        assert_eq!(
            compile_with_settings("{{greet|Rust}}", &settings),
            "<p>Hello, Rust!</p>\n"
        );
    }

    #[test]
    fn settings_carry_lua_modules() {
        let settings = Settings::new();
        settings.modules.insert(
            "Roman",
            "local p = {}\nfunction p.convert(f) return 'MCMXCIV' end\nreturn p\n",
        );
        assert_eq!(
            compile_with_settings("{{#invoke:Roman|convert|1994}}", &settings),
            "<p>MCMXCIV</p>\n"
        );
    }

    #[test]
    fn settings_choose_the_output_mode() {
        let mut settings = Settings::new();
        settings.mode = 1;
        let html = compile_with_settings("x", &settings);
        assert!(html.contains("mw-parser-output"), "{html}");
    }

    #[test]
    fn conversion_works_in_every_direction() {
        assert_eq!(
            convert(
                "# T\n\nSome **bold**.",
                Format::Markdown,
                Format::Wikitext,
                None
            ),
            "== T ==\n\nSome '''bold'''."
        );
        assert_eq!(
            convert(
                "== T ==\n\nSome '''bold'''.",
                Format::Wikitext,
                Format::Markdown,
                None
            ),
            "## T\n\nSome **bold**."
        );
        assert_eq!(
            convert("<h2>T</h2><p>x</p>", Format::Html, Format::Wikitext, None),
            "== T ==\n\nx"
        );
        assert_eq!(
            convert("'''x'''", Format::Wikitext, Format::Html, None),
            "<p><b>x</b></p>\n"
        );
    }

    #[test]
    fn wikitext_to_html_honours_settings() {
        let settings = Settings::new();
        settings.templates.insert("t", "from a template");
        assert_eq!(
            convert("{{t}}", Format::Wikitext, Format::Html, Some(&settings)),
            "<p>from a template</p>\n"
        );
    }

    #[test]
    fn describe_reports_the_page_as_json() {
        let json = describe(
            "== One ==\n\nText.\n\n[[Category:Demo|Sort]]",
            &Settings::new(),
        );
        assert!(json.contains("\"headings\":["), "{json}");
        assert!(json.contains("\"title\":\"One\""), "{json}");
        assert!(json.contains("\"categories\":["), "{json}");
        assert!(json.contains("\"name\":\"Demo\""), "{json}");
        assert!(json.contains("\"sortKey\":\"Sort\""), "{json}");
        assert!(json.contains("\"diagnostics\":["), "{json}");
        assert!(json.contains("\"plainText\":"), "{json}");
    }

    #[test]
    fn json_escapes_what_it_has_to() {
        let mut out = String::new();
        json_string(&mut out, "a \"b\" \\ c \n d \u{1}");
        assert_eq!(out, "\"a \\\"b\\\" \\\\ c \\n d \\u0001\"");
    }

    #[test]
    fn diagnostics_report_a_position() {
        let json = diagnostics_json("{| ");
        assert!(json.starts_with('['), "{json}");
        assert!(json.contains("\"line\":"), "{json}");
    }

    #[test]
    fn describe_carries_the_diagnostics_too() {
        let json = describe("{| ", &Settings::new());
        assert!(json.contains("\"diagnostics\":["), "{json}");
        assert!(json.contains("\"message\":"), "{json}");
    }

    #[test]
    fn an_empty_page_describes_itself() {
        let json = describe("", &Settings::new());
        assert!(json.contains("\"headings\":[]"), "{json}");
        assert!(json.contains("\"categories\":[]"), "{json}");
    }
}
#[cfg(test)]
mod json_shape_tests {
    use super::*;

    #[test]
    fn numbers_are_not_quoted() {
        // Everything went through json_field at first, which quotes whatever it is
        // given, so a caller in JavaScript read level back as "2" rather than 2.
        // The table opens on line 5, so the position is real rather than 0.
        let json = describe("== One ==\n\nText.\n\n{| ", &Settings::new());
        assert!(json.contains("\"level\":2"), "{json}");
        assert!(json.contains("\"line\":5"), "{json}");
        assert!(json.contains("\"column\":1"), "{json}");
        assert!(!json.contains("\"level\":\"2\""), "{json}");
        assert!(!json.contains("\"line\":\"5\""), "{json}");
        assert!(json.contains("\"references\":0"), "{json}");
        assert!(json.contains("\"externalLinks\":0"), "{json}");
    }

    #[test]
    fn strings_are_still_quoted() {
        let json = describe("== One ==\n\nText.", &Settings::new());
        assert!(json.contains("\"title\":\"One\""), "{json}");
        assert!(json.contains("\"severity\":\"warning\"") || json.contains("\"headings\":["));
    }

    #[test]
    fn a_quote_or_backslash_in_text_does_not_break_the_json() {
        let json = describe("He said \"hi\" and \\ there\n\n{| ", &Settings::new());
        assert!(json.contains("\\\"hi\\\""), "{json}");
        assert!(json.contains("\\\\"), "{json}");
        assert!(json.ends_with('}'), "{json}");
    }
}

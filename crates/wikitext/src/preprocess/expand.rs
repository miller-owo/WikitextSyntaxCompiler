use crate::options::Options;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Mutex;
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Transclusion {
    pub name: String,
    pub namespace: Option<String>,
    pub arguments: Vec<Argument>,
    pub depth: usize,
}

impl Transclusion {
    pub fn positional(&self, index: usize) -> Option<&str> {
        self.arguments
            .iter()
            .filter(|argument| argument.name.is_empty())
            .nth(index.wrapping_sub(1))
            .map(|argument| argument.value.as_str())
    }

    pub fn named(&self, name: &str) -> Option<&str> {
        let name = name.trim();
        self.arguments
            .iter()
            .find(|argument| argument.name.trim() == name)
            .map(|argument| argument.value.as_str())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Argument {
    pub name: String,
    pub value: String,
}

pub trait TemplateResolver: Send + Sync + std::fmt::Debug {
    fn resolve(&self, request: &Transclusion) -> Option<String>;

    fn exists(&self, name: &str) -> bool {
        self.resolve(&Transclusion {
            name: name.to_string(),
            namespace: None,
            arguments: Vec::new(),
            depth: 1,
        })
        .is_some()
    }

    fn in_transclusion(&self) -> bool {
        true
    }

    fn keep_comments(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "resolver"
    }
}

pub trait ModuleProvider: std::fmt::Debug {
    fn invoke(&self, module: &str, function: &str, arguments: &[Argument]) -> Option<String>;

    fn module_exists(&self, module: &str) -> bool;

    fn name(&self) -> &str {
        "modules"
    }
}

#[allow(dead_code)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParserFunction {
    pub name: String,
    pub arguments: Vec<Argument>,
}

#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub title: String,
    pub named: HashMap<String, String>,
    pub positional: Vec<String>,
}

impl Frame {
    pub fn new(title: impl Into<String>) -> Self {
        Frame {
            title: title.into(),
            named: HashMap::new(),
            positional: Vec::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        let key = key.trim();
        if let Ok(index) = key.parse::<usize>() {
            if index >= 1 {
                if let Some(value) = self.positional.get(index - 1) {
                    return Some(value);
                }
            }
        }
        self.named.get(key).map(|value| value.as_str())
    }

    pub fn push_positional(&mut self, value: impl Into<String>) {
        self.positional.push(value.into());
    }

    pub fn set_named(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.named.insert(name.into(), value.into());
    }
}

#[derive(Debug, Default)]
pub struct MemoryTemplates {
    templates: Mutex<HashMap<String, String>>,
}

impl MemoryTemplates {
    pub fn new() -> Self {
        MemoryTemplates {
            templates: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, name: &str, body: impl Into<String>) {
        let key = normalise_title(name);
        if let Ok(mut map) = self.templates.lock() {
            map.insert(key, body.into());
        }
    }

    pub fn extend<I, N, B>(&self, items: I)
    where
        I: IntoIterator<Item = (N, B)>,
        N: AsRef<str>,
        B: Into<String>,
    {
        for (name, body) in items {
            self.insert(name.as_ref(), body);
        }
    }

    pub fn len(&self) -> usize {
        self.templates.lock().map(|map| map.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(&self) -> Vec<(String, String)> {
        self.templates
            .lock()
            .map(|map| {
                map.iter()
                    .map(|(name, body)| (name.clone(), body.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn get(&self, name: &str) -> Option<String> {
        let key = normalise_title(name);
        self.templates.lock().ok()?.get(&key).cloned()
    }
}

impl TemplateResolver for MemoryTemplates {
    fn resolve(&self, request: &Transclusion) -> Option<String> {
        self.get(&request.name)
    }

    fn exists(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    fn name(&self) -> &str {
        "memory"
    }
}

pub fn normalise_title(title: &str) -> String {
    let spaced = title.trim().replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(crate) fn resolver_of(options: &Options) -> Option<&dyn TemplateResolver> {
    options.resolver.as_deref()
}

pub(crate) fn module_provider_of(options: &Options) -> Option<Rc<dyn ModuleProvider>> {
    options.module_provider.clone()
}

#[derive(Clone)]
pub(crate) struct Expander<'a> {
    options: &'a Options,
    resolver: Option<&'a dyn TemplateResolver>,
    modules: Option<Rc<dyn ModuleProvider>>,
    expansions: usize,

    pub(crate) limit_hit: bool,

    pub(crate) default_sort: Option<String>,

    pub(crate) display_title: Option<String>,
}

impl<'a> Expander<'a> {
    pub(crate) fn new(options: &'a Options) -> Self {
        Expander {
            options,
            resolver: resolver_of(options),
            modules: module_provider_of(options),
            expansions: 0,
            limit_hit: false,
            default_sort: None,
            display_title: None,
        }
    }

    pub(crate) fn run(&mut self, source: &str) -> String {
        let frame = Frame::new(self.options.title_or_empty());
        let mut text = source.to_string();
        self.expand_in_place(&mut text, &frame, 0);
        text
    }

    pub(crate) fn default_sort(&self) -> Option<&str> {
        self.default_sort.as_deref()
    }

    pub(crate) fn display_title(&self) -> Option<&str> {
        self.display_title.as_deref()
    }

    pub(crate) fn expand_in_place(&mut self, text: &mut String, frame: &Frame, depth: usize) {
        if depth > self.options.max_expansion_depth {
            self.limit_hit = true;
            return;
        }
        if self.expansions > self.options.max_expansions {
            self.limit_hit = true;
            return;
        }
        let mut search_from = 0usize;
        // An opener whose brace run is no longer than one that already failed
        // cannot match either: a longer run is what would have closed it. So a
        // failed candidate advances past every opener that is not longer, which
        // is what keeps a page full of unclosed `{{` linear instead of quadratic.
        let mut failed_run = 0usize;
        loop {
            let Some(open) = find_open(text, search_from) else {
                return;
            };
            let run = text.as_bytes()[open..]
                .iter()
                .take_while(|byte| **byte == b'{')
                .count();
            if run <= failed_run {
                search_from = open + run.max(2);
                continue;
            }
            let Some(span) = match_open(text, open) else {
                failed_run = run;
                search_from = open + run.max(2);
                continue;
            };
            let replacement = self.expand_construct(text, &span, frame, depth);
            let Some(replacement) = replacement else {
                search_from = span.end;
                continue;
            };
            if replacement.len() > self.options.max_expansion_size {
                self.limit_hit = true;
                search_from = span.end;
                continue;
            }
            self.expansions += 1;
            text.replace_range(span.start..span.end, &replacement);

            search_from = span.start + replacement.len();
        }
    }

    fn expand_construct(
        &mut self,
        text: &str,
        span: &MatchSpan,
        frame: &Frame,
        depth: usize,
    ) -> Option<String> {
        let inner = &text[span.inner_start..span.inner_end];
        if span.kind == BraceKind::Parameter {
            return Some(self.expand_parameter(inner, frame, depth));
        }
        self.expand_transclusion(inner, frame, depth)
    }

    fn expand_parameter(&mut self, inner: &str, frame: &Frame, depth: usize) -> String {
        let parts = split_arguments(inner);
        let name = parts.first().map(|part| part.value.as_str()).unwrap_or("");
        let default = if parts.len() > 1 {
            let mut joined = String::new();
            for (index, part) in parts.iter().enumerate().skip(1) {
                if index > 1 {
                    joined.push('|');
                }
                joined.push_str(&part.value);
            }
            Some(joined)
        } else {
            None
        };

        let mut name = name.to_string();
        self.expand_in_place(&mut name, frame, depth + 1);
        let name = name.trim().to_string();
        if let Some(value) = frame.get(&name) {
            let mut value = value.to_string();
            self.expand_in_place(&mut value, frame, depth + 1);
            return value;
        }
        match default {
            Some(default) => {
                let mut default = default;
                self.expand_in_place(&mut default, frame, depth + 1);
                default
            }
            None => String::new(),
        }
    }

    fn expand_transclusion(&mut self, inner: &str, frame: &Frame, depth: usize) -> Option<String> {
        let parts = split_arguments(inner);
        let raw_name = parts.first()?.value.clone();
        let mut name = raw_name.clone();

        self.expand_in_place(&mut name, frame, depth + 1);
        let name = name.trim().to_string();
        if name.is_empty() {
            return Some(format!("{{{{{inner}}}}}"));
        }

        if let Some(rest) = strip_prefix_ci(&name, "#invoke:") {
            return Some(self.invoke_module(rest, &parts[1..], frame, depth));
        }

        if let Some(rest) = name.strip_prefix('#') {
            let (function, first_argument) = match rest.split_once(':') {
                Some((function, argument)) => (function.trim(), Some(argument)),
                None => (rest.trim(), None),
            };
            let mut arguments: Vec<Argument> = Vec::with_capacity(parts.len());
            if let Some(first) = first_argument {
                arguments.push(Argument {
                    name: String::new(),
                    value: first.to_string(),
                });
            }
            arguments.extend(parts[1..].iter().cloned());
            return Some(crate::preprocess::functions::call(
                self, function, &arguments, frame, depth,
            ));
        }

        if let Some((word, value)) = name.split_once(':') {
            let mut probe = self.clone();
            if let Some(result) = crate::preprocess::functions::magic_word_with_argument(
                &mut probe, word, value, frame,
            ) {
                *self = probe;
                return Some(result);
            }
        }

        let mut request = Transclusion {
            name: name.clone(),
            namespace: None,
            arguments: parts[1..].to_vec(),
            depth: depth + 1,
        };
        if let Some((prefix, rest)) = name.split_once(':') {
            if prefix.eq_ignore_ascii_case("template") {
                request.name = rest.trim().to_string();
            } else {
                request.namespace = Some(prefix.trim().to_string());
            }
        }

        let Some(resolver) = self.resolver else {
            if let Some(value) =
                crate::preprocess::functions::magic_word(self, &request.name, frame)
            {
                return Some(value);
            }
            return Some(red_link(&request.name));
        };

        if let Some(body) = resolver.resolve(&request) {
            return Some(self.transclude(&body, &request, frame, depth));
        }
        if let Some(value) = crate::preprocess::functions::magic_word(self, &request.name, frame) {
            return Some(value);
        }
        Some(red_link(&request.name))
    }

    fn transclude(
        &mut self,
        body: &str,
        request: &Transclusion,
        _parent: &Frame,
        depth: usize,
    ) -> String {
        let mut frame = Frame::new(normalise_title(&request.name));
        for argument in &request.arguments {
            if argument.name.is_empty() {
                frame.push_positional(argument.value.clone());
            } else {
                frame.set_named(argument.name.clone(), argument.value.clone());
            }
        }

        if let Some(title) = &self.options.title {
            frame.title = title.clone();
        }
        let mut body = body.to_string();
        self.expand_in_place(&mut body, &frame, depth + 1);
        body
    }

    fn invoke_module(
        &mut self,
        module: &str,
        arguments: &[Argument],
        frame: &Frame,
        depth: usize,
    ) -> String {
        let Some((function, rest)) = arguments.split_first() else {
            return "<strong class=\"error\">invoke: missing function name</strong>".to_string();
        };
        let module = module.trim();
        let function = function.value.trim();
        let Some(provider) = self.modules.clone() else {
            return "<strong class=\"error\">invoke: no Lua module provider is configured</strong>"
                .to_string();
        };

        let mut expanded: Vec<Argument> = Vec::with_capacity(rest.len() + 1);
        for argument in rest {
            let mut value = argument.value.clone();
            self.expand_in_place(&mut value, frame, depth + 1);
            expanded.push(Argument {
                name: argument.name.clone(),
                value,
            });
        }
        match provider.invoke(module, function, &expanded) {
            Some(result) => result,
            None => format!(
                "<strong class=\"error\">invoke: module `{module}` has no function `{function}`</strong>"
            ),
        }
    }

    pub(crate) fn options(&self) -> &'a Options {
        self.options
    }

    pub(crate) fn record_default_sort(&mut self, value: String) {
        self.default_sort = Some(value);
    }

    pub(crate) fn record_display_title(&mut self, value: String) {
        self.display_title = Some(value);
    }
}

fn red_link(name: &str) -> String {
    format!("[[{}]]", normalise_title(name))
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    if text.len() >= prefix.len() && text[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&text[prefix.len()..])
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BraceKind {
    Template,
    Parameter,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct MatchSpan {
    pub start: usize,
    pub end: usize,
    pub inner_start: usize,
    pub inner_end: usize,
    pub kind: BraceKind,
}

pub(crate) fn find_open(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = from;
    while index + 1 < bytes.len() {
        if bytes[index] == b'{' && bytes[index + 1] == b'{' {
            return Some(index);
        }
        index += 1;
    }
    None
}

pub(crate) fn match_open(text: &str, open: usize) -> Option<MatchSpan> {
    let bytes = text.as_bytes();
    // Consecutive openers are one construct's worth of nesting, and they all
    // fail to match together. Skipping the run keeps an unclosed `{{{{{{…` from
    // being re-scanned once per character, which is what made it blow up.
    let opener_run = bytes[open..].iter().take_while(|b| **b == b'{').count();
    if opener_run >= 2 {
        let mut index = open + opener_run;
        while index < bytes.len() {
            if bytes[index] == b'{' {
                index += bytes[index..].iter().take_while(|b| **b == b'{').count();
                continue;
            }
            break;
        }
        if index >= bytes.len() {
            return None;
        }
    }

    let mut level = 0i64;
    let mut index = open;
    let mut opener_len = 2usize;
    while index < bytes.len() {
        if bytes[index] == b'{' {
            let run = bytes[index..].iter().take_while(|b| **b == b'{').count();
            if run >= 3 && level == 0 && index == open {
                opener_len = 3;
                level += 1;
                index += 3;
                continue;
            }
            if run >= 2 {
                level += 1;
                index += 2;
                continue;
            }
            index += 1;
        } else if bytes[index] == b'}' {
            let run = bytes[index..].iter().take_while(|b| **b == b'}').count();
            let needed = if level == 1 { opener_len } else { 2 };
            if run >= needed {
                level -= 1;
                index += needed;
                if level == 0 {
                    let inner_start = open + opener_len;
                    let inner_end = index - needed;
                    return Some(MatchSpan {
                        start: open,
                        end: index,
                        inner_start,
                        inner_end,
                        kind: if opener_len == 3 {
                            BraceKind::Parameter
                        } else {
                            BraceKind::Template
                        },
                    });
                }
                continue;
            }
            index += run;
        } else {
            index += 1;
        }
    }
    None
}

pub(crate) fn split_arguments(inner: &str) -> Vec<Argument> {
    let bytes = inner.as_bytes();
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut brackets = 0i64;
    let mut double_braces = 0i64;
    let mut triple_braces = 0i64;
    let mut ext_brackets = 0i64;
    while index < bytes.len() {
        match bytes[index] {
            b'[' => {
                if bytes[index..].starts_with(b"[[") {
                    brackets += 1;
                    index += 2;
                    continue;
                }
                ext_brackets += 1;
                index += 1;
            }
            b']' => {
                if ext_brackets > 0 {
                    ext_brackets -= 1;
                } else {
                    brackets -= 1;
                }
                index += 1;
            }
            b'{' => {
                if bytes[index..].starts_with(b"{{{") {
                    triple_braces += 1;
                    index += 3;
                    continue;
                }
                if bytes[index..].starts_with(b"{{") {
                    double_braces += 1;
                    index += 2;
                    continue;
                }
                index += 1;
            }
            b'}' => {
                if bytes[index..].starts_with(b"}}}") && triple_braces > 0 {
                    triple_braces -= 1;
                    index += 3;
                    continue;
                }
                if bytes[index..].starts_with(b"}}") && double_braces > 0 {
                    double_braces -= 1;
                    index += 2;
                    continue;
                }
                index += 1;
            }
            b'|' if brackets <= 0
                && double_braces <= 0
                && triple_braces <= 0
                && ext_brackets <= 0 =>
            {
                parts.push(inner[start..index].to_string());
                index += 1;
                start = index;
            }
            _ => index += 1,
        }
    }
    parts.push(inner[start..].to_string());
    parts
        .into_iter()
        .map(|part| match part.split_once('=') {
            Some((name, value)) if !name.contains(['[', ']', '{', '}']) => Argument {
                name: name.trim().to_string(),
                value: value.to_string(),
            },
            _ => Argument {
                name: String::new(),
                value: part,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn expand(source: &str, templates: &[(&str, &str)]) -> String {
        let store = MemoryTemplates::new();
        store.extend(templates.iter().copied());
        let options = Options::default().with_resolver(Arc::new(store));
        super::super::Preprocessor::new(&options).run(source)
    }

    #[test]
    fn expands_a_simple_template() {
        assert_eq!(expand("{{hello}}", &[("hello", "world")]), "world");
    }

    #[test]
    fn substitutes_positional_arguments() {
        assert_eq!(expand("{{t|a|b}}", &[("t", "{{{1}}}-{{{2}}}")]), "a-b");
    }

    #[test]
    fn substitutes_named_arguments() {
        assert_eq!(expand("{{t|name=x}}", &[("t", "{{{name}}}")]), "x");
    }

    #[test]
    fn uses_defaults_for_missing_parameters() {
        assert_eq!(expand("{{t}}", &[("t", "{{{1|fallback}}}")]), "fallback");
        assert_eq!(expand("{{t|given}}", &[("t", "{{{1|fallback}}}")]), "given");
    }

    #[test]
    fn missing_parameters_expand_to_nothing() {
        assert_eq!(expand("{{t}}", &[("t", "[{{{1}}}]")]), "[]");
    }

    #[test]
    fn expands_nested_templates() {
        assert_eq!(
            expand("{{outer}}", &[("outer", "{{inner}}"), ("inner", "deep")]),
            "deep"
        );
    }

    #[test]
    fn handles_arguments_containing_links() {
        assert_eq!(expand("{{t|[[a|b]]}}", &[("t", "{{{1}}}")]), "[[a|b]]");
    }

    #[test]
    fn missing_template_becomes_a_red_link() {
        assert_eq!(expand("{{nope}}", &[]), "[[Nope]]");
    }

    #[test]
    fn unmatched_braces_are_literal() {
        assert_eq!(expand("{{unclosed", &[]), "{{unclosed");
        assert_eq!(expand("a }} b", &[]), "a }} b");
    }

    #[test]
    fn nowiki_protects_transclusions() {
        assert_eq!(
            expand("<nowiki>{{hello}}</nowiki>", &[("hello", "world")]),
            "<nowiki>{{hello}}</nowiki>"
        );
    }

    #[test]
    fn arguments_are_expanded_lazily() {
        assert_eq!(
            expand("{{t|{{inner}}}}", &[("t", "[{{{1}}}]"), ("inner", "x")]),
            "[x]"
        );
    }

    #[test]
    fn splits_arguments_around_nested_markup() {
        let parts = split_arguments("a=[[b|c]]|d={{e|f}}|plain");
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].name, "a");
        assert_eq!(parts[0].value, "[[b|c]]");
        assert_eq!(parts[1].name, "d");
        assert_eq!(parts[2].value, "plain");
    }

    #[test]
    fn matches_braces() {
        let span = match_open("{{a}}", 0).unwrap();
        assert_eq!(span.kind, BraceKind::Template);
        assert_eq!(span.inner_start, 2);
        assert_eq!(span.inner_end, 3);
        let span = match_open("{{{a}}}", 0).unwrap();
        assert_eq!(span.kind, BraceKind::Parameter);
        assert_eq!(span.inner_end, 4);
        assert!(match_open("{{a}", 0).is_none());
    }

    #[test]
    fn normalises_titles() {
        assert_eq!(normalise_title("foo_bar"), "Foo bar");
        assert_eq!(normalise_title("Foo"), "Foo");
    }
}

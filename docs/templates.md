# Templates

A transclusion such as `{{Infobox|…}}` is not something the compiler can resolve on
its own. It needs the template's source, and only the application knows where that
lives: a database, a directory, an API, a wiki dump. So the compiler asks.

## The resolver

`TemplateResolver` is the trait it asks through:

```rust
pub trait TemplateResolver: Send + Sync + std::fmt::Debug {
    fn resolve(&self, request: &Transclusion) -> Option<String>;

    fn exists(&self, name: &str) -> bool { /* defaults to resolve */ }

    fn in_transclusion(&self) -> bool { /* true */ }
}
```

Return the template's source, or `None` when there is no such template. `Send + Sync`
means one resolver can serve concurrent compilations, which matters for a server.

The request is a `Transclusion`, not a bare name, so a resolver sees the whole call:

```rust
pub struct Transclusion {
    pub name: String,
    pub namespace: Option<String>,
    pub arguments: Vec<Argument>,
    pub depth: usize,
}

pub struct Argument {
    pub name: String,
    pub value: String,
}
```

`depth` is how many transclusions deep this one is, counting from 1, and `arguments`
holds the values as written, before expansion. Two methods have defaults and rarely
need overriding: `exists` falls back to `resolve`, and `in_transclusion` answers
whether the page being compiled is itself being transcluded.

Names arrive normalised the way page titles are: the first letter is upper-cased,
underscores and spaces are equivalent, and a leading `Template:` is stripped. So
`{{infobox}}`, `{{Infobox}}` and `{{Template:Infobox}}` all ask for `Infobox`.

## The built-in store

`MemoryTemplates` is a resolver over an in-memory map, useful for tests, for a small
fixed set, and for getting started:

```rust
use std::sync::Arc;
use wikitext::preprocess::MemoryTemplates;
use wikitext::{compile, Options};

let templates = MemoryTemplates::new();
templates.insert("greet", "Hello, {{{1|world}}}!");
let options = Options::default().with_resolver(Arc::new(templates));

assert_eq!(compile("{{greet|Rust}}", &options).trim(), "<p>Hello, Rust!</p>");
```

`insert` takes the name and the body. The name is normalised on insert as well, so
`MemoryTemplates::insert("Infobox", …)` and a transclusion of `{{infobox}}` find each
other.

## Parameters

Inside a template body, `{{{name|default}}}` is a parameter. Positional parameters are
`{{{1}}}`, `{{{2}}}` and so on.

```
{{{1|}}}          the first positional parameter, empty when absent
{{{name|none}}}   a named one, with a fallback
```

Two behaviours are worth knowing because they are easy to trip over:

- **Evaluation is lazy.** An argument is only expanded when the body uses it. A
  transclusion whose body never mentions `{{{1}}}` does not pay for expanding it, and
  an argument that would recurse forever is never touched.
- **A missing parameter is not an error.** `{{{1}}}` with no argument renders as
  empty, which is why the `|` fallback is worth writing.

## Loading templates from a directory

The CLI reads `*.wikitext` files from a directory and uses each file name as the
template name:

```sh
mkdir templates
printf 'Hello, {{{1|world}}}!\n' > templates/Greet.wikitext
wikitext --templates ./templates page.wikitext
```

`Greet.wikitext` answers `{{Greet|…}}`. The file's line endings do not matter; CRLF
and LF produce the same output.

## Checked errors

A template that does not exist is not a failure. The parser renders it as a red link,
which is what MediaWiki does:

```rust
let html = compile("{{greet}}", &Options::default());
assert_eq!(html, "<p><a href=\"/wiki/Greet\" class=\"mw-wikilink\">Greet</a></p>\n");
```

That is a deliberate choice: a page full of missing templates should still produce
readable output, so a wiki import does not stop at the first gap. If you need to know
which templates were missing, resolve them yourself before compiling and report the
`None` cases.

## A resolver of your own

For a real store, implement the trait. This one wraps a map and counts how often it
was asked, which is also how you would hook in a cache or a database:

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wikitext::preprocess::{TemplateResolver, Transclusion};
use wikitext::{compile, Options};

#[derive(Debug)]
struct Pages {
    pages: HashMap<String, String>,
    lookups: Mutex<usize>,
}

impl TemplateResolver for Pages {
    fn resolve(&self, request: &Transclusion) -> Option<String> {
        *self.lookups.lock().expect("not poisoned") += 1;
        self.pages.get(&request.name).cloned()
    }
}

let mut pages = HashMap::new();
pages.insert("Greet".to_string(), "Hello, {{{1|world}}}!".to_string());
let resolver = Arc::new(Pages { pages, lookups: Mutex::new(0) });

let options = Options::default().with_resolver(resolver.clone());
assert_eq!(compile("{{Greet|Rust}}", &options).trim(), "<p>Hello, Rust!</p>");
```

`resolve` is called once per transclusion, so a resolver is a good place to cache:
returning a `String` means the compiler owns its copy, and nothing is retained across
calls.

Do not call back into the same `Options` from a resolver. It runs on the thread that
is compiling, and re-entering would recurse.

## Bounds on expansion

Expansion is bounded so a page cannot make the compiler run forever:

| Limit | Default | Option |
|---|---|---|
| Expansion depth | 40 | `max_expansion_depth` |
| Expansions per page | 10 000 | `max_expansions` |
| Expanded size | 2 MB | `max_expansion_size` |

Exceeding one stops expansion and leaves the remaining markup as text, rather than
failing the compile. Raise them with `Options` if you are compiling pages that
legitimately go deeper.

## Parser functions do not need a resolver

`{{#if:…}}`, `{{#expr:…}}`, `{{#switch:…}}` and the rest are built in. Only a plain
transclusion needs the resolver, because only a plain transclusion is a page lookup.

```rust
assert_eq!(
    compile("{{#if:1|yes|no}}", &Options::default()).trim(),
    "<p>yes</p>"
);
```

## Next

- [Lua modules](lua-modules.md) for `{{#invoke:…}}`, which resolves through a separate
  provider.
- [Embedding](embedding.md) for caching a resolver behind a real store.

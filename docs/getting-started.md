# Getting started

This walks through a first compile, the command line tool, and the settings worth
knowing about on day one. It assumes nothing about wikitext beyond having seen a
wiki page.

## Install

As a library:

```sh
cargo add wikitext
```

As a command line tool, from a clone of the repository:

```sh
cargo install --path crates/wikitext-cli
```

## The first compile

```rust
use wikitext::{compile, Options};

let html = compile("Hello '''world'''", &Options::default());
assert_eq!(html, "<p>Hello <b>world</b></p>\n");
```

`compile` returns an HTML fragment. It is not a whole document, and it is not
wrapped in anything; see [Output shape](#output-shape) below.

Wikitext's markup maps onto HTML the way MediaWiki maps it:

| Wikitext | HTML | Note |
|---|---|---|
| `''italic''` | `<i>italic</i>` | two apostrophes, not a quotation mark |
| `'''bold'''` | `<b>bold</b>` | three |
| `'''''both'''''` | `<b><i>both</i></b>` | five |
| `== Heading ==` | `<h2 id="Heading">Heading</h2>` | two equals minimum |
| `* item` | `<ul><li>item</li></ul>` | a leading `*` is a list item |
| `[[Page]]` | `<a href="/wiki/Page">Page</a>` | an internal link |
| `[https://example.com label]` | `<a href="https://example.com">label</a>` | an external one |
| `{{#expr:6*7}}` | `<p>42</p>` | a parser function |

Inline markup lands inside a paragraph, so a lone `''italic''` compiles to
`<p><i>italic</i></p>`. Only block level constructs, such as a heading or a list,
stand on their own.

## The command line

```sh
echo "'''bold'''" | wikitext
# <p><b>bold</b></p>

wikitext page.wikitext > page.html
```

Reading from a file and from standard input behave the same way. Several output
shapes are available:

```sh
wikitext page.wikitext                     # a fragment, the default
wikitext --wrap page.wikitext              # wrapped in <div class="mw-parser-output">
wikitext --standalone page.wikitext        # a complete HTML document
```

`--diagnostics` reports the problems the parser recovered from. Problems are
reported rather than raised, so a page with a broken table still compiles and the
broken part stays as text.

## Output shape

The output is an HTML5 fragment shaped like MediaWiki's own: block elements are
separated by newlines, and a table row's `td` and `th` stay on the row's line.

Two details that surprise people:

- **A heading cannot be an `h1`.** MediaWiki reserves `h1` for the page title, so
  `== Heading ==`, which is a level 2 heading, becomes `<h2>`. Writing `= Heading =`
  is not a heading at all and stays literal text.
- **Attributes are generated, not copied.** `class` and `id` values follow
  MediaWiki's conventions (`mw-parser-output`, `mw-wikilink`, and so on) so that
  existing stylesheets have a chance of applying.

Nothing is sanitised. If a page writes `<script>`, the output contains it, exactly
as MediaWiki would emit it. See [SECURITY.md](../SECURITY.md).

## Settings

`Options` carries everything configurable, and every field has a default, so
`Options::default()` compiles an untitled English page.

```rust
use wikitext::{compile, Options, OutputMode};

let options = Options {
    title: Some("Ada Lovelace".to_string()),
    language: "en".to_string(),
    output_mode: OutputMode::Document,
    pretty: true,
    ..Options::default()
};
let html = compile("{{PAGENAME}}", &options);
assert!(html.contains("Ada Lovelace"));
```

The fields worth knowing on day one:

| Field | What it does |
|---|---|
| `title` | The page title. Magic words such as `{{PAGENAME}}` and `{{FULLPAGENAME}}` read it |
| `language` | Content language, used by language-aware magic words |
| `output_mode` | `Fragment`, `ParserOutput` or `Document` |
| `pretty` | Newlines between block elements. Turn it off to compare outputs byte for byte |
| `article_path` | Prefix for article links. `/wiki/` by default |
| `server_url`, `image_base_url` | Bases for absolute external link and image URLs |
| `show_categories`, `show_images` | Whether category links and images render at all |
| `diagnostics` | Whether problems are collected |
| `heading_anchors`, `edit_section_links` | MediaWiki-compatible extras in the output |
| `keep_comments` | Keep HTML comments instead of dropping them |
| `resolver` | The template store. See [Templates](templates.md) |
| `module_provider` | The Lua module store. See [Lua modules](lua-modules.md) |

A few more that come up less often: `content_language` and `namespace`, which answer
language-aware and namespace-aware magic words, `in_transclusion`, which says whether
the page being compiled is itself being transcluded, `max_heading_level` to cap the
headings a page may use, and `namespaces`, `media_namespaces`,
`category_namespace_aliases` and `file_namespace_aliases` for a wiki whose namespace
names are localised or aliased.

## Templates and modules

Out of the box there is no template store, so a transclusion renders the way
MediaWiki renders a missing template: as a red link to the template's page, with
the name capitalised the way a page title would be.

```rust
let html = compile("{{greet}}", &Options::default());
assert_eq!(html, "<p><a href=\"/wiki/Greet\" class=\"mw-wikilink\">Greet</a></p>\n");
```

Supply one and it expands:

```rust
use std::sync::Arc;
use wikitext::preprocess::MemoryTemplates;
use wikitext::{compile, Options};

let templates = MemoryTemplates::new();
templates.insert("greet", "Hello, {{{1|world}}}!");
let options = Options::default().with_resolver(Arc::new(templates));
assert_eq!(compile("{{greet|Rust}}", &options).trim(), "<p>Hello, Rust!</p>");
```

`{{#invoke:…}}` additionally needs the `lua` feature:

```sh
cargo add wikitext --features lua
```

Continue with [Templates](templates.md) and [Lua modules](lua-modules.md).

## Magic words that do not move

Date and time magic words return the Unix epoch, not the current time. The core
crate deliberately takes no clock dependency, so it stays deterministic and
portable.

```rust
assert!(compile("{{CURRENTYEAR}}", &Options::default()).contains("1970"));
```

An application that wants real dates can compute them and substitute before
compiling, or implement `TemplateResolver` and answer those names itself.

## Two step parsing

When the HTML is not enough, keep the tree:

```rust
use wikitext::{parse, serialize, Options};

let document = parse("== One ==\n\nText.", &Options::default());
assert_eq!(document.headings.len(), 1);
assert_eq!(document.headings[0].title, "One");
for diagnostic in &document.diagnostics {
    eprintln!("{:?} {}", diagnostic.severity, diagnostic.message);
}
```

`Document` also exposes the categories the page was filed under, the references it
collected, the external link count, and its plain text.

## Where to go next

- [Templates](templates.md) for a real template store.
- [Lua modules](lua-modules.md) for `#invoke`.
- [Embedding](embedding.md) to call this from an application or another language.
- [Format conversion](../README.md#format-conversion) for Markdown and HTML input.
- [AGENTS.md](../AGENTS.md) for why the parser behaves the way it does.

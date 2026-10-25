# WikitextSyntaxCompiler

A from-scratch [Wikitext](https://www.mediawiki.org/wiki/Wikitext) → HTML compiler in Rust.
No existing wikitext parser is used, and no GPL code is linked.

- **License:** BSD-2-Clause. See [LICENSE](LICENSE).
- **Dependencies:** the core crate has **none** by default; the `lua` feature pulls in the in-repo interpreter
- **No `unsafe`:** `#![forbid(unsafe_code)]` in every crate
- **MSRV:** Rust 1.75 (`wikitext-node` needs 1.77, since `napi-build` emits `cargo::` instructions)

```rust
use wikitext::{compile, Options};

let html = compile("Hello '''world'''", &Options::default());
assert_eq!(html, "<p>Hello <b>world</b></p>\n");
```

## Contents

- [Capabilities](#capabilities)
- [Install](#install)
- [Usage](#usage)
  - [Library](#library)
  - [Templates](#templates)
  - [Lua modules](#lua-modules)
  - [Command line](#command-line)
- [Syntax support](#syntax-support)
- [Format conversion](#format-conversion)
- [Bindings for other languages](#bindings-for-other-languages)
- [Robustness](#robustness)
- [Architecture](#architecture)
- [Guides](#guides)
- [Development](#development)
- [Compatibility](#compatibility)
- [Contributing](#contributing)
- [Security](#security)
- [License](#license)

## Capabilities

| | |
|---|---|
| **Complete wikitext** | Block and inline syntax, tables, extension tags, parser functions, magic words, full expression evaluation |
| **Templates** | Callback-driven, so a host can resolve pages however it likes; an in-memory store is included |
| **Lua modules** | `{{#invoke:…}}` runs on a Lua 5.1 interpreter written in this repository, with an `mw.*` surface |
| **Format conversion** | Wikitext ⇄ HTML ⇄ Markdown through one shared markup tree |
| **Fault tolerant** | Malformed input degrades to text and becomes a diagnostic; never a hard failure |
| **Embeddable** | C ABI, Go, Node.js and WebAssembly bindings; no Rust required on the calling side |

## Install

As a library:

```sh
cargo add wikitext                    # core compiler, zero dependencies
cargo add wikitext --features lua     # plus {{#invoke:…}}
```

As a command line tool:

```sh
cargo install --path crates/wikitext-cli
```

From a clone:

```sh
git clone https://github.com/miller-owo/WikitextSyntaxCompiler.git
cd WikitextSyntaxCompiler
cargo build --release
```

## Usage

### Library

Compile in one step, or keep the document tree and its diagnostics:

```rust
use wikitext::{compile, parse, serialize, Options};

let html = compile("Hello '''world'''", &Options::default());
assert_eq!(html, "<p>Hello <b>world</b></p>\n");

let document = parse("== Heading ==\n\n''italic''", &Options::default());
assert_eq!(serialize(&document).lines().count(), 2);
for heading in &document.headings {
    println!("h{} {}", heading.level, heading.title);
}
```

`Options` carries the page title, language, output mode, link and image bases, and
the template and module providers. Everything has a default, so `Options::default()`
compiles an untitled English page.

### Templates

Template expansion is driven by the `TemplateResolver` trait. With no resolver, a
transclusion renders the way MediaWiki renders a missing template, as a red link.

`MemoryTemplates` is a ready-made resolver for an in-memory map:

```rust
use std::sync::Arc;
use wikitext::preprocess::MemoryTemplates;
use wikitext::{compile, Options};

let templates = MemoryTemplates::new();
templates.insert("greet", "Hello, {{{1|world}}}!");
let options = Options::default().with_resolver(Arc::new(templates));

assert_eq!(compile("{{greet|Rust}}", &options).trim(), "<p>Hello, Rust!</p>");
```

For a real wiki, implement `TemplateResolver` to fetch page text from your store.
The trait is `Send + Sync`, so a resolver can serve concurrent compilations.

### Lua modules

With the `lua` feature, `{{#invoke:Module|function}}` runs on the interpreter in
[`crates/lua`](crates/lua). It is a complete Lua 5.1 implementation, including
metatables, closures, varargs, multiple returns, and the full pattern matcher.

```rust
use std::rc::Rc;
use wikitext::lua_modules::LuaModules;
use wikitext::{compile, Options};

let modules = LuaModules::new();
modules.insert("Roman", r#"
    local p = {}
    function p.convert(frame) return "value: " .. frame.args[1] end
    return p
"#);
let options = Options::default().with_module_provider(Rc::new(modules));

assert!(compile("{{#invoke:Roman|convert|42}}", &options).contains("value: 42"));
```

The `mw.*` surface covers `mw.text`, `mw.ustring`, `mw.title`, `mw.uri`, `mw.html`,
`mw.language` and `mw.dumpObject`. Execution is bounded by call depth, loop
iteration and instruction budgets, so a runaway module cannot hang the process.

### Command line

```console
$ echo "'''bold''' and {{#expr:6*7}}" | wikitext
<p><b>bold</b> and 42</p>

$ wikitext page.wikitext > out.html
$ wikitext --wrap --title "Some Page" page.wikitext
$ wikitext --standalone page.wikitext                  # full HTML document
$ wikitext --templates ./templates page.wikitext       # load *.wikitext as templates
$ wikitext --modules ./modules page.wikitext           # load *.lua, enables {{#invoke:…}}
$ wikitext --diagnostics page.wikitext                 # report recovered problems
$ wikitext --language de page.wikitext
```

| Option | Meaning |
|---|---|
| `--from <FORMAT>` | Read the input as `html`, `markdown` or `wikitext` (default `wikitext`) |
| `--to <FORMAT>` | Write `wikitext`, `markdown` or `html` (default `html`) |
| `-T`, `--title <TITLE>` | Page title, for `{{PAGENAME}}` and friends |
| `--wrap` | Wrap the output in `<div class="mw-parser-output">` |
| `--standalone` | Emit a complete HTML document |
| `--fragment-div` | The same output as `--wrap` |
| `-t`, `--templates <DIR>` | Every `*.wikitext`, `*.wiki`, `*.txt` or `*.mediawiki` file below the directory, named by its path |
| `-m`, `--modules <DIR>` | Every `*.lua` file below the directory, named by its path |
| `--diagnostics` | Print recovered problems to stderr |
| `--compact` | Disable pretty printing |
| `--language <LANGUAGE>` | Content language (default `en`) |

## Syntax support

### Core

- Paragraphs, soft line breaks, blank-line separation
- Headings `== … ==`, levels 1 to 6, with duplicate anchor ids disambiguated
- Lists with `*`, `#`, `;` and `:`, arbitrarily nested and mixed, including
  `; term : definition` and leading-space items
- Definition lists, horizontal rules `----`, preformatted blocks
- Tables `{| … |}`: attributes, `|+` captions, `|-` rows, `!`/`|` cells, `!!`/`||`
  separators, per-cell attributes
- Quote state machine for `''`, `'''` and `'''''`, including uneven pairings such
  as `''a'''b''`, and without misfiring on `it's`
- HTML tags, paired, self-closing and void, with quoted attributes and values
  containing `>`. A block-level tag at the start of a line is written through as
  raw HTML, so `<div style="…">` on one line and its `</div>` many lines later
  produce the wrapper the source describes instead of escaped text
- Character entities: named table, `&#…;`, `&#x…;`, Windows-1252 C1 remapping
- `<nowiki>`, `<pre>`, HTML comments
- Links: `[[Page]]`, `[[Page|label]]`, pipe trick `[[Foo (bar)|]]`,
  `[[Category:…]]` collection, `[[File:…]]` with captions, sizes, `alt=`, `class=`
  and `link=` (with no value, the image is not wrapped in an anchor), and
  `[url label]`

### Parser functions and magic words

`#if` `#ifeq` `#ifexist` `#iferror` `#ifexpr` `#switch` `#expr` (a complete
expression evaluator) `#time` `#timel` `#titleparts` `#rel2abs` `#tag`, and the
string functions `#len` `#pos` `#rpos` `#sub` `#replace` `#explode` `#urldecode`
`#urlencode` `#anchorencode` `#padleft` `#padright` `#formatnum` `#lc` `#uc`
`#lcfirst` `#ucfirst` `#format`.

Magic words: `PAGENAME` `FULLPAGENAME` `BASEPAGENAME` `SUBPAGENAME`
`ROOTPAGENAME` `NAMESPACE` `NAMESPACENUMBER` `TALKSPACE` `SERVER` `SITENAME`
`CURRENT*` `LOCAL*` `!` `=`, plus `{{DEFAULTSORT:…}}` and `{{DISPLAYTITLE:…}}`.

### Extension tags

- `<ref>` collects footnotes with `↑` backlinks; a repeated name reuses one
  footnote and increments its backlink, and `group` builds a separate list.
  Footnote bodies get full inline parsing, so `<ref>''Source'', p. 3</ref>` is
  italic.
- `<references/>` emits an `<ol class="references">` with `id`,
  `mw-cite-backlink` and `reference-text` per item, wrapped in `mw-references-wrap`
  when several groups exist.
- `<poem>` keeps line breaks and adds `<br>` per line.
- `<gallery>` turns each `File:…|caption` line into a `<ul class="gallery">` item.
- `<syntaxhighlight lang="…">` and `<source>` emit a `<pre>` carrying
  `mw-highlight-lang-…`.
- `<math>`, `<chem>` and `<ce>` are wrapped in a `data-mw-ext` marker for a
  front-end renderer to pick up.
- `<noinclude>` and `<includeonly>` follow the direction the page is read in: a
  page shows its `<noinclude>` text and hides `<includeonly>`, while a transcluded
  template body keeps `<includeonly>` and drops `<noinclude>`. The markers are
  removed before expansion, so they may span lines. `<templatedata>`, `<indicator>`
  and `<categorytree>` render empty.

### Not implemented

- `mw.loadData`, `mw.message`, `mw.site`: no data source to draw on
- `mw.title` objects compare as plain tables; only `tostring`, `..` and field
  access are guaranteed
- Parsoid-compatible `data-mw` / `data-parsoid` attributes
- TeX rendering for `<math>`, only the front-end hook

## Format conversion

[`crates/wikitext-convert`](crates/wikitext-convert) converts between the three
formats. Every direction goes through **one shared markup tree**, so a construct
is mapped once rather than once per format pair. HTML is parsed with `html5ever`,
Markdown with an in-repo CommonMark subset, and wikitext with this project's own
parser.

```rust
use wikitext_convert::{markdown_to_wikitext, wikitext_to_markdown};

assert_eq!(
    wikitext_to_markdown("== Head ==\n\nSome '''bold''' text."),
    "## Head\n\nSome **bold** text."
);
assert_eq!(
    markdown_to_wikitext("# Head\n\nSome **bold** text."),
    "== Head ==\n\nSome '''bold''' text."
);
```

Mapped constructs: headings, paragraphs, emphasis (including nesting), inline code
and fenced code blocks (with language → `<syntaxhighlight>`), lists with arbitrary
nesting and mixing, block quotes, thematic breaks, links, images, tables
(→ `wikitable`), and block and inline HTML tags.

Deliberate decisions:

- **Emit wikitext a person would write.** `'''` rather than `<b>`, `*` rather than
  `<li>`. The goal is semantic equivalence once compiled back to HTML.
- **Markdown h1 becomes `==`.** MediaWiki reserves h1 for the page title and
  starts body headings at h2; emitting `=` would not compile back to a heading.
- **List nesting uses a marker path.** Wikitext nesting is positional: the number
  of markers is the depth, and ancestor list kinds come first. An ordered item
  under `* a` is `*# b`, not `#* b`.
- **Unmappable HTML is preserved literally.** A `<span>` with classes has no clean
  wikitext counterpart, so it is kept as a literal tag rather than dropped.

**Wikitext → Markdown** maps everything Markdown can express natively, and falls
back on Markdown's embedded HTML for the rest, so nothing is lost: definition
lists stay `<dl>`, `<math>` stays `<math>`, and `<ref>` becomes a footnote marker
plus a reference list.

**Known limits:** HTML attributes other than `href`, `src`, `alt` and `class` are
not preserved; Markdown footnote syntax `[^1]`, definition-list syntax and inline
HTML tables are not parsed (they are read as embedded HTML); `rowspan` and
`colspan` on HTML tables are not mapped.

## Bindings for other languages

All three formats are available from every binding.
[`crates/wikitext-ffi`](crates/wikitext-ffi) is the C ABI foundation; its
[header](crates/wikitext-ffi/include/wikitext.h) is the authoritative contract,
with the rules a signature cannot show in
[docs/ffi.md](docs/ffi.md).

| Language | Entry point | Guide |
|---|---|---|
| C / C++ | `wikitext_convert`, `wikitext_compile` | [docs/c.md](docs/c.md) |
| Go | `wikitext.Convert`, `MarkdownToWikitext`, … | [docs/go.md](docs/go.md) |
| Node.js | `convert`, `compile`, `describe` | [docs/node.md](docs/node.md) |
| WebAssembly | `wikitextConvert`, `wikitextDescribe` | [`crates/wikitext-wasm`](crates/wikitext-wasm/README.md) |
| Anything else | the C ABI directly, for C# P/Invoke, Java FFM, Ruby fiddle, PHP FFI | [docs/ffi.md](docs/ffi.md) |

Format codes are the same in every language: `0` HTML, `1` Markdown, `2` Wikitext.
Only **Wikitext → HTML** consults the templates and modules on the options; the
other directions have nothing to expand.

```js
const wikitext = require("./wikitext_node.node");

const options = new wikitext.WikitextOptions();
options.title("Ada Lovelace");
options.addTemplate("greet", "Hello, {{{1|world}}}!");

wikitext.compile("{{greet|Rust}}", options);   // '<p>Hello, Rust!</p>\n'
wikitext.convert("# Title", 1, 2);             // '== Title =='
wikitext.describe("== One ==\n\n[[Category:Demo]]").categories;
```

## Robustness

A hostile or merely broken page will not hang the compiler or crash it.
Adversarial inputs are covered by
[`crates/wikitext/examples/fuzz.rs`](crates/wikitext/examples/fuzz.rs), and
complexity is confirmed by doubling input sizes in
[`scaling.rs`](crates/wikitext/examples/scaling.rs).

Each of these was a real defect, fixed and now held by a regression test:

| Input | Before | After |
|---|---|---|
| `<div>` × 128000 (640 KB) | 7.4 s | 89 ms |
| `{{` × 4000 (8 KB) | 8.5 s | 36 µs |
| `[[a\|` × 16000 (128 KB) | 1.01 s | 443 µs |
| `<div` with no `>` × 32000 | 3.15 s | 76 ms |
| `[[…]]` nested 800 deep (1.6 KB) | process abort (stack overflow) | fine; beyond the ceiling the inner text is kept literal |
| `<div>…</div>` nested 6400 deep | process abort (stack overflow) | fine; 51200 deep still works |

Nesting is capped at 40 levels, the same order as MediaWiki. Deeper input keeps
its content as literal text rather than being dropped.

For scale: an empty page compiles in about 1 µs, a 19 KB article in about 1.35 ms
(13.5 MB/s), and a 150 KB page in about 11.8 ms. Parse is roughly 94% of compile
time, serialization 4%.

## Architecture

```
&str ──► preprocess ──► lex ──► parse ──► serialize ──► HTML
         comment strip   lines    block state   HTML5
         nowiki protect  + inline machine
         template expand  lexing  inline stack
         parser funcs             quote machine
```

| Crate or directory | Purpose |
|---|---|
| [`crates/wikitext`](crates/wikitext) | The compiler: preprocessor, line scanner, block and inline parser, DOM, HTML serializer |
| [`crates/lua`](crates/lua) | A Lua 5.1 interpreter: lexer, parser, evaluator, standard library, pattern matcher |
| [`crates/wikitext-cli`](crates/wikitext-cli) | The `wikitext` command line compiler |
| [`crates/wikitext-convert`](crates/wikitext-convert) | Wikitext ⇄ HTML ⇄ Markdown conversion |
| [`crates/wikitext-ffi`](crates/wikitext-ffi) | C ABI for Go, Node.js, C#, Java, Ruby, PHP and anything else |
| [`crates/wikitext-node`](crates/wikitext-node) | Node.js native addon (napi-rs) |
| [`crates/wikitext-wasm`](crates/wikitext-wasm) | WebAssembly bindings, with a [JavaScript wrapper](crates/wikitext-wasm/js/index.js) |
| [`bindings/go`](bindings/go) | Go binding (cgo, zero third-party dependencies) |
| [`bindings/node`](bindings/node) | Node.js tests and notes |

Design decisions and their reasoning are recorded in
[AGENTS.md](AGENTS.md), which also holds the measured latency and robustness
data.

## Guides

| Guide | For |
|---|---|
| [Getting started](docs/getting-started.md) | First compile, the CLI, options, and what to expect |
| [Templates](docs/templates.md) | Supplying templates from your own store |
| [Lua modules](docs/lua-modules.md) | Running `{{#invoke:…}}` and writing modules |
| [Embedding](docs/embedding.md) | Calling the compiler from an application |

The [documentation index](docs/README.md) lists everything, including the per
language pages.

## Development

```sh
cargo test --workspace --all-features    # 314 tests
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
cargo run -p wikitext-cli -- --help
cargo doc --open -p wikitext
```

Measuring and probing:

```sh
cargo run -p wikitext --release --example latency    # absolute timings
cargo run -p wikitext --release --example stages     # which pipeline stage dominates
cargo run -p wikitext --release --example scaling    # linear or quadratic?
cargo run -p wikitext --release --example fuzz       # adversarial inputs
cargo run -p wikitext --release --example nestprobe -- target/release/examples/onepage
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the binding suites, what a change is
expected to come with, and the measurement discipline this project follows.

## Compatibility

This is an **independent implementation**. It was written from public syntax
documentation and observable behaviour only; no GPL code was copied or linked.
It is not derived from Wikimedia's Parsoid or from MediaWiki core's
`Parser`/`Preprocessor`.

Output targets an HTML5 fragment shaped like MediaWiki's server-side output:
block elements newline-separated, `td` and `th` kept on their row's line. It
is not guaranteed to match byte for byte.

Everything in the dependency tree is permissively licensed. Most of it is
`MIT OR Apache-2.0`: `html5ever`, `markup5ever`, `tendril`, `clap`, `wasm-bindgen`
and the crates behind them. `napi`, `napi-derive` and `napi-build` are MIT,
`libloading` (which `napi-sys` loads native libraries through) is ISC, `memchr` is
`Unlicense OR MIT`, and `unicode-ident` is `(MIT OR Apache-2.0) AND Unicode-3.0`.
No copyleft anywhere in the tree.

[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) repeats that list with the
copyright line and the full license text for each crate, which is what a binary
distribution has to reproduce.

## Contributing

Contributions are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) covers the setup, the
four checks that must pass, the testing expectations, and the conventions this
repository holds to. The short version:

- Read [AGENTS.md](AGENTS.md) before changing code. It records the decisions that are
  not obvious and why they were made, and reversing one unknowingly is the usual way
  to break something here.
- Comments are the exception, not the default. Write one only when the code cannot
  carry the information itself, and never to restate what the code does.
- No copyleft, at any depth of the dependency tree. Every crate is MIT, Apache-2.0,
  ISC, Unicode-3.0, Unlicense or BSD, and `THIRD-PARTY-NOTICES.md` is regenerated
  when one is added.
- A robustness fix needs a fuzz case, and its complexity claim confirmed by doubling
  input sizes rather than by a single timing.

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

## Security

The input to this compiler is untrusted by assumption, so denial of service counts as
a security bug here rather than a performance one. The properties that matter are that
input cannot lead to code execution, cannot cause work superlinear in input size, and
cannot abort the host process.

Resource limits are already in place for Lua execution, template expansion, nesting
depth and tag scanning, and every C ABI entry point is guarded against unwinding. See
[SECURITY.md](SECURITY.md) for the full threat model, what is in and out of scope, and
hardening notes for embedders.

To report a vulnerability, use GitHub private vulnerability reporting rather than a
public issue.

## License

BSD-2-Clause. See [LICENSE](LICENSE).

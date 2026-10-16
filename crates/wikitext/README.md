# wikitext

A from-scratch wikitext to HTML compiler.

The crate turns MediaWiki page source into an HTML5 fragment. Block and inline
syntax, tables, extension tags, parser functions, magic words and links are handled
here, and the default build has no dependencies. Malformed input is not a hard error:
the syntax degrades to literal text and a `Diagnostic` is recorded, so it compiles.

## Install

```sh
cargo add wikitext                    # the compiler, zero dependencies
cargo add wikitext --features lua     # plus {{#invoke:…}}
```

The `lua` feature adds `lua_modules::LuaModules`, which runs Scribunto modules on the
Lua 5.1 interpreter in [`crates/lua`](../lua).

## Use

```rust
use wikitext::{compile, parse, serialize, Options};

let html = compile("Hello '''world'''", &Options::default());
assert_eq!(html, "<p>Hello <b>world</b></p>\n");

// Or keep the tree and give it back to the serializer.
let document = parse("== Heading ==\n\n''italic''", &Options::default());
assert_eq!(document.headings[0].title, "Heading");
assert_eq!(serialize(&document), "<h2 id=\"Heading\">Heading</h2>\n<p><i>italic</i></p>\n");
```

`compile_with_diagnostics` returns the `Document` as well, which is where the
headings, categories, references and recovered problems live.

Templates come from the `TemplateResolver` trait; `MemoryTemplates` is a ready-made store:

```rust
use std::sync::Arc;
use wikitext::preprocess::MemoryTemplates;
use wikitext::{compile, Options};

let templates = MemoryTemplates::new();
templates.insert("greet", "Hello, {{{1|world}}}!");
let options = Options::default().with_resolver(Arc::new(templates));

assert_eq!(compile("{{greet|Rust}}", &options), "<p>Hello, Rust!</p>\n");
```

With the `lua` feature, `LuaModules` plays the same role for `{{#invoke:…}}`:

```rust
use std::rc::Rc;
use wikitext::lua_modules::LuaModules;
use wikitext::{compile, Options};

let modules = LuaModules::new();
let source = "local p = {}\nfunction p.convert(frame) return 'MCMXCIV' end\nreturn p\n";
modules.insert("Roman", source);
let options = Options::default().with_module_provider(Rc::new(modules));
assert_eq!(compile("{{#invoke:Roman|convert|1994}}", &options), "<p>MCMXCIV</p>\n");
```

## API summary

`compile` and `compile_with_diagnostics` run all three stages;
`preprocess::Preprocessor::new(&options).run(source)` expands templates without
parsing, and `decode_entities` decodes character references.

`Options` carries the page title, language, output mode, article and image bases,
expansion limits, and the resolver and module provider. Every field has a default, so
`Options::default()` compiles an untitled English page. `OutputMode` is `Fragment`
(the default), `ParserOutput` (the body wrapped in `<div class="mw-parser-output">`)
or `Document` (a complete HTML5 document).

`Document` holds `children`, `diagnostics`, `headings`, `categories`, `references`,
`redirect`, `display_title`, `default_sort` and `external_link_count`. The tree is
`Node::Element` / `Text` / `Comment` / `Raw`, with `Element` carrying a tag,
attributes, children and an `ElementKind`. A `Diagnostic` carries a `Severity`, a
`DiagnosticCode`, a message and a `Position`. `SerializeOptions` sets `pretty`, the
output mode and `escape_nbsp`. Modules: `dom`, `entity`, `error`, `extensions`, `lex`,
`lua_modules` (feature `lua`), `options`, `parse`, `preprocess`, `references`, `scan`,
`serialize`.

## Related

[Root README](../../README.md), [docs](../../docs/README.md), [AGENTS.md](../../AGENTS.md).

BSD-2-Clause. See [LICENSE](../../LICENSE).

# wikitext-node

Node.js bindings for the wikitext compiler.

A native addon built with [napi-rs](https://napi.rs), exposing the compiler and the
format converter to JavaScript. Everything is synchronous: compiling a page is a
bounded amount of work, so there is nothing to await, and a caller that needs
concurrency should run it on a worker thread. The crate sets `publish = false`: it is
built from this repository, not published to npm.

## Build

```sh
cargo build -p wikitext-node
```

Cargo produces `target/debug/wikitext_node.dll` (`libwikitext_node.so`,
`libwikitext_node.dylib`), and Node loads addons only with a `.node` extension, so the
built library is copied across under that name. `bindings/node/test.mjs` does exactly
that and then runs the test suite:

```sh
node bindings/node/test.mjs
```

This crate declares `rust-version = "1.77"` while the workspace declares 1.75, because
`napi-build` emits `cargo::` build script instructions, which need 1.77. That is the
only place the crate cannot follow the workspace floor.

## Use

```js
const wikitext = require("./wikitext_node.node");

wikitext.wikitextToHtml("Hello '''world'''");
// '<p>Hello <b>world</b></p>\n'

const options = new wikitext.WikitextOptions();
options.title("Ada Lovelace");
options.addTemplate("greet", "Hello, {{{1|world}}}!");
options.addModule("Roman", "local p = {}\nfunction p.convert(f) return 'MCMXCIV' end\nreturn p\n");

wikitext.compile("{{greet|Rust}}", options);            // '<p>Hello, Rust!</p>\n'
wikitext.compile("{{#invoke:Roman|convert|1994}}", options); // '<p>MCMXCIV</p>\n'
```

Convert between formats with the codes that `formats()` reports:

```js
const { html, markdown, wikitext: wt } = wikitext.formats();   // { html: 0, markdown: 1, wikitext: 2 }

wikitext.convert("# Title\n\nSome **bold**.", markdown, wt);   // "== Title ==\n\nSome '''bold'''."
wikitext.convert("== Title ==\n\nSome '''bold'''.", wt, markdown); // "## Title\n\nSome **bold**."
```

`describe` returns a parsed page, which is what a table of contents, a category bar or
an error panel needs:

```js
const page = wikitext.describe("== One ==\n\nText.\n\n[[Category:Demo|Sort]]");

page.headings;     // [{ level: 2, id: 'One', title: 'One' }]
page.categories;   // [{ name: 'Demo', sortKey: 'Sort' }]
page.diagnostics;  // [{ severity, code, message, line, column }]
page.plainText;
page.references;
page.html;
```

## API

`wikitextToHtml(source)`, `wikitextToDocument(source, title)`,
`compile(source, options)`, `convert(source, from, to)`, `describe(source, options?)`,
`expand(source, options?)`, `decodeEntities(source)`, `formats()`, `version()`.

`new WikitextOptions()` has `title`, `language`, `outputMode` (0 fragment, 1 parser
output, 2 full document), `pretty`, `addTemplate`, `addModule`, and the getters
`templateCount` and `moduleCount`. A handle can be reused, and changing it affects the
next call. An unknown format code throws rather than returning an empty string.

## Related

- [docs/node.md](../../docs/node.md): the full guide to this binding
- [bindings/node/test.mjs](../../bindings/node/test.mjs): the build step and the tests
- [Root README](../../README.md): install, capabilities, syntax support

BSD-2-Clause. See [LICENSE](../../LICENSE).

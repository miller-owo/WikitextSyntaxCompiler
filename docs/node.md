# Node.js binding

A native addon built with [napi-rs](https://napi.rs) (MIT). Everything is
synchronous: compiling a page is a small, bounded amount of work.

```sh
cargo build -p wikitext-node            # produces target/debug/wikitext_node.dll
node bindings/node/test.mjs             # copies it to .node and runs the tests
```

Node loads addons with a `.node` extension, so the test runner copies the built
library across. For a package, do the same in your build step and ship
`wikitext_node.node` plus `index.js`.

## Use

```js
const wikitext = require("./wikitext_node.node");

wikitext.wikitextToHtml("Hello '''world'''");
// '<p>Hello <b>world</b></p>\n'

const options = new wikitext.WikitextOptions();
options.title("Ada Lovelace");
options.addTemplate("greet", "Hello, {{{1|world}}}!");
options.addModule("Roman", `
  local p = {}
  function p.convert(frame) return "MCMXCIV" end
  return p
`);

wikitext.compile("{{greet|Rust}}", options);
// '<p>Hello, Rust!</p>\n'

wikitext.compile("{{#invoke:Roman|convert|1994}}", options);
// '<p>MCMXCIV</p>\n'
```

Convert between formats with the codes from `formats()`:

```js
const { html, markdown, wikitext: wt } = wikitext.formats();

wikitext.convert("# Title\n\nSome **bold**.", markdown, wt);
// "== Title ==\n\nSome '''bold'''."

wikitext.convert("== Title ==\n\nSome '''bold'''.", wt, markdown);
// "## Title\n\nSome **bold**."

wikitext.convert("'''x'''", wt, html);
// '<p><b>x</b></p>\n'
```

`describe` returns a parsed page, which is what a table of contents, a category
bar or an error panel needs:

```js
const page = wikitext.describe("== One ==\n\nText.\n\n[[Category:Demo|Sort]]");
page.headings;     // [{ level: 2, id: 'One', title: 'One' }]
page.categories;   // [{ name: 'Demo', sortKey: 'Sort' }]
page.diagnostics;  // [{ severity: 'warning', code: 'unclosed-table', … }]
page.plainText;
page.references;   // how many citations the page holds
page.html;
```

Also available: `wikitextToDocument(source, title)`, `expand(source, options)`
for templates without parsing, `decodeEntities(source)`, and `version()`.

## Notes

- An unknown format code throws, rather than returning an empty string.
- An `options` handle can be reused; changing it affects the next call.
- Only wikitext to HTML consults the templates and modules on the handle; the
  other directions have nothing to expand.

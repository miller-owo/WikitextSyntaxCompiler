# wikitext-wasm

WebAssembly bindings for the wikitext compiler.

The crate exposes the compiler, the format converter and the page description to
JavaScript through `wasm-bindgen`, with a thin wrapper at
[`js/index.js`](js/index.js) so a caller does not have to load the module by hand or
remember the format codes. The `wasm` feature turns the bindings on; without it the
crate builds as a plain Rust library, where the same work is reachable from Rust
through the functions listed below. It sets `publish = false`: it is built from this
repository, not published to npm.

## Build

```sh
cargo build -p wikitext-wasm --release --target wasm32-unknown-unknown --features wasm
```

That produces `target/wasm32-unknown-unknown/release/wikitext_wasm.wasm`. The glue and
the JavaScript module come from `wasm-bindgen` run over that file, and the wrapper
imports `./wikitext_wasm.js`, which is the name wasm-bindgen derives from
`wikitext_wasm.wasm`. Install the CLI with `cargo install wasm-bindgen-cli` and keep
its version in step with the `wasm-bindgen` crate in `Cargo.lock`.

For a build that only needs the Rust side:

```sh
cargo test -p wikitext-wasm     # no wasm feature, no wasm-bindgen
```

## Use

```js
import { load, FORMAT, OUTPUT } from "./js/index.js";

const wikitext = await load();

wikitext.version();                        // "0.1.0"
wikitext.toHtml("Hello '''world'''");      // '<p>Hello <b>world</b></p>\n'
wikitext.toDocument("x", "Title");         // a complete HTML document
wikitext.convert("# T", FORMAT.MARKDOWN, FORMAT.WIKITEXT);  // "== T =="

const options = new wikitext.Options();
options.title("Ada Lovelace");
options.addTemplate("greet", "Hello, {{{1|world}}}!");
wikitext.compile("{{greet|Rust}}", options);   // '<p>Hello, Rust!</p>\n'

const page = wikitext.describe("== One ==\n\n[[Category:Demo|Sort]]");
page.headings;    // [{ level: "2", id: "One", title: "One" }]
page.categories;  // [{ name: "Demo", sortKey: "Sort" }]
page.references;  // a number
```

`describe` and `diagnostics` parse the JSON for you; `load(module)` accepts an
already loaded module instead of importing one, which is what a bundler or a test
harness wants. In this binding `level`, `line` and `column` are reported as strings;
the Node binding reports the same fields as numbers.

## API

The wrapper returns `version`, `Options`, `toHtml`, `toDocument`, `compile`,
`convert`, `describe`, `diagnostics` and `expand`. Underneath, the WebAssembly module
exports `wikitextVersion`, `wikitextToHtml`, `wikitextToDocument`, `wikitextCompile`,
`wikitextConvert`, `wikitextDescribe`, `wikitextDiagnostics` and `wikitextExpand`,
with `WikitextOptions` as the options class carrying `title`, `language`, `outputMode`,
`pretty`, `addTemplate`, `addModule`, `templateCount` and `moduleCount`.

From Rust, with or without the feature, the crate offers `compile_to_html`,
`compile_with_settings`, `convert`, `describe` and `diagnostics_json` over a
`Settings` value. `diagnostics_json` returns the same array that `describe` embeds.

Format codes are `0` HTML, `1` Markdown and `2` Wikitext, and output modes are `0`
fragment, `1` parser output and `2` full document, the same numbers every other
binding uses. Only wikitext to HTML consults the templates and modules.

## Related

- [js/index.js](js/index.js): the wrapper, and the authoritative list of names
- [docs](../../docs/README.md): the guides index, including the binding notes
- [crates/wikitext-ffi](../wikitext-ffi): the C ABI the other bindings build on

BSD-2-Clause. See [LICENSE](../../LICENSE).

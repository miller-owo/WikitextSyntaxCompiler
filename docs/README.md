# Documentation

Start with the root [README](../README.md) for what the project is and how to
install it. These pages go deeper.

## Guides

| Guide | For |
|---|---|
| [Getting started](getting-started.md) | First compile, the CLI, options, and what to expect |
| [Templates](templates.md) | Supplying templates from your own store, and how transclusion works |
| [Lua modules](lua-modules.md) | Running `{{#invoke:…}}`, and writing modules against the `mw.*` surface |
| [Embedding](embedding.md) | Calling the compiler from an application, and what to watch for |

## Per language

| Page | Language |
|---|---|
| [ffi.md](ffi.md) | C ABI, the foundation every other binding builds on |
| [c.md](c.md) | C and C++, including the smoke test |
| [go.md](go.md) | Go, through cgo |
| [node.md](node.md) | Node.js, through a native addon |

Browsers use the WebAssembly binding. Its build is covered in
[`crates/wikitext-wasm`](../crates/wikitext-wasm/README.md), and the JavaScript
wrapper is at
[`crates/wikitext-wasm/js/index.js`](../crates/wikitext-wasm/js/index.js).

## Design

[AGENTS.md](../AGENTS.md) records the decisions that are not obvious and why they
were made, including the measured latency and robustness data. Read it before
changing parser code.

## Publishing

[RELEASING.md](../RELEASING.md) is the checklist for cutting a release and pushing
to GitHub.

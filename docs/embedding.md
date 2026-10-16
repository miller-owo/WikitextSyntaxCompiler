# Embedding

This is for calling the compiler from an application, or from a language other than
Rust. It covers the things that only matter once the compiler is inside something
long-lived.

## The shape of a call

Every entry point is the same shape: give it source, give it options, get HTML back.

```rust
use wikitext::{compile, Options};

let html = compile(source, &options);
```

Nothing is retained, and nothing is cached between calls. There is no global state,
so threads can compile at the same time with nothing shared.

One detail matters when you spread work across threads: **`Options` is not `Send`**,
because the Lua module provider inside it holds an `Rc`. Build one per thread rather
than sharing one. `Document` is `Send + Sync`, so a parsed tree can move between
threads freely, and any `TemplateResolver` is `Send + Sync` by its trait bound.

## Options are cheap to build, not free

`Options::default()` allocates. For a server that compiles many pages, build the
common part once and clone it per call:

```rust
let base = Options { title: Some("Home".into()), ..Options::default() };
for page in pages {
    let options = Options { title: Some(page.title.clone()), ..base.clone() };
    let html = compile(&page.text, &options);
    println!("{} -> {} bytes", page.title, html.len());
}
```

`Options` is `Clone`, and the resolver behind it is an `Arc`, so cloning does not
duplicate a template store. Clone it per call rather than per page if you can.

## Templates and modules are yours to source

Neither is built in, and that is a deliberate boundary: a compiler has no business
knowing where your pages live. Implement `TemplateResolver` for templates and
`ModuleProvider` for Lua, and hand them over.

- [Templates](templates.md) covers the resolver.
- [Lua modules](lua-modules.md) covers the provider.

A resolver is called once per transclusion and runs on the compiling thread, so it
must not block for long and must not re-enter the same `Options`.

## Diagnostics instead of errors

`compile` never fails. A page with an unclosed table, an unclosed tag or a broken
comment compiles, and the damaged part stays as literal text. Problems are collected
as `Diagnostic` values when `options.diagnostics` is on:

```rust
use wikitext::{compile_with_diagnostics, Options};

let (html, document) = compile_with_diagnostics(source, &Options::default());
for diagnostic in &document.diagnostics {
    eprintln!(
        "{:?} {} at {}:{} {}",
        diagnostic.severity,
        diagnostic.code.as_str(),
        diagnostic.position.line,
        diagnostic.position.column,
        diagnostic.message
    );
}
```

This is the same stance MediaWiki takes, and it is why importing a wiki dump does not
stop at the first bad page. If you need strictness, treat any `Error` severity
diagnostic as a failure yourself.

## Handling untrusted input

**The input to this compiler is untrusted by assumption.** What that means in
practice, in order of importance:

1. **Bound the work.** The parser is linear in input size, and Lua execution, template
   expansion and nesting depth all have ceilings. Add a wall-clock timeout anyway; a
   caller-side timeout is cheap and catches the case neither of you thought of.
2. **Cap the input size** before compiling, if your users can submit large pages.
3. **Sanitise the output** if you serve it to other users. The compiler emits HTML5
   fragments and does not remove `<script>`, exactly as a wiki would not.

[SECURITY.md](../SECURITY.md) has the full threat model and the limits in force.

## Nesting and recursion

Inline nesting is capped at 40 levels, matching MediaWiki's order of magnitude.
Beyond that the inner content is kept as literal text rather than being dropped, so
nothing is lost, but the output is not what a page that deep intended. If you are
compiling generated markup that legitimately nests deeper than that, raise the
ceiling and check what it does to your stack.

Stack depth is the reason the ceiling exists: a link label is re-parsed, so nesting
recurses. Deep tags and deep trees are safe because `Drop` and the serializer are
iterative, but the inline parser is not.

## Calling from another language

The C ABI is the foundation. Everything else builds on it or mirrors it.

| Language | Page |
|---|---|
| C and C++ | [c.md](c.md) |
| Go | [go.md](go.md) |
| Node.js | [node.md](node.md) |
| Anything else | [ffi.md](ffi.md) |

Two invariants hold at that boundary, both with tests:

- **No call aborts the process.** Every exported function is wrapped so a panic
  becomes `WIKITEXT_ERROR_PANIC` with a readable message, because unwinding into a
  foreign caller is undefined behaviour.
- **Allocation and deallocation stay on one side.** A buffer the library allocated is
  freed by the library's own free function; your data is only borrowed for the
  duration of the call.

Handles are not thread safe in any binding. Give each thread or worker its own.

## Two step use

When the HTML is not enough, keep the tree and walk it:

```rust
use wikitext::{parse, Options};

let document = parse(source, &Options::default());
for heading in &document.headings {
    println!("h{} {}", heading.level, heading.title);
}
for category in &document.categories {
    println!("category {} sort {:?}", category.name, category.sort_key);
}
println!("{} references", document.references.len());
```

`Document::text_content` returns the page's plain text, useful for search indexing
without a separate HTML-to-text pass.

## Rendering a document more than once

`Document` is plain data, so it can be serialized as often as you like with different
settings:

```rust
use wikitext::{parse, serialize, Options, OutputMode};

let document = parse(source, &Options::default());
let fragment = serialize(&document);
```

Both `parse` and `serialize` are public, so a caller can also stop between them and
inspect or rewrite the tree before serializing.

## Performance expectations

An empty page compiles in about a microsecond, a 19 KB article in about 1.35 ms, and
a 150 KB page in about 11.8 ms. Parsing is roughly 94% of that, so there is little to
gain from optimizing serialization.

Absolute numbers vary by machine, and the same configuration has been measured to
vary by a factor of two on one machine. Measure with
`cargo run -p wikitext --release --example latency` rather than trusting a figure
from here. [AGENTS.md §9](../AGENTS.md) has the breakdown by pipeline stage.

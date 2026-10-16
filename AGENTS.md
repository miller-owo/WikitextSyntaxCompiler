# Architecture and Design Decisions

This records the decisions in this project that are **not obvious**, and why they
were made that way. Read it before changing code, especially the quote engine and
the list state machine.

## 1. Pipeline

```
&str
 │
 ├─ 1. strip_comments      strip comments (comments inside <nowiki>/<pre> stay)
 ├─ 2. Protect             replace nowiki/pre/math/syntaxhighlight bodies with sentinels
 ├─ 3. Expander            expand templates, parameters and parser functions
 ├─ 4. Protect::restore    sentinels back to their source text
 │
 ├─ 5. LineScanner         split into lines, carrying byte offsets
 ├─ 6. BlockParser          block state machine (paragraph/heading/list/table/pre/hr)
 ├─ 7. InlineBuilder        inline stack construction (quotes/tags/links)
 │
 └─ 8. serialize           HTML5 output
```

**Why preprocess as a string pass instead of expanding during parsing?**

MediaWiki's real flow is "expand templates first, then parse the result as
wikitext". Keeping the two apart means each side is testable on its own: the
expander's tests need no DOM, and the parser's tests need no template store. The
cost is provenance: the text no longer records which template produced it. A future
Parsoid-style `data-mw` would need `Protect`'s sentinel mechanism extended to carry
provenance markers.

## 2. Sentinel protection (`preprocess::protect`)

Template expansion is a string operation, so it cannot tell that the `{{x}}` inside
`<nowiki>{{x}}</nowiki>` is literal. Those regions are replaced with the private-use
characters `U+E000 <id> U+E001` and swapped back after expansion.

- Private-use characters need no escaping and cannot collide with expansion output
  by accident.
- The cost: source that genuinely contains `U+E000` decodes wrongly. That is a
  deliberate trade, and the source says so.
- Comments use the same mechanism (`Payload::Comment`), which keeps markup inside a
  comment invisible to the expander and turns "keep comments" into a configuration
  flag rather than a branch in the pipeline.

## 3. Quote engine (`InlineBuilder` in `parse.rs`)

This is the hardest part of the project to get right. MediaWiki's rule is not
"pairing" but "counting":

| Input | Output | Rule |
|---|---|---|
| `''a''` | `<i>a</i>` | 2 quotes is italic |
| `'''a'''` | `<b>a</b>` | 3 quotes is bold |
| `'''''a'''''` | `<b><i>a</i></b>` | 5 quotes is bold around italic |
| `''a'''b''` | `<i>a</i>'b<i></i>` | a longer run can close a shorter one |
| `it's` | literal | 2 quotes not at a word start is not markup |
| `''a` | literal | nothing closes it |

The implementation is an explicit stack plus the rule that an opening run cannot
close itself.

```
Block { element, opened_by, open_index, children }
```

- `open_index` is the **token index of the opening run**. A run at the same index
  never closes itself, and that one rule explains both `''a''` (two runs needed)
  and `''a` (literal).
- A run of 5 quotes is handled separately: opening or closing, it splits into 3 for
  bold and 2 for the italic inside it.
- Spans still on the stack at the end are **restored to source text**
  (`append_literalised`), so an unclosed `<b>` neither swallows what follows nor
  silently drops characters.

A lexical detail: `lex_inline` treats only runs of length 2, or length 3 and above
when odd, as markup. Even runs (such as `''''`) stay in the text, which is what
keeps `it's` and `rock 'n' roll` literal.

## 4. List state machine (`ListState` in `parse.rs`)

The difficulty is that wikitext allows arbitrary nesting, and whether a line is a
sibling or a child is decided entirely by its marker string.

The stack **alternates list frames and item frames**: marker `i` occupies slots
`2i` (list) and `2i + 1` (item). Each incoming line:

1. Truncate the stack to `2n`, where `n` is this line's marker count.
2. Compare marker by marker: reuse the slot when it already holds a list of the
   same kind, otherwise close and rebuild. A nested list needs its parent item
   first.
3. Leaf item: close the slot and everything deeper, then push the new item.
   **"Close first, then push" is what distinguishes a sibling from a child.**

Definition lists are the exception: `; term : definition` yields two items (`<dt>`
and `<dd>`) that are siblings inside one `<dl>`, so the `<dl>` frame sits between
them (`apply_definition_line`). `apply_nested` covers a marker that follows the
colon.

## 5. Tables

`LineKind` separates a line starting with `{|`, `|}`, `|-`, `|` or `!`. The block
parser collects the whole table's raw lines only at `LineKind::TableStart`,
tracking nesting depth, and `build_table` then walks them.

- Cell separators depend on the row kind: `!` rows use `!!`, `|` rows use `||`.
  `split_top_level` skips separators inside `[[…]]` and `{{…}}`.
- The split between a cell's attributes and its content is a `|` that is outside
  quotes and follows an `=` (`find_attribute_pipe`). That splits
  `| style="color:red" | cell` correctly while leaving `| [[a|b]]` alone.

## 6. Extension tags (`extensions.rs` / `references.rs`)

MediaWiki's `<ref>`, `<poem>` and `<gallery>` are not HTML. Each is a parser tag
rewritten by a PHP or Lua hook, and the `extensions` module is the Rust counterpart
of those hooks.

**Why scan as a whole rather than token by token.** An extension tag's content is
not necessarily wikitext: `<poem>` keeps line breaks, `<math>` keeps source, and
`<gallery>` is one image per line. The line scanner would break a multi-line tag
apart, and in `<poem>\none\ntwo\n</poem>` the `</poem>` lands on another line and
becomes literal. So `scan_ext_tag` takes the entire element out of the source in one
pass, handling same-name nesting and raw-text content models itself, and the lexer
emits it as a **single token**.

**Block versus inline.** A block tag at the start of a line (`<poem>`, `<gallery>`,
`<references/>`, `<syntaxhighlight>` and so on) is consumed whole by the block
parser through `skip_to_offset`. An inline `<ref>` stays in the inline parser, so
`text<ref>…</ref>more` is not split across lines.
`LineScanner::skip_to_offset` keeps the line number in step, so diagnostic
positions stay correct.

**Why `<ref>` gets its own module.** It is the only extension that needs state
across tags: a `<ref>` does not render in place, it registers a footnote and leaves
a backlink, and `<references/>` is where the list appears. That state lives on
`Document::references` (`references::References`). A repeated name reuses one
footnote and increments its backlink to `↑2`, and groups deduplicate separately per
`(group, name)`.

**Footnote bodies are parsed a second time.** A `<ref>`'s content is deliberately
**not** parsed where it is written, because an `''italic''` inside it would become a
node before it could be put in the list, by which point the context is gone. It is
stored as a raw node in `References`, and `parse_reference_text` runs inline parsing
over it when `<references/>` renders. This is one instance of lazy parsing, the same
idea as lazy template argument evaluation.

**Content that does not render still has to be consumed.**
`<noinclude>`/`<includeonly>` control transclusion only and are not shown when
reading, and `<templatedata>`, `<indicator>` and `<categorytree>` render empty. The
content of these tags **must be consumed**, or body text leaks into the page. The
`noinclude_is_dropped` test pins this down.

## 7. Lua interpreter (`crates/lua`)

`#invoke` runs Lua inside a page, so the repository carries a Lua 5.1 interpreter
written from scratch. It is **not** a port of PUC-Lua or LuaJIT: the lexer, parser,
evaluator, pattern matcher and standard library are all written against the
reference manual, which keeps the license clean.

**Why tree walking rather than bytecode.** The job is "run a module once". Compiling
to bytecode buys speed inside loops and costs a register allocator plus debug
mappings. Tree walking is ample at module scale, and every semantic detail
(upvalues, multiple returns, metamethod fallbacks) can be read straight from the
source.

**Environment model and reference cycles.** Closures capture by reference, so the
environment is a chain of `Rc<RefCell<Env>>`: an `Env` holds a
`HashMap<String, Value>` and its parent scope, and a closure remembers the `EnvRef`
it was defined in. Writing an upvalue goes through `Target::Local(name, env)` to
**the scope that holds the binding**, not to the global chain. That was a real bug
during development: the first implementation wrote to the wrong level and a closure
counter kept returning 0.

The cost is that a self-reference such as `local f; f = function() ... f ... end`
forms an `Rc` cycle that is never freed. That is a **deliberate trade**: one module
evaluation drops the whole heap at the end, so the cycle costs nothing observable,
while an arena plus ids would complicate every scope operation. The trade is
recorded above `EnvRef` in `value.rs`.

**Resource limits are not ordinary errors, and `pcall` does not catch them.** There
are three ceilings: call depth, loop iterations and instruction budget. Exceeding one
returns `LimitExceeded`, which `pcall` **deliberately does not catch**, because it
means the interpreter itself gave up and letting the module continue would hit it
again immediately.

**Lua pattern matching** is its own module because it is the easiest part of `string`
to get wrong: `%b` balanced matching, `%f` frontier assertions, position captures
`()`, back-references `%1`, greedy `*`/`+` and lazy `-`. On backtracking, **every
failed probe has to restore its captures and open-group stack**, or the successful
attempt carries the ranges of a failed branch. That was the second real bug.

**Where `mw.*` lives.** The interpreter itself knows nothing about `mw`.
`wikitext/src/lua_modules.rs` installs `mw` into a **fresh interpreter** before each
call. Rebuilding each time is intentional: one module must not see another module's
globals, nor leave state behind between calls.

## 8. Format conversion (`crates/wikitext-convert`)

Wikitext, HTML and Markdown, with three front ends, three writers and one shared
tree in the middle.

**Why a shared tree.** The forward pipeline is `wikitext → Document → HTML`, and
`Document` is a tree of wikitext semantics that other formats cannot produce
directly. HTML expresses things `Document` cannot (arbitrarily nested inline
elements, `<div>` inside `<p>`), and Markdown another set. The shared tree holds
only what every front end can produce and every writer can express, so **a construct
is mapped once per format rather than once per format pair**.

**"Clean" is a choice with a cost.** The goal is wikitext a person would write
(`'''` rather than `<b>`, `*` rather than `<li>`), not 1:1 fidelity, judged by
semantic equivalence once compiled back to HTML. Unrecognised HTML (a `<span>` with
classes, an unknown tag) is **kept as a literal tag** rather than dropped, because
losing information is worse than leaving an ugly marker.

**Three real traps that only testing found.**

1. **A Markdown h1 cannot become `=`.** Two equals make a heading in MediaWiki, so
   `= Title =` is plain text and an h1 written that way would not survive a compile
   back. An h1 therefore becomes `==`.
2. **List nesting is positional.** Wikitext encodes depth as the number of markers,
   and the **ancestor list kinds come first, the item's own kind last**: an ordered
   item under `* a` is `*# b`. Writing `#* b` produces two sibling lists, and `## b`
   starts a new ordered list; neither nests. This cannot be derived by reading code,
   only by compiling every combination back to HTML.
3. **Block-level HTML tags need their own path.** The core parser originally flushed
   a paragraph only for **extension** tags (`<ref>` and so on), so ordinary block
   tags took the inline path and `<blockquote>` ended up inside a `<p>`. The fix
   gives HTML block tags their own branch (`starts_html_block` plus `html_block`),
   separate from extension tags. Folding the two into `ext_tag_block` had failed
   because it falls back to the inline path for anything that is not an extension
   tag.

**Escaping is context dependent.** In body text `{`, `[` and `]` must be escaped or
they become a template or a link, while inside a table cell `|` is the structural
character. So escaping is decided by context, `Context::Flow`, `Link` or `Table`,
not by a global replacement.

## 9. Compile latency (measured)

`cargo run -p wikitext --release --example latency` reports absolute numbers, and
`--example stages` splits the pipeline into three parts. Measured on the development
machine (release, `lto = "thin"`, `codegen-units = 1`):

| Input | Time | Throughput |
|---|---|---|
| Empty document | 1.1 µs | n/a |
| 19 KB article | 1.35 ms | 13.5 MB/s |
| 150 KB article | 11.8 ms | 12.2 MB/s |
| 400-section scaling test | 13.0 ms | 14.1 MB/s |

Scaling is linear, with no superlinear behaviour. The three stages:

| Stage | Time on the 19 KB article | Share |
|---|---|---|
| Preprocess (template and function expansion) | 0.45 ms | 19% |
| Parse (tree building) | 2.21 ms | **94%** |
| Serialize | 0.09 ms | 4% |

The shares overlap because each stage was timed independently.

**Parsing is the only place worth optimizing.** Split by content type (40 KB each),
plain text runs at 610 MB/s while marked-up text manages only 18 to 45 MB/s, so the
cost is **the construction cost of a single markup node**, not the amount of text.
The slowest are references (`<ref>`, 18 MB/s), tables and wikilinks (21 MB/s each).

**The cost of one wikilink, layer by layer** (1500 links, median, measured by
returning early at successive points inside `internal_link`):

| Stage | Time | Increment |
|---|---|---|
| No link built (baseline) | 1.03 ms | n/a |
| `href_for` only | 1.17 ms | +0.14 ms |
| An empty `<a>` built | 1.79 ms | **+0.76 ms** |
| Complete | 2.25 ms | +1.22 ms |

So roughly 500 ns goes into constructing one `Element`. The reason is that
`Attribute.name` was a `String` while every attribute name the compiler generates is
a literal (`"href"`, `"class"`, `"id"`), so each attribute cost a heap allocation.

**Two changes were made**, both of the form "this assignment should not allocate at
all" rather than trading readability for speed:

- `Attribute.name` and `RawAttribute.name` became `Cow<'static, str>`, so literal
  names stop allocating.
- `encode_title` called `format!("{byte:02X}")` per escaped byte, allocating a
  `String` each time. That became a lookup table, and the intermediate string from
  `replace(' ', "_")` is gone.

**Result: no measurable end-to-end change** (the 19 KB article stays at about
1.35 ms). The changes allocate less, but that is too small a share of a whole
article. Going further needs a sampling profiler, and this machine has none. At this
noise level "change one thing, measure once" is not reliable: the same configuration
has measured 1.03 ms and 2.16 ms on repeat runs.

## 10. Robustness (measured)

Every defect that adversarial inputs found (`cargo run -p wikitext --release
--example fuzz`) is fixed and pinned by a regression test. `--example scaling`
confirms the complexity class by doubling input sizes.

**Three crashes, all O(n²) or worse, on input a real page can contain:**

| Input | Before | After | Root cause |
|---|---|---|---|
| `{{` × 4000 (8 KB) | **8.5 s** | 36 µs | after `match_open` failed it advanced 1 byte and rescanned the same suffix; each attempt also recounted nesting from scratch |
| `[[a\|` × 16000 (128 KB) | **1.01 s** | 443 µs | the same shape in `scan_wikilink`, which advanced `index += 2`; with no `]]` ahead failure is certain, so the whole run can be skipped |
| `<div` with no `>` × 32000 (128 KB) | **3.15 s** | 76 ms | `scan_tag` searched to the end of input for a `>`, and the caller then advanced 1 byte |

**Two stack overflows, each aborting the process:**

| Input | Crashed at | Fix |
|---|---|---|
| `[[…]]` nested | 800 levels (**1.6 KB of source**) | a nesting ceiling on inline re-parsing (40 levels, the same order as MediaWiki); deeper content is kept as literal text |
| `<div>…</div>` nested | 6400 levels | two places: `Element`'s `Drop` and the serializer both became explicit stacks |

**Recursive `Drop` was the most deceptive one.** `[[` crashed at 800 levels while
`<div>` crashed at 6400, and that gap showed the problem was not in parsing logic. A
nested `Element` calls its destructor one level at a time, so **merely freeing a deep
tree can overflow the stack**, and serialization has the same shape. The worklist in
`impl Drop for Element` in `crates/wikitext/src/dom.rs` exists for this.

**The last and largest one: the number of tags.** It was first recorded as "repeated
unclosed `<div>` is ×5.3", and it resisted localization for a long time because two
variables were being compared together, bytes and tag count. Measuring strictly at
equal byte counts showed **the balanced form degrades just as badly**, which reduced
it to one variable: how many tags. Stage timing then showed lexing linear and parsing
quadratic, and the cause was **two per-tag linear scans**:

| Location | What it did | What it does now |
|---|---|---|
| `handle_tag_end` | `stack.iter().rposition(...)`, a linear search of the whole block stack by name | `open_tags: HashMap<name, Vec<stack index>>`, O(1) |
| `has_tag_closer_ahead` | scanned every remaining token per opening tag | one right-to-left pre-pass maintaining a `closers_ahead` count |

After the fix, 128000 unclosed tags went from **7422 ms to 89 ms**, and the balanced
form is linear too.

`open_tags` has a non-obvious invariant: `close_block` pops the blocks in between and
turns them into literal text, so **their slots have to leave the index at the same
time**, or a later close reads a stale position. `forget_open_tag` does that.

**One defect that is not about performance but affects every Windows user.** A `\r`
from CRLF input survived all the way into the DOM and then into the output. A
terminal reads a bare `\r` as "return to column zero", so `echo "..." | wikitext`
looked as though it had gained a line break before `</p>`. `<li>`, `<p>` and bold
text were all affected. This was not a display artefact: the byte was really in the
HTML.

The fix folds line endings at both `Preprocessor` entry points
(`normalize_line_endings`). It goes first because there is no single place later:
`protect` has already replaced `<nowiki>` and `<pre>` with sentinels, so a later pass
cannot reach their contents. The folding itself matches MediaWiki, which normalizes
the same way before parsing.

**A lesson about evidence.** The first four defects above were all confirmed by
doubling input sizes and reading the time ratio. A single measurement on this machine
can vary by a factor of two, and an early conclusion drawn from single measurements
was wrong. `crates/wikitext/examples/latency.rs` says so in its own comments.

## 11. Bindings for other languages

All three formats are available from every binding. The layering is deliberate:
**`wikitext-ffi` is the only foundation**, and Go and Node.js can build on it or
link the Rust library directly.

| Layer | Contents | Why it sits here |
|---|---|---|
| `crates/wikitext` | The core compiler | Zero dependencies by default |
| `crates/wikitext-convert` | Format conversion | The only place `html5ever` is needed |
| `crates/wikitext-ffi` | C ABI | 34 exported symbols, usable from anything that can call a shared library |
| `bindings/go` | cgo wrapper | Pure cgo, no third-party dependencies |
| `crates/wikitext-node` | napi-rs addon | Node needs a native addon to call synchronously |
| `crates/wikitext-wasm` | WebAssembly | Browsers, with a JavaScript wrapper |

**Format codes are identical in all three languages** (`0` HTML, `1` Markdown,
`2` Wikitext). Making them identical matters more than letting each layer do the
idiomatic thing, because a caller working across languages will follow one language's
documentation while writing another's code.

**Only Wikitext to HTML uses the options** (templates and modules). The other
directions have nothing to expand, so `options` may be `NULL`, `nil` or omitted. This
is stated in the header because a C caller cannot infer it from the signature.

**Two C ABI invariants**, both held by tests:

1. **No call panics or aborts.** Every `extern "C"` entry point goes through
   `catch_unwind`, and a panic becomes `WIKITEXT_ERROR_PANIC` with a readable message.
   Unwinding across an FFI boundary is undefined behaviour, so this is not optional.
2. **Allocation and deallocation always happen on the same side.** A buffer the
   library allocated is freed by `wikitext_buffer_free`; host data is only a pointer
   and a length, and the library reads without freeing. `WikitextOwnedBuffer` is
   always returned by value with only the data pointer on the heap. An earlier shape
   let a C caller put the header on its stack while the library called
   `Box::from_raw` on it, which is undefined behaviour; the current shape cannot be
   written that way.

**One constraint on the Node side**: `napi-build` emits `cargo::` instructions, which
need Rust 1.77, so `wikitext-node` sets a `rust-version` above the workspace's 1.75.
That is the only place the crate cannot follow the workspace.

## 12. Serialization

The target is HTML5: void elements get no closing tag, and `self_closing` is true
only when the source actually wrote `/>`, so `<br>` does not become `<br />`.

Block elements are newline separated and inline elements stay on their line.
**`td` and `th` are deliberately not block level**: MediaWiki writes a row's cells on
one line, and matching that keeps output diffs readable.

## 13. Error recovery

MediaWiki almost never raises a hard error, and neither does this project. Every
problem becomes a `Diagnostic` and the syntax degrades to literal text while parsing
continues. Unclosed tags, unclosed tables and unclosed comments each have a path.

## 14. Dependencies and licensing

The core crate has **no dependencies**; every parser, entity table, expression
evaluator and date calculation is written here. The only optional dependencies are
`clap` (MIT OR Apache-2.0) for the CLI and `wasm-bindgen` (MIT OR Apache-2.0) for
WASM, plus `html5ever` and its companion crates (MIT OR Apache-2.0) for conversion
and `napi`, `napi-derive` and `napi-build` (MIT) for the Node addon.

**There is no GPL code anywhere**: Parsoid (GPL-2.0+) was not ported, MediaWiki core
(GPL-2.0+) was not ported, and `parse_wiki_text` (a GPL-family crate) is not linked.
Only public syntax documentation and observable behaviour were used.

## 15. Known tradeoffs

| Tradeoff | Reason |
|---|---|
| Provenance is lost after expansion | Buys two independently testable stages |
| Sentinel characters can collide with source | Buys an implementation needing no escaping |
| `; a : b : c` splits only at the first colon | Matches MediaWiki |
| `<math>` is marked up but not typeset | TeX typesetting is not the compiler's job; a `data-mw-ext` hook is left for a front end |
| Lua closures can form reference cycles | Each evaluation drops the whole heap, so a cycle costs nothing observable, and an arena would slow every scope operation |
| Title objects compare by field, not by title | Needs `__eq` plus normalized keys; deferred until there is a real need |
| Conversion drops most HTML attributes | Clean wikitext and full fidelity conflict; this chose the former |
| Converted HTML tables ignore `rowspan`/`colspan` | Wikitext tables have no equivalent, so it can only degrade |
| Compile latency was not deeply optimized | Parse is 94% and the cost sits in single-node construction; going further needs a sampling profiler this machine lacks |
| Nesting is capped at 40 levels | The same order as MediaWiki; deeper nesting keeps its content as literal text |
| `<gallery>` emits an image list only | No gallery CSS or JS; the shape is the skin's business |
| Date magic words return the Unix epoch | The core crate takes no clock dependency; a caller can substitute before parsing |
| Output is shaped like MediaWiki but not byte-identical | The target is HTML5 semantics, not the internal order of tags |

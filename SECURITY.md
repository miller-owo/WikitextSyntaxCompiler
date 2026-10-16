# Security Policy

## Supported versions

The project is at `0.1.0` and has no released versions yet. Fixes land on the
default branch. Once there are releases, the latest minor version will receive
security fixes.

## Reporting a vulnerability

**Do not open a public issue.**

Use GitHub's private vulnerability reporting on the repository's Security tab
("Report a vulnerability"). That opens a private advisory visible only to the
maintainers.

If that is unavailable, open a public issue that says only that you have a security
report and how to reach you. Do not include the details.

Please include:

- the version, from `cargo run -p wikitext-cli -- --version`
- the input that triggers it, as a file or an exact byte sequence
- what happens, and what you expected
- which entry point: the Rust library, the CLI, the C ABI, or a language binding
- whether the input came from an untrusted source

You can expect an acknowledgement within a few days. Please allow a reasonable
window before disclosing publicly.

## Threat model

This is a compiler for wikitext, and **its input is assumed to be untrusted**. A
wiki page is written by anyone, so every entry point treats its source as hostile.
The properties that matter, in order:

1. **It must not execute code from the input.** Parsing wikitext must not lead to
   arbitrary code execution in the host process.
2. **It must not be coerced into unbounded work.** A page of a given size must
   compile in time and memory proportional to that size. A page that causes
   superlinear time or unbounded memory is a denial of service, and that is a
   security bug here, not a performance bug.
3. **It must not abort the host process.** No panic, no stack overflow, no abort.
   In a long-lived server, a crash on one page is a crash for every request.
4. **The C ABI must not corrupt memory.** No double free, no cross-allocator free, no
   unwinding across the boundary.

### In scope

- Unbounded recursion, stack overflow, or abort on any input
- Superlinear time or memory in input size
- Memory unsafety, including at the C ABI boundary
- A panic escaping any `extern "C"` entry point
- Allocation and deallocation happening on opposite sides of the FFI boundary
- Path traversal or arbitrary file read through the CLI's `--templates` and
  `--modules` directory options
- Resource limits that can be bypassed or reset by input

### Out of scope

- **Wikitext markup producing HTML.** That is the job. Whether that HTML is safe to
  serve is the caller's decision, and a sanitiser belongs after the compiler, not
  inside it.
- **Output being unsafe to interpolate into a page unescaped.** The compiler emits an
  HTML fragment by design.
- **Rendering differences from MediaWiki.** A wrong result is a correctness bug, not a
  security one. Report it as a normal issue.
- **Vulnerabilities in MediaWiki, Parsoid, or extensions.** Those are separate
  projects. This is an independent implementation and shares no code with them.
- **Lua modules written by a trusted operator.** By default, Lua execution is sandboxed
  by limits, not by a capability system. Host functions reachable from a module are the
  embedder's choice.
- **Denial of service through the CLI reading a path the operator supplied.**

## Existing controls

These are in place, each with tests. A report that one of them is insufficient is
welcome; that is exactly the kind of finding worth having.

| Area | Control |
|---|---|
| Lua call depth | 200 frames |
| Lua loop iterations | 1 000 000 per loop |
| Lua instruction budget | 20 000 000 steps per evaluation |
| Template expansion | 40 levels deep, 10 000 expansions, 2 MB expanded size |
| Inline nesting (links, tags) | 40 levels, matching MediaWiki's order; deeper input keeps its content as literal text |
| Tag scanning | a tag must close within 4096 bytes to count as one |
| C ABI | every entry point wrapped in `catch_unwind`; no panic crosses the boundary |
| Buffers | the library owns and frees its own buffers; host memory is only borrowed for the call |

Resource limits in Lua are deliberately **not** catchable by `pcall`, so a runaway
module cannot suppress its own termination.

The parser is also covered by adversarial input testing in
[crates/wikitext/examples/fuzz.rs](crates/wikitext/examples/fuzz.rs), with complexity
confirmed by doubling input sizes in
[crates/wikitext/examples/scaling.rs](crates/wikitext/examples/scaling.rs). Findings
from that work, all fixed, are recorded in [AGENTS.md §10](AGENTS.md).

## Hardening notes for embedders

- Serve the HTML your own way. If you accept untrusted wikitext from users and serve
  the result to other users, run a sanitiser over the output. The compiler does not
  remove `<script>` written in wikitext that a wiki would have allowed.
- Set a wall-clock timeout around compilation as a second line of defence. The
  internal limits bound work per construct, and the parser is linear in input size,
  but a caller-side timeout is cheap insurance against a bug that gets past both.
- Do not share an options or document handle across threads. They are not thread safe
  in any binding; give each worker its own.
- Cap input size before compiling if your users can submit very large pages.

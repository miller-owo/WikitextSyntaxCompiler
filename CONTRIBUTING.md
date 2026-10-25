# Contributing

Thanks for taking a look. This document covers what to run before opening a pull
request and the conventions this repository holds to.

By participating you agree to keep discussion civil and focused on the code.

## Getting set up

```sh
git clone https://github.com/miller-owo/WikitextSyntaxCompiler.git
cd WikitextSyntaxCompiler
cargo build --workspace --all-features
```

Rust 1.75 or newer. The `wikitext-node` crate needs 1.77, because `napi-build`
emits `cargo::` instructions; every other crate follows the workspace floor.

Before changing code, read [AGENTS.md](AGENTS.md). It records the decisions in this
project that are not obvious and the reasons behind them. Reversing one of those
decisions without knowing why it was made is the most common way to break something
here. The quote engine and the list state machine are the two places where that
bites hardest.

## Before opening a pull request

All four must pass:

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

If you touched a binding, run its suite too:

```sh
pwsh bindings/go/test.ps1 -Release
cargo build -p wikitext-node && node bindings/node/test.mjs
```

For the C ABI, build the library and compile the smoke test against it:

```sh
cargo build -p wikitext-ffi --release --target x86_64-pc-windows-gnu
gcc -std=c11 -Wall -Wextra -I crates/wikitext-ffi/include \
    -o smoke crates/wikitext-ffi/tests/c/smoke.c \
    -L target/x86_64-pc-windows-gnu/release -lwikitext
```

## Conventions

### Comments are the exception

Most code should have none. Write a comment only when the code cannot carry the
information itself: a hidden constraint, a bug in a dependency, a platform quirk, a
performance reason, a deliberate deviation. Never restate what the code does.

Ask of a candidate comment: could a competent reader work this out from the code in
under a minute? If yes, delete it. If you feel the urge to explain, first try
changing the code until it explains itself.

Comments are in English, one or two lines, no blocks and no banner dividers.

If you find a section marked as frozen, leave it alone. Those markers exist because
someone established that only that formulation works, and a cleanup task does not
override them.

### Documentation

English only. No em dashes or en dashes; use a comma, a colon or a full stop.

### Dependencies

The core crate has no dependencies and should keep it that way. If a change seems to
need a new one, open an issue first.

**No copyleft.** Nothing GPL, LGPL or AGPL may be linked, copied or adapted, in any
crate, at any depth of the dependency tree. Check before adding:

```sh
cargo tree -p <crate>
cargo deny check licenses     # if you have cargo-deny
```

Permissive licenses already in use: MIT, Apache-2.0, ISC, the Unicode license, the
Unlicense and BSD-2-Clause. A new dependency also means regenerating
`THIRD-PARTY-NOTICES.md`, because MIT and ISC require their copyright notice to travel
with a binary, and that file is where it travels.

### Code

- `#![forbid(unsafe_code)]` holds in every crate except `wikitext-ffi`, where the
  `extern "C"` boundary requires it.
- No `extern "C"` entry point may unwind. Wrap new ones in the existing `guard`, so a
  panic becomes `WIKITEXT_ERROR_PANIC` rather than undefined behaviour.
- Library code must not panic on any input. Malformed input becomes a `Diagnostic`
  and degrades to literal text, matching MediaWiki's forgiving behaviour.
- Format codes are shared across languages (`0` HTML, `1` Markdown, `2` Wikitext) and
  must stay identical in the C ABI, Go, Node.js and WebAssembly bindings.

## Tests

Tests live both inline in each crate and in a `tests/` directory:

| Suite | Command | Tests |
|---|---|---|
| Core compiler | `cargo test -p wikitext --features lua` | 153 |
| Lua interpreter | `cargo test -p wikitext-lua` | 93 |
| Format conversion | `cargo test -p wikitext-convert` | 29 |
| C ABI | `cargo test -p wikitext-ffi` | 24 |
| WebAssembly | `cargo test -p wikitext-wasm` | 15 |
| Go | `pwsh bindings/go/test.ps1 -Release` | 19 |
| Node.js | `node bindings/node/test.mjs` | 17 |
| C | compile and run `crates/wikitext-ffi/tests/c/smoke.c` | 40 |
| Everything | `cargo test --workspace --all-features` | 314 |

What is expected with a change:

- **New syntax support** needs a test that compiles it and asserts the HTML, plus a
  test for the malformed form of the same construct.
- **A bug fix** needs a regression test that fails before the fix. Name it after the
  behaviour, not the bug number.
- **A robustness fix** needs a case in `crates/wikitext/examples/fuzz.rs`, and the
  complexity claim confirmed with `crates/wikitext/examples/scaling.rs` by doubling
  input sizes. A single timing measurement is not evidence: the same configuration
  has been observed to vary by a factor of two on this project's development machine.
- **A parser change** should keep the output shaped like MediaWiki's: block elements
  newline separated, `td` and `th` on their row's line.

## Measuring

```sh
cargo run -p wikitext --release --example latency    # absolute timings
cargo run -p wikitext --release --example stages     # which stage dominates
cargo run -p wikitext --release --example scaling    # linear or quadratic?
cargo run -p wikitext --release --example fuzz       # adversarial inputs
```

Report absolute numbers rather than a ratio against a stored baseline, so a result
means the same thing on another machine.

## Commits and pull requests

- One logical change per commit. Keep formatting-only changes separate from
  behaviour changes.
- Explain *why* in the commit message. The *what* is visible in the diff.
- A pull request that changes parser output should say which MediaWiki behaviour it
  is matching, and link to it.
- Update [AGENTS.md](AGENTS.md) when you make a decision a future reader could not
  infer from the code, and [README.md](README.md) when you change the public surface.

## Reporting a bug

Include the input, the output you got, the output you expected, and the version
(`cargo run -p wikitext-cli -- --version`). For a rendering difference, say which
MediaWiki behaviour you are comparing against.

For a security issue, follow [SECURITY.md](SECURITY.md) instead of opening a public
issue.

## License

Contributions are accepted under BSD-2-Clause, the license in [LICENSE](LICENSE).

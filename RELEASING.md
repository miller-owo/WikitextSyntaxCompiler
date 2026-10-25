# Releasing

This is the checklist for pushing the repository to GitHub and for cutting a
release. Work through it in order; the first section is the part that cannot be
undone once it is public.

## Before the first push

### 1. The URLs and the name

Already set, and worth checking before the first push:

| Where | Value |
|---|---|
| `Cargo.toml` | `repository = "https://github.com/miller-owo/WikitextSyntaxCompiler"` |
| `README.md`, `CONTRIBUTING.md` | `git clone https://github.com/miller-owo/WikitextSyntaxCompiler.git` |
| `CHANGELOG.md` | two comparison links under `miller-owo/WikitextSyntaxCompiler` |
| `.github/ISSUE_TEMPLATE/*.yml` | the security and contributing links, both under the same repository |
| `SECURITY.md` | points at GitHub private reporting, which needs no URL if the repository is on GitHub |

If the repository ends up somewhere else, replace that host and path everywhere:

```sh
grep -rn "miller-owo/WikitextSyntaxCompiler" . --exclude-dir=target --exclude-dir=.git
```

### 2. The copyright holder

`LICENSE` says `Copyright (c) 2026, MELLER_OwO`, and `Cargo.toml` sets
`authors = ["MELLER_OwO"]`, which is what crates.io displays. `MELLER_OwO` is the
display name; the account login is `miller-owo`. Change either if the work is ever
assigned to an organisation, and keep the year as the year of first publication.

### 3. Check the license position

No GPL code may be linked, copied or adapted, at any depth. Confirm the tree is
clean:

```sh
cargo tree --workspace --all-features | grep -iE "gpl|agpl|lgpl"
```

An empty result is what you want. If `cargo-deny` is available, `cargo deny check
licenses` is stronger.

Then make sure the attribution file still matches the tree. It carries the copyright
line and the license text of every crate in `Cargo.lock`, which is the part a binary
distribution has to reproduce:

```sh
pwsh -File scripts/third-party-notices.ps1
git diff --stat THIRD-PARTY-NOTICES.md
```

A diff means a dependency moved, and the new rows have to be looked at. A crate whose
license is not one of the five texts already in the file is the case worth stopping
for.

### 4. Make sure nothing generated is committed

```sh
git add -A --dry-run | wc -l
```

Walk the list. It should contain only sources, tests, docs and configuration. If
`target/`, a `.node` file, a `.dll`, a `.pdb` or a `.lib` appears, `.gitignore` is
wrong. Note that `.gitignore` deliberately ignores only `target` plus the artifacts
the binding test scripts copy outside it, so anything else showing up is a mistake.

### 5. Confirm it builds from a clean tree

```sh
cargo clean
cargo build --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

Then the bindings, following [CONTRIBUTING.md](CONTRIBUTING.md#before-opening-a-pull-request).

### 6. Push

```sh
git add -A
git commit -m "Initial commit"
git remote add origin git@github.com:YOU/WikitextSyntaxCompiler.git
git push -u origin main
```

`git init` has already been run and the branch is already `main`, so there is nothing
to rename. The CI trigger lists both `main` and `master` so a fork that uses either
name still runs.

### 7. Turn on the repository settings you rely on

- **Private vulnerability reporting**, under Settings then Security. `SECURITY.md`
  sends people there, so it should be on.
- **Branch protection** on the default branch, requiring the CI jobs to pass.
- **Topics**, so the repository is findable: `rust`, `wikitext`, `mediawiki`,
  `parser`, `html`, `wasm`.

## Cutting a release

### 1. Keep the changelog honest

Move the entries under `## [Unreleased]` into a new version heading with today's date,
and leave `[Unreleased]` in place at the top for the next cycle. Update the two
comparison links at the bottom.

### 2. Bump the version

The version lives once, in the workspace manifest:

```
[workspace.package]
version = "0.1.0"
```

Every crate reads it with `version.workspace = true`, and the two `wikitext` and
`wikitext-lua` entries in `[workspace.dependencies]` carry a copy that must match.

### 3. Tag it

```sh
git tag -a v0.1.0 -m "0.1.0"
git push origin v0.1.0
```

Then create a GitHub release from the tag, pasting the changelog section.

### 4. crates.io

Five crates are publishable: `wikitext`, `wikitext-lua`, `wikitext-cli`,
`wikitext-convert` and `wikitext-ffi`. `wikitext-node` and `wikitext-wasm` set
`publish = false` on purpose, because they are consumed as build outputs rather than
as crates.

Publish in dependency order, since each one resolves the previous:

```sh
cargo publish -p wikitext-lua
cargo publish -p wikitext
cargo publish -p wikitext-convert
cargo publish -p wikitext-cli
cargo publish -p wikitext-ffi
```

Wait for each to appear in the index before running the next, or the resolution will
fail. Check the dry run first with `cargo publish --dry-run -p <crate>`, which also
confirms each crate's `README.md` is picked up through the `readme` key.

## A note on what is not automated

There are no release binaries. The CLI builds from source, the C ABI is built by
whoever needs it, and the Node addon and WebAssembly module are build outputs rather
than published artifacts. If that changes, add a workflow that builds per platform
and attaches the results to the release.

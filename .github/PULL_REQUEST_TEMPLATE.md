## What changed

<!-- One or two sentences. The diff shows the what; explain the why. -->

## Why

<!-- The problem this solves, or the behaviour it matches. For parser output,
     link to the MediaWiki behaviour being matched. -->

## How it was checked

<!-- Which tests were added or changed, and what commands you ran. -->

- [ ] `cargo test --workspace --all-features`
- [ ] `cargo clippy --workspace --all-targets --all-features`
- [ ] `cargo fmt --all --check`

If this touches a binding, say which and paste the result:

- [ ] Go: `pwsh bindings/go/test.ps1 -Release`
- [ ] Node.js: `node bindings/node/test.mjs`
- [ ] C: compiled and ran `crates/wikitext-ffi/tests/c/smoke.c`

## Notes for the reviewer

<!-- Anything that is not obvious: a tradeoff, a deliberate deviation, a
     follow-up you did not do. -->

- [ ] Comments follow the house style: only where the code cannot say it itself
- [ ] No new dependency, or the license was checked and is permissive
- [ ] Documentation updated if the public surface changed
- [ ] `AGENTS.md` updated if this makes a decision a future reader could not infer

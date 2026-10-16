# wikitext-lua

A Lua 5.1 interpreter written from scratch.

The lexer, parser, tree-walking evaluator, pattern matcher and standard library were
written against the Lua 5.1 reference manual, so no PUC-Lua and no LuaJIT code is
involved and the crate has no dependencies. It runs Scribunto modules for
`{{#invoke:…}}` in [`crates/wikitext`](../wikitext) and knows nothing about `mw.*`.

## Install

```sh
cargo add wikitext-lua
```

In normal use it arrives through the `lua` feature of the compiler crate, which
installs the `mw.*` surface for each call.

## Use

```rust
use wikitext_lua::Interp;

let mut interp = Interp::new();
let value = interp.eval("return 1 + 2 * 3").unwrap();
assert_eq!(interp.tostring(&value).unwrap(), "7");
```

`eval` returns the first value of the chunk and `eval_multi` returns all of them.
`Interp::new` installs the standard library; `Interp::bare` starts with empty globals.

A module is a chunk that returns a table, and `call_value` calls into it:

```rust
use wikitext_lua::value::TableRef;
use wikitext_lua::{Interp, Value};

let mut interp = Interp::new();
let values = interp.eval_multi(
    "local p = {}\nfunction p.greet(frame) return 'Hello, ' .. frame.args[1] .. '!' end\nreturn p",
).unwrap();
let exports = match values.first() {
    Some(Value::Table(table)) => table.clone(),
    other => panic!("a module returns a table, got {other:?}"),
};
let arguments = TableRef::new();
arguments.set(Value::from(1), Value::string("Rust"));
let frame = TableRef::new();
frame.set(Value::from("args"), Value::Table(arguments));
let function = exports.get(&Value::string("greet"));
let results = interp.call_value(function, vec![Value::Table(frame)]).unwrap();
assert_eq!(interp.tostring(&results[0]).unwrap(), "Hello, Rust!");
```

Execution is bounded, so a runaway module cannot hang the host:

```rust
use wikitext_lua::{Interp, Limits, LuaError};

let limits = Limits { call_depth: 200, loop_iterations: 1_000_000, steps: 20_000_000 };
let mut interp = Interp::new().with_limits(limits);

match interp.eval("while true do end") {
    Err(LuaError::LimitExceeded { limit }) => println!("stopped by the {limit} limit"),
    other => panic!("expected a limit, got {other:?}"),
}
```

Those are the defaults. Exceeding a limit is not an ordinary error: `pcall`
deliberately does not catch it, because the interpreter itself gave up.

## API summary

`Interp` is the interpreter: `new`, `bare`, `with_limits`, `eval`, `eval_multi`,
`call_value`, `tostring`. `Limits` holds `call_depth`, `loop_iterations` and `steps`.
`Value` is `Nil`, `Boolean`, `Number`, `String`, `Table`, `Function` or `UserData`.
`LuaError` is `Syntax`, `Runtime`, `LimitExceeded` or `Host`, and `LuaResult<T>` is
`Result<T, LuaError>`. Modules: `ast`, `error`, `interp`, `lex`, `parse`, `stdlib`,
`value`. `parse::parse` returns a `Chunk` that `Interp::run_chunk` can run again.

Supported: metatables and metamethods, closures and upvalues, varargs, multiple
returns, `pcall` and `error`, string patterns (`%b`, `%f`, position captures,
back-references, greedy and lazy repetition), and the `base`, `string`, `table`,
`math` and `os` libraries. Not supported: coroutines, `io`, `debug` and `goto`.

## Related

[crates/wikitext](../wikitext) is the compiler that drives it; [AGENTS.md](../../AGENTS.md) has the design.

BSD-2-Clause. See [LICENSE](../../LICENSE).

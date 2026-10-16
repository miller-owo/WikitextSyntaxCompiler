# Lua modules

`{{#invoke:Module|function}}` runs Lua to build part of a page. That is how most of
the complicated logic on a real wiki is written: infoboxes, citation formatting,
string manipulation that templates cannot express.

This project runs it on a Lua 5.1 interpreter written from scratch, in
[`crates/lua`](../crates/lua). It is not a port of PUC-Lua or LuaJIT, so the license
stays clean.

## Turning it on

The interpreter is behind a feature, because most compilations do not need it:

```sh
cargo add wikitext --features lua
```

Without the feature, `#invoke` compiles to an error marker. With it, and with no
provider configured, you get a different one:

```
invoke: no Lua module provider is configured
```

## Supplying modules

`LuaModules` is the built-in provider:

```rust
use std::rc::Rc;
use wikitext::lua_modules::LuaModules;
use wikitext::{compile, Options};

let modules = LuaModules::new();
modules.insert("Roman", r#"
    local p = {}
    function p.convert(frame)
        return "value: " .. frame.args[1]
    end
    return p
"#);
let options = Options::default().with_module_provider(Rc::new(modules));

assert!(compile("{{#invoke:Roman|convert|42}}", &options).contains("value: 42"));
```

A module is an ordinary Lua chunk that returns a table. Each function on that table
becomes callable through `#invoke`. `Module:Roman` and `Roman` are the same module,
and the name is normalised the way page titles are.

Note the `Rc` rather than `Arc`: Lua values are single threaded, so a provider is not
`Send` and cannot be shared across threads. Give each thread its own.

## The frame

The function receives a frame with the call's arguments:

| Field or method | What it holds |
|---|---|
| `frame.args` | The arguments after the function name, positional and named |
| `frame:getParent()` | The frame that called this one, or `nil` at the top |
| `frame:getTitle()` | The title of the page being compiled |

```lua
function p.greet(frame)
    local name = frame.args[1] or frame.args.name or "world"
    return "Hello, " .. name .. "!"
end
```

There is no `frame:expandTemplate` and no `frame:preprocess`, so a module cannot
transclude a template or re-parse wikitext itself. Whatever a module needs from
another page it has to receive as an argument.

## Loading modules from a directory

The CLI reads `*.lua` files and uses each file name as the module name:

```sh
mkdir modules
cat > modules/Roman.lua <<'LUA'
local p = {}
function p.convert(frame)
    local n = tonumber(frame.args[1]) or 0
    local map = { [1000]='M', [900]='CM', [500]='D', [400]='CD', [100]='C',
                  [90]='XC', [50]='L', [40]='XL', [10]='X', [9]='IX',
                  [5]='V', [4]='IV', [1]='I' }
    local out = {}
    for _, value in ipairs({1000,900,500,400,100,90,50,40,10,9,5,4,1}) do
        while n >= value do n = n - value; out[#out + 1] = map[value] end
    end
    return table.concat(out)
end
return p
LUA
echo 'Roman numeral of 1994 is {{#invoke:Roman|convert|1994}}.' | wikitext --modules ./modules
# <p>Roman numeral of 1994 is MCMXCIV.</p>
```

## What `mw` offers

Each `#invoke` call runs in a fresh interpreter with an `mw` table installed. Fresh
each time is deliberate: one module must not see another's globals, and a call must
not leave state behind for the next one.

| Namespace | What it covers |
|---|---|
| `mw.text` | Trimming, splitting, escaping, `mw.text.len` by bytes |
| `mw.ustring` | The same operations counted in characters rather than bytes |
| `mw.title` | Page title objects, with `tostring` and `..` |
| `mw.uri` | Percent encoding and decoding |
| `mw.html` | A chained builder for HTML fragments |
| `mw.language` | Language codes and names |
| `mw.dumpObject` | A readable dump, for debugging a module |

Three names exist so that a module written for a real wiki fails where you can see
it rather than silently doing nothing:

- `mw.loadData` raises `mw.loadData is not supported`.
- `mw.site` and `mw.message` are empty tables, and reading a field from them gives
  `nil`. There is no site configuration or interface message store behind this
  compiler, so there is nothing truthful to return.

Title objects also compare by field rather than by title; only `tostring`, `..` and
field access are guaranteed.

## The Lua dialect

The interpreter implements Lua 5.1, including the parts people actually rely on:

- `if`, `while`, `repeat`, both `for` forms, `break`, functions, `:` method
  declarations with `self`, varargs `...`, table constructors, multiple returns
- Closures capturing upvalues by reference, metatables, and the usual metamethods
  (`__index`, `__newindex`, `__call`, `__tostring`, `__concat`, `__eq`, `__lt`,
  `__le`, `__len` and the arithmetic ones)
- String and number coercion
- `string`, `table`, `math`, `os` and the `base` library, with the full Lua pattern
  matcher (`%b`, `%f`, position captures, back-references, greedy and lazy
  quantifiers) and every `string.format` conversion

## Limits

A module cannot run forever. Three ceilings apply per evaluation:

| Limit | Default |
|---|---|
| Call depth | 200 frames |
| Loop iterations | 1 000 000 per loop |
| Instruction budget | 20 000 000 steps |

Exceeding one returns a limit error rather than hanging. That error is **not**
catchable by `pcall`, on purpose: it means the interpreter gave up, and letting the
module catch it and continue would only hit the ceiling again immediately.

## Writing modules people can read

A few habits that pay off, learned from the ones that are hard to debug:

- Return early for a bad argument rather than producing `nil` that fails later.
  `frame.args[1]` is a string or `nil`, never a number, so `tonumber` it.
- Do not count arguments with `#frame.args`. Positional arguments are numbered from
  1, but named arguments share the same table and are invisible to the length
  operator, so `{{#invoke:M|f|one|two|name=three}}` gives `#frame.args == 2` while
  `pairs` sees three entries. Use `pairs` when you need to know what was passed.
- Keep the module returning a table. A chunk that returns nothing makes every
  `#invoke` on it fail.

## Next

- [Embedding](embedding.md) for serving modules from a store rather than a directory.
- [AGENTS.md](../AGENTS.md) for why the interpreter is a tree walker and what that
  costs.

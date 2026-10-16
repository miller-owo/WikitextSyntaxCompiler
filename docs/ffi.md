# C ABI

`crates/wikitext-ffi/include/wikitext.h` is the contract. It carries only the
declarations, so the rules a caller cannot read off a signature live here.

## Build

```sh
cargo build -p wikitext-ffi --release                        # host platform
cargo build -p wikitext-ffi --release --target x86_64-pc-windows-gnu
```

The output is a `cdylib` (plus a `staticlib`). Put it where the loader finds it:
next to the executable, or on `PATH` / `LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH`.

## Rules a signature does not show

**No call aborts the process.** Every fallible call returns `WIKITEXT_OK` or a
negative code and leaves a message for `wikitext_last_error`. A panic on the Rust
side becomes `WIKITEXT_ERROR_PANIC` with the panic message attached, because
unwinding into a foreign caller is undefined behaviour.

**Handles are not thread safe.** Give each thread its own options or document
handle. The last-error slot is thread local.

**Memory never crosses the boundary in both directions.** Two shapes appear:

| Shape | Who allocated | Who frees |
|---|---|---|
| `WikitextOwnedBuffer` (returned by value) | the library | `wikitext_buffer_free` |
| `(const uint8_t *, size_t)` (passed in) | you | you, after the call returns |

`WikitextOwnedBuffer.data` is heap allocated and is never NUL terminated; it is
`NULL` when `len` is 0. A buffer the library filled in is always a *value*, so
free its data pointer and never the struct itself. That distinction is why the
struct is returned by value rather than through an out-pointer.

**`NULL` options means defaults.** Convenient when you only want HTML and have
nothing to configure.

**Buffers you pass in are borrowed for the call only.** The library copies what
it needs. Nothing retains your pointer after the call returns, so a stack buffer
is fine.

**Strings are UTF-8 and lengths are in bytes, not characters.** Invalid UTF-8
returns `WIKITEXT_ERROR_UTF8`.

## Formats

`wikitext_convert` takes `WIKITEXT_FORMAT_*` for both ends: `HTML` (0),
`MARKDOWN` (1), `WIKITEXT` (2). Only `WIKITEXT -> HTML` consults the templates
and modules registered on the options; the other directions have nothing to
expand, so `options` may be `NULL`.

## Callbacks

Three callbacks appear, all of which receive your `ctx` verbatim:

- `WikitextReaderCallback` / `WikitextWriterCallback`: streaming I/O for
  `wikitext_compile_reader`. Return the number of bytes transferred, or a
  negative value to abort with `WIKITEXT_ERROR_IO`.
- `WikitextResolveCallback`: resolves a template name. Fill the body by calling
  the `WikitextTemplateWriter` you are handed; the library owns that buffer, so
  you write into it and never allocate or free it. Return `0` on success, or `-1`
  to mean "no such template", which the caller renders as a red link.

A resolver runs on the thread that is compiling. For a language that cannot pass
a function pointer with no user-data slot, the usual trick is an exported
function plus an integer key into a table. See `bindings/go/wikitext.go` for a
worked example under cgo's pointer rules.

## Independence

Nothing here needs Rust on the far side. The C ABI is usable from C#, Java (FFM
or JNI), Ruby (fiddle), PHP (FFI) and anything else that can call a shared
library.

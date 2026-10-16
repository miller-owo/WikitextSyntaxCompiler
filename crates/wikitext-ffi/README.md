# wikitext-ffi

The C ABI for the wikitext compiler.

The crate builds a shared library and a static library that expose the compiler and
the format converter to anything that can call C. Nothing on the far side needs Rust,
which is what makes C#, Java (FFM or JNI), Ruby (fiddle) and PHP (FFI) possible, and
it is what the Go and Node.js bindings build on.

`include/wikitext.h` is the contract, and [docs/ffi.md](../../docs/ffi.md) holds the
rules a signature cannot show: the error model, the memory model, the callbacks and
the threading rule. Read those before writing a binding.

## Build

```sh
cargo build -p wikitext-ffi --release
```

`[lib]` sets `name = "wikitext"` and `crate-type = ["cdylib", "staticlib"]`, so on
Windows the output is `wikitext.dll` with a `wikitext.dll.lib` import library;
elsewhere the same names take the usual `libwikitext.so` / `libwikitext.dylib` and
`libwikitext.a` forms. Link either one, and put the shared library where the loader
finds it: next to the executable, or on `PATH` / `LD_LIBRARY_PATH` /
`DYLD_LIBRARY_PATH`.

## Use

```c
#include <stdio.h>
#include <string.h>
#include "wikitext.h"

int main(void) {
    const char *source = "Hello '''world'''";
    WikitextOwnedBuffer out = {0};

    if (wikitext_compile((const uint8_t *)source, strlen(source), NULL, &out) != WIKITEXT_OK) {
        return 1;
    }

    printf("compile: ");
    fwrite(out.data, 1, out.len, stdout);   /* not NUL terminated */
    wikitext_buffer_free(&out);
    return 0;
}
```

Compiled with `gcc -I crates/wikitext-ffi/include readme.c -o readme
target/release/wikitext.dll.lib`, the buffer holds `<p>Hello <b>world</b></p>`
followed by a newline. `NULL` options means the defaults, and `data` is never NUL
terminated, so the length is what a caller writes out.

`wikitext_convert` takes the format codes `WIKITEXT_FORMAT_HTML` (0), `MARKDOWN` (1)
and `WIKITEXT` (2) at both ends. Only wikitext to HTML consults the templates and
modules registered on the options, so `options` may be `NULL` in the other directions.

## Surface

| Group | Entry points |
|---|---|
| Version and errors | `wikitext_version`, `wikitext_last_error`, `wikitext_clear_error` |
| Memory | `wikitext_free`, `wikitext_buffer_free` |
| Options | `wikitext_options_new`, `wikitext_options_free`, the `set_*` setters, `add_template`, `add_module` |
| Compile | `wikitext_compile`, `wikitext_compile_reader`, `wikitext_expand`, `wikitext_decode_entities`, `wikitext_convert` |
| Document | `wikitext_parse`, `wikitext_document_free`, `_html`, `_render_to`, `_metadata`, `_heading`, `_category`, `_diagnostic` |

The rules that matter: no call aborts the process, a panic becomes
`WIKITEXT_ERROR_PANIC` rather than unwinding into the caller, a handle is not thread
safe, and memory never crosses the boundary in both directions.

`tests/c/smoke.c` exercises the whole surface: version, options, compile, templates,
modules, a host resolver, parse and document metadata, the conversions, and an unknown
format code. Every check in it passes.

## Related

[docs/ffi.md](../../docs/ffi.md), [docs/c.md](../../docs/c.md),
[docs/go.md](../../docs/go.md), and [bindings/go](../../bindings/go) for a worked cgo
wrapper with no third-party dependencies.

BSD-2-Clause. See [LICENSE](../../LICENSE).

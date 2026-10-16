# C integration test

Builds the shared library, links `smoke.c` against it with the system C compiler, and
runs it. This is what proves the ABI works for a consumer that shares none of Rust's
assumptions.

The test checks every call and exits non-zero on the first failure, so a non-zero
result means the ABI regressed.

This page is about the C ABI itself. For the Go binding's own test, see
[go.md](go.md).

## Windows, GNU toolchain

This is the combination the project is developed against, and the one
`bindings/go/test.ps1` uses.

```sh
rustup target add x86_64-pc-windows-gnu
cargo build -p wikitext-ffi --release --target x86_64-pc-windows-gnu

gcc -std=c11 -Wall -Wextra -Werror \
    -I crates/wikitext-ffi/include \
    -o smoke.exe crates/wikitext-ffi/tests/c/smoke.c \
    -L target/x86_64-pc-windows-gnu/release -lwikitext

PATH="$PWD/target/x86_64-pc-windows-gnu/release:$PATH" ./smoke.exe
```

## Windows, MSVC toolchain

Run this from a Developer Command Prompt, so `cl` and `link` are on `PATH`. Note the
line continuation character: `^` in a command prompt, not `\`.

```bat
cargo build -p wikitext-ffi --release
cl /I crates\wikitext-ffi\include crates\wikitext-ffi\tests\c\smoke.c ^
   /Fe:smoke.exe /link target\release\wikitext.lib
smoke.exe
```

`wikitext.dll` must be next to `smoke.exe` or on `PATH`. The MSVC build cannot be
linked by mingw gcc, and the GNU build cannot be linked by `cl`, so pick the toolchain
that matches the library you built. That mismatch is why the Go binding needs the GNU
target on Windows.

## Linux and macOS

```sh
cargo build -p wikitext-ffi --release

gcc -std=c11 -Wall -Wextra -Werror \
    -I crates/wikitext-ffi/include \
    -o smoke crates/wikitext-ffi/tests/c/smoke.c \
    -L target/release -lwikitext

LD_LIBRARY_PATH=target/release ./smoke
```

On macOS the library is `libwikitext.dylib` and the loader variable is
`DYLD_LIBRARY_PATH`.

## Reading the test

`smoke.c` is deliberately plain C: no helper library, no generated binding, nothing
that a foreign language would add. It walks the ABI the way a C# `P/Invoke` or a Java
FFM caller would, which is the point. If it passes, the boundary is sound for anyone.

The rules it exercises, and the reasons behind them, are in [ffi.md](ffi.md).

# Go binding

A cgo wrapper over the C ABI. No third-party dependencies.

## Build

The easiest path is the script, which builds the library, puts it where the test
binary can find it, and sets the cgo flags:

```sh
pwsh bindings/go/test.ps1 -Release
```

By hand, from the repository root. **On Windows the library must come from the GNU
target**, because Go links with mingw gcc and the MSVC build of `wikitext.lib`
resolves against MSVC's runtime instead:

```sh
# Windows
cargo build -p wikitext-ffi --release --target x86_64-pc-windows-gnu
cd bindings/go
CGO_CFLAGS="-I<repo>/crates/wikitext-ffi/include" \
CGO_LDFLAGS="-L<repo>/target/x86_64-pc-windows-gnu/release -lwikitext" \
go test ./...

# Linux and macOS
cargo build -p wikitext-ffi --release
cd bindings/go
CGO_CFLAGS="-I<repo>/crates/wikitext-ffi/include" \
CGO_LDFLAGS="-L<repo>/target/release -lwikitext" \
go test ./...
```

Replace `<repo>` with the absolute path to your clone. At run time the shared library
must also be findable:

| system | how |
|---|---|
| Windows | put `wikitext.dll` next to the test binary, which is what the script does, or add the release directory to `PATH` |
| Linux | `LD_LIBRARY_PATH=<repo>/target/release` |
| macOS | `DYLD_LIBRARY_PATH=<repo>/target/release` |

## Use

```go
options, err := wikitext.NewOptions()
if err != nil { return err }
defer options.Close()

options.SetTitle("Ada Lovelace")
options.AddTemplate("greet", "Hello, {{{1|world}}}!")
options.AddModule("Roman", `local p = {}
function p.convert(frame) return "MCMXCIV" end
return p`)

html, err := wikitext.Compile("{{greet|Rust}} and {{#invoke:Roman|convert|1994}}", options)
```

For a template store that lives elsewhere, install a resolver:

```go
options.SetResolver(func(name string) (string, bool) {
    body, ok := myStore[name]
    return body, ok
})
```

For a table of contents or a category bar, parse and read the metadata:

```go
document, err := wikitext.Parse(source, options)
if err != nil { return err }
defer document.Close()

metadata, err := document.Metadata()
for _, heading := range metadata.Headings {
    fmt.Println(heading.Level, heading.ID, heading.Title)
}
```

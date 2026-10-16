# wikitext-cli

The `wikitext` command line compiler.

The binary reads wikitext and writes HTML, and with `--from` and `--to` it converts
between wikitext, HTML and Markdown through
[`wikitext-convert`](../wikitext-convert). Input comes from the files named on the
command line or from standard input, and output goes to standard output.

## Build

```sh
cargo install --path crates/wikitext-cli     # installs the `wikitext` binary
cargo build --release -p wikitext-cli        # or build it in place
```

## Use

```console
$ echo "'''bold''' and {{#expr:6*7}}" | wikitext
<p><b>bold</b> and 42</p>

$ wikitext page.wikitext > out.html
$ wikitext --standalone page.wikitext            # a complete HTML document
$ wikitext --title "Some Page" page.wikitext     # for {{PAGENAME}} and friends
$ wikitext --templates ./templates page.wikitext # load *.wikitext as templates
$ wikitext --modules ./modules page.wikitext     # load *.lua, enables {{#invoke:…}}
$ wikitext --diagnostics page.wikitext           # report recovered problems

$ echo "# Title" | wikitext --from markdown
== Title ==
```

## Options

| Option | Meaning |
|---|---|
| `--from <FORMAT>` | Read the input as `html`, `markdown` or `wikitext` (default `wikitext`) |
| `--to <FORMAT>` | Write `wikitext`, `markdown` or `html` (default `html`) |
| `-T`, `--title <TITLE>` | Page title, used by `{{PAGENAME}}`, `{{FULLPAGENAME}}` and friends |
| `--wrap` | Wrap the output in `<div class="mw-parser-output">` |
| `--standalone` | Emit a complete HTML document |
| `--fragment-div` | The same parser output mode as `--wrap` |
| `-t`, `--templates <DIR>` | Every `*.wikitext`, `*.wiki`, `*.txt` or `*.mediawiki` file below DIR, named by its path |
| `-m`, `--modules <DIR>` | Every `*.lua` file below DIR, named by its path with `/` separators |
| `--diagnostics` | Print recovered problems to standard error |
| `--compact` | Disable pretty printing |
| `--language <LANGUAGE>` | Content language, reported by `{{CONTENTLANGUAGE}}` (default `en`) |
| `-h`, `--help`, `-V`, `--version` | Help and version |

## Notes

- Giving either `--from` or `--to` selects conversion mode, and the two directions
  have their own defaults: `--from` is `wikitext` and `--to` is `wikitext` there, not
  `html`. `--diagnostics` is read only by the compile path.
- Several files are compiled in sequence, each written after the last. In conversion
  mode the files are read as one document.
- A template or module directory is searched recursively, so a subdirectory becomes
  part of the name.
- The exit status is non-zero and a message goes to standard error when a file cannot
  be read, a directory cannot be listed, or the output cannot be written.

## Related

- [Root README](../../README.md): install, capabilities, syntax support
- [docs](../../docs/README.md): the guides index
- [docs/ffi.md](../../docs/ffi.md): the same conversion in a C ABI

BSD-2-Clause. See [LICENSE](../../LICENSE).

# wikitext-convert

Converts between wikitext, HTML and Markdown.

There are three front ends and three writers with one shared markup tree in the
middle, so a construct is mapped once per format rather than once per pair. HTML is
parsed with `html5ever`, Markdown with an in-repo CommonMark subset, and wikitext with
[`crates/wikitext`](../wikitext). The output is wikitext a person would write, such
as `'''` rather than `<b>`, judged by semantic equivalence once compiled back to HTML.

## Install

```sh
cargo add wikitext-convert
```

## Use

```rust
use wikitext_convert::{markdown_to_wikitext, wikitext_to_markdown};

assert_eq!(
    wikitext_to_markdown("== Head ==\n\nSome '''bold''' text."),
    "## Head\n\nSome **bold** text."
);
assert_eq!(
    markdown_to_wikitext("# Head\n\nSome **bold** text."),
    "== Head ==\n\nSome '''bold''' text."
);
```

`html_to_wikitext`, `html_to_markdown` and `markdown_to_html` are the same shape:

```rust
use wikitext_convert::{html_to_markdown, html_to_wikitext, markdown_to_html};

assert_eq!(
    html_to_wikitext("<h2>Head</h2><p>Some <b>bold</b> text.</p>"),
    "== Head ==\n\nSome '''bold''' text."
);
assert_eq!(
    html_to_markdown("<h2>Head</h2><p>Some <b>bold</b> text.</p>"),
    "## Head\n\nSome **bold** text."
);
assert_eq!(
    markdown_to_html("# Head\n\nSome **bold** text."),
    "<h2 id=\"Head\">Head</h2>\n<p>Some <b>bold</b> text.</p>\n"
);
```

To hold on to the tree and render it more than once, use `to_blocks` and `render`.
`convert` is the two together, for a caller that only wants the string:

```rust
use wikitext_convert::{convert, render, to_blocks, Options, Output};

let blocks = to_blocks("== Head ==\n\nText with a [[Link|label]].", &Options::wikitext());
assert_eq!(render(&blocks, Output::Wikitext), "== Head ==\n\nText with a [[Link|label]].");
assert_eq!(render(&blocks, Output::Markdown), "## Head\n\nText with a [label](Link).");

assert_eq!(convert("# Title", &Options::markdown(), Output::Wikitext), "== Title ==");
```

## API summary

`Input` and `Output` name the source and the target format. `Options` carries only the
input format, built by `Options::html()`, `Options::markdown()` or
`Options::wikitext()`. `to_blocks` parses into `Vec<Block>`, `render` writes a tree in
one format, and `convert` does both. `to_wikitext` and `to_markdown` are `convert`
with the output fixed.

Mapped constructs: headings, paragraphs, emphasis and nesting, inline code and fenced
code blocks (a language becomes `<syntaxhighlight>`), lists with arbitrary nesting and
mixing, block quotes, thematic breaks, links, images, tables (written as a
`wikitable`) and block and inline HTML tags.

There is no `wikitext_to_html`. Wikitext to HTML is the compiler's own job, so use
`wikitext::compile`, or `convert(source, &Options::wikitext(), Output::Html)` to go
through the shared tree.

**Known limits:** HTML attributes other than `href`, `src`, `alt` and `class` are not
preserved, `rowspan` and `colspan` are not mapped, and HTML with no clean wikitext
counterpart is kept as a literal tag rather than dropped. Markdown footnote and
definition-list syntax is not parsed; it is read as embedded HTML.

## Related

The [root README](../../README.md) covers conversion in full; [crates/wikitext](../wikitext) is the compiler behind it.

BSD-2-Clause. See [LICENSE](../../LICENSE).

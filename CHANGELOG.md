# Changelog

Notable changes to this project. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing yet.

## [0.1.0]

The first version. Everything below is new.

### Added

- **Compiler.** Wikitext to an HTML5 fragment: paragraphs, headings, lists,
  definition lists, tables, horizontal rules, preformatted blocks, the quote state
  machine for bold and italic, HTML tags and attributes, character entities including
  the Windows-1252 C1 remapping, `<nowiki>`, `<pre>` and comments.
- **Links.** Internal links with labels and the pipe trick, category collection,
  images with captions, and external links.
- **Parser functions.** `#if`, `#ifeq`, `#ifexist`, `#iferror`, `#ifexpr`, `#switch`,
  `#expr` with a full expression evaluator, `#time`, `#timel`, `#titleparts`,
  `#rel2abs`, `#tag`, and the string functions `#len`, `#pos`, `#rpos`, `#sub`,
  `#replace`, `#explode`, `#urldecode`, `#urlencode`, `#anchorencode`, `#padleft`,
  `#padright`, `#formatnum`, `#lc`, `#uc`, `#lcfirst`, `#ucfirst`, `#format`.
- **Magic words.** `PAGENAME`, `FULLPAGENAME`, `BASEPAGENAME`, `SUBPAGENAME`,
  `ROOTPAGENAME`, `NAMESPACE`, `NAMESPACENUMBER`, `TALKSPACE`, `SERVER`, `SITENAME`,
  the `CURRENT*` and `LOCAL*` families, `!`, `=`, `{{DEFAULTSORT:…}}` and
  `{{DISPLAYTITLE:…}}`.
- **Templates.** A `TemplateResolver` trait, called once per transclusion, with an
  in-memory store included. Bounded expansion depth, count and size.
- **Lua modules.** `{{#invoke:…}}` on a Lua 5.1 interpreter written from scratch:
  closures, metatables and metamethods, varargs, multiple returns, and the full Lua
  pattern matcher. An `mw` surface covering `text`, `ustring`, `title`, `uri`, `html`,
  `language` and `dumpObject`. Bounded call depth, loop iterations and instructions.
- **Extension tags.** `<ref>` with backlinks and groups, `<references/>`, `<poem>`,
  `<gallery>`, `<syntaxhighlight>`, `<source>`, and `data-mw-ext` markers for
  `<math>`, `<chem>` and `<ce>`.
- **Format conversion.** Wikitext, HTML and Markdown, through one shared markup tree,
  with `html5ever` for HTML and an in-repo CommonMark subset for Markdown.
- **Command line.** A `wikitext` binary with format selection, template and module
  directories, output modes, diagnostics, and language selection.
- **Bindings.** A C ABI with a tracked header and a C smoke test, a Go binding over
  cgo, a Node.js addon built on napi-rs, and WebAssembly bindings with a JavaScript
  wrapper. Format codes are identical across all of them.
- **Diagnostics.** Malformed input degrades to literal text and becomes a
  `Diagnostic`, never a hard failure.

### Notes

- Output is shaped like MediaWiki's but is not guaranteed to match byte for byte.
- Date and time magic words return the Unix epoch, because the core crate takes no
  clock dependency.
- Nesting is capped at 40 levels and template expansion at 40 deep, 10 000
  expansions and 2 MB, matching MediaWiki's order of magnitude.
- `mw.loadData` is not supported, and `mw.site` and `mw.message` are empty, because
  there is no data source behind them here.

[Unreleased]: https://github.com/miller-owo/WikitextSyntaxCompiler/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/miller-owo/WikitextSyntaxCompiler/releases/tag/v0.1.0

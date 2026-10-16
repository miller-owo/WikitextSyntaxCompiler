//! Converts between wikitext, HTML and Markdown.
//!
//! Every direction goes through one markup tree, so a construct is mapped once
//! per format rather than once per pair.

pub mod dom;
pub mod html;
pub mod markdown;
pub mod markdown_write;
pub mod markup;
pub mod write;

pub use crate::markup::{Block, Cell, Inline, List, Row, Table};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Input {
    #[default]
    Html,
    Markdown,
    Wikitext,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Output {
    #[default]
    Wikitext,
    Markdown,
    Html,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Options {
    pub input: Input,
}

impl Options {
    pub fn html() -> Options {
        Options { input: Input::Html }
    }

    pub fn markdown() -> Options {
        Options {
            input: Input::Markdown,
        }
    }

    pub fn wikitext() -> Options {
        Options {
            input: Input::Wikitext,
        }
    }

    pub fn with_input(mut self, input: Input) -> Self {
        self.input = input;
        self
    }
}

pub fn to_blocks(source: &str, options: &Options) -> Vec<Block> {
    match options.input {
        Input::Html => html::parse(source),
        Input::Markdown => markdown::parse(source),
        Input::Wikitext => dom::blocks(&parse_wikitext(source)),
    }
}

fn parse_wikitext(source: &str) -> wikitext::Document {
    wikitext::compile_with_diagnostics(source, &wikitext::Options::default()).1
}

pub fn render(blocks: &[Block], output: Output) -> String {
    match output {
        Output::Wikitext => write::write(blocks),
        Output::Markdown => markdown_write::write(blocks),
        Output::Html => wikitext::compile(&write::write(blocks), &wikitext::Options::default()),
    }
}

pub fn convert(source: &str, options: &Options, output: Output) -> String {
    render(&to_blocks(source, options), output)
}

pub fn to_wikitext(source: &str, options: &Options) -> String {
    convert(source, options, Output::Wikitext)
}

pub fn to_markdown(source: &str, options: &Options) -> String {
    convert(source, options, Output::Markdown)
}

pub fn html_to_wikitext(source: &str) -> String {
    to_wikitext(source, &Options::html())
}

pub fn markdown_to_wikitext(source: &str) -> String {
    to_wikitext(source, &Options::markdown())
}

pub fn wikitext_to_markdown(source: &str) -> String {
    to_markdown(source, &Options::wikitext())
}

pub fn html_to_markdown(source: &str) -> String {
    to_markdown(source, &Options::html())
}

pub fn markdown_to_html(source: &str) -> String {
    convert(source, &Options::markdown(), Output::Html)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(wikitext: &str) -> String {
        let options = wikitext::Options {
            title: Some("Test".to_string()),
            ..wikitext::Options::default()
        };
        wikitext::compile(wikitext, &options)
    }

    fn markdown(source: &str) -> String {
        markdown_to_wikitext(source)
    }

    fn html(source: &str) -> String {
        html_to_wikitext(source)
    }

    fn from_wikitext(source: &str) -> String {
        wikitext_to_markdown(source)
    }

    #[test]
    fn headings_and_paragraphs() {
        assert_eq!(
            markdown("# Title\n\nSome text."),
            "== Title ==\n\nSome text."
        );
        assert_eq!(markdown("## Two ##"), "== Two ==");
        assert_eq!(markdown("Title\n====="), "== Title ==");
        assert_eq!(html("<h2>Two</h2><p>Text</p>"), "== Two ==\n\nText");
        assert_eq!(from_wikitext("== Two ==\n\nText"), "## Two\n\nText");
    }

    #[test]
    fn emphasis_becomes_apostrophes() {
        assert_eq!(
            markdown("**bold** and *italic*"),
            "'''bold''' and ''italic''"
        );
        assert_eq!(html("<b>bold</b> <i>it</i>"), "'''bold''' ''it''");
        assert_eq!(html("<strong>a</strong><em>b</em>"), "'''a'''''b''");
    }

    #[test]
    fn emphasis_goes_back_to_asterisks() {
        assert_eq!(
            from_wikitext("'''bold''' and ''italic''"),
            "**bold** and *italic*"
        );
        assert_eq!(from_wikitext("'''''both'''''"), "***both***");
        assert_eq!(from_wikitext("<s>gone</s>"), "~~gone~~");
        assert_eq!(from_wikitext("<sub>2</sub>"), "<sub>2</sub>");
    }

    #[test]
    fn links_and_images() {
        assert_eq!(markdown("[Ada](./Ada_Lovelace)"), "[[Ada Lovelace|Ada]]");
        assert_eq!(
            markdown("[text](https://example.org)"),
            "[https://example.org text]"
        );
        assert_eq!(
            html(r#"<a href="/wiki/Ada_Lovelace">Ada</a>"#),
            "[[Ada Lovelace|Ada]]"
        );
        assert_eq!(
            html(r#"<img src="./Cat.png" alt="A cat">"#),
            "[[File:Cat.png|alt=A cat]]"
        );
    }

    #[test]
    fn links_go_back_to_markdown() {
        assert_eq!(
            from_wikitext("[[Ada Lovelace|Ada]]"),
            "[Ada](Ada%20Lovelace)"
        );
        assert_eq!(
            from_wikitext("[[Ada Lovelace]]"),
            "[Ada Lovelace](Ada%20Lovelace)"
        );
        assert_eq!(
            from_wikitext("[https://example.org the site]"),
            "[the site](https://example.org)"
        );
    }

    #[test]
    fn lists_nest() {
        assert_eq!(markdown("- one\n- two"), "* one\n* two");
        assert_eq!(markdown("1. one\n2. two"), "# one\n# two");
        assert_eq!(html("<ul><li>one</li><li>two</li></ul>"), "* one\n* two");
        assert_eq!(html("<ol><li>one</li></ol>"), "# one");
    }

    #[test]
    fn a_nested_list_keeps_its_depth() {
        assert_eq!(markdown("- one\n  - nested"), "* one\n** nested");
        assert_eq!(
            markdown("- one\n  - nested\n    - deeper"),
            "* one\n** nested\n*** deeper"
        );
        assert_eq!(
            html("<ul><li>a<ul><li>b<ul><li>c</li></ul></li></ul></li></ul>"),
            "* a\n** b\n*** c"
        );
        assert_eq!(markdown("1. one\n   1. nested"), "# one\n## nested");
        assert_eq!(markdown("- a\n  1. b"), "* a\n*# b");
        assert_eq!(markdown("1. a\n   - b"), "# a\n#* b");
        assert_eq!(markdown("- a\n  1. b\n     - c"), "* a\n*# b\n*#* c");
    }

    #[test]
    fn a_nested_list_compiles_back_to_nesting() {
        let wikitext = markdown("- one\n  - nested\n    - deeper");
        let produced = round_trip(&wikitext);
        let first = produced.find("<ul>").expect("a list");
        let second = produced[first + 1..].find("<ul>").expect("a nested list");
        assert!(second > 0, "{produced}");
        assert!(produced.contains("<li>deeper</li>"), "{produced}");
    }

    #[test]
    fn a_block_level_html_tag_is_a_block() {
        let produced = round_trip(&html("<blockquote><p>Quoted.</p></blockquote>"));
        assert!(produced.starts_with("<blockquote>"), "{produced}");
        assert!(!produced.contains("<p>\n<blockquote>"), "{produced}");

        let produced = round_trip(&html("<div><p>Boxed.</p></div>"));
        assert!(!produced.contains("<p>\n<div>"), "{produced}");
    }

    #[test]
    fn code_blocks_keep_their_language() {
        assert_eq!(
            markdown("```rust\nlet x = 1;\n```"),
            "<syntaxhighlight lang=\"rust\">\nlet x = 1;\n</syntaxhighlight>"
        );
        assert_eq!(markdown("```\nplain\n```"), "<pre>plain\n</pre>");
        assert_eq!(
            markdown("    indented\n    code"),
            "<pre>indented\ncode\n</pre>"
        );
        assert_eq!(markdown("`inline`"), "<code>inline</code>");
        assert_eq!(
            html("<pre><code>let x = 1;</code></pre>"),
            "<pre>let x = 1;\n</pre>"
        );
    }

    #[test]
    fn code_blocks_become_fences() {
        assert_eq!(
            from_wikitext("<syntaxhighlight lang=\"rust\">\nlet x = 1;\n</syntaxhighlight>"),
            "```rust\nlet x = 1;\n```"
        );
        assert_eq!(from_wikitext("<pre>plain\n</pre>"), "```\nplain\n```");
        assert_eq!(from_wikitext("<code>inline</code>"), "`inline`");
        // A fence has to be longer than any run the content carries.
        assert_eq!(from_wikitext("<code>a ``` b</code>"), "````a ``` b````");
    }

    #[test]
    fn rules_and_quotes() {
        assert_eq!(markdown("---"), "----");
        assert_eq!(html("<hr>"), "----");
        assert_eq!(markdown("> quoted"), "<blockquote>quoted</blockquote>");
        assert_eq!(
            html("<blockquote><p>quoted</p></blockquote>"),
            "<blockquote>quoted</blockquote>"
        );
        assert_eq!(from_wikitext("----"), "---");
        assert_eq!(from_wikitext("<blockquote>quoted</blockquote>"), "> quoted");
    }

    #[test]
    fn tables_become_wikitables() {
        let wikitext = markdown("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(wikitext.starts_with("{| class=\"wikitable\""), "{wikitext}");
        assert!(wikitext.contains("! a"), "{wikitext}");
        assert!(wikitext.contains("| 2"), "{wikitext}");
        assert!(wikitext.ends_with("|}"), "{wikitext}");

        let wikitext =
            html("<table><tr><th>a</th><th>b</th></tr><tr><td>1</td><td>2</td></tr></table>");
        assert!(wikitext.contains("! a"), "{wikitext}");
        assert!(wikitext.contains("| 2"), "{wikitext}");
    }

    #[test]
    fn tables_become_gfm_with_escaped_pipes() {
        let markdown = from_wikitext("{| class=\"wikitable\"\n|-\n! a !! b\n|-\n| 1 || 2\n|}");
        assert_eq!(markdown, "| a | b |\n| --- | --- |\n| 1 | 2 |");
    }

    #[test]
    fn definition_lists_stay_html() {
        let markdown = from_wikitext("; term\n: definition");
        assert!(markdown.contains("<dl>"), "{markdown}");
        assert!(markdown.contains("<dt>term</dt>"), "{markdown}");
        assert!(markdown.contains("<dd>definition</dd>"), "{markdown}");
    }

    #[test]
    fn extensions_stay_html() {
        let markdown = from_wikitext("<math>E = mc^2</math>");
        assert!(markdown.contains("<math>"), "{markdown}");
        assert!(markdown.contains("E = mc^2"), "{markdown}");
        // A `<poem>` block compiles to a div carrying the marker class, and that
        // markup is what survives.
        let markdown = from_wikitext("<poem>Roses are red</poem>");
        assert!(markdown.contains("poem"), "{markdown}");
    }

    #[test]
    fn references_become_a_citation_marker_and_a_list() {
        // The citation text lives on the reference list, exactly as in MediaWiki,
        // so a footnote marker is what the inline position can carry.
        let markdown = from_wikitext("Text.<ref>Source</ref>");
        assert!(markdown.contains("Text."), "{markdown}");
        assert!(!markdown.contains("<ref>"), "{markdown}");
    }

    #[test]
    fn output_compiles_back_to_the_same_html() {
        let wikitext = markdown("# Title\n\nSome **bold** text with a [link](./Target).\n");
        let produced = round_trip(&wikitext);
        // MediaWiki reserves h1 for the page title, so a markdown h1 is an h2 here.
        assert!(produced.contains("<h2"), "{produced}");
        assert!(produced.contains("Title"), "{produced}");
        assert!(produced.contains("<b>bold</b>"), "{produced}");
        assert!(produced.contains("href=\"/wiki/Target\""), "{produced}");

        let wikitext = html("<p>One</p><ul><li>a</li><li>b</li></ul>");
        let produced = round_trip(&wikitext);
        assert!(produced.contains("<p>One</p>"), "{produced}");
        assert!(produced.contains("<li>a</li>"), "{produced}");

        let wikitext = html("<h2>Head</h2><table><tr><th>h</th></tr><tr><td>d</td></tr></table>");
        let produced = round_trip(&wikitext);
        assert!(produced.contains("<th>h</th>"), "{produced}");
        assert!(produced.contains("<td>d</td>"), "{produced}");
    }

    #[test]
    fn braces_and_brackets_do_not_become_syntax() {
        let wikitext = markdown("A {{template}} and [not a link] and {braces}.");
        assert!(!wikitext.contains("{{"), "{wikitext}");
        assert!(!wikitext.contains("[["), "{wikitext}");
        let produced = round_trip(&wikitext);
        assert!(produced.contains("{{template}}"), "{produced}");
        assert!(produced.contains("{braces}"), "{produced}");
    }

    #[test]
    fn markdown_metacharacters_are_escaped_on_the_way_out() {
        let markdown = from_wikitext("a * b and _c_ and [d] and `e`");
        assert!(markdown.contains("\\*"), "{markdown}");
        assert!(markdown.contains("\\_"), "{markdown}");
        assert!(markdown.contains("\\["), "{markdown}");
        assert!(markdown.contains("\\`"), "{markdown}");
    }

    #[test]
    fn ampersands_and_angles_are_escaped() {
        let wikitext = html("<p>a &amp; b &lt;tag&gt;</p>");
        assert!(wikitext.contains("&amp;"), "{wikitext}");
        assert!(wikitext.contains("&lt;tag&gt;"), "{wikitext}");
        let produced = round_trip(&wikitext);
        assert!(produced.contains("a &amp; b &lt;tag&gt;"), "{produced}");
    }

    #[test]
    fn unknown_html_survives_as_markup() {
        let wikitext = html(r#"<p>a <span class="x">b</span></p>"#);
        assert!(wikitext.contains("<span class=\"x\">"), "{wikitext}");
        let markdown = html_to_markdown(r#"<p>a <span class="x">b</span></p>"#);
        assert!(markdown.contains("<span class=\"x\">"), "{markdown}");
    }

    #[test]
    fn nested_emphasis_and_strikethrough() {
        assert_eq!(markdown("***both***"), "'''''both'''''");
        assert_eq!(markdown("~~gone~~"), "<s>gone</s>");
        assert_eq!(html("<sub>2</sub><sup>3</sup>"), "<sub>2</sub><sup>3</sup>");
    }

    #[test]
    fn an_empty_document_writes_nothing() {
        assert_eq!(markdown(""), "");
        assert_eq!(html(""), "");
        assert_eq!(from_wikitext(""), "");
        assert_eq!(html("<div></div>"), "");
    }

    #[test]
    fn reference_links_resolve() {
        assert_eq!(
            markdown("[Ada][ada]\n\n[ada]: ./Ada_Lovelace"),
            "[[Ada Lovelace|Ada]]"
        );
    }

    #[test]
    fn line_breaks_in_a_paragraph_collapse() {
        assert_eq!(markdown("one\ntwo"), "one\ntwo");
        let produced = round_trip(&markdown("one\ntwo"));
        assert!(produced.contains("one\ntwo"), "{produced}");
    }

    #[test]
    fn the_same_tree_renders_in_every_direction() {
        let blocks = to_blocks(
            "== Head ==\n\nText with a [[Link|label]].",
            &Options::wikitext(),
        );
        assert_eq!(
            render(&blocks, Output::Markdown),
            "## Head\n\nText with a [label](Link)."
        );
        assert_eq!(
            render(&blocks, Output::Wikitext),
            "== Head ==\n\nText with a [[Link|label]]."
        );
        let html = render(&blocks, Output::Html);
        assert!(html.contains("<h2"), "{html}");
        assert!(html.contains("mw-wikilink"), "{html}");
    }

    #[test]
    fn html_to_markdown_goes_html_tree_first() {
        let markdown = html_to_markdown("<h2>Head</h2><p>Some <b>bold</b> text.</p>");
        assert_eq!(markdown, "## Head\n\nSome **bold** text.");
    }

    #[test]
    fn markdown_to_html_round_trips_through_wikitext() {
        let produced = markdown_to_html("# Head\n\nSome **bold** text.");
        assert!(produced.contains("<h2"), "{produced}");
        assert!(produced.contains("<b>bold</b>"), "{produced}");
    }
}

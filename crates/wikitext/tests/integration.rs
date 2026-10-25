use std::sync::Arc;
use wikitext::preprocess::MemoryTemplates;
use wikitext::{compile, compile_with_diagnostics, Options, OutputMode};
fn html(source: &str) -> String {
    compile(source, &Options::default())
}

fn html_with_templates(source: &str, templates: &[(&str, &str)]) -> String {
    let store = MemoryTemplates::new();
    store.extend(templates.iter().copied());
    compile(source, &Options::default().with_resolver(Arc::new(store)))
}

#[test]
fn compiles_a_small_article() {
    let source = "\
'''Ada Lovelace''' was an English ''mathematician''.

== Work ==

She wrote the first [[algorithm]] for the [[Analytical Engine]].

* Analytical Engine
* Note G

[[Category:Mathematicians]]
";
    let options = Options::default();
    let (output, document) = compile_with_diagnostics(source, &options);
    assert!(output.contains("<b>Ada Lovelace</b>"), "{output}");
    assert!(output.contains("<i>mathematician</i>"), "{output}");
    assert!(output.contains("<h2 id=\"Work\">Work</h2>"), "{output}");
    assert!(output.contains("href=\"/wiki/algorithm\""), "{output}");
    assert!(output.contains("<ul>"), "{output}");

    assert_eq!(document.categories.len(), 1);
    assert_eq!(document.categories[0].name, "Mathematicians");
    assert!(!output.contains("Category:"), "{output}");
}

#[test]
fn templates_expand_through_the_public_api() {
    let output = html_with_templates(
        "{{greet|Rust}} and {{#expr:6*7}}",
        &[("greet", "Hello, {{{1|world}}}!")],
    );
    assert_eq!(output.trim(), "<p>Hello, Rust! and 42</p>");
}

#[test]
fn missing_templates_render_as_red_links() {
    let output = html("{{does not exist}}");
    assert!(output.contains("Does not exist"), "{output}");
}

#[test]
fn references_are_collected_and_listed() {
    let source = "\
Text.<ref name=\"a\">''Source'', p. 3.</ref> More.<ref name=\"a\" /> Later.<ref>Other.</ref>

== Notes ==

<references/>
";
    let output = html(source);

    assert_eq!(
        output.matches("class=\"reference-text\"").count(),
        2,
        "{output}"
    );
    assert!(output.contains("id=\"cite_note-a\""), "{output}");
    assert!(output.contains("id=\"cite_note-2\""), "{output}");

    assert!(output.contains("\u{2191}2"), "{output}");

    assert!(output.contains("<i>Source</i>"), "{output}");
}

#[test]
fn reference_groups_get_their_own_list() {
    let output = html("A.<ref group=\"note\">N</ref>\n\n<references group=\"note\"/>");
    assert!(output.contains("mw-references-columns-note"), "{output}");
    assert!(output.contains("cite_note-note-1"), "{output}");
}

#[test]
fn empty_references_renders_nothing() {
    let output = html("No citations here.\n\n<references/>");
    assert!(!output.contains("references"), "{output}");
}

#[test]
fn poem_keeps_its_line_breaks() {
    let output = html("<poem>\nRoses are red\nViolets are blue\n</poem>");
    assert!(output.contains("class=\"poem\""), "{output}");
    assert!(output.contains("Roses are red<br>"), "{output}");
    assert!(
        !output.contains("</poem>"),
        "the tag must be consumed: {output}"
    );
}

#[test]
fn syntax_highlight_becomes_a_classed_pre() {
    let output = html("<syntaxhighlight lang=\"rust\">\nfn main() {}\n</syntaxhighlight>");
    assert!(output.contains("mw-highlight-lang-rust"), "{output}");
    assert!(output.contains("fn main() {}"), "{output}");
    assert!(!output.contains("syntaxhighlight"), "{output}");
}

#[test]
fn gallery_lists_its_files() {
    let output = html("<gallery>\nFile:A.png|First\nFile:B.png|Second\n</gallery>");
    assert!(output.contains("alt=\"First\""), "{output}");
    assert!(output.contains("gallerytext"), "{output}");
    assert!(!output.contains("</gallery>"), "{output}");
}

#[test]
fn noinclude_text_is_shown_when_the_page_is_read() {
    assert_eq!(
        html("keep<noinclude>docs</noinclude>").trim(),
        "<p>keepdocs</p>"
    );
}

#[test]
fn includeonly_text_is_only_transcluded() {
    let template = "<noinclude>docs</noinclude><includeonly>body</includeonly>";
    let transcluded = html_with_templates("{{card}}", &[("card", template)]);
    assert_eq!(transcluded.trim(), "<p>body</p>");

    let page = html(template);
    assert_eq!(page.trim(), "<p>docs</p>");
}

#[test]
fn a_block_level_wrapper_may_span_lines() {
    let output = html("<div style=\"border:1px\">\ntext\n</div>");
    assert!(output.contains("<div style=\"border:1px\">"), "{output}");
    assert!(output.contains("<p>text</p>"), "{output}");
    assert!(!output.contains("&lt;div"), "{output}");
    assert!(!output.contains("&lt;/div&gt;"), "{output}");
}

#[test]
fn a_wrapper_left_open_on_a_line_stays_markup() {
    let output = html("<div class=\"outer\"><div class=\"inner\">x</div>");
    assert!(output.contains("<div class=\"outer\">"), "{output}");
    assert!(!output.contains("&lt;div"), "{output}");
}

#[test]
fn a_block_wrapper_may_close_after_text() {
    // Templates write `{{{caption|…}}}</div>`: the closer follows text, so the
    // line-local builder sees no opener and used to escape it.
    let output = html("<div class=\"box\">\ncaption</div>\nafter");
    assert!(output.contains("<div class=\"box\">"), "{output}");
    assert!(!output.contains("&lt;/div"), "{output}");
    let close = output.find("</div>").expect("a real closing tag");
    let after = output.find("after").expect("the following line");
    assert!(close < after, "{output}");
    assert_eq!(
        output.matches("<div").count(),
        output.matches("</div>").count(),
        "{output}"
    );
}

#[test]
fn nested_wrappers_close_after_their_text() {
    let source = "<div class=\"outer\">\n<div class=\"inner\">\ntext</div>\n<div class=\"rest\">\nmore\n</div></div>";
    let output = html(source);
    assert!(!output.contains("&lt;/div"), "{output}");
    assert_eq!(
        output.matches("<div").count(),
        output.matches("</div>").count(),
        "{output}"
    );
    let rest = output.find("<div class=\"rest\">").expect("rest wrapper");
    let more = output.find("more").expect("rest text");
    assert!(rest < more, "{output}");
}

#[test]
fn a_right_aligned_image_floats_inside_its_wrapper() {
    let output = html("<div class=\"box\">\n[[File:A.png|right|200px|link=]]\nText\n</div>");
    assert!(
        output.contains("<img src=\"/images/A.png\" alt=\"\" width=\"200\" class=\"floatright\">"),
        "{output}"
    );
    // A picture alone on its line is a block, not a paragraph.
    assert!(!output.contains("<p><img"), "{output}");
    let image = output.find("<img").expect("image");
    let text = output.find("<p>Text").expect("text");
    assert!(image < text, "{output}");
}

#[test]
fn deeply_nested_include_markers_keep_their_content() {
    // The marker scan keeps a suppression counter rather than testing its open
    // stack for every marker; nesting depth must not change the result.
    let depth = 2000;
    let kept = format!(
        "{}keep{}",
        "<noinclude>".repeat(depth),
        "</noinclude>".repeat(depth)
    );
    let page = html(&kept);
    assert!(page.contains("keep"), "{page}");
    assert!(!page.contains("noinclude"), "{page}");

    let hidden = format!(
        "{}hidden{}",
        "<includeonly>".repeat(depth),
        "</includeonly>".repeat(depth)
    );
    assert!(!html(&hidden).contains("hidden"));
}

#[test]
fn an_image_link_parameter_controls_the_anchor() {
    let plain = html("[[File:A.png]]");
    assert!(
        plain.contains("<a class=\"image\" href=\"/wiki/File%3AA.png\">"),
        "{plain}"
    );

    let unlinked = html("[[File:A.png|180px|link=]]");
    assert!(
        unlinked.contains("<img src=\"/images/A.png\""),
        "{unlinked}"
    );
    assert!(!unlinked.contains("<a"), "{unlinked}");

    let linked = html("[[File:A.png|link=Main Page]]");
    assert!(linked.contains("href=\"/wiki/Main_Page\""), "{linked}");
}

#[test]
fn nowiki_still_escapes_markup() {
    let output = html("<nowiki>'''not bold'''</nowiki>");
    assert!(output.contains("'''not bold'''"), "{output}");
    assert!(!output.contains("<b>"), "{output}");
}

#[test]
fn tables_lists_and_headings_keep_their_structure() {
    let source = "\
== A ==

* one
* two
** nested

{|
|-
! H !! H2
|-
| 1 || 2
|}
";
    let output = html(source);
    assert!(output.contains("<h2 id=\"A\">A</h2>"), "{output}");
    assert!(output.contains("<li>one</li>"), "{output}");
    assert!(output.contains("<th>H</th><th>H2</th>"), "{output}");
}

#[test]
fn output_modes_wrap_the_fragment() {
    let options = Options::default().with_output_mode(OutputMode::ParserOutput);
    let output = compile("text", &options);
    assert!(
        output.starts_with("<div class=\"mw-parser-output\">"),
        "{output}"
    );
    assert!(output.trim_end().ends_with("</div>"), "{output}");

    let options = Options::default().with_output_mode(OutputMode::Document);
    let output = compile("text", &options);
    assert!(output.starts_with("<!DOCTYPE html>"), "{output}");
    assert!(output.contains("<meta charset=\"UTF-8\">"), "{output}");
}

#[test]
fn the_pretty_option_controls_block_newlines() {
    let pretty = compile("a\n\nb", &Options::default());
    assert!(
        pretty.contains("\n"),
        "pretty output separates blocks: {pretty:?}"
    );

    let compact = compile(
        "a\n\nb",
        &Options {
            pretty: false,
            ..Options::default()
        },
    );
    assert_eq!(compact, "<p>a</p><p>b</p>");
}

#[test]
fn display_title_and_default_sort_are_recorded() {
    let (_, document) = compile_with_diagnostics(
        "{{DISPLAYTITLE:Custom}} {{DEFAULTSORT:Key}} text",
        &Options::default(),
    );
    assert_eq!(document.display_title.as_deref(), Some("Custom"));
    assert_eq!(document.default_sort.as_deref(), Some("Key"));
}

#[test]
fn unmatched_markup_degrades_to_text() {
    assert!(html("''unclosed italic").contains("''unclosed italic"));
    assert!(html("<b>unclosed bold").contains("&lt;b&gt;unclosed bold"));
    assert!(html("[[unclosed link").contains("[[unclosed link"));
    assert!(html("{| unclosed table").contains("table"));
}

#[test]
fn mixed_end_to_end_document_is_stable() {
    let source = "\
{{stub}}
'''Title'''<ref name=\"r\">Ref.</ref>

== Section ==

: indented definition
; term : definition

<poem>
line one
line two
</poem>

<references/>
";
    let output = html_with_templates(source, &[("stub", "''This is a stub.''")]);
    assert!(output.contains("<i>This is a stub.</i>"), "{output}");
    assert!(output.contains("<dl>"), "{output}");
    assert!(output.contains("class=\"poem\""), "{output}");
    assert!(output.contains("class=\"references\""), "{output}");

    assert!(!output.contains("</poem>"), "{output}");
    assert!(!output.contains("</references>"), "{output}");
    assert!(!output.contains("{{"), "{output}");
}

// The cases below were each an actual failure: a hang, a quadratic blow-up, or a
// stack overflow. They are kept as tests because nothing about the shapes looks
// unusual, so a regression would not be noticed by reading the diff.

fn compile_time(source: &str) -> std::time::Duration {
    let options = Options {
        title: Some("Robustness".to_string()),
        ..Options::default()
    };
    let started = std::time::Instant::now();
    let html = compile(source, &options);
    let elapsed = started.elapsed();
    std::hint::black_box(&html);
    elapsed
}

#[test]
fn a_page_of_unclosed_templates_stays_fast() {
    // 8 KB of `{{` took 8.5 seconds before the opener-run skip.
    let source = "{{".repeat(4_000);
    assert!(
        compile_time(&source) < std::time::Duration::from_millis(500),
        "unclosed templates are superlinear again"
    );
}

#[test]
fn a_page_of_unclosed_links_stays_fast() {
    let source = "[[a|".repeat(16_000);
    assert!(
        compile_time(&source) < std::time::Duration::from_millis(500),
        "unclosed links are superlinear again"
    );
}

#[test]
fn a_page_of_unterminated_tags_stays_fast() {
    // `<div` with no `>` scanned the whole tail for every tag, and took 3.1
    // seconds on 128 KB.
    let source = "<div".repeat(32_000);
    assert!(
        compile_time(&source) < std::time::Duration::from_secs(2),
        "unterminated tags are superlinear again"
    );
}

#[test]
fn deeply_nested_links_do_not_overflow_the_stack() {
    // 800 nested links is 1.6 KB of source and used to abort the process.
    let depth = 2_000;
    let source = format!("{}{}{}", "[[".repeat(depth), "x", "]]".repeat(depth));
    let html = compile(&source, &Options::default());
    assert!(!html.is_empty());
}

#[test]
fn deeply_nested_tags_do_not_overflow_the_stack() {
    // Dropping and serializing the tree both recursed once per level.
    let depth = 20_000;
    let source = format!("{}{}{}", "<div>".repeat(depth), "x", "</div>".repeat(depth));
    let html = compile(&source, &Options::default());
    assert!(html.starts_with("<div>"), "{}", &html[..40.min(html.len())]);
    assert!(html.ends_with("</div>\n"));
}

#[test]
fn nesting_past_the_ceiling_degrades_to_text() {
    let depth = 200;
    let source = format!("{}{}{}", "[[".repeat(depth), "x", "]]".repeat(depth));
    let html = compile(&source, &Options::default());
    // Past the ceiling the innermost label is emitted as written rather than
    // followed further, so the brackets survive somewhere in the output.
    assert!(
        html.contains("[[") || html.contains("x"),
        "{}",
        &html[..40.min(html.len())]
    );
}

#[test]
fn adversarial_pages_never_panic() {
    let shapes = [
        "''".repeat(20_000),
        "{{".repeat(20_000),
        "|}".repeat(20_000),
        "{{{".repeat(20_000),
        "<ref>".repeat(20_000),
        "<gallery>".repeat(5_000),
        "{|".repeat(20_000),
        "&amp".repeat(20_000),
        "\u{0}".repeat(5_000),
        "\u{e000}1\u{e001}".repeat(5_000),
        "\u{1f600}".repeat(5_000),
        "a\u{301}".repeat(20_000),
    ];
    for shape in shapes {
        let html = compile(&shape, &Options::default());
        std::hint::black_box(&html);
    }
}
#[test]
fn unmatched_block_closers_do_not_scan_the_open_stack() {
    // Without the per-name count, every `</div>` after a stack of `<p>` raws
    // scanned the whole stack, which is quadratic in the number of tags.
    let source = format!("{}{}", "<p>\n".repeat(20_000), "</div>\n".repeat(20_000));
    assert!(
        compile_time(&source) < std::time::Duration::from_secs(2),
        "unmatched closers scan the open stack again"
    );
}

#[test]
fn many_tags_stay_linear() {
    // Two separate lookups used to scan per tag: the block stack was searched by
    // name for every closing tag, and every opening tag scanned the remaining
    // tokens for its closer. Both are indexed now, which turned 128k tags from
    // seven seconds into under a tenth of one.
    for count in [8_000usize, 32_000, 128_000] {
        let source = "<div>".repeat(count);
        assert!(
            compile_time(&source) < std::time::Duration::from_secs(2),
            "{count} unclosed tags are superlinear again"
        );
    }
    let source = "<div></div>".repeat(32_000);
    assert!(
        compile_time(&source) < std::time::Duration::from_secs(2),
        "balanced tags are superlinear again"
    );
}

#[test]
fn many_tags_of_one_name_still_nest_correctly() {
    // The index has to keep working when the same name opens many times.
    let source = "<b>outer <b>inner</b> back</b>";
    let html = compile(source, &Options::default());
    let outer = html.find("<b>outer").expect("outer tag");
    let inner = html.find("<b>inner").expect("inner tag");
    assert!(outer < inner, "{html}");
    assert_eq!(html.matches("<b>").count(), 2, "{html}");
    assert_eq!(html.matches("</b>").count(), 2, "{html}");
}

#[test]
fn an_unclosed_tag_makes_the_interior_literal() {
    let source = "<b>one <div>two</div>";
    let html = compile(source, &Options::default());
    assert!(html.contains("&lt;b&gt;") || html.contains("<b>"), "{html}");
}
#[test]
fn crlf_input_matches_lf_input() {
    // A carriage return used to survive into the DOM and land in the output as a
    // stray byte, which a terminal reads as "return to column zero".
    let cases = [
        "== A ==\r\n\r\ntext",
        "* a\r\n* b",
        "{|\r\n| x\r\n|}",
        "'''a'''\r\n",
        "a\r\n\r\nb",
        "<ref>x</ref>\r\n",
        "<nowiki>a\r\nb</nowiki>",
        "<pre>a\r\nb</pre>",
        "a\rb",
    ];
    for case in cases {
        let with_lf = case.replace("\r\n", "\n").replace('\r', "\n");
        let lf = compile(&with_lf, &Options::default());
        let crlf = compile(case, &Options::default());
        assert_eq!(lf, crlf, "{case:?}");
        assert!(
            !crlf.contains('\r'),
            "{case:?} leaked a carriage return: {crlf:?}"
        );
    }
}
#[test]
fn crlf_template_bodies_do_not_leak_a_carriage_return() {
    // A template read from disk on Windows carries CRLF, and it reaches the page
    // through the resolver rather than through the source normalisation.
    let cases = [
        ("Hello, {{{1|world}}}!\r\n", "<p>Hello, Rust!</p>\n"),
        ("a\r\nb\r\n", "<p>a\nb</p>\n"),
        ("bare\rcr", "<p>bare\ncr</p>\n"),
    ];
    for (body, expected) in cases {
        let templates = Arc::new(MemoryTemplates::new());
        templates.insert("t", body);
        let options = Options::default().with_resolver(templates);
        let html = compile("{{t|Rust}}", &options);
        assert!(
            !html.contains('\r'),
            "{body:?} leaked a carriage return: {html:?}"
        );
        assert_eq!(html, expected, "{body:?}");
    }
}

#[test]
fn crlf_parameter_values_do_not_leak_a_carriage_return() {
    let templates = Arc::new(MemoryTemplates::new());
    templates.insert("t", "[{{{1|}}}]\r\n");
    let options = Options::default().with_resolver(templates);
    let html = compile("{{t|a\r\nb}}", &options);
    assert!(!html.contains('\r'), "{html:?}");
}
#[test]
fn diagnostics_report_a_real_line_and_column() {
    // Only the byte offset used to be filled in, so every diagnostic printed as
    // "at 0:0" and a caller had no way to point at the offending line.
    let cases = [
        ("{| ", 1u32, 1u32),
        ("line one\nline two\n{| ", 3, 1),
        ("x\n\ny\n{| ", 4, 1),
    ];
    for (source, line, column) in cases {
        let document = wikitext::parse(source, &Options::default());
        let diagnostic = document
            .diagnostics
            .first()
            .unwrap_or_else(|| panic!("no diagnostic for {source:?}"));
        assert_eq!(diagnostic.position.line, line, "{source:?}");
        assert_eq!(diagnostic.position.column, column, "{source:?}");
        assert_eq!(
            diagnostic.position.offset,
            source.rfind("{|").expect("marker"),
            "{source:?}"
        );
    }
}

#[test]
fn a_diagnostic_column_counts_characters_not_bytes() {
    // Three-byte characters must not push the column out to 10.
    let document = wikitext::parse("日本語\n{| ", &Options::default());
    let diagnostic = document.diagnostics.first().expect("a diagnostic");
    assert_eq!(diagnostic.position.line, 2);
    assert_eq!(diagnostic.position.column, 1);
}

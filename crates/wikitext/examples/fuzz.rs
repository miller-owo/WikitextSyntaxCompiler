//! Every case is a shape a real wiki sees. A panic, a hang, or a blow-up in time
//! all count as a finding.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};
use wikitext::{compile, Options};

struct Finding {
    name: String,
    detail: String,
}

fn check(name: &str, source: String, budget: Duration, findings: &mut Vec<Finding>) {
    use std::io::Write;
    print!("  {name} ({} B) ... ", source.len());
    let _ = std::io::stdout().flush();
    let options = Options {
        title: Some("Fuzz".to_string()),
        ..Options::default()
    };
    let length = source.len();
    let started = Instant::now();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let html = compile(&source, &options);
        html.len()
    }));
    let elapsed = started.elapsed();
    println!("{elapsed:.2?}");

    match result {
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_string());
            findings.push(Finding {
                name: name.to_string(),
                detail: format!("PANIC on {length} bytes: {message}"),
            });
        }
        Ok(output) => {
            if elapsed > budget {
                findings.push(Finding {
                    name: name.to_string(),
                    detail: format!(
                        "SLOW: {length} bytes took {elapsed:.2?} (budget {budget:.2?}), output {output} bytes"
                    ),
                });
            }
        }
    }
}

fn repeated(unit: &str, times: usize) -> String {
    unit.repeat(times)
}

fn main() {
    let mut findings = Vec::new();
    let budget = Duration::from_millis(250);
    let big = 200_000;

    println!("--- unbalanced delimiters ---");
    for (name, unit, times) in [
        ("unclosed bold", "'''", 20_000),
        ("unclosed italic", "''", 20_000),
        ("unclosed link", "[[", 20_000),
        ("unclosed link with bar", "[[a|", 20_000),
        ("unclosed template", "{{", 20_000),
        ("unclosed parameter", "{{{", 20_000),
        ("unclosed tag", "<div>", 20_000),
        ("unclosed ref", "<ref>", 20_000),
        ("unclosed comment", "<!--", 20_000),
        ("unclosed table", "{|", 20_000),
        ("stray table end", "|}", 20_000),
        ("unclosed entity", "&amp", 20_000),
        ("bare pipe", "|", 20_000),
        ("bare equals", "=", 20_000),
        ("bare apostrophes", "'", 40_000),
        ("bare brackets", "[", 40_000),
        ("bare braces", "{", 40_000),
        ("mixed noise", "''[[{{<>&|=", 10_000),
    ] {
        check(name, repeated(unit, times), budget, &mut findings);
    }

    println!("--- deep nesting ---");
    for depth in [100usize, 1_000, 5_000] {
        for (name, open, close) in [
            ("nesting: templates", "{{t|", "}}"),
            ("nesting: tags", "<div>", "</div>"),
            ("nesting: brackets", "[[", "]]"),
            ("nesting: bold", "'''", "'''"),
            ("nesting: italics", "''", "''"),
            ("nesting: ref", "<ref>", "</ref>"),
            ("nesting: gallery", "<gallery>", "</gallery>"),
            ("nesting: noinclude", "<noinclude>", "</noinclude>"),
            ("nesting: includeonly", "<includeonly>", "</includeonly>"),
        ] {
            let source = format!("{}{}{}", repeated(open, depth), "x", repeated(close, depth));
            if source.len() <= big * 4 {
                check(&format!("{name} x{depth}"), source, budget, &mut findings);
            }
        }
    }
    // Unclosed markers put the whole tail inside one suppression scope; a scan
    // that tested the open stack per marker would be quadratic here.
    for count in [5_000usize, 10_000, 20_000] {
        let source = format!("{}x", repeated("<noinclude>", count));
        if source.len() <= big * 4 {
            check(
                &format!("unclosed noinclude x{count}"),
                source,
                budget,
                &mut findings,
            );
        }
    }

    println!("--- lists and lines ---");
    for depth in [100usize, 1_000, 5_000, 20_000] {
        check(
            &format!("list nesting x{depth}"),
            format!("{}{}", repeated("*", depth), "item\n"),
            budget,
            &mut findings,
        );
    }
    for (name, unit) in [
        ("list lines", "* a\n"),
        ("nested list lines", "* a\n** b\n*** c\n**** d\n"),
        ("definition lines", "; a\n: b\n"),
        ("empty lines", "\n"),
        ("heading lines", "== h ==\n"),
        ("table lines", "|-\n"),
        ("preformatted lines", " x\n"),
    ] {
        check(name, repeated(unit, 20_000), budget, &mut findings);
    }

    println!("--- long single constructs ---");
    check(
        "one huge link",
        format!("[[{}|label]]", "a".repeat(big)),
        budget,
        &mut findings,
    );
    check(
        "one huge tag attribute",
        format!("<div class=\"{}\">x</div>", "a".repeat(big)),
        budget,
        &mut findings,
    );
    check(
        "one huge entity",
        format!("&{};", "a".repeat(big)),
        budget,
        &mut findings,
    );
    check(
        "one huge heading",
        format!("== {} ==", "a".repeat(big)),
        budget,
        &mut findings,
    );
    check(
        "one huge table cell",
        format!("{{|\n| {}\n|}}", "a".repeat(big)),
        budget,
        &mut findings,
    );
    check(
        "one huge list item",
        format!("* {}", "a".repeat(big)),
        budget,
        &mut findings,
    );
    check(
        "one huge ref",
        format!("<ref>{}</ref>", "a".repeat(big)),
        budget,
        &mut findings,
    );
    check(
        "one huge template argument",
        format!("{{{{t|{}}}}}", "a".repeat(big)),
        budget,
        &mut findings,
    );

    println!("--- repetition that could go quadratic ---");
    for times in [500usize, 1_000, 2_000, 4_000] {
        check(
            &format!("many refs x{times}"),
            repeated("<ref>a</ref>", times),
            budget,
            &mut findings,
        );
        check(
            &format!("many headings x{times}"),
            repeated("== h ==\n\n", times),
            budget,
            &mut findings,
        );
        check(
            &format!("many table cells x{times}"),
            format!("{{|\n{}\n|}}", repeated("| a || b || c\n", times)),
            budget,
            &mut findings,
        );
        check(
            &format!("many distinct templates x{times}"),
            repeated("{{t|1}}", times),
            budget,
            &mut findings,
        );
        check(
            &format!("many headings same text x{times}"),
            repeated("== same ==\n\n", times),
            budget,
            &mut findings,
        );
    }

    println!("--- odd bytes ---");
    check("nul bytes", "\u{0}".repeat(10_000), budget, &mut findings);
    check(
        "control characters",
        "\u{1}\u{2}\u{3}".repeat(10_000),
        budget,
        &mut findings,
    );
    check(
        "unpaired surrogate replacement",
        "\u{fffd}".repeat(10_000),
        budget,
        &mut findings,
    );
    check(
        "sentinel characters",
        "\u{e000}1\u{e001}".repeat(5_000),
        budget,
        &mut findings,
    );
    check(
        "combining marks",
        "a\u{301}".repeat(20_000),
        budget,
        &mut findings,
    );
    check(
        "rtl text",
        "\u{5d0}\u{5d1}\u{5d2}".repeat(10_000),
        budget,
        &mut findings,
    );
    check("emoji", "\u{1f600}".repeat(10_000), budget, &mut findings);
    check(
        "mixed scripts",
        "\u{4e2d}\u{6587}abc\u{5d0}".repeat(5_000),
        budget,
        &mut findings,
    );
    check(
        "bom",
        format!("\u{feff}{}", repeated("a", 10_000)),
        budget,
        &mut findings,
    );

    println!("\n=== {} findings ===", findings.len());
    for finding in &findings {
        println!("[{}] {}", finding.name, finding.detail);
    }
    if findings.is_empty() {
        println!("none");
    }
}

//! Runs one page from standard input, so a probe can hand a constructed input to
//! a child process: a stack overflow aborts the process and cannot be caught in a
//! thread.

use std::io::Read;

fn main() {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).ok();
    let options = wikitext::Options {
        title: Some("Probe".to_string()),
        ..wikitext::Options::default()
    };
    let stage = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "compile".to_string());
    let started = std::time::Instant::now();
    match stage.as_str() {
        "lex" => {
            let tokens = wikitext::lex::lex_inline(&source);
            std::hint::black_box(tokens.len());
        }
        "parse" => {
            let document = wikitext::parse(&source, &options);
            std::hint::black_box(document.children.len());
        }
        "parse-keep" => {
            let document = wikitext::parse(&source, &options);
            std::hint::black_box(document.children.len());
            std::mem::forget(document);
        }
        "serialize" => {
            let document = wikitext::parse(&source, &options);
            let html = wikitext::serialize(&document);
            std::hint::black_box(html.len());
        }
        _ => {
            let html = wikitext::compile(&source, &options);
            std::hint::black_box(html.len());
        }
    }
    eprintln!("{stage} ok in {:?}", started.elapsed());
}

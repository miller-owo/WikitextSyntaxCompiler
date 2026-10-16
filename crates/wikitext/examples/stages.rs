use std::time::{Duration, Instant};
use wikitext::{parse, serialize, Options, Preprocessor};

fn article(sections: usize, links: usize) -> String {
    let mut out = String::from("{{Infobox person|name=Test|born=1815}}\n\n");
    out.push_str("This is the lead paragraph with an [[internal link]] and an [https://example.org external one].\n\n");
    for index in 0..sections {
        out.push_str(&format!("== Section {index} ==\n\n"));
        out.push_str(&format!(
            "'''Section {index}''' was a ''notable'' thing. It had <ref name=\"n{index}\">A source.</ref> citations.\n\n"
        ));
        for link in 0..links {
            out.push_str(&format!(
                "The [[Article number {link}|article {link}]] mentions {{{{Infobox|id={index}|n={link}}}}}.\n"
            ));
        }
        out.push_str("* first item\n* second item\n** nested item\n* third item\n\n----\n\n");
    }
    out.push_str(
        "== Table ==\n\n{| class=\"wikitable\"\n|-\n! A !! B\n|-\n| 1 || 2\n|}\n\n<references/>\n",
    );
    out
}

fn time(label: &str, iterations: u32, mut body: impl FnMut()) {
    for _ in 0..3 {
        body();
    }
    let mut total = Duration::ZERO;
    let mut best = Duration::MAX;
    for _ in 0..iterations {
        let start = Instant::now();
        body();
        let elapsed = start.elapsed();
        total += elapsed;
        best = best.min(elapsed);
    }
    println!(
        "{label:<34} mean {:>9.2?}  best {:>9.2?}",
        total / iterations,
        best
    );
}

fn main() {
    let source = article(100, 4);
    let options = Options {
        title: Some("Benchmark".to_string()),
        ..Options::default()
    };
    println!("source: {} bytes\n", source.len());

    time("preprocess only", 200, || {
        let expanded = Preprocessor::new(&options).run(std::hint::black_box(&source));
        std::hint::black_box(&expanded);
    });

    let expanded = Preprocessor::new(&options).run(&source);
    println!("expanded: {} bytes\n", expanded.len());

    time("parse only (expanded input)", 200, || {
        let document = parse(std::hint::black_box(&expanded), &options);
        std::hint::black_box(&document);
    });

    let document = parse(&expanded, &options);
    time("serialize only", 200, || {
        let html = serialize(std::hint::black_box(&document));
        std::hint::black_box(&html);
    });

    time("whole compile", 200, || {
        let html = wikitext::compile(std::hint::black_box(&source), &options);
        std::hint::black_box(&html);
    });

    println!("\n--- preprocess on plain text (no markup to expand) ---");
    let plain: String = "Just some ordinary words in a paragraph. ".repeat(1000);
    time("preprocess plain", 200, || {
        let expanded = Preprocessor::new(&options).run(std::hint::black_box(&plain));
        std::hint::black_box(&expanded);
    });

    println!("\n--- parse on plain text ---");
    time("parse plain", 200, || {
        let document = parse(std::hint::black_box(&plain), &options);
        std::hint::black_box(&document);
    });

    println!("\n--- entity decoding ---");
    let entities = "a &amp; b &lt; c &gt; d &#65; e ".repeat(2000);
    time("decode entities", 200, || {
        let decoded = wikitext::decode_entities(std::hint::black_box(&entities));
        std::hint::black_box(&decoded);
    });
}

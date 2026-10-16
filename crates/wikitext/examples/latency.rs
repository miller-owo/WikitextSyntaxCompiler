//! Numbers are absolute, not compared against a stored baseline, so a run means
//! the same thing on any machine. Run-to-run spread on a shared machine hides
//! anything under about a tenth of the total; measure a stage in isolation before
//! believing a smaller difference.

use std::time::{Duration, Instant};
use wikitext::{compile, Options, OutputMode};

fn paragraph(index: usize, links: usize) -> String {
    let mut out = format!(
        "'''Section {index}''' was a ''notable'' thing that happened. \
         It had {links} links and some <ref name=\"n{index}\">A source, {index}.</ref> citations.\n\n"
    );
    for link in 0..links {
        out.push_str(&format!(
            "The [[Article number {link}|article {link}]] mentions {{{{Infobox|id={index}|n={link}}}}}.\n"
        ));
    }
    out
}

fn article(sections: usize, links: usize) -> String {
    let mut out = String::from("{{Infobox person|name=Test|born=1815}}\n\n");
    out.push_str("This is the lead paragraph with an [[internal link]] and an [https://example.org external one].\n\n");
    for index in 0..sections {
        out.push_str(&format!("== Section {index} ==\n\n"));
        out.push_str(&paragraph(index, links));
        out.push_str("* first item\n* second item\n** nested item\n* third item\n\n");
        out.push_str("----\n\n");
    }
    out.push_str("== Table ==\n\n{| class=\"wikitable\"\n|-\n! A !! B\n|-\n| 1 || 2\n|}\n\n");
    out.push_str("<references/>\n");
    out
}

fn once(source: &str, options: &Options) -> Duration {
    let start = Instant::now();
    let html = compile(source, options);
    std::hint::black_box(&html);
    start.elapsed()
}

fn bench(label: &str, source: &str, iterations: u32) {
    let options = Options {
        title: Some("Benchmark".to_string()),
        ..Options::default()
    };
    // A warm-up run keeps allocator and branch-predictor effects out of the
    // numbers, which matters at the sizes where a single run is fast.
    let _ = once(source, &options);

    let mut total = Duration::ZERO;
    let mut best = Duration::MAX;
    for _ in 0..iterations {
        let elapsed = once(source, &options);
        total += elapsed;
        best = best.min(elapsed);
    }
    let mean = total / iterations;
    let megabytes = source.len() as f64 / (1024.0 * 1024.0);
    let throughput = megabytes / mean.as_secs_f64();
    println!(
        "{label:<28} {:>8} bytes  mean {:>9.2?}  best {:>9.2?}  {:>7.1} MB/s",
        source.len(),
        mean,
        best,
        throughput
    );
}

fn bench_document(label: &str, source: &str, iterations: u32) {
    let options = Options {
        title: Some("Benchmark".to_string()),
        output_mode: OutputMode::Document,
        ..Options::default()
    };
    let _ = once(source, &options);
    let mut total = Duration::ZERO;
    for _ in 0..iterations {
        total += once(source, &options);
    }
    println!(
        "{label:<28} {:>8} bytes  mean {:>9.2?}",
        source.len(),
        total / iterations
    );
}

fn main() {
    println!("--- compile, fragment output ---");
    bench("empty", "", 2000);
    bench(
        "one paragraph",
        "Just a sentence with a '''bold''' word.",
        2000,
    );
    bench("small article", &article(4, 2), 500);
    bench("medium article", &article(40, 4), 100);
    bench("large article", &article(200, 8), 20);

    println!("\n--- compile, full document ---");
    bench_document("medium article", &article(40, 4), 100);

    println!("\n--- scaling ---");
    for sections in [10usize, 50, 100, 200, 400] {
        let source = article(sections, 4);
        let options = Options {
            title: Some("Benchmark".to_string()),
            ..Options::default()
        };
        let _ = once(&source, &options);
        let iterations = 200 / (sections / 10).max(1) as u32;
        let mut total = Duration::ZERO;
        for _ in 0..iterations.max(3) {
            total += once(&source, &options);
        }
        let mean = total / iterations.max(3);
        let megabytes = source.len() as f64 / (1024.0 * 1024.0);
        println!(
            "{sections:>5} sections  {:>8} bytes  mean {:>9.2?}  {:>7.1} MB/s",
            source.len(),
            mean,
            megabytes / mean.as_secs_f64()
        );
    }
}

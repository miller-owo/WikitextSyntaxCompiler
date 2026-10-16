use std::time::{Duration, Instant};
use wikitext::{compile, Options};

fn measure(name: &str, build: impl Fn(usize) -> String, sizes: &[usize]) {
    let options = Options {
        title: Some("Scaling".to_string()),
        ..Options::default()
    };
    println!("\n--- {name} ---");
    let mut previous: Option<(usize, Duration)> = None;
    for &size in sizes {
        let source = build(size);
        let started = Instant::now();
        let html = compile(&source, &options);
        let elapsed = started.elapsed();
        std::hint::black_box(&html);

        let ratio = match previous {
            Some((previous_size, previous_time)) if previous_time.as_secs_f64() > 0.0 => {
                let size_ratio = size as f64 / previous_size as f64;
                let time_ratio = elapsed.as_secs_f64() / previous_time.as_secs_f64();
                format!("  x{size_ratio:.1} size -> x{time_ratio:.1} time")
            }
            _ => String::new(),
        };
        println!(
            "  {size:>7} units  {:>9} bytes  {elapsed:>10.2?}{ratio}",
            source.len()
        );
        previous = Some((size, elapsed));

        if elapsed > Duration::from_secs(20) {
            println!("  (stopping: too slow to continue)");
            break;
        }
    }
}

fn main() {
    let sizes = [1_000usize, 2_000, 4_000, 8_000, 16_000, 32_000];

    // Sizes double, so linear shapes show roughly x2 per step and x4 is quadratic.
    measure("unclosed [[", |n| "[[".repeat(n), &sizes);
    measure("unclosed [[a|", |n| "[[a|".repeat(n), &sizes);
    measure("unclosed {{", |n| "{{".repeat(n), &sizes);
    measure("unclosed {{{", |n| "{{{".repeat(n), &sizes);
    measure("<div with no >", |n| "<div".repeat(n), &sizes);
    measure("unclosed <div>", |n| "<div>".repeat(n), &sizes);
    measure("unclosed <ref>", |n| "<ref>".repeat(n), &sizes);
    measure("unclosed <!--", |n| "<!--".repeat(n), &sizes);
    measure("unclosed {|", |n| "{|".repeat(n), &sizes);
    measure("unclosed &amp", |n| "&amp".repeat(n), &sizes);
    measure("single apostrophes", |n| "'".repeat(n * 3), &sizes);
    measure("balanced [[x]]", |n| "[[x]]".repeat(n), &sizes);
    measure("balanced {{t}}", |n| "{{t}}".repeat(n), &sizes);
}

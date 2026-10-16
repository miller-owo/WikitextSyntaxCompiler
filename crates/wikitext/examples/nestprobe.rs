//! A stack overflow aborts the process, so each case runs in its own child
//! process rather than a thread that cannot be caught.

use std::process::{Command, Stdio};

fn main() {
    let child = std::env::args().nth(1).expect("child path");
    let cases: [(&str, &str, &str); 8] = [
        ("brackets", "[[", "]]"),
        ("templates", "{{t|", "}}"),
        ("tags", "<div>", "</div>"),
        ("inline tags", "<span>", "</span>"),
        ("ref", "<ref>", "</ref>"),
        ("gallery", "<gallery>", "</gallery>"),
        ("bold", "'''", "'''"),
        ("italics", "''", "''"),
    ];
    for (name, open, close) in cases {
        let mut last_ok = 0usize;
        for depth in [50usize, 100, 200, 400, 800, 1600, 3200, 6400] {
            let source = format!("{}{}{}", open.repeat(depth), "x", close.repeat(depth));
            // The source goes through stdin, because a command line has a length
            // limit that nesting tests hit quickly.
            let mut child_process = Command::new(&child)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn");
            {
                use std::io::Write;
                let stdin = child_process.stdin.as_mut().expect("stdin");
                let _ = stdin.write_all(source.as_bytes());
            }
            let status = child_process.wait().expect("wait");
            if status.success() {
                last_ok = depth;
            } else {
                println!("{name:>12}: crashes at depth {depth} (last ok {last_ok})");
                break;
            }
            if depth == 6400 {
                println!("{name:>12}: ok at depth {depth}");
            }
        }
    }
}

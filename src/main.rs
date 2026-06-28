//! CLI. FROZEN — do not edit as part of autoresearch.
//!
//!   bootstrap eval    score the algorithm against the fixed fixtures (wall-clock)

use std::process::exit;

use bootstrap::harness::eval;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("eval") => exit(eval::run()),
        _ => {
            eprintln!("usage:\n  bootstrap eval");
            exit(2);
        }
    }
}

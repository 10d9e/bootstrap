//! Evaluation + scoring. FROZEN — do not edit as part of autoresearch.
//!
//! SCORE = median wall-clock time of one programmable bootstrap (LOWER IS BETTER), gated on
//! the correctness fixtures: every fixture must decode to `f(message)` AND the refreshed
//! output must have a comfortable noise margin (a genuine, noise-reducing bootstrap, not a
//! pass-through). Timing is machine-dependent — the winner is decided on a fixed reference
//! runner; locally it tells you whether you moved the needle.

use std::hint::black_box;
use std::time::Instant;

use crate::algorithm::{bootstrap, keygen};
use crate::harness::fixtures;
use crate::harness::params::{decrypt, encrypt, gen_secret_key, output_noise, params};

const SECRET_SEED: u64 = 0x5EED_5EED;
const KEYGEN_SEED: u64 = 0xB0_07_57_A9;
const WARMUP: usize = 5;
const ITERS: usize = 31; // odd → exact median
/// Minimum output-noise margin (bits below the Δ/2 decode boundary) for a fixture to count
/// as a genuine refresh.
const MARGIN_MIN_BITS: f64 = 3.0;

pub fn run() -> i32 {
    let p = params();
    let sk = gen_secret_key(p, SECRET_SEED);
    let server = keygen(p, &sk, KEYGEN_SEED);

    println!("{:<16} {:>4} {:>4} {:>9}  {}", "fixture", "got", "want", "margin", "ok");

    let mut all_ok = true;
    for fx in fixtures::all() {
        let ct = encrypt(p, &sk, fx.message, fx.seed);
        let out = bootstrap(&server, &ct, &fx.lut);
        let got = decrypt(p, &sk, &out);
        let want = fx.lut.values[fx.message as usize];
        let noise = output_noise(p, &sk, &out, want).max(1);
        let margin = ((p.delta() / 2) as f64 / noise as f64).log2();
        let ok = got == want && margin > MARGIN_MIN_BITS;
        if !ok {
            all_ok = false;
        }
        println!(
            "{:<16} {:>4} {:>4} {:>7.1}b  {}",
            fx.name,
            got,
            want,
            margin,
            if ok { "OK" } else { "FAIL!" }
        );
    }

    println!("{}", "-".repeat(44));
    if !all_ok {
        println!("\nSCORE: INVALID (correctness gate failed)");
        return 1;
    }

    // Timed score: median of ITERS bootstraps on the representative input (keygen is free).
    let ct = encrypt(p, &sk, fixtures::TIMING_MESSAGE, fixtures::TIMING_SEED);
    let lut = fixtures::timing_lut();
    for _ in 0..WARMUP {
        black_box(bootstrap(&server, black_box(&ct), &lut));
    }
    let mut times = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        let out = bootstrap(&server, &ct, &lut);
        times.push(t0.elapsed().as_nanos() as u64);
        black_box(out);
    }
    times.sort_unstable();
    let median = times[ITERS / 2];
    let best = times[0];

    println!(
        "\nSCORE: {} ns/bootstrap  (median of {}; best {} ns) — LOWER IS BETTER",
        median, ITERS, best
    );
    println!("       = {:.3} ms", median as f64 / 1e6);
    0
}

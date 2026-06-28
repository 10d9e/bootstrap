//! Evaluation + scoring. FROZEN — do not edit as part of autoresearch.
//!
//! Gates, in order: (1) the submission's `params()` use the required `message_bits`;
//! (2) both the LWE and GLWE instances clear ≥128-bit security (classical core-SVP, see
//! [`crate::harness::security`]); (3) every correctness fixture decodes to `f(message)`
//! with a comfortable noise margin (a genuine refresh). If all pass, SCORE = median
//! wall-clock time of one bootstrap (LOWER IS BETTER). Wall-clock is machine-dependent —
//! the winner is decided on a fixed reference runner.

use std::hint::black_box;
use std::time::Instant;

use crate::algorithm::{bootstrap, keygen, params};
use crate::harness::fixtures;
use crate::harness::params::{
    decrypt, encrypt, gen_secret_key, output_noise, security_bits, REQUIRED_MESSAGE_BITS,
    SECURITY_BITS_REQUIRED,
};

const SECRET_SEED: u64 = 0x5EED_5EED;
const KEYGEN_SEED: u64 = 0xB0_07_57_A9;
const WARMUP: usize = 3;
const ITERS: usize = 21; // odd → exact median
const MARGIN_MIN_BITS: f64 = 3.0;

pub fn run() -> i32 {
    let p = params();

    // (1) functional spec.
    if p.message_bits != REQUIRED_MESSAGE_BITS {
        println!(
            "SCORE: INVALID (message_bits = {}, must be {})",
            p.message_bits, REQUIRED_MESSAGE_BITS
        );
        return 1;
    }

    // (2) security gate.
    let (lwe_bits, glwe_bits) = security_bits(&p);
    println!(
        "params: n={} k={} N={} | LWE σ=2^{:.1} dim {} → {:.1} bits | GLWE σ=2^{:.1} dim {} → {:.1} bits",
        p.n, p.k, p.poly,
        p.lwe_sigma.log2(), p.n, lwe_bits,
        p.glwe_sigma.log2(), p.k * p.poly, glwe_bits,
    );
    if lwe_bits < SECURITY_BITS_REQUIRED || glwe_bits < SECURITY_BITS_REQUIRED {
        println!(
            "SCORE: INVALID (security {:.1}/{:.1} bits < {} required)",
            lwe_bits, glwe_bits, SECURITY_BITS_REQUIRED
        );
        return 1;
    }
    println!("security gate: OK (≥{} core-SVP bits)", SECURITY_BITS_REQUIRED);

    // (3) correctness gate.
    let sk = gen_secret_key(p, SECRET_SEED);
    let server = keygen(&sk, KEYGEN_SEED);
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
            fx.name, got, want, margin, if ok { "OK" } else { "FAIL!" }
        );
    }
    println!("{}", "-".repeat(44));
    if !all_ok {
        println!("\nSCORE: INVALID (correctness gate failed)");
        return 1;
    }

    // Timed score (keygen is free).
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

    println!(
        "\nSCORE: {} ns/bootstrap  (median of {}; best {} ns) — LOWER IS BETTER",
        median, ITERS, times[0]
    );
    println!("       = {:.3} ms", median as f64 / 1e6);
    0
}

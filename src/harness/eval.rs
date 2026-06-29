//! Evaluation + scoring. FROZEN — do not edit as part of autoresearch.
//!
//! Gates, in order: (1) the submission's `params()` use the required `message_bits`;
//! (2) both the LWE and GLWE instances clear ≥128-bit security (standard estimator, see
//! [`crate::harness::security`]); (3a) every correctness fixture decodes to `f(message)`;
//! (3b) the output-noise σ gives a decryption-failure margin `log2(gap/σ) ≥ 3.5` bits
//! (≈ 2⁻⁶⁰ failure — a genuine refresh). If all pass, SCORE = median wall-clock time of one
//! bootstrap (LOWER IS BETTER). Wall-clock is machine-dependent — decided on a fixed runner.

use std::hint::black_box;
use std::time::Instant;

use crate::algorithm::{bootstrap, keygen, params};
use crate::harness::fixtures;
use crate::harness::params::{
    decrypt, encrypt, failure_margin_bits, gen_secret_key, security_bits, REQUIRED_MESSAGE_BITS,
    SECURITY_BITS_REQUIRED,
};

const SECRET_SEED: u64 = 0x5EED_5EED;
const KEYGEN_SEED: u64 = 0xB0_07_57_A9;
const WARMUP: usize = 3;
const ITERS: usize = 21; // odd → exact median
/// Minimum decryption-failure margin (bits): log2(gap/σ) ≥ 3.5 ⇒ failure ≈ 2⁻⁶⁰.
const MARGIN_MIN_BITS: f64 = 3.5;

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
    println!("security gate: OK (≥{} bits, standard model)", SECURITY_BITS_REQUIRED);

    // (3a) functional correctness: every fixture must decode to f(message).
    let sk = gen_secret_key(p, SECRET_SEED);
    let server = keygen(&sk, KEYGEN_SEED);
    println!("{:<16} {:>4} {:>4}  {}", "fixture", "got", "want", "ok");
    let mut all_ok = true;
    for fx in fixtures::all() {
        let ct = encrypt(p, &sk, fx.message, fx.seed);
        let out = bootstrap(&server, &ct, &fx.lut);
        let got = decrypt(p, &sk, &out);
        let want = fx.lut.values[fx.message as usize];
        let ok = got == want;
        all_ok &= ok;
        println!("{:<16} {:>4} {:>4}  {}", fx.name, got, want, if ok { "OK" } else { "FAIL!" });
    }
    println!("{}", "-".repeat(44));
    if !all_ok {
        println!("\nSCORE: INVALID (a fixture decoded incorrectly)");
        return 1;
    }

    // (3b) noise / refresh gate: estimate the output-noise σ over many fresh bootstraps and
    // require the decryption-failure margin log2(gap/σ) ≥ MARGIN_MIN_BITS (the standard
    // failure-probability correctness measure, not a worst single sample).
    let lut = fixtures::timing_lut();
    let out_msg = lut.values[fixtures::TIMING_MESSAGE as usize];
    let n_samples = 64;
    let mut sumsq = 0.0f64;
    for i in 0..n_samples {
        let ct = encrypt(p, &sk, fixtures::TIMING_MESSAGE, 0xA50_000 + i);
        let out = bootstrap(&server, &ct, &lut);
        let e = crate::harness::params::output_noise_signed(p, &sk, &out, out_msg) as f64;
        sumsq += e * e;
    }
    let sigma = (sumsq / n_samples as f64).sqrt().max(1.0);
    let margin = failure_margin_bits(p, sigma);
    println!(
        "output noise: σ=2^{:.1}, failure margin {:.1} bits (need ≥{})",
        sigma.log2(), margin, MARGIN_MIN_BITS
    );
    if margin < MARGIN_MIN_BITS {
        println!("\nSCORE: INVALID (noise margin {:.1} < {} bits)", margin, MARGIN_MIN_BITS);
        return 1;
    }

    // Timed score (keygen is free).
    let ct = encrypt(p, &sk, fixtures::TIMING_MESSAGE, fixtures::TIMING_SEED);
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

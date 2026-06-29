//! Correctness gate. FROZEN — do not edit as part of autoresearch.
//!
//! These tests confirm a submission performs a genuine programmable bootstrap: it must
//! apply the LUT to the encrypted message AND reduce noise (refresh). They use synthetic
//! inputs distinct from the scored fixtures, so a candidate cannot pass by overfitting.

use bootstrap::algorithm::{bootstrap, keygen, params};
use bootstrap::harness::params::{
    decrypt, encrypt, failure_margin_bits, gen_secret_key, output_noise_signed, security_bits, Lut,
};

#[test]
fn params_clear_128_bit_gate() {
    let (lwe, glwe) = security_bits(&params());
    assert!(lwe >= 128.0, "LWE security only {lwe:.1} bits");
    assert!(glwe >= 128.0, "GLWE security only {glwe:.1} bits");
}

#[test]
fn applies_lut_to_encrypted_message() {
    let p = params();
    let sk = gen_secret_key(p, 0xA11CE);
    let server = keygen(&sk, 0xB0B);
    let modulus = p.msg_modulus();

    // A few LUTs, including non-identity ones a pass-through could not satisfy.
    let luts: Vec<Vec<u64>> = vec![
        (0..modulus).collect(),                       // identity
        (0..modulus).map(|m| (m + 1) % modulus).collect(), // +1
        (0..modulus).map(|m| (modulus - 1) - m).collect(), // reverse
        (0..modulus).map(|_| 1).collect(),            // constant 1
    ];

    for (li, values) in luts.iter().enumerate() {
        let lut = Lut { values: values.clone() };
        for m in 0..modulus {
            let ct = encrypt(p, &sk, m, 0xBEEF + li as u64 * 17 + m);
            let out = bootstrap(&server, &ct, &lut);
            assert_eq!(
                decrypt(p, &sk, &out),
                values[m as usize],
                "lut #{li}, message {m}: bootstrap did not apply the LUT"
            );
        }
    }
}

#[test]
fn refreshes_noise() {
    // A genuine refresh: estimate the output-noise σ over many fresh bootstraps and require
    // the decryption-failure margin log2(gap/σ) ≥ 3.5 bits (≈ 2⁻⁶⁰ failure) — the standard
    // failure-probability measure, not a worst single sample. Decoding must also be correct.
    let p = params();
    let sk = gen_secret_key(p, 0x5151);
    let server = keygen(&sk, 0x6262);
    let modulus = p.msg_modulus();
    let lut = Lut {
        values: (0..modulus).map(|m| (m + 1) % modulus).collect(),
    };

    let samples = 80u64;
    let mut sumsq = 0.0f64;
    for t in 0..samples {
        let m = t % modulus;
        let ct = encrypt(p, &sk, m, 0x9000 + t);
        let out = bootstrap(&server, &ct, &lut);
        let want = (m + 1) % modulus;
        assert_eq!(decrypt(p, &sk, &out), want, "message {m}");
        let e = output_noise_signed(p, &sk, &out, want) as f64;
        sumsq += e * e;
    }
    let sigma = (sumsq / samples as f64).sqrt().max(1.0);
    let margin = failure_margin_bits(p, sigma);
    assert!(margin >= 3.5, "noise margin too tight: {margin:.1} bits (σ=2^{:.1})", sigma.log2());
}

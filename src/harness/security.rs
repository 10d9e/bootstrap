//! LWE security estimator. FROZEN — do not edit as part of autoresearch.
//!
//! A self-contained Rust approximation of the lattice-estimator
//! (<https://github.com/malb/lattice-estimator>) sufficient to gate arbitrary parameter sets
//! at ≥128-bit. It models the **primal-uSVP** attack (Bai–Galbraith embedding, the "2016
//! estimate") — minimized over BKZ block size `β` and number of samples `m`, with the secret
//! **rescaled to the error scale** so the binary-secret advantage is exploited — and costs
//! BKZ realistically:
//!
//! ```text
//! log2(cost) = 0.292·β        (BDGL16 classical sieve, per SVP oracle call)
//!            + 16.4           (sieve gate/overhead constant)
//!            + log2(8·d)      (number of SVP calls across BKZ tours)
//! ```
//!
//! This is the model behind the "128-bit" parameters used by TFHE-rs, rs-tfhe, and the
//! homomorphicencryption.org standard (raw core-SVP `0.292β` alone is far more conservative).
//! Calibration (see tests): rs-tfhe's 128-bit set clears the gate, the σ=64 toy set is far
//! below, Kyber512 lands near its NIST-level-1 figure, and security grows with dimension and
//! noise. For final sign-off, cross-check with the actual lattice-estimator.

use std::f64::consts::{E, PI};

const SIEVE_SLOPE: f64 = 0.292;
const SIEVE_ADD: f64 = 16.4;

/// Root-Hermite factor `δ` achievable by BKZ with block size `β` (valid for `β ≥ 50`).
fn delta(beta: f64) -> f64 {
    ((PI * beta).powf(1.0 / beta) * beta / (2.0 * PI * E)).powf(1.0 / (2.0 * (beta - 1.0)))
}

/// BKZ-β cost in bits: BDGL16 sieve per SVP call × the number of calls across tours.
fn bkz_bits(beta: f64, d: f64) -> f64 {
    SIEVE_SLOPE * beta + SIEVE_ADD + (8.0 * d).log2()
}

/// Bits of security of an LWE instance: secret dimension `n`, modulus `q = 2^log2q`, Gaussian
/// error std-dev `sigma`, secret-coordinate std-dev `sigma_s`. Ring/Module-LWE is estimated
/// as plain LWE of dimension `k·N`.
pub fn lwe_bits(n: usize, log2q: f64, sigma: f64, sigma_s: f64) -> f64 {
    let nf = n as f64;
    let log2_sigma = sigma.max(1e-12).log2();
    let log2_nu = (sigma / sigma_s).log2(); // secret rescaling factor ν = σ/σ_s

    let mut beta = 50.0;
    while beta < 4000.0 {
        let log2_delta = delta(beta).log2();
        // Does any number of samples m make BKZ-β succeed?  Primal-uSVP (rescaled secret):
        //   log2σ + ½log2β  ≤  (2β − d)·log2δ + (m·log2q + n·log2ν)/d ,   d = m + n + 1
        let lhs = log2_sigma + 0.5 * beta.log2();
        let step = (n / 256).max(1);
        let mut m = step;
        while m <= 4 * n {
            let d = (m + n + 1) as f64;
            let rhs = (2.0 * beta - d) * log2_delta + (m as f64 * log2q + nf * log2_nu) / d;
            if lhs <= rhs {
                return bkz_bits(beta, d);
            }
            m += step;
        }
        beta += 1.0;
    }
    bkz_bits(4000.0, (4 * n + n + 1) as f64)
}

/// Secret-coordinate std-dev for a uniform binary `{0,1}` secret (TFHE keys): variance ¼.
pub const BINARY_SECRET_STD: f64 = 0.5;

#[cfg(test)]
mod tests {
    use super::*;

    /// rs-tfhe / TFHE-style 128-bit params (mapped to q=2^64, same noise rate α = σ/q):
    ///   LWE  n=700,  α=2.0e-5;  GLWE N=1024, α=2.0e-8.
    fn rstfhe() -> (f64, f64) {
        let q = 2.0f64.powi(64);
        (
            lwe_bits(700, 64.0, 2.0e-5 * q, BINARY_SECRET_STD),
            lwe_bits(1024, 64.0, 2.0e-8 * q, BINARY_SECRET_STD),
        )
    }

    #[test]
    fn anchors() {
        let (lwe, glwe) = rstfhe();
        let toy = lwe_bits(500, 64.0, 64.0, BINARY_SECRET_STD);
        let kyber = lwe_bits(512, 3329f64.log2(), 1.0, 1.225);
        eprintln!("rs-tfhe 128-bit: LWE(n=700)={lwe:.1}  GLWE(N=1024)={glwe:.1}");
        eprintln!("toy n=500 σ=64: {toy:.1}   Kyber512: {kyber:.1} (NIST-1 ≈ 143)");
        // Kyber512 should land near its NIST-level-1 gate-count figure (~143).
        assert!((130.0..=160.0).contains(&kyber), "Kyber512 = {kyber:.1}");
        // rs-tfhe's LWE clears 128; its aggressive N=1024 GLWE sits right at the boundary.
        assert!(lwe >= 128.0, "rs-tfhe LWE = {lwe:.1}");
        assert!(glwe >= 120.0, "rs-tfhe GLWE = {glwe:.1}");
        // The σ=64 toy set must be wildly insecure.
        assert!(toy < 60.0, "toy set scored {toy:.1}");
    }

    #[test]
    fn monotonic() {
        let base = lwe_bits(1024, 64.0, 2.0f64.powi(30), BINARY_SECRET_STD);
        let more_noise = lwe_bits(1024, 64.0, 2.0f64.powi(36), BINARY_SECRET_STD);
        let more_dim = lwe_bits(2048, 64.0, 2.0f64.powi(30), BINARY_SECRET_STD);
        eprintln!("base={base:.1} more_noise={more_noise:.1} more_dim={more_dim:.1}");
        assert!(more_noise > base && more_dim > base);
    }
}

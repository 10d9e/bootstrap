//! Conservative LWE security estimator. FROZEN — do not edit as part of autoresearch.
//!
//! A self-contained Rust approximation of the lattice-estimator
//! (<https://github.com/malb/lattice-estimator>), sufficient to gate arbitrary parameter
//! sets at ≥128-bit security. It models the **primal-uSVP** attack (Bai–Galbraith embedding,
//! the "2016 estimate") under the **classical core-SVP** cost model `2^{0.292·β}`, minimized
//! over BKZ block size `β` and number of samples `m`, with the secret rescaled to the error
//! scale (so small/binary secrets are accounted for).
//!
//! Core-SVP counts a single SVP oracle call, so it is a *lower bound* on the real attack
//! cost: if this estimator reports ≥128 bits, the full lattice-estimator reports at least as
//! much. It is therefore safe to use as a gate (it may slightly under-credit secure-but-
//! marginal sets). For final sign-off, cross-check with the actual lattice-estimator.

use std::f64::consts::{E, PI};

/// Classical core-SVP cost exponent: `log2(cost) = CORE_SVP · β`.
const CORE_SVP: f64 = 0.292;

/// Root-Hermite factor `δ` achievable by BKZ with block size `β` (valid for `β ≥ 50`).
fn delta(beta: f64) -> f64 {
    ((PI * beta).powf(1.0 / beta) * beta / (2.0 * PI * E)).powf(1.0 / (2.0 * (beta - 1.0)))
}

/// Bits of security of an LWE instance: secret dimension `n`, modulus `q = 2^log2q`, Gaussian
/// error std-dev `sigma`, secret-coordinate std-dev `sigma_s`. Ring/Module-LWE is estimated
/// as plain LWE of dimension `k·N` (the standard, conservative treatment).
pub fn lwe_bits(n: usize, log2q: f64, sigma: f64, sigma_s: f64) -> f64 {
    let nf = n as f64;
    let log2_sigma = sigma.max(1e-12).log2();
    let log2_nu = (sigma / sigma_s).log2(); // secret rescaling factor ν = σ/σ_s

    let mut beta = 50.0;
    while beta < 4000.0 {
        let log2_delta = delta(beta).log2();
        // Does any number of samples m make BKZ-β succeed?  Condition (log2):
        //   log2σ + ½log2β  ≤  (2β − d)·log2δ + (m·log2q + n·log2ν)/d ,   d = m + n + 1
        let lhs = log2_sigma + 0.5 * beta.log2();
        let step = (n / 256).max(1);
        let mut m = step;
        let mut works = false;
        while m <= 4 * n {
            let d = (m + n + 1) as f64;
            let rhs = (2.0 * beta - d) * log2_delta + (m as f64 * log2q + nf * log2_nu) / d;
            if lhs <= rhs {
                works = true;
                break;
            }
            m += step;
        }
        if works {
            return CORE_SVP * beta;
        }
        beta += 1.0;
    }
    CORE_SVP * 4000.0
}

/// Secret-coordinate std-dev for a uniform binary `{0,1}` secret (TFHE keys): variance ¼.
pub const BINARY_SECRET_STD: f64 = 0.5;

#[cfg(test)]
mod tests {
    use super::*;

    // Calibration anchor with a precisely PUBLISHED classical core-SVP figure: Kyber512 is
    // 2^118 (NIST PQC spec). Module-LWE rank-2 deg-256 ⇒ LWE dim 512, q=3329, error CBD η2=2
    // (σ=1.0), secret CBD η1=3 (σ_s=1.225). Our core-SVP estimate must land near 118.
    #[test]
    fn calibrates_against_kyber512() {
        let bits = lwe_bits(512, 3329f64.log2(), 1.0, (1.5f64).sqrt());
        eprintln!("Kyber512 -> {bits:.1} core-SVP bits (published ≈ 118)");
        assert!(
            (105.0..=132.0).contains(&bits),
            "Kyber512 gave {bits:.1} bits (expected ≈118)"
        );
    }

    #[test]
    fn baseline_candidate_is_128_bit() {
        // Candidate baseline: small LWE key dim 1024 (σ=2^44) + GLWE dim k·N = 2048 (σ=2^29),
        // q = 2^64, binary secret. Both must clear 128-bit core-SVP.
        let lwe = lwe_bits(1024, 64.0, 2.0f64.powi(44), BINARY_SECRET_STD);
        let glwe = lwe_bits(2048, 64.0, 2.0f64.powi(29), BINARY_SECRET_STD);
        eprintln!("candidate: LWE(1024,2^44)={lwe:.1}  GLWE(2048,2^29)={glwe:.1}");
        assert!(lwe >= 128.0, "LWE only {lwe:.1} bits");
        assert!(glwe >= 128.0, "GLWE only {glwe:.1} bits");
    }

    #[test]
    fn toy_params_are_insecure() {
        // The old toy set (n=500, q=2^64, σ=64) must be far below 128 bits.
        let bits = lwe_bits(500, 64.0, 64.0, BINARY_SECRET_STD);
        eprintln!("toy n=500 σ=64 -> {bits:.1} bits");
        assert!(bits < 40.0, "toy params unexpectedly scored {bits:.1} bits");
    }

    #[test]
    fn more_noise_and_dimension_increase_security() {
        let base = lwe_bits(1024, 64.0, 2.0f64.powi(40), BINARY_SECRET_STD);
        let more_noise = lwe_bits(1024, 64.0, 2.0f64.powi(44), BINARY_SECRET_STD);
        let more_dim = lwe_bits(2048, 64.0, 2.0f64.powi(40), BINARY_SECRET_STD);
        eprintln!("base={base:.1} more_noise={more_noise:.1} more_dim={more_dim:.1}");
        assert!(more_noise > base, "more noise should not reduce security");
        assert!(more_dim > base, "more dimension should not reduce security");
    }
}

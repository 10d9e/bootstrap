//! Frozen TFHE parameters, ciphertext / key types, and the encryption oracle.
//! FROZEN — do not edit as part of autoresearch.
//!
//! The harness owns all *secret* material and the LWE encode/decode: it generates the
//! secret key, encrypts (noisy) inputs, decrypts outputs, and verifies security. The
//! parameters themselves are chosen by the **submission** (`algorithm::params()`); the
//! harness only constrains the functional spec (`message_bits`) and gates security at
//! ≥128 bits via [`crate::harness::security`]. A submission may pick any `n`, `k`, `N`,
//! decomposition, and noise that clear the gate.
//!
//! Torus is `u64` (`q = 2^64`, wrapping arithmetic).

use crate::harness::security::{lwe_bits, BINARY_SECRET_STD};

/// Modulus exponent: `q = 2^64`.
pub const LOG2_Q: f64 = 64.0;
/// Required security level (classical core-SVP bits) for both the LWE and GLWE instances.
pub const SECURITY_BITS_REQUIRED: f64 = 128.0;
/// The fixed functional spec: total bits incl. padding ⇒ `2^(message_bits-1)` messages.
/// A submission's `params().message_bits` must equal this (so the message space can't be
/// shrunk to game the score).
pub const REQUIRED_MESSAGE_BITS: u32 = 3;

/// TFHE parameter set. Chosen by the submission, then validated by the harness.
#[derive(Clone, Copy)]
pub struct Params {
    pub n: usize,          // input/output LWE dimension (blind-rotation length)
    pub k: usize,          // GLWE dimension
    pub poly: usize,       // ring degree N
    pub pbs_l: usize,      // bootstrap-key decomposition levels
    pub pbs_baselog: u32,  // bootstrap-key decomposition base log
    pub ks_l: usize,       // key-switch decomposition levels
    pub ks_baselog: u32,   // key-switch decomposition base log
    pub message_bits: u32, // total bits incl. padding bit (must == REQUIRED_MESSAGE_BITS)
    pub lwe_sigma: f64,    // input/key-switch LWE noise std-dev (secures dim n)
    pub glwe_sigma: f64,   // GLWE / bootstrap-key noise std-dev (secures dim k·N)
}

impl Params {
    /// Δ = q / 2^message_bits (the plaintext scaling).
    pub fn delta(&self) -> u64 {
        1u64 << (64 - self.message_bits)
    }
    /// Usable message modulus (after the padding bit).
    pub fn msg_modulus(&self) -> u64 {
        1u64 << (self.message_bits - 1)
    }
}

/// Estimated classical core-SVP security (bits) of `(LWE dim n, GLWE dim k·N)`. Both must be
/// ≥ [`SECURITY_BITS_REQUIRED`]. TFHE secret keys are binary.
pub fn security_bits(p: &Params) -> (f64, f64) {
    let lwe = lwe_bits(p.n, LOG2_Q, p.lwe_sigma, BINARY_SECRET_STD);
    let glwe = lwe_bits(p.k * p.poly, LOG2_Q, p.glwe_sigma, BINARY_SECRET_STD);
    (lwe, glwe)
}

/// LWE ciphertext: `b = <a, s> + μ + e`.
#[derive(Clone)]
pub struct Lwe {
    pub a: Vec<u64>,
    pub b: u64,
}

/// The LWE secret key `s ∈ {0,1}^n`.
pub struct SecretKey {
    pub lwe: Vec<u64>,
}

/// A programmable-bootstrap LUT: `values[m] = f(m)` for `m ∈ [0, msg_modulus)`.
#[derive(Clone)]
pub struct Lut {
    pub values: Vec<u64>,
}

/// Small reproducible PRNG (splitmix64) + Gaussian torus noise.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn next_bit(&mut self) -> u64 {
        self.next_u64() & 1
    }
    /// Gaussian torus noise with std-dev `sigma` (Box–Muller).
    pub fn gaussian(&mut self, sigma: f64) -> u64 {
        let u1 = ((self.next_u64() >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
        let u2 = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        (z * sigma).round() as i64 as u64
    }
}

/// Generate the LWE secret key.
pub fn gen_secret_key(p: Params, seed: u64) -> SecretKey {
    let mut rng = Rng::new(seed);
    SecretKey {
        lwe: (0..p.n).map(|_| rng.next_bit()).collect(),
    }
}

/// Encrypt a message in `[0, msg_modulus)` as a fresh input LWE under `sk` (noise σ = lwe_sigma).
pub fn encrypt(p: Params, sk: &SecretKey, message: u64, seed: u64) -> Lwe {
    let mut rng = Rng::new(seed);
    let mu = message.wrapping_mul(p.delta());
    let a: Vec<u64> = (0..p.n).map(|_| rng.next_u64()).collect();
    let mut b = mu.wrapping_add(rng.gaussian(p.lwe_sigma));
    for (ai, si) in a.iter().zip(&sk.lwe) {
        b = b.wrapping_add(ai.wrapping_mul(*si));
    }
    Lwe { a, b }
}

/// The phase `b − <a, s>`.
pub fn phase(sk: &SecretKey, ct: &Lwe) -> u64 {
    let mut ph = ct.b;
    for (ai, si) in ct.a.iter().zip(&sk.lwe) {
        ph = ph.wrapping_sub(ai.wrapping_mul(*si));
    }
    ph
}

/// Decode a phase to a message in `[0, 2^message_bits)`.
pub fn decode(p: Params, ph: u64) -> u64 {
    (ph.wrapping_add(p.delta() >> 1) >> (64 - p.message_bits)) & ((1 << p.message_bits) - 1)
}

/// Decrypt + decode.
pub fn decrypt(p: Params, sk: &SecretKey, ct: &Lwe) -> u64 {
    decode(p, phase(sk, ct))
}

/// Residual noise of `ct`: circular distance of its phase from `expected_message · Δ`.
pub fn output_noise(p: Params, sk: &SecretKey, ct: &Lwe, expected_message: u64) -> u64 {
    let ideal = expected_message.wrapping_mul(p.delta());
    let ph = phase(sk, ct);
    ph.wrapping_sub(ideal).min(ideal.wrapping_sub(ph))
}

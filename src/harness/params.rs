//! Frozen TFHE parameters, ciphertext / key types, and the encryption oracle.
//! FROZEN — do not edit as part of autoresearch.
//!
//! The harness owns all *secret* material and the LWE encode/decode: it generates the
//! secret key, encrypts (noisy) inputs, and decrypts/decodes outputs. A submission only
//! ever sees the public key material and ciphertexts (via `algorithm::keygen` /
//! `algorithm::bootstrap`), so it cannot pass the correctness gate without performing a
//! genuine, noise-reducing programmable bootstrap.
//!
//! Torus is `u64` (`q = 2^64`, wrapping arithmetic). Parameters are chosen for functional
//! correctness and representative timing — NOT 128-bit security (σ is small so the
//! round-trip gate is robust; wall-clock timing is independent of σ).

/// TFHE parameter set. `message_bits` includes the padding bit, so the usable message
/// modulus is `2^(message_bits-1)`.
#[derive(Clone, Copy)]
pub struct Params {
    pub n: usize,          // input LWE dimension (blind-rotation length)
    pub k: usize,          // GLWE dimension
    pub poly: usize,       // ring degree N
    pub pbs_l: usize,      // bootstrap-key decomposition levels
    pub pbs_baselog: u32,  // bootstrap-key decomposition base log
    pub ks_l: usize,       // key-switch decomposition levels
    pub ks_baselog: u32,   // key-switch decomposition base log
    pub message_bits: u32, // total bits incl. padding bit
    pub sigma: f64,        // input-noise std-dev in torus units
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

/// The fixed parameter set every submission is scored against.
pub fn params() -> Params {
    Params {
        n: 500,
        k: 1,
        poly: 1024,
        pbs_l: 2,
        pbs_baselog: 12,
        ks_l: 5,
        ks_baselog: 4,
        message_bits: 3, // 1 padding + 2 message bits ⇒ 4 messages
        sigma: 64.0,
    }
}

/// LWE ciphertext: `b = <a, s> + μ + e`. Fields are public so the algorithm can read the
/// input and build the refreshed output.
#[derive(Clone)]
pub struct Lwe {
    pub a: Vec<u64>,
    pub b: u64,
}

/// The LWE secret key `s ∈ {0,1}^n`. The PBS output is an LWE under `s`; the GLWE key the
/// algorithm needs internally is the algorithm's own choice (it does not affect the
/// input/output encoding), so it is not part of this type.
pub struct SecretKey {
    pub lwe: Vec<u64>,
}

/// A look-up table for the programmable bootstrap: `values[m] = f(m)` for the usable
/// messages `m ∈ [0, msg_modulus)`.
#[derive(Clone)]
pub struct Lut {
    pub values: Vec<u64>,
}

/// Small reproducible PRNG (splitmix64) + Gaussian torus noise. Available to the algorithm
/// for its (untimed) key generation.
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

/// Encrypt a message in `[0, msg_modulus)` as a fresh (noisy) input LWE under `sk`.
pub fn encrypt(p: Params, sk: &SecretKey, message: u64, seed: u64) -> Lwe {
    let mut rng = Rng::new(seed);
    let mu = message.wrapping_mul(p.delta());
    let a: Vec<u64> = (0..p.n).map(|_| rng.next_u64()).collect();
    let mut b = mu.wrapping_add(rng.gaussian(p.sigma));
    for (ai, si) in a.iter().zip(&sk.lwe) {
        b = b.wrapping_add(ai.wrapping_mul(*si));
    }
    Lwe { a, b }
}

/// The phase `b − <a, s>` of an LWE under `sk`.
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

/// Decrypt + decode an LWE under `sk`.
pub fn decrypt(p: Params, sk: &SecretKey, ct: &Lwe) -> u64 {
    decode(p, phase(sk, ct))
}

/// Residual noise of `ct`: the circular distance of its phase from the ideal
/// `expected_message · Δ` (lower = cleaner refresh).
pub fn output_noise(p: Params, sk: &SecretKey, ct: &Lwe, expected_message: u64) -> u64 {
    let ideal = expected_message.wrapping_mul(p.delta());
    let ph = phase(sk, ct);
    ph.wrapping_sub(ideal).min(ideal.wrapping_sub(ph))
}

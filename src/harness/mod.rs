//! Frozen harness — parameters, the secret/encryption oracle, fixtures, and the
//! wall-clock scoring. FROZEN — do not edit as part of autoresearch.

pub mod eval;
pub mod fixtures;
pub mod params;
pub mod security;

pub use params::{
    decode, decrypt, encrypt, failure_margin_bits, gen_secret_key, output_noise,
    output_noise_signed, phase, security_bits, Lut, Lwe, Params, Rng, SecretKey,
    REQUIRED_MESSAGE_BITS, SECURITY_BITS_REQUIRED,
};

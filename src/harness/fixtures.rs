//! Fixed correctness fixtures + the timing input. FROZEN — do not edit as part of
//! autoresearch.
//!
//! Each fixture is a (message, LUT, seed) triple. The LUTs include a non-identity function,
//! so a pass-through (no-op) cannot satisfy the gate — only a real programmable bootstrap
//! that applies `f` to the encrypted message decodes correctly.

use crate::harness::params::{Lut, REQUIRED_MESSAGE_BITS};

pub struct Fixture {
    pub name: String,
    pub message: u64,
    pub lut: Lut,
    pub seed: u64,
}

/// The fixed message modulus the challenge bootstraps over.
fn msg_modulus() -> u64 {
    1u64 << (REQUIRED_MESSAGE_BITS - 1)
}

/// The LUT used for the timing measurement (identity — refresh the message unchanged).
pub fn timing_lut() -> Lut {
    Lut {
        values: (0..msg_modulus()).collect(),
    }
}

/// The message / seed of the representative timing input (`1` is in range for any message space).
pub const TIMING_MESSAGE: u64 = 1;
pub const TIMING_SEED: u64 = 0xC0FF_EE00;

/// All scored correctness fixtures: every message under two LUTs (identity, and +1 — which for a
/// boolean space is NOT). A pass-through cannot satisfy the non-identity LUT.
pub fn all() -> Vec<Fixture> {
    let modulus = msg_modulus();
    let identity: Vec<u64> = (0..modulus).collect();
    let increment: Vec<u64> = (0..modulus).map(|m| (m + 1) % modulus).collect();

    let mut v = Vec::new();
    for m in 0..modulus {
        v.push(Fixture {
            name: format!("identity/m{m}"),
            message: m,
            lut: Lut {
                values: identity.clone(),
            },
            seed: 0x1000 + m,
        });
        v.push(Fixture {
            name: format!("increment/m{m}"),
            message: m,
            lut: Lut {
                values: increment.clone(),
            },
            seed: 0x2000 + m,
        });
    }
    v
}

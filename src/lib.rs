//! Crate root.
//!
//! FROZEN FILE — do not edit as part of autoresearch. It only wires the editable
//! `algorithm` module to the frozen `harness`.

pub mod algorithm;
pub mod harness;

/// The stable contract the harness depends on.
pub use algorithm::{bootstrap, keygen, params, ServerKey};
pub use harness::{Lut, Lwe, Params, SecretKey};

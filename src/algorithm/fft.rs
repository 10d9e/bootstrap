//! Approximate negacyclic polynomial multiplication via an f64 complex FFT — the
//! "lighter transform" TFHE uses. FFT rounding error lands in the low bits, well below the
//! LWE noise floor, so it is harmless for bootstrapping.
//!
//! For `R = Z_q[X]/(X^N + 1)` with **real** coefficients we use a length-`N/2` complex FFT:
//! pack even/odd coefficients into one complex stream, run a half-size *negacyclic* DFT (a
//! cyclic FFT pre-twisted by `ψ^j`, `ψ = exp(-iπ/M)`, `M = N/2`), then untangle the even/odd
//! negacyclic sub-spectra with a final radix-2 step. This halves the FFT size (and all the
//! spectral pointwise work) vs a full length-`N` transform. Torus (`u64`) is carried as
//! centered `i64 → f64`.
//!
//! This whole file is editable — swap it for an NTT, a different FFT, target SIMD, etc.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::cell::RefCell;
use std::sync::Arc;

type Cf = Complex<f64>;

/// A spectrum of a degree-`<N` real polynomial: `N/2` independent complex values.
pub type Fourier = Vec<Cf>;

/// Reusable negacyclic-FFT context (size `N`, internal half-size `M = N/2`).
pub struct NegacyclicFft {
    n: usize,
    m: usize,
    fwd: Arc<dyn Fft<f64>>,
    inv: Arc<dyn Fft<f64>>,
    twist: Vec<Cf>,          // ψ_M^j = exp(-iπ j / M)
    untwist_scaled: Vec<Cf>, // conj(ψ_M^j) / M
    omega: Vec<Cf>,          // exp(-iπ(2k+1)/N)
    omega_inv_half: Vec<Cf>, // 0.5 / omega[k]
    scratch: RefCell<Vec<Cf>>,
}

impl NegacyclicFft {
    pub fn new(n: usize) -> Self {
        assert!(n % 2 == 0);
        let m = n / 2;
        let mut planner = FftPlanner::new();
        let fwd = planner.plan_fft_forward(m);
        let inv = planner.plan_fft_inverse(m);
        let twist: Vec<Cf> = (0..m)
            .map(|j| Complex::from_polar(1.0, -std::f64::consts::PI * j as f64 / m as f64))
            .collect();
        let untwist_scaled = twist.iter().map(|w| w.conj() / m as f64).collect();
        let omega: Vec<Cf> = (0..m)
            .map(|k| Complex::from_polar(1.0, -std::f64::consts::PI * (2 * k + 1) as f64 / n as f64))
            .collect();
        let omega_inv_half = omega.iter().map(|w| 0.5 / w).collect();
        let slen = fwd
            .get_inplace_scratch_len()
            .max(inv.get_inplace_scratch_len());
        Self {
            n,
            m,
            fwd,
            inv,
            twist,
            untwist_scaled,
            omega,
            omega_inv_half,
            scratch: RefCell::new(vec![Complex::new(0.0, 0.0); slen]),
        }
    }

    pub fn n(&self) -> usize {
        self.n
    }
    pub fn spectrum_len(&self) -> usize {
        self.m
    }

    fn forward(&self, real: impl Fn(usize) -> f64) -> Fourier {
        let m = self.m;
        let mut buf: Fourier = (0..m)
            .map(|j| Complex::new(real(2 * j), real(2 * j + 1)) * self.twist[j])
            .collect();
        self.fwd
            .process_with_scratch(&mut buf, &mut self.scratch.borrow_mut());
        let mut spec = vec![Complex::new(0.0, 0.0); m];
        let half_i = Complex::new(0.0, -0.5);
        for k in 0..m {
            let ck = buf[k];
            let cc = buf[m - 1 - k].conj();
            spec[k] = (ck + cc) * 0.5 + self.omega[k] * ((ck - cc) * half_i);
        }
        spec
    }

    /// Forward transform of a torus polynomial (`u64` reinterpreted as centered `i64`).
    pub fn forward_torus(&self, coeffs: &[u64]) -> Fourier {
        self.forward(|i| (coeffs[i] as i64) as f64)
    }

    /// Allocation-free forward of signed coefficients into `spec_out`, using `pack` as
    /// scratch (both length `N/2`).
    pub fn forward_signed_into(&self, coeffs: &[i64], spec_out: &mut [Cf], pack: &mut [Cf]) {
        let m = self.m;
        for j in 0..m {
            pack[j] = Complex::new(coeffs[2 * j] as f64, coeffs[2 * j + 1] as f64) * self.twist[j];
        }
        self.fwd
            .process_with_scratch(pack, &mut self.scratch.borrow_mut());
        let half_i = Complex::new(0.0, -0.5);
        for k in 0..m {
            let ck = pack[k];
            let cc = pack[m - 1 - k].conj();
            spec_out[k] = (ck + cc) * 0.5 + self.omega[k] * ((ck - cc) * half_i);
        }
    }

    /// Allocation-free inverse of `spec` into torus coefficients `out`, using `buf` scratch.
    pub fn inverse_into(&self, spec: &[Cf], out: &mut [u64], buf: &mut [Cf]) {
        let m = self.m;
        for k in 0..m {
            let sk = spec[k];
            let skm = spec[m - 1 - k].conj();
            let e = (sk + skm) * 0.5;
            let o = (sk - skm) * self.omega_inv_half[k];
            buf[k] = e + Complex::new(-o.im, o.re);
        }
        self.inv
            .process_with_scratch(buf, &mut self.scratch.borrow_mut());
        let two64 = 2.0f64.powi(64);
        let inv_two64 = 2.0f64.powi(-64);
        let reduce = |v: f64| -> u64 {
            let r = v - (v * inv_two64).round() * two64;
            r.round() as i64 as u64
        };
        for j in 0..m {
            let c = buf[j] * self.untwist_scaled[j];
            out[2 * j] = reduce(c.re);
            out[2 * j + 1] = reduce(c.im);
        }
    }

    /// Inverse of a spectrum back to torus coefficients (allocating; for key generation).
    pub fn inverse_to_torus(&self, spec: Fourier, out: &mut [u64]) {
        let buf = spec;
        let m = self.m;
        let mut tmp = vec![Complex::new(0.0, 0.0); m];
        for k in 0..m {
            let sk = buf[k];
            let skm = buf[m - 1 - k].conj();
            let e = (sk + skm) * 0.5;
            let o = (sk - skm) * self.omega_inv_half[k];
            tmp[k] = e + Complex::new(-o.im, o.re);
        }
        self.inv
            .process_with_scratch(&mut tmp, &mut self.scratch.borrow_mut());
        let two64 = 2.0f64.powi(64);
        let inv_two64 = 2.0f64.powi(-64);
        let reduce = |v: f64| -> u64 {
            let r = v - (v * inv_two64).round() * two64;
            r.round() as i64 as u64
        };
        for j in 0..m {
            let c = tmp[j] * self.untwist_scaled[j];
            out[2 * j] = reduce(c.re);
            out[2 * j + 1] = reduce(c.im);
        }
    }
}

/// In place, `acc += a · b` (complex pointwise multiply-accumulate over a spectrum).
#[inline]
pub fn fma(acc: &mut [Cf], a: &[Cf], b: &[Cf]) {
    for (acc, (a, b)) in acc.iter_mut().zip(a.iter().zip(b.iter())) {
        *acc += a * b;
    }
}

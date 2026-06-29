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
//! The length-`M` complex FFT is a **hand-written, allocation-free, split-format** (SoA: real
//! and imaginary parts in separate arrays) **radix-4** Cooley–Tukey (each radix-4 stage fuses
//! two radix-2 stages, halving the memory passes), with one trailing radix-2 stage when
//! `log2 M` is odd. SoA layout lets the negacyclic twist fuse into the load, the untangle into
//! the store, and the butterfly loops vectorize cleanly (see [`simd`]). No external FFT crate —
//! `num_complex::Complex` is only the public `(re, im)` value type at the spectrum boundary.
//!
//! This whole file is editable — swap it for an NTT, a different FFT, target SIMD, etc.

use rustfft::num_complex::Complex;
use std::cell::RefCell;

type Cf = Complex<f64>;

/// A spectrum of a degree-`<N` real polynomial: `M = N/2` complex values in **split format**,
/// a flat `[re(0..M) | im(M..2M)]` `f64` array. SoA so the pointwise MAC ([`fma`]) and the
/// FFT↔spectrum boundary vectorize without AoS shuffles.
pub type Fourier = Vec<f64>;

/// Per-call SoA scratch (real/imag arrays of length `M`).
struct Scratch {
    re: Vec<f64>,
    im: Vec<f64>,
}

/// Twiddles for one fused **radix-4** stage over `4·h`-sized blocks (quad stride `h`). `wa = ω_2h^j`
/// is the inner (radix-2) twiddle; `w0 = ω_4h^j` the outer; both `j = 0..h`, unit stride. `_f`/`_i`
/// are forward / inverse (conjugated) imaginary parts.
struct R4Stage {
    h: usize,
    wa_re: Vec<f64>,
    wa_im_f: Vec<f64>,
    wa_im_i: Vec<f64>,
    w0_re: Vec<f64>,
    w0_im_f: Vec<f64>,
    w0_im_i: Vec<f64>,
}

/// Twiddles for a trailing radix-2 stage (`w[j] = ω_2h^j`, `j = 0..half`).
struct R2Stage {
    half: usize,
    wr: Vec<f64>,
    wi_f: Vec<f64>,
    wi_i: Vec<f64>,
}

/// Reusable negacyclic-FFT context (size `N`, internal half-size `M = N/2`).
pub struct NegacyclicFft {
    n: usize,
    m: usize,
    bitrev: Vec<u32>,           // bit-reversal permutation of 0..M
    r4: Vec<R4Stage>,           // fused radix-4 stages (quad stride h = 1,4,16,…)
    tail: Option<R2Stage>,      // trailing radix-2 stage when log2(M) is odd
    twist: Vec<Cf>,             // ψ_M^j = exp(-iπ j / M)
    untwist_scaled: Vec<Cf>,    // conj(ψ_M^j) / M
    omega: Vec<Cf>,             // exp(-iπ(2k+1)/N)
    omega_inv_half: Vec<Cf>,    // 0.5 / omega[k]
    scratch: RefCell<Scratch>,
}

impl NegacyclicFft {
    pub fn new(n: usize) -> Self {
        assert!(n % 2 == 0);
        let m = n / 2;
        assert!(m.is_power_of_two(), "half-size M = N/2 must be a power of two");

        let bits = m.trailing_zeros();
        let bitrev: Vec<u32> = (0..m)
            .map(|i| (i as u32).reverse_bits() >> (32 - bits))
            .collect();

        // Fused radix-4 stages: quad stride h = 1, 4, 16, …, each covering two radix-2 stages
        // (sub-transform sizes 2h then 4h). After k stages the transform size is 4^k = h.
        let mut r4 = Vec::new();
        let mut h = 1usize;
        while 4 * h <= m {
            let l2 = 2 * h; // inner sub-transform size
            let l4 = 4 * h; // outer sub-transform size
            let wa: Vec<Cf> = (0..h)
                .map(|j| Complex::from_polar(1.0, -2.0 * std::f64::consts::PI * j as f64 / l2 as f64))
                .collect();
            let w0: Vec<Cf> = (0..h)
                .map(|j| Complex::from_polar(1.0, -2.0 * std::f64::consts::PI * j as f64 / l4 as f64))
                .collect();
            r4.push(R4Stage {
                h,
                wa_re: wa.iter().map(|c| c.re).collect(),
                wa_im_f: wa.iter().map(|c| c.im).collect(),
                wa_im_i: wa.iter().map(|c| -c.im).collect(),
                w0_re: w0.iter().map(|c| c.re).collect(),
                w0_im_f: w0.iter().map(|c| c.im).collect(),
                w0_im_i: w0.iter().map(|c| -c.im).collect(),
            });
            h *= 4;
        }
        // Trailing radix-2 stage if log2(M) is odd (h covered only half of M).
        let tail = if h < m {
            let len = 2 * h; // == m
            debug_assert_eq!(len, m);
            let w: Vec<Cf> = (0..h)
                .map(|j| Complex::from_polar(1.0, -2.0 * std::f64::consts::PI * j as f64 / len as f64))
                .collect();
            Some(R2Stage {
                half: h,
                wr: w.iter().map(|c| c.re).collect(),
                wi_f: w.iter().map(|c| c.im).collect(),
                wi_i: w.iter().map(|c| -c.im).collect(),
            })
        } else {
            None
        };

        let twist: Vec<Cf> = (0..m)
            .map(|j| Complex::from_polar(1.0, -std::f64::consts::PI * j as f64 / m as f64))
            .collect();
        let untwist_scaled = twist.iter().map(|w| w.conj() / m as f64).collect();
        let omega: Vec<Cf> = (0..m)
            .map(|k| Complex::from_polar(1.0, -std::f64::consts::PI * (2 * k + 1) as f64 / n as f64))
            .collect();
        let omega_inv_half = omega.iter().map(|w| 0.5 / w).collect();
        Self {
            n,
            m,
            bitrev,
            r4,
            tail,
            twist,
            untwist_scaled,
            omega,
            omega_inv_half,
            scratch: RefCell::new(Scratch {
                re: vec![0.0; m],
                im: vec![0.0; m],
            }),
        }
    }

    pub fn n(&self) -> usize {
        self.n
    }
    pub fn spectrum_len(&self) -> usize {
        self.m
    }

    /// In-place length-`M` complex FFT (radix-4 + optional radix-2 DIT, split format, no
    /// normalization), matching rustfft's unnormalized convention. `forward` selects sign.
    /// Input in natural order (does the bit-reversal); used by the correctness test.
    #[cfg(test)]
    fn transform(&self, re: &mut [f64], im: &mut [f64], forward: bool) {
        let m = self.m;
        let rev = &self.bitrev;
        for i in 0..m {
            let j = rev[i] as usize;
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        self.stages(re, im, forward);
    }

    /// The DIT stage loop, **assuming input is already bit-reversed**. The hot paths fold the
    /// bit-reversal into their load (twist-pack / omega) so it costs no extra pass.
    #[inline]
    fn stages(&self, re: &mut [f64], im: &mut [f64], forward: bool) {
        let m = self.m;
        let isign = if forward { 1.0 } else { -1.0 };
        for st in &self.r4 {
            let (wa_im, w0_im) = if forward {
                (&st.wa_im_f, &st.w0_im_f)
            } else {
                (&st.wa_im_i, &st.w0_im_i)
            };
            let block = 4 * st.h;
            let mut base = 0;
            while base < m {
                simd::butterfly4(
                    &mut re[base..base + block],
                    &mut im[base..base + block],
                    &st.wa_re,
                    wa_im,
                    &st.w0_re,
                    w0_im,
                    st.h,
                    isign,
                );
                base += block;
            }
        }
        if let Some(st) = &self.tail {
            let wi = if forward { &st.wi_f } else { &st.wi_i };
            let len = st.half << 1;
            let mut start = 0;
            while start < m {
                simd::butterfly2(
                    &mut re[start..start + len],
                    &mut im[start..start + len],
                    &st.wr,
                    wi,
                    st.half,
                );
                start += len;
            }
        }
    }

    fn forward(&self, real: impl Fn(usize) -> f64) -> Fourier {
        let m = self.m;
        let mut sc = self.scratch.borrow_mut();
        let Scratch { re, im } = &mut *sc;
        for j in 0..m {
            let cr = real(2 * j);
            let ci = real(2 * j + 1);
            let t = self.twist[j];
            let r = self.bitrev[j] as usize; // fold bit-reversal into the load
            re[r] = cr * t.re - ci * t.im;
            im[r] = cr * t.im + ci * t.re;
        }
        self.stages(re, im, true);
        let mut spec = vec![0.0; 2 * m];
        self.untangle_forward(re, im, &mut spec);
        spec
    }

    /// Forward transform of a torus polynomial (`u64` reinterpreted as centered `i64`).
    pub fn forward_torus(&self, coeffs: &[u64]) -> Fourier {
        self.forward(|i| (coeffs[i] as i64) as f64)
    }

    /// Allocation-free forward of signed coefficients into split-format `spec_out` (length `2M`).
    pub fn forward_signed_into(&self, coeffs: &[i64], spec_out: &mut [f64]) {
        let m = self.m;
        let mut sc = self.scratch.borrow_mut();
        let Scratch { re, im } = &mut *sc;
        for j in 0..m {
            let cr = coeffs[2 * j] as f64;
            let ci = coeffs[2 * j + 1] as f64;
            let t = self.twist[j];
            let r = self.bitrev[j] as usize; // fold bit-reversal into the twist-pack load
            re[r] = cr * t.re - ci * t.im;
            im[r] = cr * t.im + ci * t.re;
        }
        self.stages(re, im, true);
        self.untangle_forward(re, im, spec_out);
    }

    /// The even/odd negacyclic untangle: combine the half-size cyclic spectrum (SoA `re`/`im`)
    /// into the `M` negacyclic spectral coefficients, written split-format into `spec`.
    #[inline]
    fn untangle_forward(&self, re: &[f64], im: &[f64], spec: &mut [f64]) {
        let m = self.m;
        let half_i = Complex::new(0.0, -0.5);
        let (sre, sim) = spec.split_at_mut(m);
        for k in 0..m {
            let km = m - 1 - k;
            let ck = Complex::new(re[k], im[k]);
            let cc = Complex::new(re[km], -im[km]);
            let v = (ck + cc) * 0.5 + self.omega[k] * ((ck - cc) * half_i);
            sre[k] = v.re;
            sim[k] = v.im;
        }
    }

    /// Allocation-free inverse of split-format `spec` (length `2M`) into torus coefficients `out`.
    pub fn inverse_into(&self, spec: &[f64], out: &mut [u64]) {
        let m = self.m;
        let mut sc = self.scratch.borrow_mut();
        let Scratch { re, im } = &mut *sc;
        let (sre, sim) = spec.split_at(m);
        for k in 0..m {
            let km = m - 1 - k;
            let sk = Complex::new(sre[k], sim[k]);
            let skm = Complex::new(sre[km], -sim[km]);
            let e = (sk + skm) * 0.5;
            let o = (sk - skm) * self.omega_inv_half[k];
            let r = self.bitrev[k] as usize; // fold bit-reversal into the omega load
            re[r] = e.re - o.im;
            im[r] = e.im + o.re;
        }
        self.stages(re, im, false);
        let two64 = 2.0f64.powi(64);
        let inv_two64 = 2.0f64.powi(-64);
        let reduce = |v: f64| -> u64 {
            let r = v - (v * inv_two64).round() * two64;
            r.round() as i64 as u64
        };
        for j in 0..m {
            let t = self.untwist_scaled[j];
            let cr = re[j] * t.re - im[j] * t.im;
            let ci = re[j] * t.im + im[j] * t.re;
            out[2 * j] = reduce(cr);
            out[2 * j + 1] = reduce(ci);
        }
    }

    /// Inverse of a spectrum back to torus coefficients (allocating; for key generation).
    pub fn inverse_to_torus(&self, spec: Fourier, out: &mut [u64]) {
        self.inverse_into(&spec, out);
    }
}

/// In place, `acc += a · b`: split-format complex pointwise multiply-accumulate over a spectrum
/// (all length `2M`, `[re(M) | im(M)]`). Auto-vectorizes (NEON/AVX2) — the hot loop of the
/// GGSW external product.
#[inline]
pub fn fma(acc: &mut [f64], a: &[f64], b: &[f64]) {
    let m = acc.len() / 2;
    let (a_re, a_im) = a.split_at(m);
    let (b_re, b_im) = b.split_at(m);
    let (acc_re, acc_im) = acc.split_at_mut(m);
    for i in 0..m {
        acc_re[i] += a_re[i] * b_re[i] - a_im[i] * b_im[i];
        acc_im[i] += a_re[i] * b_im[i] + a_im[i] * b_re[i];
    }
}

/// SIMD butterfly kernels (split format, unit-stride twiddles). `butterfly4` is one fused
/// radix-4 stage over a `4·h` block (quad stride `h`); `butterfly2` is one radix-2 stage.
/// Both dispatch to NEON (aarch64) / AVX2+FMA (x86_64), else an auto-vectorized scalar form.
mod simd {
    /// Fused radix-4 DIT butterfly (equivalent to two radix-2 stages). For each `j`:
    /// quad `(a,b,c,d) = arr[base+{j, j+h, j+2h, j+3h}]`, inner twiddle `wa = ω_2h^j`, outer
    /// `w0 = ω_4h^j`, and `wB1 = -i·w0` (forward) via the `isign` sign on the imaginary cross-term:
    /// ```text
    /// a'=a+wa·b  b'=a−wa·b   c'=c+wa·d  d'=c−wa·d
    /// e=w0·c'    f=(∓i)·(w0·d')
    /// out = [a'+e, b'+f, a'−e, b'−f]   at  j, j+h, j+2h, j+3h
    /// ```
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn butterfly4(
        re: &mut [f64],
        im: &mut [f64],
        wa_re: &[f64],
        wa_im: &[f64],
        w0_re: &[f64],
        w0_im: &[f64],
        h: usize,
        isign: f64,
    ) {
        #[cfg(target_arch = "aarch64")]
        {
            if h >= 2 {
                unsafe { neon4(re, im, wa_re, wa_im, w0_re, w0_im, h, isign) };
                return;
            }
        }
        #[cfg(target_arch = "x86_64")]
        {
            if h >= 4
                && std::is_x86_feature_detected!("avx2")
                && std::is_x86_feature_detected!("fma")
            {
                unsafe { avx4(re, im, wa_re, wa_im, w0_re, w0_im, h, isign) };
                return;
            }
        }
        scalar4(re, im, wa_re, wa_im, w0_re, w0_im, h, isign);
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn scalar4(
        re: &mut [f64],
        im: &mut [f64],
        wa_re: &[f64],
        wa_im: &[f64],
        w0_re: &[f64],
        w0_im: &[f64],
        h: usize,
        isign: f64,
    ) {
        for j in 0..h {
            quad_scalar(re, im, j, h, wa_re[j], wa_im[j], w0_re[j], w0_im[j], isign);
        }
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn quad_scalar(
        re: &mut [f64],
        im: &mut [f64],
        j: usize,
        h: usize,
        war: f64,
        wai: f64,
        w0r: f64,
        w0i: f64,
        isign: f64,
    ) {
        let (i0, i1, i2, i3) = (j, j + h, j + 2 * h, j + 3 * h);
        let (ar, ai) = (re[i0], im[i0]);
        let (br, bi) = (re[i1], im[i1]);
        let (cr, ci) = (re[i2], im[i2]);
        let (dr, di) = (re[i3], im[i3]);
        // a' = a + wa·b ; b' = a − wa·b
        let wbr = br * war - bi * wai;
        let wbi = br * wai + bi * war;
        let (apr, api) = (ar + wbr, ai + wbi);
        let (bpr, bpi) = (ar - wbr, ai - wbi);
        // c' = c + wa·d ; d' = c − wa·d
        let wdr = dr * war - di * wai;
        let wdi = dr * wai + di * war;
        let (cpr, cpi) = (cr + wdr, ci + wdi);
        let (dpr, dpi) = (cr - wdr, ci - wdi);
        // e = w0·c'
        let er = cpr * w0r - cpi * w0i;
        let ei = cpr * w0i + cpi * w0r;
        // g = w0·d' ; f = (∓i)·g  →  f = (isign·g.im, −isign·g.re)
        let gr = dpr * w0r - dpi * w0i;
        let gi = dpr * w0i + dpi * w0r;
        let fr = isign * gi;
        let fi = -isign * gr;
        re[i0] = apr + er;
        im[i0] = api + ei;
        re[i2] = apr - er;
        im[i2] = api - ei;
        re[i1] = bpr + fr;
        im[i1] = bpi + fi;
        re[i3] = bpr - fr;
        im[i3] = bpi - fi;
    }

    #[cfg(target_arch = "aarch64")]
    #[target_feature(enable = "neon")]
    #[allow(clippy::too_many_arguments)]
    unsafe fn neon4(
        re: &mut [f64],
        im: &mut [f64],
        wa_re: &[f64],
        wa_im: &[f64],
        w0_re: &[f64],
        w0_im: &[f64],
        h: usize,
        isign: f64,
    ) {
        use std::arch::aarch64::*;
        let rp = re.as_mut_ptr();
        let ip = im.as_mut_ptr();
        let sign = vdupq_n_f64(isign);
        let nsign = vdupq_n_f64(-isign);
        let chunks = h / 2;
        for c in 0..chunks {
            let o = c * 2;
            let (o0, o1, o2, o3) = (o, o + h, o + 2 * h, o + 3 * h);
            let war = vld1q_f64(wa_re.as_ptr().add(o));
            let wai = vld1q_f64(wa_im.as_ptr().add(o));
            let w0r = vld1q_f64(w0_re.as_ptr().add(o));
            let w0i = vld1q_f64(w0_im.as_ptr().add(o));
            let ar = vld1q_f64(rp.add(o0));
            let ai = vld1q_f64(ip.add(o0));
            let br = vld1q_f64(rp.add(o1));
            let bi = vld1q_f64(ip.add(o1));
            let cr = vld1q_f64(rp.add(o2));
            let ci = vld1q_f64(ip.add(o2));
            let dr = vld1q_f64(rp.add(o3));
            let di = vld1q_f64(ip.add(o3));
            // wa·b, wa·d
            let wbr = vfmsq_f64(vmulq_f64(br, war), bi, wai);
            let wbi = vfmaq_f64(vmulq_f64(br, wai), bi, war);
            let wdr = vfmsq_f64(vmulq_f64(dr, war), di, wai);
            let wdi = vfmaq_f64(vmulq_f64(dr, wai), di, war);
            let apr = vaddq_f64(ar, wbr);
            let api = vaddq_f64(ai, wbi);
            let bpr = vsubq_f64(ar, wbr);
            let bpi = vsubq_f64(ai, wbi);
            let cpr = vaddq_f64(cr, wdr);
            let cpi = vaddq_f64(ci, wdi);
            let dpr = vsubq_f64(cr, wdr);
            let dpi = vsubq_f64(ci, wdi);
            // e = w0·c'
            let er = vfmsq_f64(vmulq_f64(cpr, w0r), cpi, w0i);
            let ei = vfmaq_f64(vmulq_f64(cpr, w0i), cpi, w0r);
            // g = w0·d' ; f = (isign·g.im, −isign·g.re)
            let gr = vfmsq_f64(vmulq_f64(dpr, w0r), dpi, w0i);
            let gi = vfmaq_f64(vmulq_f64(dpr, w0i), dpi, w0r);
            let fr = vmulq_f64(sign, gi);
            let fi = vmulq_f64(nsign, gr);
            vst1q_f64(rp.add(o0), vaddq_f64(apr, er));
            vst1q_f64(ip.add(o0), vaddq_f64(api, ei));
            vst1q_f64(rp.add(o2), vsubq_f64(apr, er));
            vst1q_f64(ip.add(o2), vsubq_f64(api, ei));
            vst1q_f64(rp.add(o1), vaddq_f64(bpr, fr));
            vst1q_f64(ip.add(o1), vaddq_f64(bpi, fi));
            vst1q_f64(rp.add(o3), vsubq_f64(bpr, fr));
            vst1q_f64(ip.add(o3), vsubq_f64(bpi, fi));
        }
        for j in chunks * 2..h {
            quad_scalar(re, im, j, h, wa_re[j], wa_im[j], w0_re[j], w0_im[j], isign);
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    #[allow(clippy::too_many_arguments)]
    unsafe fn avx4(
        re: &mut [f64],
        im: &mut [f64],
        wa_re: &[f64],
        wa_im: &[f64],
        w0_re: &[f64],
        w0_im: &[f64],
        h: usize,
        isign: f64,
    ) {
        use std::arch::x86_64::*;
        let rp = re.as_mut_ptr();
        let ip = im.as_mut_ptr();
        let sign = _mm256_set1_pd(isign);
        let nsign = _mm256_set1_pd(-isign);
        let chunks = h / 4;
        for c in 0..chunks {
            let o = c * 4;
            let (o0, o1, o2, o3) = (o, o + h, o + 2 * h, o + 3 * h);
            let war = _mm256_loadu_pd(wa_re.as_ptr().add(o));
            let wai = _mm256_loadu_pd(wa_im.as_ptr().add(o));
            let w0r = _mm256_loadu_pd(w0_re.as_ptr().add(o));
            let w0i = _mm256_loadu_pd(w0_im.as_ptr().add(o));
            let ar = _mm256_loadu_pd(rp.add(o0));
            let ai = _mm256_loadu_pd(ip.add(o0));
            let br = _mm256_loadu_pd(rp.add(o1));
            let bi = _mm256_loadu_pd(ip.add(o1));
            let cr = _mm256_loadu_pd(rp.add(o2));
            let ci = _mm256_loadu_pd(ip.add(o2));
            let dr = _mm256_loadu_pd(rp.add(o3));
            let di = _mm256_loadu_pd(ip.add(o3));
            let wbr = _mm256_fmsub_pd(br, war, _mm256_mul_pd(bi, wai));
            let wbi = _mm256_fmadd_pd(br, wai, _mm256_mul_pd(bi, war));
            let wdr = _mm256_fmsub_pd(dr, war, _mm256_mul_pd(di, wai));
            let wdi = _mm256_fmadd_pd(dr, wai, _mm256_mul_pd(di, war));
            let apr = _mm256_add_pd(ar, wbr);
            let api = _mm256_add_pd(ai, wbi);
            let bpr = _mm256_sub_pd(ar, wbr);
            let bpi = _mm256_sub_pd(ai, wbi);
            let cpr = _mm256_add_pd(cr, wdr);
            let cpi = _mm256_add_pd(ci, wdi);
            let dpr = _mm256_sub_pd(cr, wdr);
            let dpi = _mm256_sub_pd(ci, wdi);
            let er = _mm256_fmsub_pd(cpr, w0r, _mm256_mul_pd(cpi, w0i));
            let ei = _mm256_fmadd_pd(cpr, w0i, _mm256_mul_pd(cpi, w0r));
            let gr = _mm256_fmsub_pd(dpr, w0r, _mm256_mul_pd(dpi, w0i));
            let gi = _mm256_fmadd_pd(dpr, w0i, _mm256_mul_pd(dpi, w0r));
            let fr = _mm256_mul_pd(sign, gi);
            let fi = _mm256_mul_pd(nsign, gr);
            _mm256_storeu_pd(rp.add(o0), _mm256_add_pd(apr, er));
            _mm256_storeu_pd(ip.add(o0), _mm256_add_pd(api, ei));
            _mm256_storeu_pd(rp.add(o2), _mm256_sub_pd(apr, er));
            _mm256_storeu_pd(ip.add(o2), _mm256_sub_pd(api, ei));
            _mm256_storeu_pd(rp.add(o1), _mm256_add_pd(bpr, fr));
            _mm256_storeu_pd(ip.add(o1), _mm256_add_pd(bpi, fi));
            _mm256_storeu_pd(rp.add(o3), _mm256_sub_pd(bpr, fr));
            _mm256_storeu_pd(ip.add(o3), _mm256_sub_pd(bpi, fi));
        }
        for j in chunks * 4..h {
            quad_scalar(re, im, j, h, wa_re[j], wa_im[j], w0_re[j], w0_im[j], isign);
        }
    }

    // ----- radix-2 (trailing stage) -----

    #[inline]
    pub fn butterfly2(re: &mut [f64], im: &mut [f64], wr: &[f64], wi: &[f64], half: usize) {
        let (lo_re, hi_re) = re.split_at_mut(half);
        let (lo_im, hi_im) = im.split_at_mut(half);
        for j in 0..half {
            let (wrj, wij) = (wr[j], wi[j]);
            let (xr, xi) = (hi_re[j], hi_im[j]);
            let br = xr * wrj - xi * wij;
            let bi = xr * wij + xi * wrj;
            let (ar, ai) = (lo_re[j], lo_im[j]);
            lo_re[j] = ar + br;
            lo_im[j] = ai + bi;
            hi_re[j] = ar - br;
            hi_im[j] = ai - bi;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Naive O(M²) DFT reference for the internal cyclic transform.
    fn naive(re: &[f64], im: &[f64], forward: bool) -> (Vec<f64>, Vec<f64>) {
        let m = re.len();
        let sign = if forward { -1.0 } else { 1.0 };
        let (mut or, mut oi) = (vec![0.0; m], vec![0.0; m]);
        for k in 0..m {
            for j in 0..m {
                let ang = sign * 2.0 * std::f64::consts::PI * (j * k) as f64 / m as f64;
                let (c, s) = (ang.cos(), ang.sin());
                or[k] += re[j] * c - im[j] * s;
                oi[k] += re[j] * s + im[j] * c;
            }
        }
        (or, oi)
    }

    fn check(n: usize) {
        let f = NegacyclicFft::new(n);
        let m = f.m;
        for &fwd in &[true, false] {
            let re0: Vec<f64> = (0..m).map(|j| (j as f64 * 1.3 + 0.2).sin() * 7.0).collect();
            let im0: Vec<f64> = (0..m).map(|j| (j as f64 * 0.7 - 0.1).cos() * 3.0).collect();
            let (mut re, mut im) = (re0.clone(), im0.clone());
            f.transform(&mut re, &mut im, fwd);
            let (er, ei) = naive(&re0, &im0, fwd);
            for k in 0..m {
                assert!((re[k] - er[k]).abs() < 1e-7, "n={n} re[{k}] fwd={fwd}: {} vs {}", re[k], er[k]);
                assert!((im[k] - ei[k]).abs() < 1e-7, "n={n} im[{k}] fwd={fwd}");
            }
        }
    }

    #[test]
    fn transform_matches_naive_dft() {
        // Even log2(M): M=16,64 (pure radix-4); odd: M=8,32,512 (radix-4 + trailing radix-2).
        for &n in &[16usize, 32, 64, 128, 1024] {
            check(n);
        }
    }

    #[test]
    #[ignore] // timing probe: cargo test --release fft::tests::timing -- --ignored --nocapture
    fn timing() {
        use std::time::Instant;
        let f = NegacyclicFft::new(1024); // N=1024, M=512 (our bootstrap size)
        let coeffs: Vec<i64> = (0..1024i64).map(|j| (j.wrapping_mul(2654435761) >> 20) & 0xffff).collect();
        let mut spec = vec![0.0f64; 1024];
        let mut out = vec![0u64; 1024];
        let iters = 200_000;
        let t0 = Instant::now();
        for _ in 0..iters {
            f.forward_signed_into(std::hint::black_box(&coeffs), &mut spec);
            std::hint::black_box(&spec);
        }
        let fwd = t0.elapsed().as_nanos() as f64 / iters as f64;
        let t1 = Instant::now();
        for _ in 0..iters {
            f.inverse_into(std::hint::black_box(&spec), &mut out);
            std::hint::black_box(&out);
        }
        let inv = t1.elapsed().as_nanos() as f64 / iters as f64;
        eprintln!("forward {fwd:.0} ns/op   inverse {inv:.0} ns/op");
    }
}

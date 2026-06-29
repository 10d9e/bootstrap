//! Algorithm entry point — the TFHE **programmable bootstrap**.
//!
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │ FROZEN CONTRACT — do NOT change these signatures:                   │
//! │     pub struct ServerKey;                                           │
//! │     pub fn params() -> Params;                                      │
//! │     pub fn keygen(sk: &SecretKey, seed: u64) -> ServerKey;          │
//! │     pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe;   │
//! │ The bodies, and everything in this directory, are yours to improve. │
//! │ Invariant: bootstrap(encrypt(m)) must decrypt to lut[m] under the   │
//! │ input LWE key, with reduced (refreshed) noise.                      │
//! └─────────────────────────────────────────────────────────────────────┘
//!
//! `params()` declares the parameter set. The harness gates it at **≥128-bit security**
//! (standard lattice-estimator model over the LWE dim `n` and GLWE dim `k·N`) and fixes `message_bits`;
//! within that, any `n`, `k`, `N`, decomposition, and noise are fair game. `keygen` builds
//! the public bootstrap material from the secret key (picking its own internal GLWE key) and
//! is NOT timed. `bootstrap` is the timed operation: a CGGI programmable bootstrap —
//! accumulator init + `n` CMux external products (blind rotation) + sample-extract +
//! key-switch — over an approximate f64 complex-FFT negacyclic multiplier ([`fft`]).

mod fft;

use crate::harness::{Lut, Lwe, Params, Rng, SecretKey};
use fft::{fma, Fourier, NegacyclicFft};

// ---------------------------------------------------------------------------
// Internal ciphertext types
// ---------------------------------------------------------------------------

/// GLWE ciphertext: `k` mask polynomials + a body, degree `<N`.
#[derive(Clone)]
struct Glwe {
    mask: Vec<Vec<u64>>,
    body: Vec<u64>,
}

impl Glwe {
    fn trivial(k: usize, body: Vec<u64>) -> Self {
        let n = body.len();
        Glwe {
            mask: vec![vec![0u64; n]; k],
            body,
        }
    }
    fn comp(&self, i: usize) -> &[u64] {
        if i < self.mask.len() {
            &self.mask[i]
        } else {
            &self.body
        }
    }
    fn comp_mut(&mut self, i: usize) -> &mut [u64] {
        if i < self.mask.len() {
            &mut self.mask[i]
        } else {
            &mut self.body
        }
    }
}

/// A GGSW row in the Fourier domain (a GLWE: `k+1` spectra).
type GgswRow = Vec<Fourier>;
/// GGSW ciphertext in the Fourier domain: `(k+1)·ℓ` rows.
struct GgswFourier {
    rows: Vec<GgswRow>,
}

/// Reusable scratch buffers for the blind-rotation hot loop (no allocation per CMux). Spectra
/// are split-format `[re(M) | im(M)]`, length `2M`.
struct FftWs {
    digits: Vec<Vec<i64>>, // pbs_l × N
    spec: Vec<f64>,        // 2·(N/2)
    acc: Vec<Vec<f64>>,    // (k+1) × 2·(N/2)
    offset: u64,           // branchless-decomposition rounding bias
}

impl FftWs {
    fn new(p: &Params, m: usize) -> Self {
        // offset = Σ_{i<l} (B/2)·2^(64−(i+1)·baselog): rounds + centers, so digits need no carry.
        let mut offset = 0u64;
        for i in 0..p.pbs_l as u32 {
            offset = offset.wrapping_add((1u64 << (p.pbs_baselog - 1)) << (64 - (i + 1) * p.pbs_baselog));
        }
        FftWs {
            digits: vec![vec![0i64; p.poly]; p.pbs_l],
            spec: vec![0.0; 2 * m],
            acc: vec![vec![0.0; 2 * m]; p.k + 1],
            offset,
        }
    }
}

// ---------------------------------------------------------------------------
// Plaintext-space helpers
// ---------------------------------------------------------------------------

const MAX_L: usize = 8;

/// Balanced gadget decomposition of `v` into `l` signed digits, base `2^baselog`.
fn decompose(v: u64, l: usize, baselog: u32) -> [i64; MAX_L] {
    let bits = l as u32 * baselog;
    let shift = 64 - bits;
    let mut state = (v >> (shift - 1)).wrapping_add(1) >> 1;
    let mut out = [0i64; MAX_L];
    let base = 1u64 << baselog;
    let half = base >> 1;
    for slot in out.iter_mut().take(l).rev() {
        let mut digit = state & (base - 1);
        state >>= baselog;
        if digit >= half {
            digit = digit.wrapping_sub(base);
            state = state.wrapping_add(1);
        }
        *slot = digit as i64;
    }
    out
}

/// `X^r * p mod (X^N+1)`, `r in [0, 2N)` (coefficients wrapping past `X^N` are negated).
fn mul_monomial(p: &[u64], r: usize) -> Vec<u64> {
    let n = p.len();
    let r = r % (2 * n);
    let mut out = vec![0u64; n];
    for (i, &val) in p.iter().enumerate() {
        let mut pos = i + r;
        let mut v = val;
        while pos >= n {
            pos -= n;
            v = v.wrapping_neg();
        }
        out[pos] = out[pos].wrapping_add(v);
    }
    out
}

/// `mod_switch(x)` into `[0, 2N)` (round `x · 2N / q`).
fn mod_switch(x: u64, log2_2n: u32) -> usize {
    let shift = 64 - log2_2n;
    ((x.wrapping_add(1u64 << (shift - 1)) >> shift) as usize) & ((1 << log2_2n) - 1)
}

// ---------------------------------------------------------------------------
// Key generation (untimed)
// ---------------------------------------------------------------------------

/// Public evaluation key: per-input-bit GGSW bootstrap key + key-switch key + the FFT
/// context, all built from the secret key.
pub struct ServerKey {
    params: Params,
    fft: NegacyclicFft,
    bsk: Vec<GgswFourier>,
    ksk: Vec<Vec<Lwe>>,
}

/// The parameter set this submission targets. Must clear the harness's ≥128-bit security
/// gate (over the LWE dimension `n` and the GLWE dimension `k·N`, `q = 2^64`, binary keys)
/// and use the required `message_bits`. Any secure choice is allowed.
pub fn params() -> Params {
    // rs-tfhe 128-bit boolean-gate parameters (n=700, N=1024), adapted to q=2^64. The boolean
    // noise budget (gap = q/4) is roomier than the old 4-message space, so n drops to rs-tfhe's
    // 700 — and with our FFT now faster than rustfft, this targets sub-rs-tfhe wall-clock.
    Params {
        n: 700,            // input/output LWE dim — matches rs-tfhe (α=2e-5 ⇒ σ≈2^48)
        k: 1,
        poly: 1024,        // GLWE dim k·N = 1024
        pbs_l: 3,
        pbs_baselog: 7,    // 21-bit precision (min for q=2^64 decomposition error)
        ks_l: 4,
        ks_baselog: 5,     // fewer KS levels (boolean budget has slack) ⇒ faster key-switch
        message_bits: 2,   // == REQUIRED_MESSAGE_BITS (boolean gate: 2 messages)
        lwe_sigma: 2.0f64.powi(48),
        glwe_sigma: 2.0f64.powi(39),
    }
}

/// Build the server key. Generates an internal GLWE key, the GGSW bootstrap key (each
/// encrypting an LWE-key bit under the GLWE key), and the key-switch key (from the
/// sample-extracted GLWE key back to the input LWE key). Untimed.
pub fn keygen(sk: &SecretKey, seed: u64) -> ServerKey {
    let params = params();
    let fft = NegacyclicFft::new(params.poly);
    let mut rng = Rng::new(seed);
    let n = params.poly;

    let glwe_key: Vec<Vec<u64>> = (0..params.k)
        .map(|_| (0..n).map(|_| rng.next_bit()).collect())
        .collect();
    let glwe_flat: Vec<u64> = glwe_key.iter().flatten().copied().collect();

    let bsk: Vec<GgswFourier> = (0..params.n)
        .map(|i| ggsw_encrypt_scalar(&params, &fft, &glwe_key, sk.lwe[i], &mut rng))
        .collect();

    let ksk: Vec<Vec<Lwe>> = (0..glwe_flat.len())
        .map(|j| {
            (0..params.ks_l)
                .map(|lev| {
                    let weight = 1u64 << (64 - (lev as u32 + 1) * params.ks_baselog);
                    lwe_encrypt_raw(&params, &sk.lwe, glwe_flat[j].wrapping_mul(weight), &mut rng)
                })
                .collect()
        })
        .collect();

    ServerKey {
        params,
        fft,
        bsk,
        ksk,
    }
}

/// Raw LWE encryption of a torus value `mu` under `key` (for the key-switch key). Uses the
/// LWE noise (the key-switch output lands under the small LWE key).
fn lwe_encrypt_raw(p: &Params, key: &[u64], mu: u64, rng: &mut Rng) -> Lwe {
    let a: Vec<u64> = (0..p.n).map(|_| rng.next_u64()).collect();
    let mut b = mu.wrapping_add(rng.gaussian(p.lwe_sigma));
    for (ai, si) in a.iter().zip(key) {
        b = b.wrapping_add(ai.wrapping_mul(*si));
    }
    Lwe { a, b }
}

/// Negacyclic product via the FFT (key generation only; its error is folded into noise).
fn ring_mul(fft: &NegacyclicFft, a: &[u64], s: &[u64]) -> Vec<u64> {
    let fa = fft.forward_torus(a);
    let fs = fft.forward_torus(s);
    let mut prod = vec![0.0; 2 * fft.spectrum_len()];
    fma(&mut prod, &fa, &fs);
    let mut out = vec![0u64; fft.n()];
    fft.inverse_to_torus(prod, &mut out);
    out
}

/// GLWE encryption of a message polynomial under the GLWE key.
fn glwe_encrypt(
    p: &Params,
    fft: &NegacyclicFft,
    glwe_key: &[Vec<u64>],
    message: &[u64],
    rng: &mut Rng,
) -> Glwe {
    let n = p.poly;
    let mask: Vec<Vec<u64>> = (0..p.k)
        .map(|_| (0..n).map(|_| rng.next_u64()).collect())
        .collect();
    let mut body: Vec<u64> = (0..n)
        .map(|j| message[j].wrapping_add(rng.gaussian(p.glwe_sigma)))
        .collect();
    for (ai, si) in mask.iter().zip(glwe_key) {
        let prod = ring_mul(fft, ai, si);
        for (b, pr) in body.iter_mut().zip(&prod) {
            *b = b.wrapping_add(*pr);
        }
    }
    Glwe { mask, body }
}

/// GGSW encryption of a scalar `mu ∈ {0,1}` under the GLWE key, returned in Fourier form.
fn ggsw_encrypt_scalar(
    p: &Params,
    fft: &NegacyclicFft,
    glwe_key: &[Vec<u64>],
    mu: u64,
    rng: &mut Rng,
) -> GgswFourier {
    let n = p.poly;
    let kp1 = p.k + 1;
    let zero = vec![0u64; n];
    let mut rows = Vec::with_capacity(kp1 * p.pbs_l);
    for i in 0..kp1 {
        for j in 0..p.pbs_l {
            let mut g = glwe_encrypt(p, fft, glwe_key, &zero, rng);
            let weight = 1u64 << (64 - (j as u32 + 1) * p.pbs_baselog);
            g.comp_mut(i)[0] = g.comp_mut(i)[0].wrapping_add(mu.wrapping_mul(weight));
            let row: GgswRow = (0..kp1).map(|c| fft.forward_torus(g.comp(c))).collect();
            rows.push(row);
        }
    }
    GgswFourier { rows }
}

// ---------------------------------------------------------------------------
// Bootstrap (timed)
// ---------------------------------------------------------------------------

/// Allocation-free external product `GGSW(μ) ⊡ glwe_in → out` in the Fourier domain.
fn external_product_into(
    p: &Params,
    fft: &NegacyclicFft,
    ggsw: &GgswFourier,
    glwe_in: &Glwe,
    out: &mut Glwe,
    ws: &mut FftWs,
) {
    let kp1 = p.k + 1;
    for spec in ws.acc.iter_mut() {
        spec.iter_mut().for_each(|x| *x = 0.0);
    }
    let l = p.pbs_l;
    let baselog = p.pbs_baselog;
    let base = 1u64 << baselog;
    let mask = base - 1;
    let half = (base >> 1) as i64;
    let offset = ws.offset;
    for i in 0..kp1 {
        let comp = glwe_in.comp(i);
        // Branchless gadget decomposition (offset trick): one vectorizable pass per level,
        // digit = ((c+offset) >> shift & mask) − B/2. No carry chain, no data-dependent branch.
        for lev in 0..l {
            let shift = 64 - (lev as u32 + 1) * baselog;
            let dst = &mut ws.digits[lev];
            for (ci, &c) in comp.iter().enumerate() {
                dst[ci] = (((c.wrapping_add(offset) >> shift) & mask) as i64) - half;
            }
        }
        for j in 0..p.pbs_l {
            fft.forward_signed_into(&ws.digits[j], &mut ws.spec);
            let row = &ggsw.rows[i * p.pbs_l + j];
            for oc in 0..kp1 {
                fma(&mut ws.acc[oc], &ws.spec, &row[oc]);
            }
        }
    }
    for oc in 0..kp1 {
        fft.inverse_into(&ws.acc[oc], out.comp_mut(oc));
    }
}

/// `dst = X^r · src` (negacyclic monomial rotation of every component).
fn glwe_mul_monomial_into(src: &Glwe, r: usize, dst: &mut Glwe) {
    for c in 0..src.mask.len() {
        mul_monomial_into(&src.mask[c], r, &mut dst.mask[c]);
    }
    mul_monomial_into(&src.body, r, &mut dst.body);
}

/// Allocation-free `dst = X^r * src mod (X^N+1)`, as two contiguous regions (`copy_from_slice`
/// + a negation loop — both vectorize), instead of branchy scattered writes.
fn mul_monomial_into(src: &[u64], r: usize, dst: &mut [u64]) {
    let n = src.len();
    let r = r % (2 * n);
    if r < n {
        dst[r..].copy_from_slice(&src[..n - r]);
        for i in 0..r {
            dst[i] = src[n - r + i].wrapping_neg();
        }
    } else {
        let r = r - n; // X^{n+r'} = −X^{r'}
        for i in 0..n - r {
            dst[r + i] = src[i].wrapping_neg();
        }
        dst[..r].copy_from_slice(&src[n - r..]);
    }
}

/// Extract the constant coefficient of a GLWE into an LWE under the flattened GLWE key.
fn sample_extract(p: &Params, glwe: &Glwe) -> Lwe {
    let n = p.poly;
    let mut a = vec![0u64; p.k * n];
    for i in 0..p.k {
        let m = &glwe.mask[i];
        a[i * n] = m[0];
        for j in 1..n {
            a[i * n + j] = m[n - j].wrapping_neg();
        }
    }
    Lwe {
        a,
        b: glwe.body[0],
    }
}

/// Key-switch an LWE under the flattened GLWE key (dim `k·N`) back to the input key (dim `n`).
fn key_switch(p: &Params, ksk: &[Vec<Lwe>], ct: &Lwe) -> Lwe {
    let mut a = vec![0u64; p.n];
    let mut b = ct.b;
    for (j, aj) in ct.a.iter().enumerate() {
        let digits = decompose(*aj, p.ks_l, p.ks_baselog);
        for lev in 0..p.ks_l {
            let d = digits[lev];
            let row = &ksk[j][lev];
            b = b.wrapping_sub((d as u64).wrapping_mul(row.b));
            for (ai, ri) in a.iter_mut().zip(&row.a) {
                *ai = ai.wrapping_sub((d as u64).wrapping_mul(*ri));
            }
        }
    }
    Lwe { a, b }
}

/// Build the accumulator test polynomial encoding LUT `f` (half-box centered, redundant).
fn build_test_poly(p: &Params, lut: &Lut) -> Vec<u64> {
    let n = p.poly;
    let p_eff = p.msg_modulus() as usize;
    let box_size = n / p_eff;
    let delta = p.delta();
    let mut tv = vec![0u64; n];
    for i in 0..n {
        let m = (i / box_size) % p_eff;
        tv[i] = lut.values[m].wrapping_mul(delta);
    }
    mul_monomial(&tv, 2 * n - box_size / 2)
}

/// Programmable bootstrap: refresh `ct` (LWE under `s`) while applying `lut`. Returns a
/// fresh LWE under `s` encrypting `lut[message]`. This is the timed operation.
pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe {
    let p = &sk.params;
    let fft = &sk.fft;
    let n = p.poly;
    let log2_2n = (2 * n).trailing_zeros();

    // 1. Accumulator = test polynomial rotated by -b̃ (trivial GLWE).
    let test = build_test_poly(p, lut);
    let b_tilde = mod_switch(ct.b, log2_2n);
    let mut acc = Glwe::trivial(p.k, mul_monomial(&test, 2 * n - b_tilde));

    // 2. Blind rotation: n CMux external products, reusing scratch buffers.
    let mut ws = FftWs::new(p, fft.spectrum_len());
    let mut rotated = Glwe::trivial(p.k, vec![0u64; n]);
    let mut diff = Glwe::trivial(p.k, vec![0u64; n]);
    let mut prod = Glwe::trivial(p.k, vec![0u64; n]);
    for i in 0..p.n {
        let a_tilde = mod_switch(ct.a[i], log2_2n);
        if a_tilde == 0 {
            continue; // X^0 ⇒ no-op CMux
        }
        glwe_mul_monomial_into(&acc, a_tilde, &mut rotated);
        for c in 0..p.k + 1 {
            for (d, (r, a)) in diff
                .comp_mut(c)
                .iter_mut()
                .zip(rotated.comp(c).iter().zip(acc.comp(c)))
            {
                *d = r.wrapping_sub(*a);
            }
        }
        external_product_into(p, fft, &sk.bsk[i], &diff, &mut prod, &mut ws);
        for c in 0..p.k + 1 {
            for (a, pr) in acc.comp_mut(c).iter_mut().zip(prod.comp(c)) {
                *a = a.wrapping_add(*pr);
            }
        }
    }

    // 3. Sample-extract the constant term, then key-switch back to the input key.
    let extracted = sample_extract(p, &acc);
    key_switch(p, &sk.ksk, &extracted)
}

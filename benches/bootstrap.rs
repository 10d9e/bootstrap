//! Wall-clock timing benchmark for the programmable bootstrap. The SCORE is wall-clock
//! (see `bootstrap eval`); this criterion bench is the detailed cross-check.
//!
//! Run with `cargo bench`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bootstrap::algorithm::{bootstrap, keygen, params};
use bootstrap::harness::params::{decrypt, encrypt, gen_secret_key, Lut};

fn bench(c: &mut Criterion) {
    let p = params();
    let sk = gen_secret_key(p, 0x5EED_5EED);
    let server = keygen(&sk, 0xB0_07_57_A9);
    let ct = encrypt(p, &sk, 2, 0xC0FF_EE00);
    let lut = Lut {
        values: (0..p.msg_modulus()).collect(), // identity
    };

    // Functional sanity on these keys.
    let out = bootstrap(&server, &ct, &lut);
    assert_eq!(decrypt(p, &sk, &out), 2);

    let mut g = c.benchmark_group("bootstrap");
    g.sample_size(30);
    g.bench_function("programmable_bootstrap", |b| {
        b.iter(|| black_box(bootstrap(&server, black_box(&ct), &lut)));
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);

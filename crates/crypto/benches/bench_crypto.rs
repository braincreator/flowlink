use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use flowlink_crypto::{decrypt, encrypt, KeyPair};
use hkdf::Hkdf;
use sha2::Sha256;

fn bench_keypair_generate(c: &mut Criterion) {
    c.bench_function("crypto/keypair_generate", |b| {
        b.iter(|| KeyPair::generate())
    });
}

fn bench_encrypt(c: &mut Criterion) {
    let alice = KeyPair::generate();
    let bob = KeyPair::generate();
    let mut group = c.benchmark_group("crypto/encrypt");

    for size in [1024, 10_240, 102_400] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let data = vec![0xAB_u8; size];
            b.iter(|| encrypt(&alice, &bob.public_key, &data).unwrap())
        });
    }
    group.finish();
}

fn bench_decrypt(c: &mut Criterion) {
    let alice = KeyPair::generate();
    let bob = KeyPair::generate();
    let data = vec![0xAB_u8; 10240];
    let envelope = encrypt(&alice, &bob.public_key, &data).unwrap();

    c.bench_function("crypto/decrypt_10kb", |b| {
        b.iter(|| decrypt(&bob, &envelope).unwrap())
    });
}

fn bench_hkdf(c: &mut Criterion) {
    let ikm = [0xAB_u8; 32];
    let mut group = c.benchmark_group("crypto/hkdf");

    group.bench_function("derive_32_bytes", |b| {
        b.iter(|| {
            let hk = Hkdf::<Sha256>::new(None, &ikm);
            let mut okm = [0u8; 32];
            hk.expand(b"flowlink-e2ee-v1", &mut okm).unwrap();
            okm
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_keypair_generate,
    bench_encrypt,
    bench_decrypt,
    bench_hkdf
);
criterion_main!(benches);

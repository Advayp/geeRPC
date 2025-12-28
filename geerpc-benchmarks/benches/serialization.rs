use criterion::{black_box, criterion_group, criterion_main, Criterion};
use geerpc::encode_payload;
use geerpc::decode_payload;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct SmallPayload {
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct MediumPayload {
    data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct LargePayload {
    items: Vec<Vec<u8>>,
    metadata: String,
}

fn generate_small_payload() -> SmallPayload {
    SmallPayload {
        message: "Hello, World! This is a small payload for benchmarking.".to_string(),
    }
}

fn generate_medium_payload() -> MediumPayload {
    MediumPayload {
        data: (0..5120).map(|i| (i % 256) as u8).collect(),
    }
}

fn generate_large_payload() -> LargePayload {
    let mut items = Vec::new();
    for _ in 0..100 {
        let item: Vec<u8> = (0..5120).map(|i| (i % 256) as u8).collect();
        items.push(item);
    }
    LargePayload {
        items,
        metadata: "Large payload metadata for benchmarking".to_string(),
    }
}

fn bench_encode_small(c: &mut Criterion) {
    let payload = generate_small_payload();
    c.bench_function("encode_small", |b| {
        b.iter(|| {
            let _ = encode_payload(black_box(&payload)).unwrap();
        });
    });
}

fn bench_decode_small(c: &mut Criterion) {
    let payload = generate_small_payload();
    let encoded = encode_payload(&payload).unwrap();
    c.bench_function("decode_small", |b| {
        b.iter(|| {
            let _: SmallPayload = decode_payload(black_box(encoded.clone())).unwrap();
        });
    });
}

fn bench_encode_medium(c: &mut Criterion) {
    let payload = generate_medium_payload();
    c.bench_function("encode_medium", |b| {
        b.iter(|| {
            let _ = encode_payload(black_box(&payload)).unwrap();
        });
    });
}

fn bench_decode_medium(c: &mut Criterion) {
    let payload = generate_medium_payload();
    let encoded = encode_payload(&payload).unwrap();
    c.bench_function("decode_medium", |b| {
        b.iter(|| {
            let _: MediumPayload = decode_payload(black_box(encoded.clone())).unwrap();
        });
    });
}

fn bench_encode_large(c: &mut Criterion) {
    let payload = generate_large_payload();
    c.bench_function("encode_large", |b| {
        b.iter(|| {
            let _ = encode_payload(black_box(&payload)).unwrap();
        });
    });
}

fn bench_decode_large(c: &mut Criterion) {
    let payload = generate_large_payload();
    let encoded = encode_payload(&payload).unwrap();
    c.bench_function("decode_large", |b| {
        b.iter(|| {
            let _: LargePayload = decode_payload(black_box(encoded.clone())).unwrap();
        });
    });
}

fn bench_encode_decode_roundtrip_small(c: &mut Criterion) {
    let payload = generate_small_payload();
    c.bench_function("encode_decode_roundtrip_small", |b| {
        b.iter(|| {
            let encoded = encode_payload(black_box(&payload)).unwrap();
            let _: SmallPayload = decode_payload(black_box(encoded)).unwrap();
        });
    });
}

fn bench_encode_decode_roundtrip_medium(c: &mut Criterion) {
    let payload = generate_medium_payload();
    c.bench_function("encode_decode_roundtrip_medium", |b| {
        b.iter(|| {
            let encoded = encode_payload(black_box(&payload)).unwrap();
            let _: MediumPayload = decode_payload(black_box(encoded)).unwrap();
        });
    });
}

fn bench_encode_decode_roundtrip_large(c: &mut Criterion) {
    let payload = generate_large_payload();
    c.bench_function("encode_decode_roundtrip_large", |b| {
        b.iter(|| {
            let encoded = encode_payload(black_box(&payload)).unwrap();
            let _: LargePayload = decode_payload(black_box(encoded)).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_encode_small,
    bench_decode_small,
    bench_encode_medium,
    bench_decode_medium,
    bench_encode_large,
    bench_decode_large,
    bench_encode_decode_roundtrip_small,
    bench_encode_decode_roundtrip_medium,
    bench_encode_decode_roundtrip_large
);
criterion_main!(benches);


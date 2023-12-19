use std::time::Instant;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::SliceRandom;
use rand::{thread_rng, Rng};

use art::art::Tree;
use art::{FixedKey, VariableKey};

pub fn inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("inserts");
    group.throughput(Throughput::Elements(1));
    group.bench_function("seq_insert", |b| {
        let mut tree = Tree::<FixedKey<16>, _>::new();
        let mut key = 0u64;
        b.iter(|| {
            tree.insert(&key.into(), key, 0, 0);
            key += 1;
        })
    });

    let keys = gen_keys(3, 2, 3);

    group.bench_function("rand_insert", |b| {
        let mut tree = Tree::<FixedKey<16>, _>::new();
        let mut rng = thread_rng();
        b.iter(|| {
            let key = &keys[rng.gen_range(0..keys.len())];
            tree.insert(&key.into(), key.clone(), 0, 0);
        })
    });

    group.finish();
}

pub fn deletes(c: &mut Criterion) {
    let mut group = c.benchmark_group("deletes");
    group.throughput(Throughput::Elements(1));
    group.bench_function("seq_delete", |b| {
        let mut tree = Tree::<FixedKey<16>, _>::new();
        b.iter_custom(|iters| {
            for i in 0..iters {
                tree.insert(&i.into(), i, 0, 0);
            }
            let start = Instant::now();
            for i in 0..iters {
                tree.remove(&i.into());
            }
            start.elapsed()
        })
    });

    group.throughput(Throughput::Elements(1));
    group.bench_function("rand_delete", |b| {
        let keys = gen_keys(3, 2, 3);
        let mut tree = Tree::<FixedKey<16>, _>::new();
        let mut rng = thread_rng();
        for key in &keys {
            tree.insert(&key.into(), key, 0, 0);
        }
        b.iter(|| {
            let key = &keys[rng.gen_range(0..keys.len())];
            tree.remove(&key.into());
        })
    });

    group.finish();
}

pub fn reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("reads");

    group.throughput(Throughput::Elements(1));

    for size in [100u64, 1000, 10_000, 100_000, 1_000_000] {
        let mut tree = Tree::<FixedKey<16>, _>::new();
        for i in 0..size {
            tree.insert(&i.into(), i, 0, 0).unwrap();
        }

        group.bench_with_input(BenchmarkId::new("rand_get", size), &size, |b, size| {
            let mut rng = thread_rng();
            b.iter(|| {
                let key: u64 = rng.gen_range(0..*size);
                tree.get(&key.into(), 0).unwrap();
            })
        });
    }

    for size in [100u64, 1000, 10_000, 100_000, 1_000_000] {
        group.bench_with_input(BenchmarkId::new("seq_get", size), &size, |b, size| {
            let mut tree = Tree::<FixedKey<16>, _>::new();
            for i in 0..*size {
                tree.insert(&i.into(), i, 0, 0).unwrap();
            }

            let mut key = 0u64;
            b.iter(|| {
                tree.get(&key.into(), 0);
                key += 1;
            })
        });
    }

    group.finish();
}

pub fn rand_get_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("random_get_str");
    let keys = gen_keys(3, 2, 3);

    {
        let size = 1_000_000;
        let mut tree = Tree::<FixedKey<16>, _>::new();
        for (i, key) in keys.iter().enumerate() {
            tree.insert(&key.into(), i, 0, 0).unwrap();
        }
        group.bench_with_input(BenchmarkId::new("art", size), &size, |b, _size| {
            let mut rng = thread_rng();
            b.iter(|| {
                let key = &keys[rng.gen_range(0..keys.len())];
                criterion::black_box(tree.get(&key.into(), 0));
            })
        });
    }

    group.finish();
}

fn gen_keys(l1_prefix: usize, l2_prefix: usize, suffix: usize) -> Vec<String> {
    let mut keys = Vec::new();
    let chars: Vec<char> = ('a'..='z').collect();
    for i in 0..chars.len() {
        let level1_prefix = chars[i].to_string().repeat(l1_prefix);
        for i in 0..chars.len() {
            let level2_prefix = chars[i].to_string().repeat(l2_prefix);
            let key_prefix = level1_prefix.clone() + &level2_prefix;
            for _ in 0..=u8::MAX {
                let suffix: String = (0..suffix)
                    .map(|_| chars[thread_rng().gen_range(0..chars.len())])
                    .collect();
                let k = key_prefix.clone() + &suffix;
                keys.push(k);
            }
        }
    }

    keys.shuffle(&mut thread_rng());
    keys
}

pub fn iters(c: &mut Criterion) {
    let mut group = c.benchmark_group("iters");
    group.throughput(Throughput::Elements(1));
    for size in [100u64, 1000, 10_000, 100_000] {
        // 1_000_000 requires a very long time
        group.bench_with_input(BenchmarkId::new("iter_u64", size), &size, |b, size| {
            let mut tree = Tree::<FixedKey<16>, _>::new();
            for i in 0..*size {
                tree.insert(&i.into(), i, 0, 0).unwrap();
            }
            b.iter(|| {
                tree.iter().count();
            })
        });
    }

    group.bench_function("iter_variable_size_key", |b| {
        let mut tree = Tree::<VariableKey, _>::new();
        for i in gen_keys(2, 2, 2) {
            tree.insert(&VariableKey::from_slice(i.as_bytes()), i, 0, 0)
                .unwrap();
        }
        b.iter(|| {
            tree.iter().count();
        })
    });

    group.finish();
}

criterion_group!(delete_benches, deletes);
criterion_group!(insert_benches, inserts);
criterion_group!(read_benches, reads, rand_get_str);
criterion_group!(iter_benches, iters);
criterion_main!(insert_benches, read_benches, delete_benches, iter_benches);

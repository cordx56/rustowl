use criterion::{criterion_group, criterion_main, Criterion};
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use rustowl::models::Loc;
use rustowl::utils::{index_to_line_char, line_char_to_index};
use std::hint::black_box;

fn bench_line_col(c: &mut Criterion) {
    let mut group = c.benchmark_group("line_col_conversion");
    let mut rng = SmallRng::seed_from_u64(42);

    // Construct a synthetic source with mixed line lengths & unicode
    let mut source = String::new();
    for i in 0..10_000u32 {
        let len = (i % 40 + 5) as usize; // vary line length
        for _ in 0..len {
            let v: u8 = rng.r#gen::<u8>();
            source.push(char::from(b'a' + (v % 26)));
        }
        if i % 17 == 0 { source.push('\r'); } // occasional CR
        source.push('\n');
        if i % 1111 == 0 { source.push_str("🦀"); } // some unicode
    }

    let chars: Vec<_> = source.chars().collect();
    let total = chars.len() as u32;

    group.bench_function("index_to_line_char", |b| {
        b.iter(|| {
            let idx = Loc(rng.gen_range(0..total));
            let (l, c) = index_to_line_char(&source, idx);
            black_box((l, c));
        });
    });

    group.bench_function("line_char_to_index", |b| {
        b.iter(|| {
            // random line, then column 0 for simplicity
            let line = rng.gen_range(0..10_000u32);
            let idx = line_char_to_index(&source, line, 0);
            black_box(idx);
        });
    });

    group.finish();
}

criterion_group!(benches_line_col, bench_line_col);
criterion_main!(benches_line_col);

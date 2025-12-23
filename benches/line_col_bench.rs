use divan::{AllocProfiler, Bencher, black_box};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rustowl::models::Loc;
use rustowl::utils::{index_to_line_char, line_char_to_index};
use std::cell::RefCell;
use std::sync::Arc;

#[cfg(all(not(target_env = "msvc"), not(miri)))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(not(target_env = "msvc"), not(miri)))]
#[global_allocator]
static ALLOC: AllocProfiler<Jemalloc> = AllocProfiler::new(Jemalloc);

#[cfg(any(target_env = "msvc", miri))]
#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    divan::main();
}

thread_local! {
    static SOURCE: RefCell<Option<(Arc<str>, u32)>> = const { RefCell::new(None) };
    static RNG: RefCell<SmallRng> = RefCell::new(SmallRng::seed_from_u64(42));
}

fn get_or_init_source() -> (Arc<str>, u32) {
    SOURCE.with(|cell| {
        let mut borrowed = cell.borrow_mut();
        if borrowed.is_none() {
            let mut rng = SmallRng::seed_from_u64(42);
            let mut source = String::new();
            for i in 0..10_000u32 {
                let len = (i % 40 + 5) as usize;
                for _ in 0..len {
                    let v: u8 = rng.random::<u8>();
                    source.push(char::from(b'a' + (v % 26)));
                }
                if i % 17 == 0 {
                    source.push('\r');
                }
                source.push('\n');
                if i % 1111 == 0 {
                    source.push('🦀');
                }
            }
            let total = source.chars().filter(|&c| c != '\r').count() as u32;
            *borrowed = Some((Arc::<str>::from(source), total));
        }
        borrowed.as_ref().unwrap().clone()
    })
}

#[divan::bench_group(name = "line_col_conversion")]
mod line_col_conversion {
    use super::*;

    #[divan::bench]
    fn index_to_line_char_bench(bencher: Bencher) {
        bencher
            .with_inputs(get_or_init_source)
            .bench_values(|(source, total)| {
                let idx = RNG.with(|rng| Loc(rng.borrow_mut().random_range(0..total)));
                let (l, c) = index_to_line_char(&source, idx);
                black_box((l, c));
            });
    }

    #[divan::bench]
    fn line_char_to_index_bench(bencher: Bencher) {
        bencher
            .with_inputs(|| get_or_init_source().0)
            .bench_values(|source| {
                let line = RNG.with(|rng| rng.borrow_mut().random_range(0..10_000u32));
                let idx = line_char_to_index(&source, line, 0);
                black_box(idx);
            });
    }
}

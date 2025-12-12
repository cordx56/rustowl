use divan::{AllocProfiler, Bencher, black_box};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rustowl::models::Loc;
use rustowl::utils::{index_to_line_char, line_char_to_index};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

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
    static SOURCE: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn get_or_init_source() -> String {
    SOURCE.with(|s| {
        let mut borrowed = s.borrow_mut();
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
            *borrowed = Some(source);
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
            .with_inputs(|| {
                let source = get_or_init_source();
                let chars: Vec<_> = source.chars().collect();
                let total = chars.len() as u32;
                let rng = SmallRng::seed_from_u64(42);
                (source, total, Arc::new(Mutex::new(rng)))
            })
            .bench_values(|(source, total, rng)| {
                let idx = Loc(rng.lock().unwrap().random_range(0..total));
                let (l, c) = index_to_line_char(&source, idx);
                black_box((l, c));
            });
    }

    #[divan::bench]
    fn line_char_to_index_bench(bencher: Bencher) {
        bencher
            .with_inputs(|| {
                let source = get_or_init_source();
                let rng = SmallRng::seed_from_u64(42);
                (source, Arc::new(Mutex::new(rng)))
            })
            .bench_values(|(source, rng)| {
                let line = rng.lock().unwrap().random_range(0..10_000u32);
                let idx = line_char_to_index(&source, line, 0);
                black_box(idx);
            });
    }
}

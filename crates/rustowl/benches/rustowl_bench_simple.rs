use divan::{AllocProfiler, Bencher, black_box};
use std::process::Command;

#[cfg(all(any(target_os = "linux", target_os = "macos"), not(miri)))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(any(target_os = "linux", target_os = "macos"), not(miri)))]
#[global_allocator]
static ALLOC: AllocProfiler<Jemalloc> = AllocProfiler::new(Jemalloc);

#[cfg(any(target_os = "windows", miri))]
#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    // Ensure rustowl binary is built before running benchmarks
    let output = Command::new("cargo")
        .args(["build", "--release", "--bin", "rustowl"])
        .output()
        .expect("Failed to build rustowl");

    if !output.status.success() {
        panic!(
            "Failed to build rustowl: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    divan::main();
}

const DUMMY_PACKAGE: &str = "./perf-tests/dummy-package";
const BINARY_PATH: &str = "./target/release/rustowl";

#[divan::bench_group(name = "rustowl_check", sample_count = 20)]
mod rustowl_check {
    use super::*;

    #[divan::bench]
    fn default(bencher: Bencher) {
        bencher.bench(|| {
            let output = Command::new(BINARY_PATH)
                .args(["check", DUMMY_PACKAGE])
                .output()
                .expect("Failed to run rustowl check");
            black_box(output.status.success());
        });
    }

    #[divan::bench]
    fn all_targets(bencher: Bencher) {
        bencher.bench(|| {
            let output = Command::new(BINARY_PATH)
                .args(["check", DUMMY_PACKAGE, "--all-targets"])
                .output()
                .expect("Failed to run rustowl check with all targets");
            black_box(output.status.success());
        });
    }

    #[divan::bench]
    fn all_features(bencher: Bencher) {
        bencher.bench(|| {
            let output = Command::new(BINARY_PATH)
                .args(["check", DUMMY_PACKAGE, "--all-features"])
                .output()
                .expect("Failed to run rustowl check with all features");
            black_box(output.status.success());
        });
    }
}

#[divan::bench_group(name = "rustowl_comprehensive", sample_count = 20)]
mod rustowl_comprehensive {
    use super::*;

    #[divan::bench]
    fn comprehensive(bencher: Bencher) {
        bencher.bench(|| {
            let output = Command::new(BINARY_PATH)
                .args(["check", DUMMY_PACKAGE, "--all-targets", "--all-features"])
                .output()
                .expect("Failed to run comprehensive rustowl check");
            black_box(output.status.success());
        });
    }
}

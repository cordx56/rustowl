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

fn rustowl_bin_path() -> std::path::PathBuf {
    // `cargo bench -p rustowl` runs the bench binary with CWD set
    // to `crates/rustowl`, but `cargo build -p rustowl` writes the binary
    // to the workspace root `target/`.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("../../target/release/rustowl"),
        manifest_dir.join("../../target/release/rustowl.exe"),
        manifest_dir.join("target/release/rustowl"),
        manifest_dir.join("target/release/rustowl.exe"),
    ];

    for path in candidates {
        if path.is_file() {
            return path;
        }
    }

    // Fall back to whatever is on PATH; this keeps the benchmark usable
    // even if run outside the workspace layout.
    std::path::PathBuf::from("rustowl")
}

#[divan::bench_group(name = "rustowl_check", sample_count = 20)]
mod rustowl_check {
    use super::*;

    #[divan::bench]
    fn default(bencher: Bencher) {
        bencher.bench(|| {
            let output = Command::new(rustowl_bin_path())
                .args(["check", DUMMY_PACKAGE])
                .output()
                .expect("Failed to run rustowl check");
            black_box(output.status.success());
        });
    }

    #[divan::bench]
    fn all_targets(bencher: Bencher) {
        bencher.bench(|| {
            let output = Command::new(rustowl_bin_path())
                .args(["check", DUMMY_PACKAGE, "--all-targets"])
                .output()
                .expect("Failed to run rustowl check with all targets");
            black_box(output.status.success());
        });
    }

    #[divan::bench]
    fn all_features(bencher: Bencher) {
        bencher.bench(|| {
            let output = Command::new(rustowl_bin_path())
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
            let output = Command::new(rustowl_bin_path())
                .args(["check", DUMMY_PACKAGE, "--all-targets", "--all-features"])
                .output()
                .expect("Failed to run comprehensive rustowl check");
            black_box(output.status.success());
        });
    }
}

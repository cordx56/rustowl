use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::process::Command;
use std::time::Duration;

fn bench_rustowl_check(c: &mut Criterion) {
    let dummy_package = "./perf-tests/dummy-package";

    let mut group = c.benchmark_group("rustowl_check");
    group
        .sample_size(20)
        .measurement_time(Duration::from_secs(300))
        .warm_up_time(Duration::from_secs(5));

    // Ensure rustowl binary is built
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

    let binary_path = "./target/release/rustowl";

    group.bench_function("default", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(["check", dummy_package])
                .output()
                .expect("Failed to run rustowl check");
            black_box(output.status.success());
        })
    });

    group.bench_function("all_targets", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(["check", dummy_package, "--all-targets"])
                .output()
                .expect("Failed to run rustowl check with all targets");
            black_box(output.status.success());
        })
    });

    group.bench_function("all_features", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(["check", dummy_package, "--all-features"])
                .output()
                .expect("Failed to run rustowl check with all features");
            black_box(output.status.success());
        })
    });

    group.finish();
}

fn bench_rustowl_comprehensive(c: &mut Criterion) {
    let dummy_package = "./perf-tests/dummy-package";
    let binary_path = "./target/release/rustowl";

    let mut group = c.benchmark_group("rustowl_comprehensive");
    group
        .sample_size(20)
        .measurement_time(Duration::from_secs(200))
        .warm_up_time(Duration::from_secs(5));

    group.bench_function("comprehensive", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(["check", dummy_package, "--all-targets", "--all-features"])
                .output()
                .expect("Failed to run comprehensive rustowl check");
            black_box(output.status.success());
        })
    });

    group.finish();
}

criterion_group!(benches, bench_rustowl_check, bench_rustowl_comprehensive);
criterion_main!(benches);

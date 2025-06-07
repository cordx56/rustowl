use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::process::Command;

fn bench_rustowl_check(c: &mut Criterion) {
    let dummy_package = "./perf-tests/dummy-package";
    
    // Ensure rustowl binary is built
    let output = Command::new("cargo")
        .args(&["build", "--release", "--bin", "rustowl"])
        .output()
        .expect("Failed to build rustowl");
    
    if !output.status.success() {
        panic!("Failed to build rustowl: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    let binary_path = "./target/release/rustowl";
    
    c.bench_function("rustowl_check_default", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(&["check", dummy_package])
                .output()
                .expect("Failed to run rustowl check");
            black_box(output.status.success());
        })
    });
    
    c.bench_function("rustowl_check_all_targets", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(&["check", dummy_package, "--all-targets"])
                .output()
                .expect("Failed to run rustowl check with all targets");
            black_box(output.status.success());
        })
    });
    
    c.bench_function("rustowl_check_all_features", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(&["check", dummy_package, "--all-features"])
                .output()
                .expect("Failed to run rustowl check with all features");
            black_box(output.status.success());
        })
    });
}

fn bench_rustowl_comprehensive(c: &mut Criterion) {
    let dummy_package = "./perf-tests/dummy-package";
    let binary_path = "./target/release/rustowl";
    
    c.bench_function("rustowl_comprehensive", |b| {
        b.iter(|| {
            let output = Command::new(binary_path)
                .args(&["check", dummy_package, "--all-targets", "--all-features"])
                .output()
                .expect("Failed to run comprehensive rustowl check");
            black_box(output.status.success());
        })
    });
}

criterion_group!(benches, bench_rustowl_check, bench_rustowl_comprehensive);
criterion_main!(benches);

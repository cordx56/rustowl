use std::path::Path;
use std::process::Command;

#[test]
fn rustowlc_emits_workspace_json_for_simple_crate() {
    let temp = tempfile::tempdir().expect("tempdir");
    let crate_dir = temp.path();

    // Keep the directory around on failure for debugging.
    eprintln!("rustowlc integration temp crate: {}", crate_dir.display());

    std::fs::write(
        crate_dir.join("Cargo.toml"),
        r#"[package]
name = "rustowlc_integ"
version = "0.1.0"
edition = "2021"

[lib]
path = "src/lib.rs"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(crate_dir.join("src")).unwrap();
    std::fs::write(
        crate_dir.join("src/lib.rs"),
        r#"pub fn foo() -> i32 {
    let x = 1;
    x + 1
}
"#,
    )
    .unwrap();

    // Prefer the instrumented rustowlc that `cargo llvm-cov` builds under `target/llvm-cov-target`.
    // Fall back to the normal `target/debug` binary for non-coverage runs.
    let exe = std::env::consts::EXE_SUFFIX;

    // Prefer the instrumented rustowlc that `cargo llvm-cov` builds under `target/llvm-cov-target`.
    // Fall back to the normal `target/debug` binary for non-coverage runs.
    let instrumented_rustowlc_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("target/llvm-cov-target/debug/rustowlc{exe}"));
    let rustowlc_path = if instrumented_rustowlc_path.is_file() {
        instrumented_rustowlc_path
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/debug/rustowlc{exe}"))
    };
    assert!(
        rustowlc_path.is_file(),
        "missing rustowlc at {}",
        rustowlc_path.display()
    );

    // Drive rustc via cargo so it behaves like real usage.
    // We explicitly disable incremental compilation to avoid artifacts affecting output.
    // Ensure sccache doesn't insert itself in front of our wrapper.
    let mut cmd = Command::new("cargo");
    cmd.arg("clean")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("SCCACHE")
        .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
        .env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .current_dir(crate_dir);
    let clean_out = cmd.output().expect("cargo clean");
    assert!(clean_out.status.success());

    let sysroot = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("rustc --print sysroot")
        .stdout;
    let sysroot = String::from_utf8_lossy(&sysroot).trim().to_string();

    let llvm_profile_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/llvm-cov-target");
    std::fs::create_dir_all(&llvm_profile_dir).unwrap();
    let llvm_profile_file = llvm_profile_dir.join("rustowlc-integration-%p-%m.profraw");

    // Use an absolute path outside of the temp crate to avoid any target-dir sandboxing.
    let output_path = std::env::temp_dir().join(format!(
        "rustowl_output_{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&output_path);

    let rustc_path = std::process::Command::new("rustc")
        .args(["--print", "sysroot"]) // just to verify rustc exists
        .output()
        .expect("rustc exists");
    drop(rustc_path);

    let mut cmd = Command::new("cargo");
    cmd.arg("check")
        .arg("--release")
        // Ensure we compile the workspace crate itself (not just deps).
        .arg("--lib")
        // Make cargo invoke: `rustowlc rustc ...` so `argv0 == argv1` and analysis runs.
        .env(
            "RUSTC",
            std::process::Command::new("rustc")
                .arg("--print")
                .arg("rustc")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "rustc".to_string()),
        )
        .env("RUSTC_WORKSPACE_WRAPPER", &rustowlc_path)
        .env("CARGO_INCREMENTAL", "0")
        .env("RUSTOWL_OUTPUT_PATH", &output_path)
        // Ensure coverage from the rustowlc subprocess is captured.
        .env("LLVM_PROFILE_FILE", &llvm_profile_file)
        // rustowlc depends on rustc private dylibs.
        .env("LD_LIBRARY_PATH", format!("{}/lib", sysroot))
        // Ensure no outer wrapper like sccache interferes.
        .env_remove("RUSTC_WRAPPER")
        .env_remove("SCCACHE")
        .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
        .env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .current_dir(crate_dir);

    let output = cmd.output().expect("run cargo check");

    if !output_path.is_file() {
        // Helpful diagnostics: show exactly how cargo invokes rustc.
        let mut verbose_cmd = Command::new("cargo");
        verbose_cmd
            .arg("check")
            .arg("--lib")
            .arg("-v")
            .env(
                "RUSTC",
                std::process::Command::new("rustc")
                    .arg("--print")
                    .arg("rustc")
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "rustc".to_string()),
            )
            .env("RUSTC_WORKSPACE_WRAPPER", &rustowlc_path)
            .env("CARGO_INCREMENTAL", "0")
            .env("RUSTOWL_OUTPUT_PATH", &output_path)
            .env("LLVM_PROFILE_FILE", &llvm_profile_file)
            .env("LD_LIBRARY_PATH", format!("{}/lib", sysroot))
            .env_remove("RUSTC_WRAPPER")
            .env_remove("SCCACHE")
            .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
            .env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
            .env("CARGO_BUILD_RUSTC_WRAPPER", "")
            .current_dir(crate_dir);

        let verbose = verbose_cmd.output().expect("run cargo check -v");
        eprintln!(
            "cargo -v stdout:\n{}",
            String::from_utf8_lossy(&verbose.stdout)
        );
        eprintln!(
            "cargo -v stderr:\n{}",
            String::from_utf8_lossy(&verbose.stderr)
        );
    }

    assert!(
        output.status.success(),
        "cargo failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Cargo may suppress compiler stdout. We instead ask rustowlc to append JSON lines to a file.
    // If we didn't run analysis, the file won't exist.
    assert!(
        output_path.is_file(),
        "expected rustowl output file at {}; crate dir entries: {:?}; /tmp entries include output?={}",
        output_path.display(),
        std::fs::read_dir(crate_dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect::<Vec<_>>(),
        output_path.exists()
    );

    let output_contents = std::fs::read_to_string(&output_path).expect("read rustowl output file");
    assert!(
        !output_contents.trim().is_empty(),
        "expected rustowl output to be non-empty"
    );
    assert!(
        output_contents.contains("\"rustowlc_integ\"")
            || output_contents.contains("rustowlc_integ"),
        "expected crate name in output"
    );
    assert!(
        output_contents.contains("src/lib.rs"),
        "expected output to mention src/lib.rs; output was:\n{output_contents}"
    );
}

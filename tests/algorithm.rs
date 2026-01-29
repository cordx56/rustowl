use std::process::Command;
use std::sync::Once;

static BUILD_ONCE: Once = Once::new();

fn ensure_rustowl_built() {
    BUILD_ONCE.call_once(|| {
        let status = Command::new("cargo")
            .args(["build"])
            .status()
            .expect("Failed to build rustowl");
        assert!(status.success(), "Failed to build rustowl");
    });
}

fn get_rustowl_output(function_path: &str, variable: &str) -> String {
    ensure_rustowl_built();

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "rustowl",
            "--",
            "show",
            "--path",
            "algo-tests/src/vec.rs",
            function_path,
            variable,
        ])
        .output()
        .expect("Failed to run rustowl");

    assert!(output.status.success(), "rustowl command failed");
    String::from_utf8(output.stdout).expect("Invalid UTF-8")
}

#[test]
fn test_f1_v1() {
    let output = get_rustowl_output("vec::f1", "v1");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f1_v2() {
    let output = get_rustowl_output("vec::f1", "v2");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f2_v1() {
    let output = get_rustowl_output("vec::f2", "v1");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f2_v2() {
    let output = get_rustowl_output("vec::f2", "v2");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f3_v1() {
    let output = get_rustowl_output("vec::f3", "v1");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f3_v2() {
    let output = get_rustowl_output("vec::f3", "v2");
    insta::assert_snapshot!(output);
}

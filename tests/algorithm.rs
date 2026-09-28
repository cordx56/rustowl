use std::process::Command;
use std::sync::Once;

static BUILD_ONCE: Once = Once::new();

fn ensure_rustowl_built() {
    BUILD_ONCE.call_once(|| {
        let mut cmd = Command::new("cargo");
        if cfg!(windows) {
            cmd.args(["build", "--profile", "windows-release"]);
        } else {
            cmd.args(["build", "--release"]);
        }
        let output = cmd
            .output()
            .unwrap_or_else(|e| panic!("Failed to execute cargo build: {e}"));
        assert!(
            output.status.success(),
            "Failed to build rustowl.\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    });
}

fn get_rustowl_output(function_path: &str, variable: &str) -> String {
    ensure_rustowl_built();

    let module = function_path
        .split("::")
        .next()
        .expect("function path must start with a module name");

    let exe_name = if cfg!(windows) {
        "rustowl.exe"
    } else {
        "rustowl"
    };
    let profile_dir = if cfg!(windows) {
        "windows-release"
    } else {
        "release"
    };

    let rustowl_path = format!(
        "target{}{}{}{}",
        std::path::MAIN_SEPARATOR,
        profile_dir,
        std::path::MAIN_SEPARATOR,
        exe_name
    );

    let output = Command::new(&rustowl_path)
        .args([
            "show",
            "--path",
            &format!(
                "algo-tests{}src{}{module}.rs",
                std::path::MAIN_SEPARATOR,
                std::path::MAIN_SEPARATOR
            ),
            function_path,
            variable,
        ])
        .output()
        .unwrap_or_else(|e| panic!("Failed to execute {rustowl_path}: {e}"));

    assert!(
        output.status.success(),
        "{rustowl_path} command failed.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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

#[test]
fn test_f4_v1() {
    let output = get_rustowl_output("vec::f4", "v1");
    insta::assert_snapshot!(output);
}

#[test]
fn test_f5_r() {
    let output = get_rustowl_output("vec::f5", "r");
    insta::assert_snapshot!(output);
}

// must_live soundness check
#[test]
fn test_must_live_reassign_block_a() {
    let output = get_rustowl_output("must_live::reassign_block", "a");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_reassign_move_a() {
    let output = get_rustowl_output("must_live::reassign_move", "a");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_loop_local_s() {
    let output = get_rustowl_output("must_live::loop_local", "s");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_macro_use_m() {
    let output = get_rustowl_output("must_live::macro_use", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_deref_use_m() {
    let output = get_rustowl_output("must_live::deref_use", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_call_use_m() {
    let output = get_rustowl_output("must_live::call_use", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_copy_use_m() {
    let output = get_rustowl_output("must_live::copy_use", "m");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_move_while_borrowed_a() {
    let output = get_rustowl_output("must_live::move_while_borrowed", "a");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_drop_use_s() {
    let output = get_rustowl_output("must_live::drop_use", "s");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_nll_v() {
    let output = get_rustowl_output("must_live::nll", "v");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_struct_borrow_s() {
    let output = get_rustowl_output("must_live::struct_borrow", "s");
    insta::assert_snapshot!(output);
}

#[test]
fn test_must_live_guard_m() {
    let output = get_rustowl_output("must_live::guard", "m");
    insta::assert_snapshot!(output);
}

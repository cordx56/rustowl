//! The command line cases, under `tests/cmd`. Each runs in a copy of the
//! fixtures, so the file is analysed on its own.

#[test]
fn cli() {
    trycmd::TestCases::new()
        .default_bin_path(env!("CARGO_BIN_EXE_rustowl"))
        .case("tests/cmd/*.toml");
}

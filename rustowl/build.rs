use std::io::Read;
use std::process::{Command, Stdio};

fn main() {
    let child = match Command::new("rustup")
        .args(["show", "active-toolchain"])
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(v) => v,
        Err(_) => return,
    };
    let mut stdout = child.stdout.unwrap();
    let mut output = String::new();
    stdout.read_to_string(&mut output).unwrap();
    let toolchain = output.split("\n").nth(0).unwrap().trim();
    println!("cargo::rustc-env=RUSTOWL_TOOLCHAIN={toolchain}");
}

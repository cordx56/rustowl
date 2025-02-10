mod toolchain_version;

use std::env;
use std::path::PathBuf;
use std::process::{exit, Command};

#[allow(unused)]
use toolchain_version::TOOLCHAIN_VERSION;

fn main() {
    simple_logger::init().unwrap();

    let self_path = PathBuf::from(env::args().next().unwrap());
    let root_dir = PathBuf::from(env::args().nth(2).unwrap_or(".".to_owned()));
    let target_dir = PathBuf::from(env::args().nth(3).unwrap_or("./target".to_owned()));

    #[cfg(windows)]
    {
        let triple_suffix = env::var("RUSTUP_TOOLCHAIN")
            .unwrap()
            .split("-")
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(4)
            .rev()
            .collect::<Vec<_>>()
            .join("-");
        let mut paths = env::split_paths(&env::var_os("Path").unwrap())
            .collect::<std::collections::VecDeque<_>>();
        paths.push_front(
            env::var("RUSTUP_HOME")
                .map(|path| format!("{path}\\toolchains\\{TOOLCHAIN_VERSION}-{triple_suffix}\\bin"))
                .unwrap()
                .into(),
        );
        unsafe {
            env::set_var("Path", env::join_paths(paths).unwrap());
        }
    }

    let mut command = Command::new("cargo");
    command
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            self_path.with_file_name("rustowlc"),
        )
        .env("CARGO_TARGET_DIR", &target_dir)
        .env_remove("RUSTC_WRAPPER")
        .arg("check")
        .current_dir(&root_dir);

    let code = command.spawn().unwrap().wait().unwrap().code().unwrap();
    exit(code);
}

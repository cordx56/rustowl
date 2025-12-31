use divan::{AllocProfiler, Bencher, black_box};

#[cfg(all(not(target_env = "msvc"), not(miri)))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(not(target_env = "msvc"), not(miri)))]
#[global_allocator]
static ALLOC: AllocProfiler<Jemalloc> = AllocProfiler::new(Jemalloc);

#[cfg(any(target_env = "msvc", miri))]
#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    divan::main();
}

// Small but representative cargo message examples.
const COMPILER_ARTIFACT: &str = r#"{"reason":"compiler-artifact","package_id":"foo 0.1.0 (path+file:///tmp/foo)","target":{"kind":["lib"],"crate_types":["lib"],"name":"foo","src_path":"/tmp/foo/src/lib.rs","edition":"2021"}}"#;

// `Workspace` is a transparent newtype around an IndexMap; a minimal value is `{}`.
const WORKSPACE: &str = r#"{}"#;

// Cargo emits many JSON messages that we ignore; they still contain a `reason` field.
const OTHER_CARGO_MESSAGE: &str = r#"{"reason":"build-script-executed","package_id":"bar 0.1.0 (path+file:///tmp/bar)","linked_libs":[],"linked_paths":[]}"#;

#[derive(serde::Deserialize, Clone, Debug)]
struct CargoCheckMessageTarget {
    name: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(tag = "reason", rename_all = "kebab-case")]
enum CargoCheckMessage {
    CompilerArtifact {
        target: CargoCheckMessageTarget,
    },
    #[allow(unused)]
    BuildFinished {},
}

#[derive(serde::Deserialize, Clone, Debug)]
#[serde(transparent)]
struct Workspace(#[allow(dead_code)] std::collections::BTreeMap<String, serde_json::Value>);

fn baseline_parse_line(line: &str) -> (usize, bool) {
    let mut artifacts = 0usize;
    let mut saw_workspace = false;

    if let Ok(CargoCheckMessage::CompilerArtifact { target }) =
        serde_json::from_str::<CargoCheckMessage>(line)
    {
        black_box(&target.name);
        artifacts += 1;
    }
    if let Ok(_ws) = serde_json::from_str::<Workspace>(line) {
        saw_workspace = true;
    }

    (artifacts, saw_workspace)
}

fn optimized_parse_line(buf: &[u8]) -> (usize, bool) {
    let mut artifacts = 0usize;
    let mut saw_workspace = false;

    let artifact_marker = b"\"reason\":\"compiler-artifact\"";
    let reason_marker = b"\"reason\":";

    if memchr::memmem::find(buf, artifact_marker).is_some() {
        if let Ok(CargoCheckMessage::CompilerArtifact { target }) =
            serde_json::from_slice::<CargoCheckMessage>(buf)
        {
            black_box(&target.name);
            artifacts += 1;
        }
        return (artifacts, false);
    }

    if memchr::memmem::find(buf, reason_marker).is_some() {
        return (0, false);
    }

    if serde_json::from_slice::<Workspace>(buf).is_ok() {
        saw_workspace = true;
    }

    (artifacts, saw_workspace)
}

fn make_lines(count: usize) -> Vec<String> {
    let mut lines = Vec::with_capacity(count);
    for i in 0..count {
        if i % 10 == 0 {
            lines.push(WORKSPACE.to_string());
        } else if i % 3 == 0 {
            lines.push(COMPILER_ARTIFACT.to_string());
        } else {
            lines.push(OTHER_CARGO_MESSAGE.to_string());
        }
    }
    lines
}

#[divan::bench(sample_count = 30)]
fn parse_baseline(bencher: Bencher) {
    let lines = make_lines(5_000);
    bencher.bench(|| {
        let mut artifacts = 0usize;
        let mut workspaces = 0usize;
        for line in &lines {
            let (a, w) = baseline_parse_line(line);
            artifacts += a;
            workspaces += usize::from(w);
        }
        black_box((artifacts, workspaces));
    });
}

#[divan::bench(sample_count = 30)]
fn parse_optimized(bencher: Bencher) {
    let lines = make_lines(5_000);
    let bytes: Vec<Vec<u8>> = lines.iter().map(|s| s.as_bytes().to_vec()).collect();

    bencher.bench(|| {
        let mut artifacts = 0usize;
        let mut workspaces = 0usize;
        for buf in &bytes {
            let (a, w) = optimized_parse_line(buf);
            artifacts += a;
            workspaces += usize::from(w);
        }
        black_box((artifacts, workspaces));
    });
}

use divan::{AllocProfiler, Bencher, black_box};

#[cfg(all(any(target_os = "linux", target_os = "macos"), not(miri)))]
use tikv_jemallocator::Jemalloc;

#[cfg(all(any(target_os = "linux", target_os = "macos"), not(miri)))]
#[global_allocator]
static ALLOC: AllocProfiler<Jemalloc> = AllocProfiler::new(Jemalloc);

#[cfg(any(target_os = "windows", miri))]
#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

#[cfg(not(feature = "bench"))]
fn main() {
    eprintln!("`decos_bench` requires `--features bench`");
}

#[cfg(feature = "bench")]
fn main() {
    divan::main();
}

// Benchmarks repeated cursor decoration queries after a one-time analysis preload.
//
// Run with:
// `cargo bench --bench decos_bench --features bench`
#[cfg(feature = "bench")]
#[divan::bench(sample_count = 20)]
fn cursor_decos_hot_path(bencher: Bencher) {
    use rustowl::lsp::backend::Backend;
    use rustowl::lsp::decoration::CursorRequest;
    use std::path::Path;
    use tower_lsp_server::LanguageServer;
    use tower_lsp_server::ls_types::{Position, TextDocumentIdentifier, Uri};

    const DUMMY_PACKAGE: &str = "./perf-tests/dummy-package";
    const TARGET_FILE: &str = "./perf-tests/dummy-package/src/lib.rs";

    let target_path = std::fs::canonicalize(TARGET_FILE).expect("canonicalize TARGET_FILE");
    let target_uri: Uri = format!("file:///{}", target_path.display())
        .parse()
        .expect("valid file URI");

    let sysroot = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("run rustc")
        .stdout;
    let sysroot = String::from_utf8_lossy(&sysroot).trim().to_string();
    unsafe {
        std::env::set_var("RUSTOWL_SYSROOT", sysroot);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let (service, _) = tower_lsp_server::LspService::build(Backend::new(1)).finish();
    let backend = service.inner();

    let ok = rt.block_on(async {
        backend
            .load_analyzed_state_for_bench(Path::new(DUMMY_PACKAGE), false, false)
            .await
    });
    assert!(ok, "analysis preload failed; dummy package not analyzed");

    // Seed the open-doc cache similar to a real LSP client.
    rt.block_on(async {
        backend
            .did_open(tower_lsp_server::ls_types::DidOpenTextDocumentParams {
                text_document: tower_lsp_server::ls_types::TextDocumentItem {
                    uri: target_uri.clone(),
                    language_id: "rust".to_string(),
                    version: 1,
                    text: tokio::fs::read_to_string(&target_path)
                        .await
                        .expect("read target file"),
                },
            })
            .await;
    });

    let req = CursorRequest {
        document: TextDocumentIdentifier { uri: target_uri },
        // Point somewhere on a local variable (`files`).
        position: Position {
            line: 73,
            character: 16,
        },
    };

    // Sanity check: make sure we actually produce decorations.
    let warmup = rt
        .block_on(async { backend.cursor(req.clone()).await })
        .expect("cursor request failed");
    assert!(
        !warmup.decorations.is_empty(),
        "cursor warmup produced no decorations"
    );

    bencher.bench(|| {
        let decorations = rt.block_on(async { backend.cursor(req.clone()).await });
        black_box(decorations.is_ok());
    });
}

#[cfg(feature = "bench")]
#[divan::bench(sample_count = 20)]
fn cursor_decos_disk_fallback(bencher: Bencher) {
    use rustowl::lsp::backend::Backend;
    use rustowl::lsp::decoration::CursorRequest;
    use std::path::Path;
    use tower_lsp_server::ls_types::{Position, TextDocumentIdentifier, Uri};

    const DUMMY_PACKAGE: &str = "./perf-tests/dummy-package";
    const TARGET_FILE: &str = "./perf-tests/dummy-package/src/lib.rs";

    let target_path = std::fs::canonicalize(TARGET_FILE).expect("canonicalize TARGET_FILE");
    let target_uri: Uri = format!("file:///{}", target_path.display())
        .parse()
        .expect("valid file URI");

    let sysroot = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .expect("run rustc")
        .stdout;
    let sysroot = String::from_utf8_lossy(&sysroot).trim().to_string();
    unsafe {
        std::env::set_var("RUSTOWL_SYSROOT", sysroot);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let (service, _) = tower_lsp_server::LspService::build(Backend::new(1)).finish();
    let backend = service.inner();

    let ok = rt.block_on(async {
        backend
            .load_analyzed_state_for_bench(Path::new(DUMMY_PACKAGE), false, false)
            .await
    });
    assert!(ok, "analysis preload failed; dummy package not analyzed");

    // Intentionally do NOT call `did_open`; `cursor` must read from disk.
    let req = CursorRequest {
        document: TextDocumentIdentifier { uri: target_uri },
        // Point somewhere on a local variable (`files`).
        position: Position {
            line: 73,
            character: 16,
        },
    };

    let warmup = rt
        .block_on(async { backend.cursor(req.clone()).await })
        .expect("cursor request failed");
    assert!(
        !warmup.decorations.is_empty(),
        "cursor warmup produced no decorations"
    );

    bencher.bench(|| {
        let decorations = rt.block_on(async { backend.cursor(req.clone()).await });
        black_box(decorations.is_ok());
    });
}

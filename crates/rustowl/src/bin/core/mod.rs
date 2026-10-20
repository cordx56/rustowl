pub mod analyze;
pub mod cache;
pub mod compiler;

use analyze::{AnalyzeResult, MirAnalyzer, MirAnalyzerInitResult};
use compiler::AsRustc;
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_interface::interface;
use rustc_middle::{ty::TyCtxt, util::Providers};
use rustc_session::config;
use rustowl::models::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, LazyLock, Mutex, atomic::AtomicBool};
use tokio::{
    runtime::{Builder, Runtime},
    task::JoinHandle,
};

use rustc_middle::queries;

#[rustversion::since(2026-10-02)]
use rustc_driver::compiler_entrypoint as run_compiler_entrypoint;
#[rustversion::before(2026-10-02)]
use rustc_driver::run_compiler as run_compiler_entrypoint;

pub struct RustcCallback;
impl rustc_driver::Callbacks for RustcCallback {}

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);
static TASKS: LazyLock<Mutex<Vec<JoinHandle<AnalyzeResult>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
// make tokio runtime
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    let worker_threads = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(2, 8))
        .unwrap_or(4);

    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .thread_stack_size(128 * 1024 * 1024)
        .build()
        .unwrap()
});

static DEFAULT_MIR_BORROWCK: LazyLock<
    fn(TyCtxt<'_>, LocalDefId) -> queries::mir_borrowck::ProvidedValue<'_>,
> = LazyLock::new(|| {
    let mut providers = rustc_middle::query::Providers::default();
    rustc_borrowck::provide(&mut providers);
    providers.mir_borrowck
});

/// The set of file paths the editor has open, or `None` when unset.
fn open_files() -> Option<HashSet<PathBuf>> {
    let value = std::env::var("RUSTOWL_OPEN_FILES").ok()?;
    let paths: Vec<PathBuf> = serde_json::from_str(&value).ok()?;
    Some(paths.into_iter().collect())
}

/// Whether `def_id`'s source file is one the editor asked for.
///
/// The list is resolved once per crate and cached against the crate name, so
/// this is a path comparison rather than a per-root recomputation.
fn is_file_open(tcx: TyCtxt<'_>, def_id: LocalDefId) -> bool {
    /// crate name -> the open-file set resolved for it, shared by every root
    type PerCrate = HashMap<String, Arc<Option<HashSet<PathBuf>>>>;
    static CACHE: LazyLock<Mutex<PerCrate>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    let crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
    let set = {
        let mut cache = CACHE.lock().unwrap();
        cache
            .entry(crate_name)
            .or_insert_with(|| Arc::new(open_files()))
            .clone()
    };
    let Some(set) = set.as_ref() else {
        return true;
    };
    let source_map = tcx.sess.source_map();
    let span = tcx.def_span(def_id.to_def_id());
    let file_name = source_map.span_to_filename(span);
    let Some(local_path) = file_name.into_local_path() else {
        return true;
    };
    let path = source_map
        .path_mapping()
        .to_real_filename(source_map.working_dir(), local_path);
    match path.local_path() {
        // local_path is relative to the working directory, while the editor
        // sends absolute paths, so compare against the absolute form
        Some(local) => {
            let absolute = if local.is_absolute() {
                local.to_path_buf()
            } else {
                match source_map.working_dir().local_path() {
                    Some(dir) => dir.join(local),
                    None => return true,
                }
            };
            set.contains(&absolute)
        }
        None => true,
    }
}

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.queries.mir_borrowck = mir_borrowck;
}
fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> queries::mir_borrowck::ProvidedValue<'_> {
    tracing::debug!("start borrowck of {def_id:?}");

    // The editor told us which files it has open; analysing the rest would
    // spend a second borrow check per body for decorations nobody can see.
    // Absent or malformed input means eager mode.
    //
    // borrow check still has to run for skipped files -- this query has to
    // return a result -- so it is called here rather than further down.
    if !is_file_open(tcx, def_id) {
        return DEFAULT_MIR_BORROWCK(tcx, def_id);
    }

    // Must precede DEFAULT_MIR_BORROWCK: that provider steals `tcx.mir_built`,
    // and the mir_built index is keyed on hashing it. Hash it after the steal
    // and every root reports "already stolen", the index stays empty, and the
    // whole optimisation is dead code.
    let analyzers = MirAnalyzer::init(AsRustc::from_rustc(tcx), AsRustc::from_rustc(def_id));

    let default_borrowck_result = DEFAULT_MIR_BORROWCK(tcx, def_id);
    {
        let mut tasks = TASKS.lock().unwrap();
        for analyzer in analyzers {
            match analyzer {
                MirAnalyzerInitResult::Cached(cached) => {
                    handle_analyzed_result(Ok(cached));
                }
                MirAnalyzerInitResult::Analyzer(analyzer) => {
                    tasks.push(
                        RUNTIME
                            .handle()
                            .spawn(async move { analyzer.await.analyze() }),
                    );
                }
            }
        }

        tracing::debug!("there are {} tasks", tasks.len());
        let mut ready = Vec::new();
        let mut i = 0;
        while i < tasks.len() {
            if tasks[i].is_finished() {
                ready.push(tasks.swap_remove(i));
            } else {
                i += 1;
            }
        }
        drop(tasks);
        for handle in ready {
            handle_analyzed_result(RUNTIME.block_on(handle).map_err(|e| e.to_string()));
        }
    }

    default_borrowck_result
}

/// Results of one compilation, printed as one `Workspace` per crate: the
/// receiver merges whole crates, so a crate costs one wrap, one parse and one
/// write instead of one per function.
#[derive(Default)]
pub struct Batch {
    results: Vec<AnalyzeResult>,
}

static BATCH: Mutex<Batch> = Mutex::new(Batch {
    results: Vec::new(),
});

impl Batch {
    fn push(&mut self, result: Result<AnalyzeResult, String>) {
        match result {
            Ok(result) => {
                // The entry must stay in the cache: a later compile reads it to
                // skip borrowck for an unchanged root, which saves far more
                // than the copy costs.
                if !result.from_cache {
                    cache::put_entry(
                        &result.key,
                        result.base,
                        std::sync::Arc::clone(&result.analyzed),
                    );
                }
                self.results.push(result);
            }
            Err(e) => tracing::warn!("analysis task failed: {e}"),
        }
    }

    fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    fn build_workspace(&self, crate_name: &str) -> Workspace {
        let mut files: HashMap<String, File> = HashMap::new();
        for result in &self.results {
            files
                .entry(result.file_path.to_string_lossy().into_owned())
                .or_insert_with(|| File { items: Vec::new() })
                .items
                .push(std::sync::Arc::clone(&result.analyzed));
        }
        Workspace(HashMap::from([(crate_name.to_string(), Crate(files))]))
    }
}

fn handle_analyzed_result(result: Result<AnalyzeResult, String>) {
    BATCH.lock().unwrap().push(result);
}

pub struct AnalyzerCallback;
impl rustc_driver::Callbacks for AnalyzerCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.using_internal_features = &ATOMIC_TRUE;
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.polonius = config::Polonius::Next;
        config.opts.incremental = None;
        config.override_queries = Some(override_queries);
        config.make_codegen_backend = None;
    }
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        cache::init_cache(&tcx.crate_name(LOCAL_CRATE).to_string());
        *BATCH.lock().unwrap() = Batch::default();
        let result = rustc_driver::catch_fatal_errors(|| tcx.analysis(()));

        // Join all tasks after all analysis finished.
        loop {
            // guard dropped at the end of this statement
            let next = TASKS.lock().unwrap().pop();
            let Some(handle) = next else {
                break;
            };
            handle_analyzed_result(RUNTIME.block_on(handle).map_err(|e| e.to_string()));
        }
        {
            let batch = BATCH.lock().unwrap();
            if !batch.is_empty() {
                let ws = batch.build_workspace(&tcx.crate_name(LOCAL_CRATE).to_string());
                println!("{}", serde_json::to_string(&ws).unwrap());
            }
        }
        cache::persist_entries(&tcx.crate_name(LOCAL_CRATE).to_string());

        if result.is_ok() {
            rustc_driver::Compilation::Continue
        } else {
            rustc_driver::Compilation::Stop
        }
    }
}

pub fn run_compiler() -> ExitCode {
    let mut args: Vec<String> = wild::args().collect();
    // by using `RUSTC_WORKSPACE_WRAPPER`, arguments will be as follows:
    // For dependencies: rustowlc [args...]
    // For user workspace: rustowlc rustowlc [args...]
    // So we skip analysis if currently-compiling crate is one of the dependencies
    if args.first() == args.get(1) {
        args = args.into_iter().skip(1).collect();
    } else {
        return rustc_driver::catch_with_exit_code(|| {
            run_compiler_entrypoint(&args, &mut RustcCallback)
        });
    }

    for arg in &args {
        // utilize default rustc to avoid unexpected behavior if these arguments are passed
        if arg == "-vV" || arg == "--version" || arg.starts_with("--print") {
            return rustc_driver::catch_with_exit_code(|| {
                run_compiler_entrypoint(&args, &mut RustcCallback)
            });
        }
    }

    rustc_driver::catch_with_exit_code(|| {
        run_compiler_entrypoint(&args, &mut AnalyzerCallback);
    })
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    use rustowl::models::{Function, MirDecl};
    use std::path::PathBuf;

    fn result(path: &str, fn_id: u32) -> AnalyzeResult {
        AnalyzeResult {
            file_path: PathBuf::from(path),
            key: cache::key::FunctionKey {
                def_path: u64::from(fn_id),
                fn_text_hash: 0,
                mir_hash: String::new(),
                analysis_version: 0,
                toolchain_hash: 0,
            },
            base: Loc(0),
            from_cache: false,
            analyzed: Arc::new(Function {
                fn_id,
                name: format!("f{fn_id}"),
                basic_blocks: Vec::new(),
                decls: Vec::<MirDecl>::new(),
            }),
        }
    }

    #[test]
    fn a_batched_emission_survives_a_failed_result() {
        let mut batch = Batch::default();
        batch.push(Ok(result("lib.rs", 1)));
        batch.push(Err("analysis task failed".to_string()));
        batch.push(Ok(result("lib.rs", 2)));

        assert_eq!(
            batch.build_workspace("crate").0["crate"].0["lib.rs"]
                .items
                .len(),
            2
        );
    }

    #[test]
    fn an_empty_batch_prints_nothing() {
        let mut batch = Batch::default();
        batch.push(Err("analysis task failed".to_string()));
        assert!(batch.is_empty());
    }

    #[test]
    fn the_batch_keeps_the_arc_it_was_given() {
        let entry = result("lib.rs", 1);
        let shared = Arc::clone(&entry.analyzed);
        let mut batch = Batch::default();
        batch.push(Ok(entry));

        let ws = batch.build_workspace("crate");
        assert!(
            Arc::ptr_eq(&shared, &ws.0["crate"].0["lib.rs"].items[0]),
            "the emitted body must be the same allocation the cache holds"
        );
    }
}

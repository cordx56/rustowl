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
use std::collections::HashMap;
use std::env;
use std::process::ExitCode;
use std::sync::{LazyLock, Mutex, atomic::AtomicBool};
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

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.queries.mir_borrowck = mir_borrowck;
}
fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> queries::mir_borrowck::ProvidedValue<'_> {
    log::debug!("start borrowck of {def_id:?}");

    let default_borrowck_result = DEFAULT_MIR_BORROWCK(tcx, def_id);
    let analyzers = MirAnalyzer::init(AsRustc::from_rustc(tcx), AsRustc::from_rustc(def_id));
    {
        let mut tasks = TASKS.lock().unwrap();
        for (_, analyzer) in analyzers {
            match analyzer {
                MirAnalyzerInitResult::Cached(cached) => {
                    handle_analyzed_result(tcx, cached);
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

        log::debug!("there are {} tasks", tasks.len());
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
            match RUNTIME.block_on(handle) {
                Ok(result) => {
                    log::debug!("one task joined");
                    handle_analyzed_result(tcx, result);
                }
                Err(e) => log::warn!("analysis task failed: {e}"),
            }
        }
    }

    default_borrowck_result
}

pub struct AnalyzerCallback;
impl rustc_driver::Callbacks for AnalyzerCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.using_internal_features = &ATOMIC_TRUE;
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.polonius = config::Polonius::Next;
        config.opts.output_types =
            config::OutputTypes::new(&[(config::OutputType::Metadata, None)]);
        config.opts.unstable_opts.no_codegen = true;
        config.opts.incremental = None;
        config.override_queries = Some(override_queries);
        config.make_codegen_backend = None;
    }
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        let result = rustc_driver::catch_fatal_errors(|| tcx.analysis(()));

        // Join all tasks after all analysis finished.
        loop {
            // guard dropped at the end of this statement
            let next = TASKS.lock().unwrap().pop();
            let Some(handle) = next else {
                break;
            };
            match RUNTIME.block_on(handle) {
                Ok(result) => {
                    log::debug!("one task joined");
                    handle_analyzed_result(tcx, result);
                }
                Err(e) => log::warn!("analysis task failed: {e}"),
            }
        }
        if let Some(cache) = cache::CACHE.lock().unwrap().as_ref() {
            cache::write_cache(&tcx.crate_name(LOCAL_CRATE).to_string(), cache);
        }

        if result.is_ok() {
            rustc_driver::Compilation::Continue
        } else {
            rustc_driver::Compilation::Stop
        }
    }
}

pub fn handle_analyzed_result(tcx: TyCtxt<'_>, analyzed: AnalyzeResult) {
    let AnalyzeResult {
        file_path,
        file_hash,
        mir_hash,
        analyzed: function,
    } = analyzed;
    // cached by move, then taken back out for the emitted document, so the
    // whole Function is never deep-cloned
    let function = {
        let mut cache = cache::CACHE.lock().unwrap();
        match cache.as_mut() {
            Some(cache) => {
                cache.insert_cache(file_hash.clone(), mir_hash.clone(), function);
                cache
                    .take_cache(&file_hash, &mir_hash)
                    .unwrap_or_else(|| unreachable!("just inserted"))
            }
            None => function,
        }
    };
    let krate = Crate(HashMap::from([(
        file_path.to_string_lossy().to_string(),
        File {
            items: vec![function],
        },
    )]));
    // get currently-compiling crate name
    let crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
    let ws = Workspace(HashMap::from([(crate_name.clone(), krate)]));
    println!("{}", serde_json::to_string(&ws).unwrap());
}

pub fn run_compiler() -> ExitCode {
    let mut args: Vec<String> = env::args().collect();
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

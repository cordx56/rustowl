mod analyze;
mod cache;

use analyze::{AnalyzeResult, MirAnalyzer, MirAnalyzerInitResult};
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_interface::interface;
use rustc_middle::{mir::ConcreteOpaqueTypes, query::queries, ty::TyCtxt, util::Providers};
use rustc_session::config;
use rustowl::models::*;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::env;
use std::sync::{LazyLock, Mutex, atomic::AtomicBool};
use tokio::{
    runtime::{Builder, Runtime},
    task::JoinSet,
};

pub struct RustcCallback;
impl rustc_driver::Callbacks for RustcCallback {}

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);
static TASKS: LazyLock<Mutex<JoinSet<AnalyzeResult>>> =
    LazyLock::new(|| Mutex::new(JoinSet::new()));
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

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.mir_borrowck = mir_borrowck;
}
fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> queries::mir_borrowck::ProvidedValue<'_> {
    log::info!("start borrowck of {def_id:?}");

    let analyzer = MirAnalyzer::init(tcx, def_id);

    {
        let mut tasks = TASKS.lock().unwrap();
        match analyzer {
            MirAnalyzerInitResult::Cached(cached) => {
                handle_analyzed_result(tcx, cached);
            }
            MirAnalyzerInitResult::Analyzer(analyzer) => {
                tasks.spawn_on(async move { analyzer.await.analyze() }, RUNTIME.handle());
            }
        }

        log::info!("there are {} tasks", tasks.len());
        while let Some(Ok(result)) = tasks.try_join_next() {
            log::info!("one task joined");
            handle_analyzed_result(tcx, result);
        }
    }

    for def_id in tcx.nested_bodies_within(def_id) {
        let _ = mir_borrowck(tcx, def_id);
    }

    Ok(tcx.arena.alloc(ConcreteOpaqueTypes(
        rustc_data_structures::fx::FxIndexMap::default(),
    )))
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
        let result = rustc_driver::catch_fatal_errors(|| tcx.analysis(()));

        // join all tasks after all analysis finished
        //
        // allow clippy::await_holding_lock because `tokio::sync::Mutex` cannot use
        // for TASKS because block_on cannot be used in `mir_borrowck`.
        #[allow(clippy::await_holding_lock)]
        RUNTIME.block_on(async move {
            while let Some(Ok(result)) = { TASKS.lock().unwrap().join_next().await } {
                log::info!("one task joined");
                handle_analyzed_result(tcx, result);
            }
            if let Some(cache) = cache::CACHE.lock().unwrap().as_ref() {
                cache::write_cache(&tcx.crate_name(LOCAL_CRATE).to_string(), cache);
            }
        });

        if result.is_ok() {
            rustc_driver::Compilation::Continue
        } else {
            rustc_driver::Compilation::Stop
        }
    }
}

pub fn handle_analyzed_result(tcx: TyCtxt<'_>, analyzed: AnalyzeResult) {
    if let Some(cache) = cache::CACHE.lock().unwrap().as_mut() {
        cache.insert_cache(
            analyzed.file_hash.clone(),
            analyzed.mir_hash.clone(),
            analyzed.analyzed.clone(),
        );
    }
    let krate = Crate(HashMap::from([(
        analyzed.file_name.to_owned(),
        File {
            items: SmallVec::from_vec(vec![analyzed.analyzed]),
        },
    )]));
    // get currently-compiling crate name
    let crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
    let ws = Workspace(HashMap::from([(crate_name.clone(), krate)]));
    println!("{}", serde_json::to_string(&ws).unwrap());
}

pub fn run_compiler() -> i32 {
    let mut args: Vec<String> = env::args().collect();
    // by using `RUSTC_WORKSPACE_WRAPPER`, arguments will be as follows:
    // For dependencies: rustowlc [args...]
    // For user workspace: rustowlc rustowlc [args...]
    // So we skip analysis if currently-compiling crate is one of the dependencies
    if args.first() == args.get(1) {
        args = args.into_iter().skip(1).collect();
    } else {
        return rustc_driver::catch_with_exit_code(|| {
            rustc_driver::run_compiler(&args, &mut RustcCallback)
        });
    }

    for arg in &args {
        // utilize default rustc to avoid unexpected behavior if these arguments are passed
        if arg == "-vV" || arg == "--version" || arg.starts_with("--print") {
            return rustc_driver::catch_with_exit_code(|| {
                rustc_driver::run_compiler(&args, &mut RustcCallback)
            });
        }
    }

    rustc_driver::catch_with_exit_code(|| {
        rustc_driver::run_compiler(&args, &mut AnalyzerCallback);
    })
}

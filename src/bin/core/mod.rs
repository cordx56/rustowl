mod analyze;
mod cache;

use analyze::{AnalyzeResult, MirAnalyzer, MirAnalyzerInitResult};
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_interface::interface;
use rustc_middle::{mir::ConcreteOpaqueTypes, query::queries, ty::TyCtxt, util::Providers};
use rustc_session::config;
use rustowl::models::FoldIndexMap as HashMap;
use rustowl::models::*;
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
    tracing::info!("start borrowck of {def_id:?}");

    let analyzer = MirAnalyzer::init(tcx, def_id);

    {
        let mut tasks = TASKS.lock().unwrap();
        match analyzer {
            MirAnalyzerInitResult::Cached(cached) => {
                handle_analyzed_result(tcx, *cached);
            }
            MirAnalyzerInitResult::Analyzer(analyzer) => {
                tasks.spawn_on(async move { analyzer.await.analyze() }, RUNTIME.handle());
            }
        }

        tracing::info!("there are {} tasks", tasks.len());
        while let Some(Ok(result)) = tasks.try_join_next() {
            tracing::info!("one task joined");
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
        // Drain all remaining analysis tasks synchronously
        loop {
            // First collect any tasks that have already finished
            while let Some(Ok(result)) = {
                let mut guard = TASKS.lock().unwrap();
                guard.try_join_next()
            } {
                tracing::info!("one task joined");
                handle_analyzed_result(tcx, result);
            }

            // Check if all tasks are done
            let has_tasks = {
                let guard = TASKS.lock().unwrap();
                !guard.is_empty()
            };
            if !has_tasks {
                break;
            }

            // Wait for at least one more task to finish
            let result = {
                let mut guard = TASKS.lock().unwrap();
                RUNTIME.block_on(guard.join_next())
            };
            if let Some(Ok(result)) = result {
                tracing::info!("one task joined");
                handle_analyzed_result(tcx, result);
            }
        }

        if let Some(cache) = cache::CACHE.lock().unwrap().as_ref() {
            // Log cache statistics before writing
            let stats = cache.get_stats();
            tracing::info!(
                "Cache statistics: {} hits, {} misses, {:.1}% hit rate, {} evictions",
                stats.hits,
                stats.misses,
                stats.hit_rate() * 100.0,
                stats.evictions
            );
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
    if let Some(cache) = cache::CACHE.lock().unwrap().as_mut() {
        // Pass file name for potential file modification time validation
        cache.insert_cache_with_file_path(
            analyzed.file_hash.clone(),
            analyzed.mir_hash.clone(),
            analyzed.analyzed.clone(),
            Some(&analyzed.file_name),
        );
    }
    let mut map = HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
    map.insert(
        analyzed.file_name.to_owned(),
        File {
            items: smallvec::smallvec![analyzed.analyzed],
        },
    );
    let krate = Crate(map);
    // get currently-compiling crate name
    let crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
    let mut ws_map =
        HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
    ws_map.insert(crate_name.clone(), krate);
    let ws = Workspace(ws_map);
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

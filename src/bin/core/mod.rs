mod analyze;
mod cache;

use analyze::{AnalyzeResult, MirAnalyzer, MirAnalyzerInitResult};
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_interface::interface;
use rustc_middle::{mir::ConcreteOpaqueTypes, query::queries, ty::TyCtxt, util::Providers};
use rustc_session::config;
use rustowl::models::*;
use std::collections::HashMap;
use std::env;
use std::sync::{LazyLock, atomic::AtomicBool};
use tokio::{
    runtime::{Builder, Handle, Runtime},
    sync::Mutex,
    task::JoinSet,
};

pub struct RustcCallback;
impl rustc_driver::Callbacks for RustcCallback {}

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);
static TASKS: LazyLock<Mutex<JoinSet<MirAnalyzer<'static>>>> =
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
// tokio runtime handler
static HANDLE: LazyLock<Handle> = LazyLock::new(|| RUNTIME.handle().clone());

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.mir_borrowck = mir_borrowck;
}
fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> queries::mir_borrowck::ProvidedValue<'_> {
    log::info!("start borrowck of {def_id:?}");

    let mut def_ids = vec![def_id];
    def_ids.extend_from_slice(tcx.nested_bodies_within(def_id));

    for def_id in def_ids {
        let analyzer = MirAnalyzer::init(
            // To run analysis tasks in parallel, we ignore lifetime annotation in some types.
            // This can be done since the TyCtxt object lives until the analysis tasks joined
            // in after_analysis method.
            unsafe { std::mem::transmute::<TyCtxt<'_>, TyCtxt<'_>>(tcx) },
            def_id,
        );
        RUNTIME.block_on(async move {
            let mut locked = TASKS.lock().await;

            match analyzer {
                MirAnalyzerInitResult::Cached(cached) => {
                    handle_analyzed_result(tcx, cached);
                }
                MirAnalyzerInitResult::Analyzer(analyzer) => {
                    locked.spawn_on(analyzer, &HANDLE);
                }
            }
        });
    }

    RUNTIME.block_on(async move {
        let mut tasks = TASKS.lock().await;
        log::info!("there are {} tasks", tasks.len());
        while let Some(Ok(analyzer)) = tasks.try_join_next() {
            log::info!("one task joined");
            handle_analyzed_result(tcx, analyzer.analyze());
        }
    });

    Ok(tcx
        .arena
        .alloc(ConcreteOpaqueTypes(indexmap::IndexMap::default())))
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
        let _ = rustc_driver::catch_fatal_errors(|| tcx.analysis(()));

        // join all tasks after all analysis finished
        RUNTIME.block_on(async move {
            while let Some(Ok(analyzer)) = { TASKS.lock().await.join_next().await } {
                log::info!("one task joined");
                handle_analyzed_result(tcx, analyzer.analyze());
            }
            if let Some(cache) = cache::CACHE.lock().unwrap().as_ref() {
                cache::write_cache(&tcx.crate_name(LOCAL_CRATE).to_string(), cache);
            }
        });

        rustc_driver::Compilation::Continue
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
            items: vec![analyzed.analyzed],
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

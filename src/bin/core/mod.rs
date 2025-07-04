#![allow(clippy::await_holding_lock)]

mod analyze;

use analyze::MirAnalyzer;
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_interface::interface;
use rustc_middle::{
    mir::ConcreteOpaqueTypes, query::queries::mir_borrowck::ProvidedValue, ty::TyCtxt,
    util::Providers,
};
use rustc_session::config;
use rustowl::models::*;
use std::collections::HashMap;
use std::env;
use std::sync::{LazyLock, Mutex, atomic::AtomicBool};
use tokio::{
    runtime::{Builder, Handle, Runtime},
    task::JoinSet,
};

pub struct RustcCallback;
impl rustc_driver::Callbacks for RustcCallback {}

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);
static TASKS: LazyLock<Mutex<JoinSet<MirAnalyzer<'static>>>> =
    LazyLock::new(|| Mutex::new(JoinSet::new()));
// make tokio runtime
static RUNTIME: LazyLock<Mutex<Runtime>> = LazyLock::new(|| {
    let worker_threads = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(2, 8))
        .unwrap_or(4);

    Mutex::new(
        Builder::new_multi_thread()
            .enable_all()
            .worker_threads(worker_threads)
            .thread_stack_size(128 * 1024 * 1024)
            .build()
            .unwrap(),
    )
});
// tokio runtime handler
static HANDLE: LazyLock<Handle> = LazyLock::new(|| RUNTIME.lock().unwrap().handle().clone());
static ANALYZED: LazyLock<Mutex<Vec<LocalDefId>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.mir_borrowck = mir_borrowck;
}
fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> ProvidedValue<'_> {
    log::info!("start borrowck of {def_id:?}");

    let analyzer = MirAnalyzer::new(
        // To run analysis tasks in parallel, we ignore lifetime annotation in some types.
        // This can be done since the TyCtxt object lives until the analysis tasks joined
        // in after_analysis method.
        unsafe {
            std::mem::transmute::<TyCtxt<'_>, TyCtxt<'_>>(tcx)
        },
        def_id,
    );
    {
        let mut locked = TASKS.lock().unwrap();
        locked.spawn_on(analyzer, &HANDLE);
    }
    {
        let mut locked = ANALYZED.lock().unwrap();
        locked.push(def_id);
        let current = locked.len();
        let mir_len = tcx
            .mir_keys(())
            .into_iter()
            .filter(|v| tcx.hir_node_by_def_id(**v).body_id().is_some())
            .count();
        log::info!("borrow checked: {current} / {mir_len}");
    }

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

    /// join all tasks after all analysis finished
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        // get currently-compiling crate name
        let crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
        RUNTIME.lock().unwrap().block_on(async move {
            while let Some(task) = { TASKS.lock().unwrap().join_next().await } {
                let (filename, analyzed) = task.unwrap().analyze();
                log::info!("analyzed one item of {filename}");
                let krate = Crate(HashMap::from([(
                    filename,
                    File {
                        items: vec![analyzed],
                    },
                )]));
                let ws = Workspace(HashMap::from([(crate_name.clone(), krate)]));
                println!("{}", serde_json::to_string(&ws).unwrap());
            }
        });
        rustc_driver::Compilation::Continue
    }
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

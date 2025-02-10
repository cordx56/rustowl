#![feature(rustc_private)]

pub extern crate indexmap;
pub extern crate polonius_engine;
pub extern crate rustc_borrowck;
pub extern crate rustc_driver;
pub extern crate rustc_errors;
pub extern crate rustc_hash;
pub extern crate rustc_hir;
pub extern crate rustc_index;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_session;
pub extern crate rustc_span;
pub extern crate smallvec;

mod analyze;
mod from_rustc;

use analyze::MirAnalyzer;
use models::*;
use rustc_driver::{Callbacks, RunCompiler};
use rustc_hir::def_id::LocalDefId;
use rustc_interface::interface;
use rustc_middle::{
    mir::BorrowCheckResult, query::queries::mir_borrowck::ProvidedValue, ty::TyCtxt,
    util::Providers,
};
use rustc_session::{config, EarlyDiagCtxt};
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, LazyLock, Mutex};
use tokio::{
    runtime::{Builder, Handle, Runtime},
    task::JoinSet,
};

pub struct RustcCallback;
impl Callbacks for RustcCallback {}

static TASKS: LazyLock<Mutex<JoinSet<MirAnalyzer<'static>>>> =
    LazyLock::new(|| Mutex::new(JoinSet::new()));
static RUNTIME: LazyLock<Mutex<Runtime>> = LazyLock::new(|| {
    Mutex::new(
        Builder::new_multi_thread()
            .enable_all()
            .worker_threads(8)
            .thread_stack_size(1024 * 1024 * 1024)
            .build()
            .unwrap(),
    )
});
static HANDLE: LazyLock<Handle> = LazyLock::new(|| RUNTIME.lock().unwrap().handle().clone());
static ANALYZED: LazyLock<Mutex<Vec<LocalDefId>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.mir_borrowck = mir_borrowck;
}
#[allow(clippy::await_holding_lock)]
fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> ProvidedValue<'_> {
    log::info!("start borrowck of {def_id:?}");

    let analyzer = MirAnalyzer::new(
        unsafe {
            std::mem::transmute::<rustc_middle::ty::TyCtxt<'_>, rustc_middle::ty::TyCtxt<'_>>(tcx)
        },
        def_id,
    );
    {
        let mut locked = TASKS.lock().unwrap();
        locked.spawn_on(analyzer, &HANDLE);
    }
    let (current, mir_len) = {
        let mut locked = ANALYZED.lock().unwrap();
        locked.push(def_id);
        let current = locked.len();
        let mir_len = tcx
            .mir_keys(())
            .into_iter()
            .filter(|v| tcx.hir_node_by_def_id(**v).body_id().is_some())
            .count();
        log::info!("borrow checked: {} / {}", current, mir_len);
        (current, mir_len)
    };
    if current == mir_len {
        RUNTIME.lock().unwrap().block_on(async move {
            while let Some(task) = { TASKS.lock().unwrap().join_next() }.await {
                let (filename, analyzed) = task.unwrap().analyze();
                log::info!("analyzed one item of {}", filename);
                let ws = Workspace(HashMap::from([(
                    filename,
                    File {
                        items: vec![analyzed],
                    },
                )]));
                println!("{}", serde_json::to_string(&ws).unwrap());
            }
        })
    }

    let result = BorrowCheckResult {
        concrete_opaque_types: indexmap::IndexMap::default(),
        closure_requirements: None,
        used_mut_upvars: smallvec::SmallVec::new(),
        tainted_by_errors: None,
    };

    tcx.arena.alloc(result)
}

pub struct AnalyzerCallback;
impl Callbacks for AnalyzerCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.polonius = config::Polonius::Next;
        config.opts.incremental = None;
        config.override_queries = Some(override_queries);
        config.make_codegen_backend = None;
    }
}

pub fn run_compiler() -> i32 {
    let ctxt = EarlyDiagCtxt::new(config::ErrorOutputType::default());
    let args = rustc_driver::args::raw_args(&ctxt).unwrap();
    let args = &args[1..];
    for arg in args {
        if arg == "-vV" || arg.starts_with("--print") {
            let mut callback = RustcCallback;
            let runner = RunCompiler::new(args, &mut callback);
            return rustc_driver::catch_with_exit_code(|| runner.run());
        }
    }
    let mut callback = AnalyzerCallback;
    let mut runner = RunCompiler::new(args, &mut callback);
    runner.set_make_codegen_backend(None);
    rustc_driver::catch_with_exit_code(|| {
        runner
            .set_using_internal_features(Arc::new(AtomicBool::new(true)))
            .run()
    })
}

mod polonius_analyzer;
//mod transform;

use super::cache;
pub use super::compiler::*;
use rustowl::models::*;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

pub type MirAnalyzeFuture = Pin<Box<dyn Future<Output = MirAnalyzer> + Send + Sync>>;

#[derive(Clone, Debug)]
pub struct AnalyzeResult {
    pub file_path: PathBuf,
    pub file_hash: String,
    pub mir_hash: String,
    pub analyzed: Function,
}

pub enum MirAnalyzerInitResult {
    Cached(AnalyzeResult),
    Analyzer(MirAnalyzeFuture),
}

pub struct MirAnalyzer {
    file_path: PathBuf,
    local_decls: HashMap<LocalId, String>,
    user_vars: HashMap<LocalId, (Range, String)>,
    input: PoloniusInput,
    basic_blocks: Vec<MirBasicBlock>,
    fn_id: DefId,
    name: String,
    file_hash: String,
    mir_hash: String,
    accurate_live: HashMap<LocalId, Vec<Range>>,
    must_live: HashMap<LocalId, Vec<Range>>,
    shared_live: HashMap<LocalId, Vec<Range>>,
    mutable_live: HashMap<LocalId, Vec<Range>>,
    drop_range: HashMap<LocalId, Vec<Range>>,
}
impl MirAnalyzer {
    /// initialize analyzer
    pub fn init(tcx: TyCtxt<'_>, fn_id: DefId) -> HashMap<DefId, MirAnalyzerInitResult> {
        let mut result = HashMap::new();

        let facts = tcx.get_borrowck_facts(fn_id);
        for (fn_id, mut facts) in facts {
            let source_info = tcx.source_info_from_span(facts.body().span());
            let name = tcx.def_name(fn_id);
            log::debug!("facts of {fn_id:?} ({name}) prepared; start analyze...");

            let body = facts.body();

            // collect local declared vars
            // this must be done in local thread
            let local_decls = body.get_local_decls();

            let file_path = source_info.path().to_path_buf();

            // region variables should not be hashed (it results an error)
            // so we erase region variables and set 'static as new region
            let mir_hash = tcx.get_hash(body.clone().erase_region_variables(tcx).as_rustc());
            let file_hash = tcx.get_hash(source_info.source());

            let mut cache = cache::CACHE.lock().unwrap();

            // setup cache
            if cache.is_none() {
                *cache = cache::get_cache(&tcx.crate_name());
            }
            if let Some(cache) = cache.as_mut()
                && let Some(analyzed) = cache.get_cache(&file_hash, &mir_hash)
            {
                log::debug!("MIR cache hit: {fn_id:?}");
                result.insert(
                    fn_id,
                    MirAnalyzerInitResult::Cached(AnalyzeResult {
                        file_path: source_info.path().to_path_buf(),
                        file_hash,
                        mir_hash,
                        analyzed: analyzed.clone(),
                    }),
                );
                continue;
            }
            drop(cache);

            // collect user defined vars
            // this must be done in local thread
            let user_vars = body.collect_user_variables(&source_info);

            // build basic blocks map
            // this must be done in local thread
            let basic_blocks = tcx.collect_basic_blocks(fn_id, &body, &source_info);

            // collect borrow data
            // this must be done in local thread
            let borrow_data = facts.borrow_map();

            let input = facts.polonius_input();
            let location_table = facts.location_table();

            let analyzer = Box::pin(async move {
                log::debug!("start re-computing borrow check with dump: true");
                // compute accurate region, which may eliminate invalid region
                let output = input.compute();
                log::debug!("second borrow check finished");

                let accurate_live =
                    polonius_analyzer::get_accurate_live(&output, &location_table, &basic_blocks);

                let must_live = polonius_analyzer::get_must_live(
                    &output,
                    &location_table,
                    &borrow_data,
                    &basic_blocks,
                );

                let (shared_live, mutable_live) = polonius_analyzer::get_borrow_live(
                    &output,
                    &location_table,
                    &borrow_data,
                    &basic_blocks,
                );

                let drop_range =
                    polonius_analyzer::drop_range(&output, &location_table, &basic_blocks);

                MirAnalyzer {
                    file_path,
                    local_decls,
                    input,
                    user_vars,
                    basic_blocks,
                    fn_id,
                    name,
                    file_hash,
                    mir_hash,
                    accurate_live,
                    must_live,
                    shared_live,
                    mutable_live,
                    drop_range,
                }
            });
            result.insert(fn_id, MirAnalyzerInitResult::Analyzer(analyzer));
        }
        result
    }

    /// collect declared variables in MIR body
    /// final step of analysis
    fn collect_decls(&self) -> Vec<MirDecl> {
        let user_vars = &self.user_vars;
        let lives = &self.accurate_live;
        let must_live_at = &self.must_live;

        let drop_range = &self.drop_range;
        self.local_decls
            .iter()
            .map(|(local, ty)| {
                let ty = ty.clone();
                let must_live_at = must_live_at.get(local).cloned().unwrap_or(Vec::new());
                let lives = lives.get(local).cloned().unwrap_or(Vec::new());
                let shared_borrow = self.shared_live.get(local).cloned().unwrap_or(Vec::new());
                let mutable_borrow = self.mutable_live.get(local).cloned().unwrap_or(Vec::new());
                let drop = self.is_drop(*local);
                let drop_range = drop_range.get(local).cloned().unwrap_or(Vec::new());
                let fn_local = FnLocal::new(local.as_u32(), self.fn_id.as_u32());
                if let Some((span, name)) = user_vars.get(local).cloned() {
                    MirDecl::User {
                        local: fn_local,
                        name,
                        span,
                        ty,
                        lives,
                        shared_borrow,
                        mutable_borrow,
                        must_live_at,
                        drop,
                        drop_range,
                    }
                } else {
                    MirDecl::Other {
                        local: fn_local,
                        ty,
                        lives,
                        shared_borrow,
                        mutable_borrow,
                        drop,
                        drop_range,
                        must_live_at,
                    }
                }
            })
            .collect()
    }

    fn is_drop(&self, local: LocalId) -> bool {
        for (drop_local, _) in self.input.var_dropped_at().iter() {
            if *drop_local == local {
                return true;
            }
        }
        false
    }

    /// analyze MIR to get JSON-serializable, TypeScript friendly representation
    pub fn analyze(self) -> AnalyzeResult {
        let decls = self.collect_decls();
        let basic_blocks = self.basic_blocks;

        AnalyzeResult {
            file_path: self.file_path,
            file_hash: self.file_hash,
            mir_hash: self.mir_hash,
            analyzed: Function {
                fn_id: self.fn_id.as_u32(),
                name: self.name,
                basic_blocks,
                decls,
            },
        }
    }
}

mod dataflow_analyzer;
mod polonius_analyzer;

use super::cache;
pub use super::compiler::*;
use indexmap::IndexMap;
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
    local_decls: IndexMap<LocalId, MirType>,
    user_vars: IndexMap<LocalId, (Range, String)>,
    input: PoloniusInput,
    basic_blocks: IndexMap<BasicBlockId, MirBasicBlock>,
    fn_id: DefId,
    name: String,
    file_hash: String,
    mir_hash: String,
    accurate_live: HashMap<LocalId, Vec<Range>>,
    deficit: HashMap<LocalId, Vec<Range>>,
    shared_live: HashMap<LocalId, Vec<Range>>,
    mutable_live: HashMap<LocalId, Vec<Range>>,
    drop_range: HashMap<LocalId, Vec<Range>>,
    storage_range: HashMap<LocalId, Vec<Range>>,
    definitely_live_range: HashMap<LocalId, Vec<Range>>,
    maybe_init_range: HashMap<LocalId, Vec<Range>>,
}
impl MirAnalyzer {
    /// initialize analyzer
    pub fn init(tcx: TyCtxt<'_>, fn_id: DefId) -> Vec<MirAnalyzerInitResult> {
        let mut result = Vec::new();

        {
            let mut cache = cache::CACHE.lock().unwrap();
            if cache.is_none() {
                *cache = cache::get_cache(&tcx.crate_name());
            }
            let resolved = cache.as_ref().and_then(|cache| {
                // Only hash MIR when there is a non-empty index to consult; a
                // cold cache pays nothing for this lookup.
                if cache.by_built.is_empty() {
                    return None;
                }
                let built_hash = tcx.mir_built_hash(fn_id)?;
                cache.bodies_for(&built_hash).and_then(|bodies| {
                    bodies
                        .iter()
                        .map(|body| {
                            cache
                                .get_cache(&body.file_hash, &body.mir_hash)
                                .map(|analyzed| {
                                    MirAnalyzerInitResult::Cached(AnalyzeResult {
                                        file_path: body.file_path.clone(),
                                        file_hash: body.file_hash.clone(),
                                        mir_hash: body.mir_hash.clone(),
                                        analyzed,
                                    })
                                })
                        })
                        .collect::<Option<Vec<_>>>()
                })
            });
            if let Some(hits) = resolved {
                log::debug!("all bodies of {fn_id:?} served from the mir_built index");
                result.extend(hits);
                return result;
            }
        }

        let built_hash = tcx.mir_built_hash(fn_id);

        let facts = tcx.get_borrowck_facts(fn_id);
        for (fn_id, mut facts) in facts {
            let source_info = if let Some(v) = source_info_from_span(tcx, facts.body().span()) {
                v
            } else {
                continue;
            };
            let name = tcx.def_name(fn_id);
            log::debug!("facts of {fn_id:?} ({name}) prepared; start analyze...");

            let body = facts.body();

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
                && let Some(analyzed) = cache.take_cache(&file_hash, &mir_hash)
            {
                log::debug!("MIR cache hit: {fn_id:?}");
                result.push(MirAnalyzerInitResult::Cached(AnalyzeResult {
                    file_path: source_info.path().to_path_buf(),
                    file_hash: file_hash.clone(),
                    mir_hash: mir_hash.clone(),
                    analyzed,
                }));
                // record that this body is still cached, so a later compile that
                // rebuilds MIR can tell which roots were reused
                if let Some(built_hash) = built_hash.as_deref().map(str::to_owned) {
                    cache.index_built(
                        built_hash,
                        cache::CachedBody {
                            def_id: fn_id.as_u32(),
                            file_hash,
                            mir_hash,
                            file_path,
                        },
                    );
                }
                continue;
            }
            drop(cache);

            // these are only needed to build the analyzer, so they stay behind the
            // cache check: get_local_decls pretty-prints a type per local
            let local_decls = body.get_local_decls();
            // collect `RegionVid` for references' lifetime analysis
            let region_vids = body.get_local_region_vids();

            // collect user defined vars
            // this must be done in local thread
            let user_vars = body.collect_user_variables(&source_info);

            // build a Location -> source range map directly from the MIR body.
            let location_ranges = body.get_location_ranges(&source_info);

            // build basic blocks map
            // this must be done in local thread
            let basic_blocks =
                tcx.collect_basic_blocks(fn_id, &body, &source_info, &location_ranges);

            // compute storage ranges based on StorageLive/StorageDead
            // this must be done in local thread as body cannot be sent across threads
            let storage_range = body.compute_storage_ranges(&source_info);

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

                let accurate_live = polonius_analyzer::get_accurate_live(
                    &output,
                    &location_table,
                    &location_ranges,
                );

                let value_ends = dataflow_analyzer::collect_value_ends(&basic_blocks);
                let deficit = polonius_analyzer::get_deficit(
                    &input,
                    &output,
                    &location_table,
                    &borrow_data,
                    &value_ends,
                    &location_ranges,
                );

                let (shared_live, mutable_live) = polonius_analyzer::get_borrow_live(
                    &output,
                    &location_table,
                    &borrow_data,
                    &location_ranges,
                );

                let drop_range =
                    polonius_analyzer::drop_range(&output, &location_table, &location_ranges);

                let reference_local_live = polonius_analyzer::reference_local_live_range(
                    &output,
                    region_vids.into_iter(),
                    &location_table,
                    &location_ranges,
                );

                // CFG based liveness analysis
                log::debug!("start CFG based liveness check");
                let cfg_analysis_output = dataflow_analyzer::walk_cfg(&basic_blocks);
                log::debug!("CFG based liveness check finished");
                let mut definitely_live_range =
                    dataflow_analyzer::get_definitely_lives(&cfg_analysis_output, &location_ranges);
                let mut maybe_init_range = dataflow_analyzer::get_maybe_initialized(
                    &cfg_analysis_output,
                    &location_ranges,
                );

                // overwrite live ranges by reference_local_live if the local is
                // reference (lifetime of reference is differ from variable's lifetime)
                let mut reference_local_live = reference_local_live;
                for (local, ranges) in &mut maybe_init_range {
                    if let Some(ref_ranges) = reference_local_live.shift_remove(local) {
                        *ranges = ref_ranges.clone();
                        if let Some(ranges) = definitely_live_range.get_mut(local) {
                            *ranges = ref_ranges;
                        }
                    }
                }

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
                    deficit,
                    shared_live,
                    mutable_live,
                    drop_range,
                    storage_range,
                    definitely_live_range,
                    maybe_init_range,
                }
            });
            result.push(MirAnalyzerInitResult::Analyzer(analyzer));
        }
        result
    }

    /// collect declared variables in MIR body
    /// final step of analysis
    fn collect_decls(&mut self) -> Vec<MirDecl> {
        // taken out of self so each local's ranges are moved, not cloned
        let mut deficit_at = std::mem::take(&mut self.deficit);
        let mut lives = std::mem::take(&mut self.accurate_live);
        let mut shared_live = std::mem::take(&mut self.shared_live);
        let mut mutable_live = std::mem::take(&mut self.mutable_live);
        let mut drop_range = std::mem::take(&mut self.drop_range);
        let mut storage_range = std::mem::take(&mut self.storage_range);
        let mut definitely_live_range = std::mem::take(&mut self.definitely_live_range);
        let mut maybe_init_range = std::mem::take(&mut self.maybe_init_range);
        let mut user_vars = std::mem::take(&mut self.user_vars);
        let fn_id = self.fn_id;
        std::mem::take(&mut self.local_decls)
            .into_iter()
            .map(|(local, ty)| {
                let deficit_at = deficit_at.remove(&local).unwrap_or_default();
                let lives = lives.remove(&local).unwrap_or_default();
                let shared_borrow = shared_live.remove(&local).unwrap_or_default();
                let mutable_borrow = mutable_live.remove(&local).unwrap_or_default();
                let drop = self.is_drop(local);
                let drop_range = drop_range.remove(&local).unwrap_or_default();
                let storage_range = storage_range.remove(&local).unwrap_or_default();
                let fn_local = FnLocal::new(local.as_u32(), fn_id.as_u32());

                // liveness range based on CFG analysis
                let definitely_live_at = definitely_live_range.remove(&local).unwrap_or_default();
                let maybe_init_at = maybe_init_range.remove(&local).unwrap_or_default();

                if let Some((span, name)) = user_vars.shift_remove(&local) {
                    MirDecl::User {
                        local: fn_local,
                        name,
                        span,
                        ty,
                        lives,
                        shared_borrow,
                        mutable_borrow,
                        deficit_at,
                        drop,
                        drop_range,
                        storage_range,
                        definitely_live_at,
                        maybe_init_at,
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
                        deficit_at,
                        storage_range,
                        definitely_live_at,
                        maybe_init_at,
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
    pub fn analyze(mut self) -> AnalyzeResult {
        let decls = self.collect_decls();
        let basic_blocks: Vec<MirBasicBlock> = self.basic_blocks.into_values().collect();

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

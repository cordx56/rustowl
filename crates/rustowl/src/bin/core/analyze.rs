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
use std::sync::Arc;

pub type MirAnalyzeFuture = Pin<Box<dyn Future<Output = MirAnalyzer> + Send + Sync>>;

#[derive(Clone, Debug)]
pub struct AnalyzeResult {
    pub file_path: PathBuf,
    pub key: cache::key::FunctionKey,
    /// Where this body started when it was analysed.
    pub base: Loc,
    /// A body served from cache is already stored under its key and stays valid:
    /// `base` is read at use time, so it never needs rewriting.
    pub from_cache: bool,
    /// Shared with the cache: an analysed body is stored and emitted from one
    /// allocation instead of deep-cloned on the way out.
    pub analyzed: archery::SharedPointer<Function, archery::ArcK>,
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
    key: cache::key::FunctionKey,
    base: Loc,
    accurate_live: HashMap<LocalId, Vec<Range>>,
    deficit: HashMap<LocalId, Vec<Range>>,
    shared_live: HashMap<LocalId, Vec<Range>>,
    mutable_live: HashMap<LocalId, Vec<Range>>,
    drop_range: HashMap<LocalId, Vec<Range>>,
    storage_range: HashMap<LocalId, Vec<Range>>,
    definitely_live_range: HashMap<LocalId, Arc<[Range]>>,
    maybe_init_range: HashMap<LocalId, Arc<[Range]>>,
}

/// Content hash of source text.
///
/// `tcx.get_hash` cannot be used: rustc's stable hashing interns string symbols,
/// so it returns the same value for different text.
fn text_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Cheap: the source index is memoised and this needs no borrow check, so it is
/// available on the cache-hit path too.
fn body_anchor(source_info: &SourceInfo, span: Span) -> Loc {
    let offset = source_info.offset();
    source_info
        .index()
        .loc_at(span.lo().saturating_sub(offset) as usize)
}

impl MirAnalyzer {
    /// initialize analyzer
    pub fn init(tcx: TyCtxt<'_>, fn_id: DefId) -> Vec<MirAnalyzerInitResult> {
        let mut result = Vec::new();

        let facts = tcx.get_borrowck_facts(fn_id);
        for (fn_id, mut facts) in facts {
            let source_info = if let Some(v) = source_info_from_span(tcx, facts.span()) {
                v
            } else {
                continue;
            };
            let name = tcx.def_name(fn_id);
            tracing::debug!("facts of {fn_id:?} ({name}) prepared; start analyze...");

            // The body is copied out of the facts: the hash consumes it
            // (rustc's erase_region_variables takes it), and the analysis
            // needs it afterwards.
            let body = facts.body();

            let file_path = source_info.path().to_path_buf();

            // region variables should not be hashed (it results an error)
            // so we erase region variables and set 'static as new region
            let mir_hash = {
                use std::hash::{Hash, Hasher};
                let hash = tcx.get_hash(body.clone().erase_region_variables(tcx).as_rustc());
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                hash.hash(&mut hasher);
                hasher.finish()
            };

            let base = body_anchor(&source_info, body.span());
            // The function's own text: a comment inside the body leaves MIR unchanged,
            // so mir_hash alone would serve stale spans.
            let span = body.span();
            let offset = source_info.offset();
            let own_text = source_info
                .source()
                .get(
                    span.lo().saturating_sub(offset) as usize
                        ..span.hi().saturating_sub(offset) as usize,
                )
                .unwrap_or_default();
            let fn_text_hash = text_hash(own_text);
            let key = cache::key::FunctionKey {
                def_path: tcx.def_path_hash(fn_id),
                fn_text_hash,
                mir_hash,
                analysis_version: cache::ANALYSIS_VERSION,
                toolchain_hash: cache::toolchain_hash(),
            };

            if let Some(entry) = cache::get_entry(&key) {
                tracing::debug!("MIR cache hit: {fn_id:?}");
                let analyzed = cache::key::resolve(&entry, base);
                result.push(MirAnalyzerInitResult::Cached(AnalyzeResult {
                    file_path: source_info.path().to_path_buf(),
                    key,
                    base,
                    from_cache: true,
                    analyzed,
                }));
                continue;
            }

            // collect borrow data
            // this must be done in local thread
            let borrow_data = facts.borrow_map();

            let input = facts.polonius_input();
            let location_table = facts.location_table();

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

            let analyzer = Box::pin(async move {
                tracing::debug!("start re-computing borrow check with dump: true");
                // compute accurate region, which may eliminate invalid region
                let output = input.compute();
                tracing::debug!("second borrow check finished");

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
                tracing::debug!("start CFG based liveness check");
                let cfg_analysis_output = dataflow_analyzer::walk_cfg(&basic_blocks);
                tracing::debug!("CFG based liveness check finished");
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
                        *ranges = Arc::clone(&ref_ranges);
                        if let Some(ranges) = definitely_live_range.get_mut(local) {
                            *ranges = Arc::clone(&ref_ranges);
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
                    key,
                    base,
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
                        name: Arc::from(name),
                        span,
                        ty,
                        lives: Arc::from(lives),
                        shared_borrow: Arc::from(shared_borrow),
                        mutable_borrow: Arc::from(mutable_borrow),
                        deficit_at: Arc::from(deficit_at),
                        drop,
                        drop_range: Arc::from(drop_range),
                        storage_range: Arc::from(storage_range),
                        definitely_live_at: Arc::clone(&definitely_live_at),
                        maybe_init_at: Arc::clone(&maybe_init_at),
                    }
                } else {
                    MirDecl::Other {
                        local: fn_local,
                        ty,
                        lives: Arc::from(lives),
                        shared_borrow: Arc::from(shared_borrow),
                        mutable_borrow: Arc::from(mutable_borrow),
                        drop,
                        drop_range: Arc::from(drop_range),
                        deficit_at: Arc::from(deficit_at),
                        storage_range: Arc::from(storage_range),
                        definitely_live_at: Arc::clone(&definitely_live_at),
                        maybe_init_at: Arc::clone(&maybe_init_at),
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
            key: self.key,
            base: self.base,
            from_cache: false,
            analyzed: archery::SharedPointer::new(Function {
                fn_id: self.fn_id.as_u32(),
                name: Arc::from(self.name),
                basic_blocks: Arc::from(basic_blocks),
                decls,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::text_hash;

    #[test]
    fn fn_text_hash_reacts_to_the_functions_own_text() {
        let before = text_hash("fn f() {\n    let a = 1;\n}\n");
        let comment = text_hash("fn f() {\n    // a comment\n    let a = 1;\n}\n");
        let changed = text_hash("fn f() {\n    let a = 2;\n}\n");
        let other = text_hash("fn g() {\n    let a = 1;\n}\n");

        assert_ne!(before, comment, "a comment must invalidate the key");
        assert_ne!(before, changed);
        assert_ne!(before, other);
        assert_eq!(before, text_hash("fn f() {\n    let a = 1;\n}\n"));
    }
}

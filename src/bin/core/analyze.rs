mod polonius_analyzer;
mod transform;

use super::cache;
use polonius_engine::FactTypes;
use rustc_borrowck::consumers::{
    BorrowSet, ConsumerOptions, PoloniusInput, PoloniusLocationTable, PoloniusOutput, RustcFacts,
    get_body_with_borrowck_facts,
};
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_middle::{
    mir::{BasicBlock, Body, Local},
    ty::TyCtxt,
};
use rustc_span::Span;
use rustowl::{models::*, utils};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

pub type MirAnalyzeFuture<'tcx> =
    Pin<Box<dyn Future<Output = MirAnalyzer<'tcx>> + Send + Sync + 'tcx>>;

#[derive(Clone, Debug)]
pub struct AnalyzeResult {
    pub file_name: String,
    pub file_hash: String,
    pub mir_hash: String,
    pub analyzed: Function,
}

pub enum MirAnalyzerInitResult<'tcx> {
    Cached(AnalyzeResult),
    Analyzer(MirAnalyzeFuture<'tcx>),
}

type Region = <RustcFacts as FactTypes>::Origin;

fn range_from_span(source: &str, span: Span, offset: u32) -> Option<Range> {
    let from = Loc::new(source, span.lo().0, offset);
    let until = Loc::new(source, span.hi().0, offset);
    Range::new(from, until)
}
fn sort_locs(v: &mut [(BasicBlock, usize)]) {
    v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
}
fn erase_superset(mut ranges: Vec<Range>, erase_subset: bool) -> Vec<Range> {
    let mut len = ranges.len();
    let mut i = 0;
    while i < len {
        let mut j = i + 1;
        while j < len {
            let cond_j_i = !erase_subset
                && ((ranges[j].from() <= ranges[i].from()
                    && ranges[i].until() < ranges[j].until())
                    || (ranges[j].from() < ranges[i].from()
                        && ranges[i].until() <= ranges[j].until()));
            let cond_i_j = erase_subset
                && ((ranges[i].from() <= ranges[j].from()
                    && ranges[j].until() < ranges[i].until())
                    || (ranges[i].from() < ranges[j].from()
                        && ranges[j].until() <= ranges[i].until()));
            if cond_j_i || cond_i_j {
                ranges.remove(j);
            } else {
                j += 1;
            }
            len = ranges.len();
        }
        i += 1;
    }
    ranges
}

pub struct MirAnalyzer<'tcx> {
    file_name: String,
    location_table: PoloniusLocationTable,
    borrow_set: BorrowSet<'tcx>,
    body: Body<'tcx>,
    user_vars: HashMap<Local, (Range, String)>,
    input: PoloniusInput,
    output_insensitive: PoloniusOutput,
    output_datafrog: PoloniusOutput,
    basic_blocks: Vec<MirBasicBlock>,
    fn_id: LocalDefId,
    file_hash: String,
    mir_hash: String,
    accurate_live: HashMap<Local, Vec<Range>>,
    drop_range: HashMap<Local, Vec<Range>>,
}
impl MirAnalyzer<'_> {
    /// initialize analyzer
    pub fn init(tcx: TyCtxt<'_>, fn_id: LocalDefId) -> MirAnalyzerInitResult<'_> {
        let mut facts =
            get_body_with_borrowck_facts(tcx, fn_id, ConsumerOptions::PoloniusOutputFacts);
        let input = *facts.input_facts.take().unwrap();
        let body = facts.body.clone();
        let location_table = facts.location_table.take().unwrap();
        let borrow_set = facts.borrow_set;

        let source_map = tcx.sess.source_map();

        let file_name = source_map.span_to_filename(facts.body.span);
        let source_file = source_map.get_source_file(&file_name).unwrap();
        let offset = source_file.start_pos.0;
        let file_name = source_map.path_mapping().to_embeddable_absolute_path(
            rustc_span::RealFileName::LocalPath(file_name.into_local_path().unwrap()),
            &rustc_span::RealFileName::LocalPath(std::env::current_dir().unwrap()),
        );
        let path = file_name.to_path(rustc_span::FileNameDisplayPreference::Local);
        let source = std::fs::read_to_string(path).unwrap();
        let file_name = path.to_string_lossy().to_string();
        log::info!("facts of {fn_id:?} prepared; start analyze of {fn_id:?}");

        // region variables should not be hashed (it results an error)
        // so we erase region variables and set 'static as new region
        let mir_hash =
            cache::Hasher::get_hash(tcx, transform::erase_region_variables(tcx, body.clone()));
        let file_hash = cache::Hasher::get_hash(tcx, &source);
        let mut cache = cache::CACHE.lock().unwrap();

        // setup cache
        if cache.is_none() {
            *cache = cache::get_cache(&tcx.crate_name(LOCAL_CRATE).to_string());
        }
        if let Some(cache) = cache.as_mut()
            && let Some(analyzed) = cache.get_cache(&file_hash, &mir_hash)
        {
            log::info!("MIR cache hit: {fn_id:?}");
            return MirAnalyzerInitResult::Cached(AnalyzeResult {
                file_name,
                file_hash,
                mir_hash,
                analyzed: analyzed.clone(),
            });
        }
        drop(cache);

        // collect user defined vars
        // this should be done in local thread
        let user_vars = transform::collect_user_vars(&source, offset, &body);

        // build basic blocks map
        // this should be done in local thread
        let basic_blocks = transform::collect_basic_blocks(
            fn_id,
            &source,
            offset,
            &facts.body.basic_blocks,
            tcx.sess.source_map(),
        );

        let analyzer = Box::pin(async move {
            log::info!("start re-computing borrow check with dump: true");
            // compute insensitive
            // it may include invalid region, which can be used at showing wrong region
            let output_insensitive = PoloniusOutput::compute(
                &input,
                polonius_engine::Algorithm::LocationInsensitive,
                true,
            );
            // compute accurate region, which may eliminate invalid region
            let output_datafrog =
                PoloniusOutput::compute(&input, polonius_engine::Algorithm::DatafrogOpt, true);
            log::info!("borrow check finished");

            let accurate_live = polonius_analyzer::get_accurate_live(
                &output_datafrog,
                &location_table,
                &basic_blocks,
            );

            let drop_range =
                polonius_analyzer::drop_range(&output_datafrog, &location_table, &basic_blocks);

            MirAnalyzer {
                file_name,
                location_table,
                borrow_set,
                body,
                input,
                output_insensitive,
                output_datafrog,
                user_vars,
                basic_blocks,
                fn_id,
                file_hash,
                mir_hash,
                accurate_live,
                drop_range,
            }
        });
        MirAnalyzerInitResult::Analyzer(analyzer)
    }

    /// collect declared variables in MIR body
    /// final step of analysis
    fn collect_decls(&self) -> Vec<MirDecl> {
        let user_vars = &self.user_vars;
        let lives = &self.accurate_live;

        let (shared, mutable) = polonius_analyzer::get_borrow_live(
            &self.output_datafrog,
            &self.location_table,
            &self.borrow_set,
            &self.basic_blocks,
        );
        let must_live_at = polonius_analyzer::get_must_live(
            &self.output_insensitive,
            &self.location_table,
            &self.borrow_set,
            &self.basic_blocks,
        );
        let drop_range = &self.drop_range;
        self.body
            .local_decls
            .iter_enumerated()
            .map(|(local, decl)| {
                let ty = decl.ty.to_string();
                let must_live_at = utils::eliminated_ranges(
                    must_live_at.get(&local).cloned().unwrap_or(Vec::new()),
                );
                let lives = lives.get(&local).cloned().unwrap_or(Vec::new());
                let shared_borrow = shared.get(&local).cloned().unwrap_or(Vec::new());
                let mutable_borrow = mutable.get(&local).cloned().unwrap_or(Vec::new());
                let drop = self.is_drop(local);
                let drop_range = drop_range.get(&local).cloned().unwrap_or(Vec::new());
                let fn_local = FnLocal::new(local.as_u32(), self.fn_id.local_def_index.as_u32());
                if let Some((span, name)) = user_vars.get(&local).cloned() {
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

    fn is_drop(&self, local: Local) -> bool {
        for (drop_local, _) in self.input.var_dropped_at.iter() {
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
            file_name: self.file_name,
            file_hash: self.file_hash,
            mir_hash: self.mir_hash,
            analyzed: Function {
                fn_id: self.fn_id.local_def_index.as_u32(),
                basic_blocks,
                decls,
            },
        }
    }
}

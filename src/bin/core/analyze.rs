use super::{cache, mir_transform};
use polonius_engine::FactTypes;
use rustc_borrowck::consumers::{
    BorrowSet, ConsumerOptions, PoloniusInput, PoloniusLocationTable, PoloniusOutput, RichLocation,
    RustcFacts, get_body_with_borrowck_facts,
};
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_middle::{
    mir::{
        BasicBlock, BasicBlockData, BasicBlocks, Body, BorrowKind, Local, Operand, Rvalue,
        StatementKind, TerminatorKind, VarDebugInfoContents,
    },
    ty::TyCtxt,
};
use rustc_span::{Span, source_map::SourceMap};
use rustowl::{models::*, utils};
use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::hash::Hash;
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

type Borrow = <RustcFacts as FactTypes>::Loan;
type Region = <RustcFacts as FactTypes>::Origin;

trait Append<K, V>
where
    K: Hash + Eq + Clone,
{
    fn append(&mut self, key: &K, value: V);
}
impl<K, V> Append<K, V> for HashMap<K, Vec<V>>
where
    K: Hash + Eq + Clone,
{
    fn append(&mut self, key: &K, value: V) {
        if let Some(v) = self.get_mut(key) {
            v.push(value);
        } else {
            self.insert(key.clone(), vec![value]);
        }
    }
}
impl<K, V> Append<K, V> for HashMap<K, BTreeSet<V>>
where
    K: Hash + Eq + Clone,
    V: Ord,
{
    fn append(&mut self, key: &K, value: V) {
        if let Some(v) = self.get_mut(key) {
            v.insert(value);
        } else {
            let mut set = BTreeSet::new();
            set.insert(value);
            self.insert(key.clone(), set);
        }
    }
}

fn range_from_span(source: &str, span: Span, offset: u32) -> Option<Range> {
    let from = Loc::new(source, span.lo().0, offset);
    let until = Loc::new(source, span.hi().0, offset);
    Range::new(from, until)
}

pub struct MirAnalyzer<'tcx> {
    file_name: String,
    source: String,
    offset: u32,
    location_table: PoloniusLocationTable,
    borrow_set: BorrowSet<'tcx>,
    body: Body<'tcx>,
    input: PoloniusInput,
    output_insensitive: PoloniusOutput,
    output_datafrog: PoloniusOutput,
    bb_map: HashMap<BasicBlock, BasicBlockData<'tcx>>,
    borrow_locals: HashMap<Borrow, Local>,
    basic_blocks: Vec<MirBasicBlock>,
    fn_id: LocalDefId,
    file_hash: String,
    mir_hash: String,
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
        let mir_hash = cache::Hasher::get_hash(
            tcx,
            mir_transform::erase_region_variables(tcx, body.clone()),
        );
        let file_hash = cache::Hasher::get_hash(tcx, &source);
        let mut cache = cache::CACHE.lock().unwrap();

        // setup cache
        if cache.is_none() {
            *cache = cache::get_cache(&tcx.crate_name(LOCAL_CRATE).to_string());
        }
        if let Some(cache) = cache.as_mut() {
            if let Some(analyzed) = cache.get_cache(&file_hash, &mir_hash) {
                log::info!("MIR cache hit: {fn_id:?}");
                return MirAnalyzerInitResult::Cached(AnalyzeResult {
                    file_name,
                    file_hash,
                    mir_hash,
                    analyzed: analyzed.clone(),
                });
            }
        }

        // local -> all borrows on that local
        let mut borrow_locals = HashMap::new();
        for (local, borrow_idc) in borrow_set.local_map().iter() {
            for borrow_idx in borrow_idc {
                borrow_locals.insert(*borrow_idx, *local);
            }
        }

        // build basic blocks map
        let bb_map = facts
            .body
            .basic_blocks
            .iter_enumerated()
            .map(|(b, d)| (b, d.clone()))
            .collect();
        let basic_blocks = Self::basic_blocks(
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

            MirAnalyzer {
                file_name,
                source,
                offset,
                location_table,
                borrow_set,
                body,
                input,
                output_insensitive,
                output_datafrog,
                bb_map,
                borrow_locals,
                basic_blocks,
                fn_id,
                file_hash,
                mir_hash,
            }
        });
        MirAnalyzerInitResult::Analyzer(analyzer)
    }

    fn sort_locs(v: &mut [(BasicBlock, usize)]) {
        v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    }
    fn stmt_location_to_range(&self, bb: BasicBlock, stmt_index: usize) -> Option<Range> {
        self.bb_map
            .get(&bb)
            .map(|bb| {
                if stmt_index < bb.statements.len() {
                    bb.statements.get(stmt_index).unwrap().source_info.span
                } else {
                    bb.terminator().source_info.span
                }
            })
            .and_then(|span| range_from_span(&self.source, span, self.offset))
    }
    fn rich_locations_to_ranges(&self, locations: &[RichLocation]) -> Vec<Range> {
        let mut starts = Vec::new();
        let mut mids = Vec::new();
        for rich in locations {
            match rich {
                RichLocation::Start(l) => {
                    starts.push((l.block, l.statement_index));
                }
                RichLocation::Mid(l) => {
                    mids.push((l.block, l.statement_index));
                }
            }
        }
        Self::sort_locs(&mut starts);
        Self::sort_locs(&mut mids);
        starts
            .iter()
            .zip(mids.iter())
            .filter_map(|(s, m)| {
                let sr = self.stmt_location_to_range(s.0, s.1);
                let mr = self.stmt_location_to_range(m.0, m.1);
                match (sr, mr) {
                    (Some(s), Some(m)) => Range::new(s.from(), m.until()),
                    _ => None,
                }
            })
            .collect()
    }

    /// obtain map from local id to living range
    fn drop_range(&self) -> HashMap<Local, Vec<Range>> {
        let mut local_live_locs = HashMap::new();
        for (loc_idx, locals) in self.output_datafrog.var_drop_live_on_entry.iter() {
            let location = self.location_table.to_rich_location(*loc_idx);
            for local in locals {
                local_live_locs
                    .entry(*local)
                    .or_insert_with(Vec::new)
                    .push(location);
            }
        }
        local_live_locs
            .into_iter()
            .map(|(local, locations)| {
                (
                    local,
                    utils::eliminated_ranges(self.rich_locations_to_ranges(&locations)),
                )
            })
            .collect()
    }

    /// collect user defined variables from debug info in MIR
    fn collect_user_vars(&self) -> HashMap<Local, (Range, String)> {
        self.body
            .var_debug_info
            .iter()
            .filter_map(|debug| match &debug.value {
                VarDebugInfoContents::Place(place) => {
                    range_from_span(&self.source, debug.source_info.span, self.offset)
                        .map(|range| (place.local, (range, debug.name.as_str().to_owned())))
                }
                _ => None,
            })
            .collect()
    }
    /// collect declared variables in MIR body
    fn collect_decls(&self) -> Vec<MirDecl> {
        let user_vars = self.collect_user_vars();
        let lives = self.get_accurate_live();
        let (shared, mutable) = self.get_borrow_live();
        let must_live_at = self.get_must_live();
        let drop_range = self.drop_range();
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

    /// collect and translate basic blocks
    fn basic_blocks(
        fn_id: LocalDefId,
        source: &str,
        offset: u32,
        basic_blocks: &BasicBlocks<'_>,
        source_map: &SourceMap,
    ) -> Vec<MirBasicBlock> {
        basic_blocks
            .iter_enumerated()
            .map(|(b, d)| (b, d.clone()))
            .map(|(_bb, bb_data)| {
                let statements = bb_data
                    .statements
                    .iter()
                    .filter_map(|statement| {
                        if !statement.source_info.span.is_visible(source_map) {
                            return None;
                        }
                        match &statement.kind {
                            StatementKind::Assign(v) => {
                                let (place, rval) = &**v;
                                let target_local_index = place.local.as_u32();
                                let rv = match rval {
                                    Rvalue::Use(Operand::Move(p)) => {
                                        let local = p.local;
                                        range_from_span(source, statement.source_info.span, offset)
                                            .map(|range| MirRval::Move {
                                                target_local: FnLocal::new(
                                                    local.as_u32(),
                                                    fn_id.local_def_index.as_u32(),
                                                ),
                                                range,
                                            })
                                    }
                                    Rvalue::Ref(_region, kind, place) => {
                                        let mutable = matches!(kind, BorrowKind::Mut { .. });
                                        let local = place.local;
                                        let outlive = None;
                                        range_from_span(source, statement.source_info.span, offset)
                                            .map(|range| MirRval::Borrow {
                                                target_local: FnLocal::new(
                                                    local.as_u32(),
                                                    fn_id.local_def_index.as_u32(),
                                                ),
                                                range,
                                                mutable,
                                                outlive,
                                            })
                                    }
                                    _ => None,
                                };
                                range_from_span(source, statement.source_info.span, offset).map(
                                    |range| MirStatement::Assign {
                                        target_local: FnLocal::new(
                                            target_local_index,
                                            fn_id.local_def_index.as_u32(),
                                        ),
                                        range,
                                        rval: rv,
                                    },
                                )
                            }
                            _ => None,
                        }
                    })
                    .collect();
                let terminator =
                    bb_data
                        .terminator
                        .as_ref()
                        .and_then(|terminator| match &terminator.kind {
                            TerminatorKind::Drop { place, .. } => {
                                range_from_span(source, terminator.source_info.span, offset).map(
                                    |range| MirTerminator::Drop {
                                        local: FnLocal::new(
                                            place.local.as_u32(),
                                            fn_id.local_def_index.as_u32(),
                                        ),
                                        range,
                                    },
                                )
                            }
                            TerminatorKind::Call {
                                destination,
                                fn_span,
                                ..
                            } => range_from_span(source, *fn_span, offset).map(|fn_span| {
                                MirTerminator::Call {
                                    destination_local: FnLocal::new(
                                        destination.local.as_u32(),
                                        fn_id.local_def_index.as_u32(),
                                    ),
                                    fn_span,
                                }
                            }),
                            _ => Some(MirTerminator::Other),
                        });
                MirBasicBlock {
                    statements,
                    terminator,
                }
            })
            .collect()
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

    fn get_accurate_live(&self) -> HashMap<Local, Vec<Range>> {
        let output = &self.output_datafrog;
        let mut lives = HashMap::new();
        for (location_idx, vars) in output.var_live_on_entry.iter() {
            let location = self.location_table.to_rich_location(*location_idx);
            for var in vars {
                lives.entry(*var).or_insert_with(Vec::new).push(location);
            }
        }
        lives
            .into_iter()
            .map(|(local, locations)| {
                (
                    local,
                    utils::eliminated_ranges(self.rich_locations_to_ranges(&locations)),
                )
            })
            .collect()
    }
    /// returns (shared, mutable)
    fn get_borrow_live(&self) -> (HashMap<Local, Vec<Range>>, HashMap<Local, Vec<Range>>) {
        let output = &self.output_datafrog;
        let mut shared_borrows = HashMap::new();
        let mut mutable_borrows = HashMap::new();
        for (location_idx, borrow_idc) in output.loan_live_at.iter() {
            let location = self.location_table.to_rich_location(*location_idx);
            for borrow_idx in borrow_idc {
                let borrow_data = &self.borrow_set[*borrow_idx];
                let local = borrow_data.borrowed_place().local;
                if borrow_data.kind().mutability().is_mut() {
                    mutable_borrows
                        .entry(local)
                        .or_insert_with(Vec::new)
                        .push(location);
                } else {
                    shared_borrows
                        .entry(local)
                        .or_insert_with(Vec::new)
                        .push(location);
                }
            }
        }
        (
            shared_borrows
                .into_iter()
                .map(|(local, locations)| {
                    (
                        local,
                        utils::eliminated_ranges(self.rich_locations_to_ranges(&locations)),
                    )
                })
                .collect(),
            mutable_borrows
                .into_iter()
                .map(|(local, locations)| {
                    (
                        local,
                        utils::eliminated_ranges(self.rich_locations_to_ranges(&locations)),
                    )
                })
                .collect(),
        )
    }

    fn get_must_live(&self) -> HashMap<Local, Vec<Range>> {
        self.live_range_from_region(&self.output_insensitive)
    }

    fn live_range_from_region(&self, output: &PoloniusOutput) -> HashMap<Local, Vec<Range>> {
        let mut region_locations = HashMap::new();
        for (location_idx, region_idc) in output.origin_live_on_entry.iter() {
            let location = self.location_table.to_rich_location(*location_idx);
            for region_idx in region_idc {
                region_locations
                    .entry(*region_idx)
                    .or_insert_with(Vec::new)
                    .push(location);
            }
        }

        // compute regions where the local must be live
        let mut local_must_regions: HashMap<Local, BTreeSet<Region>> = HashMap::new();
        for (region_idx, borrow_idc) in output.origin_contains_loan_anywhere.iter() {
            for borrow_idx in borrow_idc {
                if let Some(local) = self.borrow_locals.get(borrow_idx) {
                    local_must_regions.append(local, *region_idx);
                }
            }
        }

        HashMap::from_iter(local_must_regions.iter().map(|(local, regions)| {
            (
                *local,
                Self::erase_superset(
                    self.rich_locations_to_ranges(
                        &regions
                            .iter()
                            .filter_map(|v| region_locations.get(v).cloned())
                            .flatten()
                            .collect::<Vec<_>>(),
                    ),
                    false,
                ),
            )
        }))
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

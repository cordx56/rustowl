use ecow::EcoVec;
use rayon::prelude::*;
use rustc_borrowck::consumers::{BorrowIndex, BorrowSet, RichLocation};
use rustc_hir::def_id::LocalDefId;
use rustc_middle::{
    mir::{
        BasicBlock, BasicBlocks, Body, BorrowKind, Local, Location, Operand, Rvalue, StatementKind,
        TerminatorKind, VarDebugInfoContents,
    },
    ty::{TyCtxt, TypeFoldable, TypeFolder},
};
use rustc_span::source_map::SourceMap;
use rustowl::models::*;
use rustowl::models::{FoldIndexMap as HashMap, FoldIndexSet as HashSet};

/// RegionEraser to erase region variables from MIR body
/// This is required to hash MIR body
struct RegionEraser<'tcx> {
    tcx: TyCtxt<'tcx>,
}
impl<'tcx> TypeFolder<TyCtxt<'tcx>> for RegionEraser<'tcx> {
    fn cx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }
    fn fold_region(
        &mut self,
        _r: <TyCtxt<'tcx> as rustc_type_ir::Interner>::Region,
    ) -> <TyCtxt<'tcx> as rustc_type_ir::Interner>::Region {
        self.tcx.lifetimes.re_static
    }
}

/// Erase region variables in MIR body
/// Refer: [`RegionEraser`]
pub fn erase_region_variables<'tcx>(tcx: TyCtxt<'tcx>, body: Body<'tcx>) -> Body<'tcx> {
    let mut eraser = RegionEraser { tcx };

    body.fold_with(&mut eraser)
}

/// collect user defined variables from debug info in MIR
pub fn collect_user_vars(
    source: &str,
    offset: u32,
    body: &Body<'_>,
) -> HashMap<Local, (Range, String)> {
    let index = rustowl::utils::NormalizedByteCharIndex::new(source);

    let mut result = HashMap::with_capacity_and_hasher(
        body.var_debug_info.len(),
        foldhash::quality::RandomState::default(),
    );
    for debug in &body.var_debug_info {
        let VarDebugInfoContents::Place(place) = &debug.value else {
            continue;
        };

        let Some(range) =
            super::shared::range_from_span_indexed(&index, debug.source_info.span, offset)
        else {
            continue;
        };

        result.insert(place.local, (range, debug.name.as_str().to_owned()));
    }
    result
}

/// Collect and transform [`BasicBlocks`] into our data structure [`MirBasicBlock`]s.
pub fn collect_basic_blocks(
    fn_id: LocalDefId,
    source: &str,
    offset: u32,
    basic_blocks: &BasicBlocks<'_>,
    source_map: &SourceMap,
) -> EcoVec<MirBasicBlock> {
    // Building the byte→Loc index once per file removes the previous
    // `Loc::new` per-span scan hot spot.
    let index = rustowl::utils::NormalizedByteCharIndex::new(source);
    let fn_u32 = fn_id.local_def_index.as_u32();

    // A small threshold helps avoid rayon overhead on tiny blocks.
    const PAR_THRESHOLD: usize = 64;

    let mut result = EcoVec::with_capacity(basic_blocks.len());

    for (_bb, bb_data) in basic_blocks.iter_enumerated() {
        // `source_map` is not Send, so the visibility filter must run on the
        // current thread.
        let mut visible = Vec::with_capacity(bb_data.statements.len());
        for stmt in &bb_data.statements {
            if stmt.source_info.span.is_visible(source_map) {
                visible.push(stmt);
            }
        }

        let mut bb_statements = StatementVec::with_capacity(visible.len());
        if visible.len() >= PAR_THRESHOLD {
            let collected_statements: Vec<_> = visible
                .par_iter()
                .filter_map(|statement| statement_to_mir(&index, fn_u32, offset, statement))
                .collect();
            bb_statements.extend(collected_statements);
        } else {
            bb_statements.extend(
                visible
                    .iter()
                    .filter_map(|statement| statement_to_mir(&index, fn_u32, offset, statement)),
            );
        }

        let terminator =
            bb_data
                .terminator
                .as_ref()
                .and_then(|terminator| match &terminator.kind {
                    TerminatorKind::Drop { place, .. } => super::shared::range_from_span_indexed(
                        &index,
                        terminator.source_info.span,
                        offset,
                    )
                    .map(|range| MirTerminator::Drop {
                        local: FnLocal::new(place.local.as_u32(), fn_u32),
                        range,
                    }),
                    TerminatorKind::Call {
                        destination,
                        fn_span,
                        ..
                    } => super::shared::range_from_span_indexed(&index, *fn_span, offset).map(
                        |fn_span| MirTerminator::Call {
                            destination_local: FnLocal::new(destination.local.as_u32(), fn_u32),
                            fn_span,
                        },
                    ),
                    _ => super::shared::range_from_span_indexed(
                        &index,
                        terminator.source_info.span,
                        offset,
                    )
                    .map(|range| MirTerminator::Other { range }),
                });

        result.push(MirBasicBlock {
            statements: bb_statements,
            terminator,
        });
    }

    result
}

fn statement_to_mir(
    index: &rustowl::utils::NormalizedByteCharIndex,
    fn_u32: u32,
    offset: u32,
    statement: &rustc_middle::mir::Statement<'_>,
) -> Option<MirStatement> {
    match &statement.kind {
        StatementKind::Assign(v) => {
            let (place, rval) = &**v;
            let target_local_index = place.local.as_u32();
            let range_opt =
                super::shared::range_from_span_indexed(index, statement.source_info.span, offset);

            let rv = match rval {
                Rvalue::Use(Operand::Move(p)) => {
                    let local = p.local;
                    range_opt.map(|range| MirRval::Move {
                        target_local: FnLocal::new(local.as_u32(), fn_u32),
                        range,
                    })
                }
                Rvalue::Ref(_region, kind, place) => {
                    let mutable = matches!(kind, BorrowKind::Mut { .. });
                    let local = place.local;
                    let outlive = None;
                    range_opt.map(|range| MirRval::Borrow {
                        target_local: FnLocal::new(local.as_u32(), fn_u32),
                        range,
                        mutable,
                        outlive,
                    })
                }
                _ => None,
            };

            range_opt.map(|range| MirStatement::Assign {
                target_local: FnLocal::new(target_local_index, fn_u32),
                range,
                rval: rv,
            })
        }
        _ => super::shared::range_from_span_indexed(index, statement.source_info.span, offset)
            .map(|range| MirStatement::Other { range }),
    }
}

fn statement_location_to_range(
    basic_blocks: &[MirBasicBlock],
    basic_block: usize,
    statement: usize,
) -> Option<Range> {
    basic_blocks.get(basic_block).and_then(|bb| {
        if statement < bb.statements.len() {
            bb.statements.get(statement).map(|v| v.range())
        } else {
            bb.terminator.as_ref().map(|v| v.range())
        }
    })
}

pub fn rich_locations_to_ranges(
    basic_blocks: &[MirBasicBlock],
    locations: &[RichLocation],
) -> Vec<Range> {
    let mut starts: Vec<(BasicBlock, usize)> = Vec::new();
    let mut mids: Vec<(BasicBlock, usize)> = Vec::new();

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

    super::shared::sort_locs(&mut starts);
    super::shared::sort_locs(&mut mids);

    let n = starts.len().min(mids.len());
    if n != starts.len() || n != mids.len() {
        tracing::debug!(
            "rich_locations_to_ranges: starts({}) != mids({}); truncating to {}",
            starts.len(),
            mids.len(),
            n
        );
    }
    starts[..n]
        .par_iter()
        .zip(mids[..n].par_iter())
        .filter_map(|(s, m)| {
            let sr = statement_location_to_range(basic_blocks, s.0.index(), s.1);
            let mr = statement_location_to_range(basic_blocks, m.0.index(), m.1);
            match (sr, mr) {
                (Some(s), Some(m)) => Range::new(s.from(), m.until()),
                _ => None,
            }
        })
        .collect()
}

/// Our representation of [`rustc_borrowck::consumers::BorrowData`]
pub enum BorrowData {
    Shared {
        borrowed: Local,
        #[allow(dead_code)]
        assigned: Local,
    },
    Mutable {
        borrowed: Local,
        #[allow(dead_code)]
        assigned: Local,
    },
}

/// A map type from [`BorrowIndex`] to [`BorrowData`]
pub struct BorrowMap {
    location_map: Vec<(Location, BorrowData)>,
    local_map: HashMap<Local, HashSet<BorrowIndex>>,
}
impl BorrowMap {
    /// Get [`BorrowMap`] from [`BorrowSet`]
    pub fn new(borrow_set: &BorrowSet<'_>) -> Self {
        let mut location_map = Vec::new();
        // BorrowIndex corresponds to Location index
        for (location, data) in borrow_set.location_map().iter() {
            let data = if data.kind().mutability().is_mut() {
                BorrowData::Mutable {
                    borrowed: data.borrowed_place().local,
                    assigned: data.assigned_place().local,
                }
            } else {
                BorrowData::Shared {
                    borrowed: data.borrowed_place().local,
                    assigned: data.assigned_place().local,
                }
            };
            location_map.push((*location, data));
        }
        let local_map = borrow_set
            .local_map()
            .iter()
            .map(|(local, borrows)| (*local, borrows.iter().copied().collect()))
            .collect();
        Self {
            location_map,
            local_map,
        }
    }
    pub fn get_from_borrow_index(&self, borrow: BorrowIndex) -> Option<&(Location, BorrowData)> {
        self.location_map.get(borrow.index())
    }
    pub fn local_map(&self) -> &HashMap<Local, HashSet<BorrowIndex>> {
        &self.local_map
    }
}

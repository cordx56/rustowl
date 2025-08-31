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
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

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
    let mut result = HashMap::with_capacity(body.var_debug_info.len());
    for debug in &body.var_debug_info {
        if let VarDebugInfoContents::Place(place) = &debug.value
            && let Some(range) = super::range_from_span(source, debug.source_info.span, offset)
        {
            result.insert(place.local, (range, debug.name.as_str().to_owned()));
        }
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
) -> SmallVec<[MirBasicBlock; 8]> {
    let mut result = SmallVec::with_capacity(basic_blocks.len());

    for (_bb, bb_data) in basic_blocks.iter_enumerated() {
        let statements: Vec<_> = bb_data
            .statements
            .iter()
            // `source_map` is not Send
            .filter(|stmt| stmt.source_info.span.is_visible(source_map))
            .collect();

        let mut bb_statements = StatementVec::with_capacity(statements.len());
        let collected_statements: Vec<_> = statements
            .par_iter()
            .filter_map(|statement| match &statement.kind {
                StatementKind::Assign(v) => {
                    let (place, rval) = &**v;
                    let target_local_index = place.local.as_u32();
                    let rv =
                        match rval {
                            Rvalue::Use(Operand::Move(p)) => {
                                let local = p.local;
                                super::range_from_span(source, statement.source_info.span, offset)
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
                                super::range_from_span(source, statement.source_info.span, offset)
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
                    super::range_from_span(source, statement.source_info.span, offset).map(
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
                _ => super::range_from_span(source, statement.source_info.span, offset)
                    .map(|range| MirStatement::Other { range }),
            })
            .collect();
        bb_statements.extend(collected_statements);

        let terminator =
            bb_data
                .terminator
                .as_ref()
                .and_then(|terminator| match &terminator.kind {
                    TerminatorKind::Drop { place, .. } => super::range_from_span(
                        source,
                        terminator.source_info.span,
                        offset,
                    )
                    .map(|range| MirTerminator::Drop {
                        local: FnLocal::new(place.local.as_u32(), fn_id.local_def_index.as_u32()),
                        range,
                    }),
                    TerminatorKind::Call {
                        destination,
                        fn_span,
                        ..
                    } => super::range_from_span(source, *fn_span, offset).map(|fn_span| {
                        MirTerminator::Call {
                            destination_local: FnLocal::new(
                                destination.local.as_u32(),
                                fn_id.local_def_index.as_u32(),
                            ),
                            fn_span,
                        }
                    }),
                    _ => super::range_from_span(source, terminator.source_info.span, offset)
                        .map(|range| MirTerminator::Other { range }),
                });

        result.push(MirBasicBlock {
            statements: bb_statements,
            terminator,
        });
    }

    result
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
    let mut starts = SmallVec::<[(BasicBlock, usize); 16]>::new();
    let mut mids = SmallVec::<[(BasicBlock, usize); 16]>::new();

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

    super::sort_locs(&mut starts);
    super::sort_locs(&mut mids);

    starts
        .par_iter()
        .zip(mids.par_iter())
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

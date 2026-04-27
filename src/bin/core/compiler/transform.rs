use super::*;
use rustowl::utils;

use rustc_data_structures::indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// RegionEraser to erase region variables from MIR body
/// This is required to hash MIR body
pub struct RegionEraser<'tcx> {
    tcx: TyCtxt<'tcx>,
}
impl<'tcx> rustc_middle::ty::TypeFolder<rustc_middle::ty::TyCtxt<'tcx>> for RegionEraser<'tcx> {
    fn cx(&self) -> rustc_middle::ty::TyCtxt<'tcx> {
        *self.tcx.as_rustc()
    }
    fn fold_region(
        &mut self,
        _r: <rustc_middle::ty::TyCtxt<'tcx> as rustc_type_ir::Interner>::Region,
    ) -> <rustc_middle::ty::TyCtxt<'tcx> as rustc_type_ir::Interner>::Region {
        self.cx().lifetimes.re_static
    }
}

impl<'tcx> Body<'tcx> {
    /// Erase region variables in MIR body
    /// Refer: [`RegionEraser`]
    pub fn erase_region_variables(self, tcx: TyCtxt<'tcx>) -> Body<'tcx> {
        let mut eraser = RegionEraser { tcx };
        use rustc_middle::ty::TypeFoldable;
        AsRustc::from_rustc(self.into_rustc().fold_with(&mut eraser))
    }
}

impl<'tcx> TyCtxt<'tcx> {
    /// Collect and transform [`rustc_middle::mir::BasicBlocks`] into our data structure [`MirBasicBlock`]s.
    pub fn collect_basic_blocks(
        &self,
        fn_id: DefId,
        body: &Body<'tcx>,
        source_info: &SourceInfo,
        location_ranges: &LocationRanges,
    ) -> IndexMap<BasicBlockId, MirBasicBlock> {
        use rustc_middle::mir::*;

        body.as_rustc()
            .basic_blocks
            .iter_enumerated()
            .map(|(block, bb_data)| {
                let statements: Vec<_> = bb_data.statements.iter().collect();
                let statements = statements
                    .iter()
                    .enumerate()
                    .map(|(statement_index, statement)| {
                        let location = Location {
                            block,
                            statement_index,
                        };
                        let range = location_ranges
                            .get(&AsRustc::from_rustc(location))
                            .map(|v| *v);
                        match &statement.kind {
                            StatementKind::StorageLive(local) => MirStatement::StorageLive {
                                target_local: FnLocal::new(local.as_u32(), fn_id.as_u32()),
                                range,
                            },
                            StatementKind::StorageDead(local) => MirStatement::StorageDead {
                                target_local: FnLocal::new(local.as_u32(), fn_id.as_u32()),
                                range,
                            },
                            StatementKind::Assign(v) => {
                                let (place, rval) = &**v;
                                let target_local_index = place.local.as_u32();
                                let rv = match rval {
                                    Rvalue::Use(Operand::Move(p)) => {
                                        let local = p.local;
                                        range.map(|range| MirRval::Move {
                                            target_local: FnLocal::new(
                                                local.as_u32(),
                                                fn_id.as_u32(),
                                            ),
                                            range,
                                        })
                                    }
                                    Rvalue::Ref(_region, kind, place) => {
                                        let mutable = matches!(kind, BorrowKind::Mut { .. });
                                        let local = place.local;
                                        let outlive = None;
                                        range.map(|range| MirRval::Borrow {
                                            target_local: FnLocal::new(
                                                local.as_u32(),
                                                fn_id.as_u32(),
                                            ),
                                            range,
                                            mutable,
                                            outlive,
                                        })
                                    }
                                    _ => None,
                                };
                                MirStatement::Assign {
                                    target_local: FnLocal::new(target_local_index, fn_id.as_u32()),
                                    range,
                                    rval: rv,
                                }
                            }
                            _ => MirStatement::Other { range },
                        }
                    })
                    .collect();
                let terminator = bb_data.terminator.as_ref().map(|terminator| {
                    let location = Location {
                        block,
                        statement_index: bb_data.statements.len(),
                    };
                    let range = location_ranges
                        .get(&AsRustc::from_rustc(location))
                        .map(|v| *v);
                    let successors = terminator
                        .successors()
                        .map(|v| BasicBlockId(v.as_usize()))
                        .collect();
                    match &terminator.kind {
                        TerminatorKind::Drop { place, .. } => MirTerminator::Drop {
                            local: FnLocal::new(place.local.as_u32(), fn_id.as_u32()),
                            range,
                            successors,
                        },
                        TerminatorKind::Call {
                            destination,
                            fn_span,
                            ..
                        } => {
                            let fn_span = range_from_span(
                                source_info.source(),
                                AsRustc::from_rustc(*fn_span),
                                source_info.offset,
                            );
                            MirTerminator::Call {
                                destination_local: FnLocal::new(
                                    destination.local.as_u32(),
                                    fn_id.as_u32(),
                                ),
                                fn_span,
                                successors,
                            }
                        }
                        _ => MirTerminator::Other { range, successors },
                    }
                });
                (
                    BasicBlockId(block.as_usize()),
                    MirBasicBlock {
                        statements,
                        terminator,
                    },
                )
            })
            .collect()
    }
}

/// Our representation of `rustc_borrowck::consumers::BorrowData`
#[derive(Clone, Debug)]
pub enum BorrowData {
    Shared {
        borrowed: LocalId,
        assigned: LocalId,
    },
    Mutable {
        borrowed: LocalId,
        assigned: LocalId,
    },
}

impl_as_rustc!(
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    Location,
    rustc_middle::mir::Location,
);
impl Location {
    pub fn block(&self) -> u32 {
        self.as_rustc().block.as_u32()
    }
    pub fn statement(&self) -> u32 {
        self.as_rustc().statement_index as u32
    }
}

impl_as_rustc!(
    #[derive(Clone, Copy, Debug)]
    RustcRichLocation,
    rustc_borrowck::consumers::RichLocation,
);
impl RustcRichLocation {
    pub fn rich_location(&self) -> RichLocation {
        match self.as_rustc() {
            rustc_borrowck::consumers::RichLocation::Start(l) => {
                RichLocation::Start(AsRustc::from_rustc(*l))
            }
            rustc_borrowck::consumers::RichLocation::Mid(l) => {
                RichLocation::Mid(AsRustc::from_rustc(*l))
            }
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum RichLocation {
    Start(Location),
    Mid(Location),
}

/// [`Location`] to [`Range`] map
pub struct LocationRanges {
    map: HashMap<Location, Range>,
}
impl LocationRanges {
    /// Build a [`Location`] -> source [`Range`] map from the MIR body.
    pub fn compute(body: &Body, source_info: &SourceInfo) -> Self {
        use rustc_middle::mir::{StatementKind, TerminatorKind};

        let user_locals = body.collect_user_variables(source_info);
        let mut map = HashMap::new();
        for (block, bb_data) in body.as_rustc().basic_blocks.iter_enumerated() {
            let stmt_count = bb_data.statements.len();
            let total = stmt_count + bb_data.terminator.as_ref().map(|_| 1).unwrap_or(0);
            for statement_index in 0..total {
                let location = rustc_middle::mir::Location {
                    block,
                    statement_index,
                };
                let span = body.as_rustc().source_info(location).span.source_callsite();
                let Some(range) = range_from_span(
                    &source_info.source,
                    AsRustc::from_rustc(span),
                    source_info.offset,
                ) else {
                    continue;
                };

                // check whether the statement touches a user local variable
                let touches_user_local = if statement_index < stmt_count {
                    match &bb_data.statements[statement_index].kind {
                        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                            user_locals.contains_key(&AsRustc::from_rustc(*local))
                        }
                        StatementKind::Assign(boxed) => {
                            user_locals.contains_key(&AsRustc::from_rustc(boxed.0.local))
                        }
                        _ => false,
                    }
                } else {
                    match bb_data.terminator.as_ref().map(|t| &t.kind) {
                        Some(TerminatorKind::Drop { place, .. }) => {
                            user_locals.contains_key(&AsRustc::from_rustc(place.local))
                        }
                        Some(TerminatorKind::Call { .. }) => {
                            // A return value of method call can be important if it is assigned to
                            // a temporary variable, so we always visualize them.
                            true
                        }
                        _ => false,
                    }
                };

                // If a range spans multiple lines, we ignore the range which may be annoying,
                // except for a user variable related one.
                if !touches_user_local && utils::range_is_multiline(&source_info.source, range) {
                    continue;
                }

                map.insert(AsRustc::from_rustc(location), range);
            }
        }
        Self { map }
    }
    pub fn get(&self, location: &Location) -> Option<&Range> {
        self.map.get(location)
    }
}

pub fn rich_locations_to_ranges(
    location_ranges: &LocationRanges,
    locations: &[RichLocation],
) -> Vec<Range> {
    locations
        .iter()
        .filter_map(|rich| {
            let loc = match rich {
                RichLocation::Start(l) | RichLocation::Mid(l) => l,
            };
            location_ranges.get(loc).copied()
        })
        .collect()
}

pub struct BorrowMap {
    location_map: HashMap<Borrow, (Location, BorrowData)>,
    local_map: HashMap<LocalId, HashSet<Borrow>>,
}
impl BorrowMap {
    pub fn new(borrow_set: &rustc_borrowck::consumers::BorrowSet<'_>) -> Self {
        let mut location_map = HashMap::new();
        // BorrowIndex corresponds to Location index
        for (location, data) in borrow_set.location_map().iter() {
            let data = if data.kind().mutability().is_mut() {
                BorrowData::Mutable {
                    borrowed: AsRustc::from_rustc(data.borrowed_place().local),
                    assigned: AsRustc::from_rustc(data.assigned_place().local),
                }
            } else {
                BorrowData::Shared {
                    borrowed: AsRustc::from_rustc(data.borrowed_place().local),
                    assigned: AsRustc::from_rustc(data.assigned_place().local),
                }
            };
            if let Some(borrows) = borrow_set.activation_map().get(location) {
                for borrow in borrows {
                    location_map.insert(
                        AsRustc::from_rustc(*borrow),
                        (AsRustc::from_rustc(*location), data.clone()),
                    );
                }
            }
        }
        let local_map = borrow_set
            .local_map()
            .iter()
            .map(|(local, borrows)| {
                (
                    AsRustc::from_rustc(*local),
                    borrows.iter().map(|b| AsRustc::from_rustc(*b)).collect(),
                )
            })
            .collect();
        Self {
            location_map,
            local_map,
        }
    }
    pub fn get_from_borrow(&self, borrow: &Borrow) -> Option<&(Location, BorrowData)> {
        self.location_map.get(borrow)
    }
    pub fn local_map(&self) -> &HashMap<LocalId, HashSet<Borrow>> {
        &self.local_map
    }
}

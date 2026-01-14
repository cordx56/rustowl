use super::*;

use std::collections::{HashMap, HashSet};

/// RegionEraser to erase region variables from MIR body
/// This is required to hash MIR body
struct RegionEraser<'tcx> {
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
    pub fn erase_region_variables(tcx: TyCtxt<'tcx>, body: Body<'tcx>) -> Body<'tcx> {
        let mut eraser = RegionEraser { tcx };
        use rustc_middle::ty::TypeFoldable;
        AsRustc::from_rustc(body.as_rustc().clone().fold_with(&mut eraser))
    }
}

impl<'tcx> TyCtxt<'tcx> {
    /// Collect and transform [`rustc_middle::mir::BasicBlocks`] into our data structure [`MirBasicBlock`]s.
    pub fn collect_basic_blocks(
        &self,
        fn_id: DefId,
        body: &Body<'tcx>,
        source_info: &SourceInfo,
    ) -> Vec<MirBasicBlock> {
        use rustc_middle::mir::*;

        let source_map = self.as_rustc().sess.source_map();
        body.as_rustc()
            .basic_blocks
            .iter_enumerated()
            .map(|(_bb, bb_data)| {
                let statements: Vec<_> = bb_data
                    .statements
                    .iter()
                    // `source_map` is not Send
                    .filter(|stmt| stmt.source_info.span.is_visible(source_map))
                    .collect();
                let statements =
                    statements
                        .par_iter()
                        .filter_map(|statement| {
                            let span = AsRustc::from_rustc(statement.source_info.span);
                            match &statement.kind {
                                StatementKind::Assign(v) => {
                                    let (place, rval) = &**v;
                                    let target_local_index = place.local.as_u32();
                                    let rv =
                                        match rval {
                                            Rvalue::Use(Operand::Move(p)) => {
                                                let local = p.local;
                                                range_from_span(
                                                    &source_info.source,
                                                    span,
                                                    source_info.offset,
                                                )
                                                .map(|range| MirRval::Move {
                                                    target_local: FnLocal::new(
                                                        local.as_u32(),
                                                        fn_id.as_u32(),
                                                    ),
                                                    range,
                                                })
                                            }
                                            Rvalue::Ref(_region, kind, place) => {
                                                let mutable =
                                                    matches!(kind, BorrowKind::Mut { .. });
                                                let local = place.local;
                                                let outlive = None;
                                                range_from_span(
                                                    &source_info.source,
                                                    span,
                                                    source_info.offset,
                                                )
                                                .map(|range| MirRval::Borrow {
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
                                    range_from_span(&source_info.source, span, source_info.offset)
                                        .map(|range| MirStatement::Assign {
                                            target_local: FnLocal::new(
                                                target_local_index,
                                                fn_id.as_u32(),
                                            ),
                                            range,
                                            rval: rv,
                                        })
                                }
                                _ => range_from_span(&source_info.source, span, source_info.offset)
                                    .map(|range| MirStatement::Other { range }),
                            }
                        })
                        .collect();
                let terminator = bb_data.terminator.as_ref().and_then(|terminator| {
                    let span = AsRustc::from_rustc(terminator.source_info.span);
                    match &terminator.kind {
                        TerminatorKind::Drop { place, .. } => {
                            range_from_span(&source_info.source, span, source_info.offset).map(
                                |range| MirTerminator::Drop {
                                    local: FnLocal::new(place.local.as_u32(), fn_id.as_u32()),
                                    range,
                                },
                            )
                        }
                        TerminatorKind::Call {
                            destination,
                            fn_span,
                            ..
                        } => range_from_span(
                            &source_info.source,
                            AsRustc::from_rustc(*fn_span),
                            source_info.offset,
                        )
                        .map(|fn_span| MirTerminator::Call {
                            destination_local: FnLocal::new(
                                destination.local.as_u32(),
                                fn_id.as_u32(),
                            ),
                            fn_span,
                        }),
                        _ => range_from_span(&source_info.source, span, source_info.offset)
                            .map(|range| MirTerminator::Other { range }),
                    }
                });
                MirBasicBlock {
                    statements,
                    terminator,
                }
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
            rustc_borrowck::consumers::RichLocation::Start(l) => RichLocation::Start(AsRustc::from_rustc(*l)),
            rustc_borrowck::consumers::RichLocation::Mid(l) => RichLocation::Mid(AsRustc::from_rustc(*l)),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum RichLocation {
    Start(Location),
    Mid(Location),
}

fn sort_locs(v: &mut [(u32, u32)]) {
    v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
}
fn statement_location_to_range(
    basic_blocks: &[MirBasicBlock],
    basic_block: u32,
    statement: u32,
) -> Option<Range> {
    basic_blocks.get(basic_block as usize).and_then(|bb| {
        if (statement as usize) < bb.statements.len() {
            bb.statements.get(statement as usize).map(|v| v.range())
        } else {
            bb.terminator.as_ref().map(|v| v.range())
        }
    })
}

pub fn rich_locations_to_ranges(
    basic_blocks: &[MirBasicBlock],
    locations: &[RichLocation],
) -> Vec<Range> {
    let mut starts = Vec::new();
    let mut mids = Vec::new();
    for rich in locations {
        match rich {
            RichLocation::Start(l) => {
                starts.push((l.block(), l.statement()));
            }
            RichLocation::Mid(l) => {
                mids.push((l.block(), l.statement()));
            }
        }
    }
    sort_locs(&mut starts);
    sort_locs(&mut mids);
    starts
        .par_iter()
        .zip(mids.par_iter())
        .filter_map(|(s, m)| {
            let sr = statement_location_to_range(basic_blocks, s.0, s.1);
            let mr = statement_location_to_range(basic_blocks, m.0, m.1);
            match (sr, mr) {
                (Some(s), Some(m)) => Range::new(s.from(), m.until()),
                _ => None,
            }
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

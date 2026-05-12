use super::*;
use rustowl::utils;

use indexmap::IndexMap;
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
        body.as_rustc()
            .basic_blocks
            .iter_enumerated()
            .map(|(block, bb_data)| {
                let statements: Vec<_> = bb_data.statements.iter().collect();
                let statements = statements
                    .iter()
                    .enumerate()
                    .map(|(statement_index, statement)| {
                        Statement::from_rustc((*statement).clone()).transform(
                            fn_id,
                            BasicBlockId(block.as_usize()),
                            statement_index,
                            location_ranges,
                        )
                    })
                    .collect();
                let terminator = bb_data.terminator.as_ref().map(|terminator| {
                    Terminator::from_rustc(terminator.clone()).transform(
                        fn_id,
                        BasicBlockId(block.as_usize()),
                        bb_data.statements.len(),
                        source_info,
                        location_ranges,
                    )
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
    pub fn successor(&self) -> Self {
        let next_location = self.into_rustc();
        AsRustc::from_rustc(next_location.successor_within_block())
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
                // source_callsite is wide, for macro invocation
                let span_callsite = body.as_rustc().source_info(location).span.source_callsite();
                let range = if let Some(v) = range_from_span(
                    &source_info.source,
                    AsRustc::from_rustc(span_callsite),
                    source_info.offset,
                ) {
                    v
                } else {
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
                if !touches_user_local
                    && utils::range_is_multiline(source_info.cleaned_source(), range)
                {
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

impl_as_rustc!(
    #[derive(Clone, Hash, Debug)]
    Place<'tcx>,
    rustc_middle::mir::Place<'tcx>,
);
impl Place<'_> {
    pub fn transform(&self, fn_id: DefId) -> MirPlace {
        let place = &self.as_rustc();
        use rustc_middle::mir::ProjectionElem;
        let local = FnLocal::new(place.local.as_u32(), fn_id.as_u32());
        let projection = place
            .projection
            .iter()
            .map(|e| match e {
                ProjectionElem::Deref => MirProjectionElem::Deref,
                ProjectionElem::Field(idx, _ty) => MirProjectionElem::Field {
                    index: idx.as_usize(),
                },
                ProjectionElem::Index(local) => MirProjectionElem::Index {
                    local: FnLocal::new(local.as_u32(), fn_id.as_u32()),
                },
                _ => MirProjectionElem::Other,
            })
            .collect();
        MirPlace { local, projection }
    }
}

impl_as_rustc!(
    #[derive(Clone, Hash, Debug)]
    Operand<'tcx>,
    rustc_middle::mir::Operand<'tcx>,
);
impl Operand<'_> {
    pub fn transform(&self, fn_id: DefId) -> MirOperand {
        use rustc_middle::mir::Operand;
        match &self.as_rustc() {
            Operand::Copy(place) => MirOperand::Copy {
                place: Place::from_rustc(*place).transform(fn_id),
            },
            Operand::Move(place) => MirOperand::Move {
                place: Place::from_rustc(*place).transform(fn_id),
            },
            _ => MirOperand::Other,
        }
    }
}

impl_as_rustc!(
    #[derive(Clone, Hash, Debug)]
    Rvalue<'tcx>,
    rustc_middle::mir::Rvalue<'tcx>,
);
impl Rvalue<'_> {
    pub fn transform(&self, fn_id: DefId) -> MirRval {
        use rustc_middle::mir::Rvalue;
        match &self.as_rustc() {
            Rvalue::Use(operand) => {
                let operand = Operand::from_rustc(operand.clone()).transform(fn_id);
                MirRval::Use { operand }
            }
            Rvalue::Repeat(operand, _) => {
                let operand = Operand::from_rustc(operand.clone()).transform(fn_id);
                MirRval::Repeat { operand }
            }
            Rvalue::Ref(_region, kind, place) => {
                let place = Place::from_rustc(*place).transform(fn_id);
                let mutable = kind.mutability().is_mut();
                MirRval::Ref { place, mutable }
            }
            Rvalue::Cast(_kind, operand, _ty) => {
                let operand = Operand::from_rustc(operand.clone()).transform(fn_id);
                MirRval::Cast { operand }
            }
            Rvalue::BinaryOp(_op, boxed) => {
                let left = Operand::from_rustc((**boxed).0.clone()).transform(fn_id);
                let right = Operand::from_rustc((**boxed).1.clone()).transform(fn_id);
                MirRval::BinaryOp { left, right }
            }
            Rvalue::UnaryOp(_op, operand) => {
                let operand = Operand::from_rustc(operand.clone()).transform(fn_id);
                MirRval::UnaryOp { operand }
            }
            Rvalue::Aggregate(_kind, operands) => {
                let fields = operands
                    .iter()
                    .map(|v| Operand::from_rustc(v.clone()).transform(fn_id))
                    .collect();
                MirRval::Aggregate { fields }
            }
            _ => MirRval::Other,
        }
    }
}

impl_as_rustc!(
    #[derive(Clone, Debug)]
    Statement<'tcx>,
    rustc_middle::mir::Statement<'tcx>,
);
impl Statement<'_> {
    pub fn transform(
        &self,
        fn_id: DefId,
        block: BasicBlockId,
        statement_index: usize,
        location_ranges: &LocationRanges,
    ) -> MirStatement {
        use rustc_middle::mir::StatementKind;
        let location = rustc_middle::mir::Location {
            block: rustc_middle::mir::BasicBlock::from_usize(block.0),
            statement_index,
        };
        let range = location_ranges
            .get(&Location::from_rustc(location))
            .copied();
        match &self.as_rustc().kind {
            StatementKind::Assign(boxed) => {
                let place = Place::from_rustc((**boxed).0).transform(fn_id);
                let rval = Rvalue::from_rustc((**boxed).1.clone()).transform(fn_id);
                let kind = MirStatementKind::Assign { place, rval };
                MirStatement { kind, range }
            }
            StatementKind::StorageLive(local) => MirStatement {
                kind: MirStatementKind::StorageLive {
                    local: FnLocal::new(local.as_u32(), fn_id.as_u32()),
                },
                range,
            },
            StatementKind::StorageDead(local) => MirStatement {
                kind: MirStatementKind::StorageDead {
                    local: FnLocal::new(local.as_u32(), fn_id.as_u32()),
                },
                range,
            },
            StatementKind::Nop => MirStatement {
                kind: MirStatementKind::Nop,
                range,
            },
            _ => MirStatement {
                kind: MirStatementKind::Other,
                range,
            },
        }
    }
}

impl_as_rustc!(
    #[derive(Clone, Debug)]
    Terminator<'tcx>,
    rustc_middle::mir::Terminator<'tcx>,
);
impl Terminator<'_> {
    pub fn transform(
        &self,
        fn_id: DefId,
        block: BasicBlockId,
        statement_index: usize,
        source_info: &SourceInfo,
        location_ranges: &LocationRanges,
    ) -> MirTerminator {
        use rustc_middle::mir::TerminatorKind;
        let location = rustc_middle::mir::Location {
            block: rustc_middle::mir::BasicBlock::from_usize(block.0),
            statement_index,
        };
        let range = location_ranges
            .get(&Location::from_rustc(location))
            .copied();
        match &self.as_rustc().kind {
            TerminatorKind::Goto { target } => MirTerminator {
                kind: MirTerminatorKind::Goto {
                    target: BasicBlockId(target.as_usize()),
                },
                range,
            },
            TerminatorKind::SwitchInt { discr, targets } => {
                let discr = Operand::from_rustc(discr.clone()).transform(fn_id);
                let targets = targets
                    .all_targets()
                    .iter()
                    .map(|v| BasicBlockId(v.as_usize()))
                    .collect();
                MirTerminator {
                    kind: MirTerminatorKind::SwitchInt { discr, targets },
                    range,
                }
            }
            TerminatorKind::Return => MirTerminator {
                kind: MirTerminatorKind::Return,
                range,
            },
            TerminatorKind::Unreachable => MirTerminator {
                kind: MirTerminatorKind::Unreachable,
                range,
            },
            TerminatorKind::Drop { place, target, .. } => {
                let kind = MirTerminatorKind::Drop {
                    place: Place::from_rustc(*place).transform(fn_id),
                    target: BasicBlockId(target.as_usize()),
                };
                MirTerminator { kind, range }
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                fn_span,
                ..
            } => {
                let func = Operand::from_rustc(func.clone()).transform(fn_id);
                let args = args
                    .iter()
                    .map(|v| Operand::from_rustc(v.node.clone()).transform(fn_id))
                    .collect();
                let destination = Place::from_rustc(*destination).transform(fn_id);
                let fn_range = range_from_span(
                    source_info.source(),
                    Span::from_rustc(*fn_span),
                    source_info.offset,
                );
                let kind = MirTerminatorKind::Call {
                    func,
                    args,
                    destination,
                    target: target.map(|v| BasicBlockId(v.as_usize())),
                    fn_range,
                };
                MirTerminator { kind, range }
            }
            TerminatorKind::TailCall {
                func,
                args,
                fn_span,
            } => {
                let func = Operand::from_rustc(func.clone()).transform(fn_id);
                let args = args
                    .iter()
                    .map(|v| Operand::from_rustc(v.node.clone()).transform(fn_id))
                    .collect();
                let fn_range = range_from_span(
                    source_info.source(),
                    Span::from_rustc(*fn_span),
                    source_info.offset,
                );
                let kind = MirTerminatorKind::TailCall {
                    func,
                    args,
                    fn_range,
                };
                MirTerminator { kind, range }
            }
            TerminatorKind::Assert { cond, target, .. } => {
                let cond = Operand::from_rustc(cond.clone()).transform(fn_id);
                MirTerminator {
                    kind: MirTerminatorKind::Assert {
                        cond,
                        target: BasicBlockId(target.as_usize()),
                    },
                    range,
                }
            }
            _ => {
                let successors = self
                    .as_rustc()
                    .successors()
                    .map(|v| BasicBlockId(v.as_usize()))
                    .collect();
                MirTerminator {
                    kind: MirTerminatorKind::Other { successors },
                    range,
                }
            }
        }
    }
}

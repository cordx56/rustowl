use crate::models::*;
use crate::AnalyzedMir;
use rustc_borrowck::consumers::{
    BodyWithBorrowckFacts, BorrowIndex, LocationTable, PoloniusOutput, RichLocation,
};
use rustc_interface::interface::Compiler;
use rustc_middle::{
    mir::{
        BasicBlock, BasicBlockData, Body, BorrowKind, Local, LocalDecl, Location, Operand, Rvalue,
        Statement, StatementKind, TerminatorKind, VarDebugInfoContents,
    },
    ty::{RegionKind, TyKind},
};
use rustc_span::Span;
use std::collections::{BTreeSet, HashMap, LinkedList};
use std::str::FromStr;

pub struct MirAnalyzer<'c, 'tcx> {
    compiler: &'c Compiler,
    location_table: &'c LocationTable,
    body: Body<'tcx>,
    output: PoloniusOutput,
    bb_map: HashMap<BasicBlock, BasicBlockData<'tcx>>,
    local_loan_live_at: HashMap<Local, Vec<RichLocation>>,
    local_super_regions: HashMap<Local, Vec<RichLocation>>,
}
impl<'c, 'tcx> MirAnalyzer<'c, 'tcx> {
    /// initialize analyzer
    pub fn new(compiler: &'c Compiler, facts: &'c BodyWithBorrowckFacts<'tcx>) -> Self {
        let af = &**facts.input_facts.as_ref().unwrap();
        let location_table = facts.location_table.as_ref().unwrap();

        // local -> all borrows on that local
        let local_borrow: HashMap<Local, Vec<BorrowIndex>> = HashMap::from_iter(
            facts
                .borrow_set
                .local_map
                .iter()
                .map(|(local, borrow_idc)| {
                    (*local, borrow_idc.iter().map(|v| v.clone()).collect())
                }),
        );
        let mut borrow_idx_local = HashMap::new();
        for (local, borrow_idc) in local_borrow.iter() {
            for borrow_idx in borrow_idc {
                let locals = match borrow_idx_local.get_mut(borrow_idx) {
                    Some(v) => v,
                    None => {
                        borrow_idx_local.insert(*borrow_idx, Vec::new());
                        borrow_idx_local.get_mut(borrow_idx).unwrap()
                    }
                };
                locals.push(*local);
            }
        }
        let body = facts.body.clone();
        log::info!("start re-computing borrow check with dump: true");
        let output = PoloniusOutput::compute(af, FromStr::from_str("Hybrid").unwrap(), true);
        log::info!("borrow check finished");

        //let local_loan_live: HashMap<Local, >
        //println!("{:?}", output);
        let mut local_loan_live_at = HashMap::new();
        for (location_idx, borrow_idc) in output.loan_live_at.iter() {
            let location = location_table.to_location(*location_idx);
            for borrow_idx in borrow_idc {
                if let Some(locals) = borrow_idx_local.get(borrow_idx) {
                    for local_idx in locals {
                        let locations = match local_loan_live_at.get_mut(local_idx) {
                            Some(v) => v,
                            None => {
                                local_loan_live_at.insert(*local_idx, Vec::new());
                                local_loan_live_at.get_mut(local_idx).unwrap()
                            }
                        };
                        locations.push(location);
                    }
                }
            }
        }

        // local's living range in provided source code

        /*
        let mut region_idx_location_idc = HashMap::new();
        for (location_idx, region_idc) in output.origin_live_on_entry.iter(){
            for region_idx in region_idc {
                match region_idc.get_mut(region_idx) {
                    Some(v) => v,
                    None => region_idc.insert(*region_idx)
                }
            }
        }
        */

        // locations that region includes
        let mut region_idx_locations = HashMap::new();
        for (location_idx, region_idc) in output.origin_live_on_entry.iter() {
            for region_idx in region_idc {
                let insert = match region_idx_locations.get_mut(region_idx) {
                    Some(v) => v,
                    None => {
                        region_idx_locations.insert(*region_idx, Vec::new());
                        region_idx_locations.get_mut(region_idx).unwrap()
                    }
                };
                insert.push(location_table.to_location(*location_idx).clone());
            }
        }

        // to know the regions that the region[i] :> for all locals
        // this must hold
        let mut local_idx_super_region_idc = HashMap::new();
        for (region_idx, borrow_idc) in output.origin_contains_loan_anywhere.iter() {
            for borrow_idx in borrow_idc {
                if let Some(locals) = borrow_idx_local.get(borrow_idx) {
                    for local in locals {
                        let insert = match local_idx_super_region_idc.get_mut(local) {
                            Some(v) => v,
                            None => {
                                local_idx_super_region_idc.insert(*local, BTreeSet::new());
                                local_idx_super_region_idc.get_mut(local).unwrap()
                            }
                        };
                        insert.insert(region_idx);
                    }
                }
            }
        }

        let mut local_super_regions = HashMap::new();
        for (local_idx, super_idc) in local_idx_super_region_idc.iter() {
            local_super_regions.insert(*local_idx, Vec::new());
            let insert = local_super_regions.get_mut(local_idx).unwrap();
            for super_idx in super_idc {
                if let Some(locations) = region_idx_locations.get(&super_idx) {
                    insert.extend_from_slice(locations);
                }
            }
        }

        // all subset that must hold
        // borrows lives in mapped region indices
        // regions must includes all borrows, their key
        // region :> borrow[i] must hold
        let mut borrow_idx_region_idc = HashMap::new();
        for (region_id, borrow_idc) in output.origin_contains_loan_anywhere.iter() {
            for borrow_idx in borrow_idc.iter() {
                let insert = match borrow_idx_region_idc.get_mut(borrow_idx) {
                    Some(v) => v,
                    None => {
                        borrow_idx_region_idc.insert(*borrow_idx, BTreeSet::new());
                        borrow_idx_region_idc.get_mut(borrow_idx).unwrap()
                    }
                };
                insert.insert(*region_id);
            }
        }
        // mapped regions must includes locals living
        //for (sup, subs) in output.subset_anywhere.iter() {}

        // build basic blocks map
        let bb_map = body
            .basic_blocks
            .iter_enumerated()
            .map(|(b, d)| (b, d.clone()))
            .collect();
        Self {
            compiler,
            location_table,
            body,
            output,
            bb_map,
            local_loan_live_at,
            local_super_regions,
        }
    }

    fn sort_locs(v: &mut Vec<(BasicBlock, usize)>) {
        v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    }
    fn stmt_location_to_range(&self, bb: BasicBlock, stmt_index: usize) -> Option<Range> {
        self.bb_map
            .get(&bb)
            .map(|bb| bb.statements.get(stmt_index))
            .flatten()
            .map(|stmt| stmt.source_info.span.into())
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
                    (Some(s), Some(m)) => Some(Range::new(s.from, m.until)),
                    _ => None,
                }
            })
            .collect()
    }

    /// obtain map from local id to living range
    fn lives(&self) -> HashMap<Local, Vec<Range>> {
        let mut local_live_locs = HashMap::new();
        for (loc_idx, locals) in self.output.var_drop_live_on_entry.iter() {
            //for (loc_idx, locals) in self.output.var_drop_live_on_entry.iter() {
            let location = self.location_table.to_location(*loc_idx);
            for local in locals {
                let insert = match local_live_locs.get_mut(local) {
                    Some(v) => v,
                    None => {
                        local_live_locs.insert(*local, Vec::new());
                        local_live_locs.get_mut(local).unwrap()
                    }
                };
                insert.push(location);
            }

            /*
            let location = self.location_table.to_location(*loc_idx);
            for local in locals {
                if local_live_locs.get(local).is_none() {
                    local_live_locs.insert(*local, (Vec::new(), Vec::new()));
                }
                let (starts, mids) = local_live_locs.get_mut(local).unwrap();
                match location {
                    RichLocation::Start(l) => {
                        starts.push((l.block, l.statement_index));
                    }
                    RichLocation::Mid(l) => {
                        mids.push((l.block, l.statement_index));
                    }
                }
            }
            */
        }
        HashMap::from_iter(
            local_live_locs
                .into_iter()
                .map(|(local, richs)| (local, self.rich_locations_to_ranges(&richs))),
        )
        /*
        HashMap::from_iter(
            local_live_locs
                .into_iter()
                .map(|(local, (mut start, mut mid))| {
                    Self::sort_locs(&mut start);
                    Self::sort_locs(&mut mid);
                    //for (start, mid) in start.iter().zip(mid.iter()) {
                    (
                        local,
                        start
                            .iter()
                            .zip(mid.iter())
                            .filter_map(|(start, mid)| {
                                let start = self
                                    .bb_map
                                    .get(&start.0)
                                    .map(|bb| bb.statements.get(start.1))
                                    .flatten();
                                let mid = self
                                    .bb_map
                                    .get(&mid.0)
                                    .map(|bb| bb.statements.get(mid.1))
                                    .flatten();
                                match (start, mid) {
                                    (Some(start), Some(mid)) => Some(Range::new(
                                        start.source_info.span.lo().0.into(),
                                        mid.source_info.span.hi().0.into(),
                                    )),
                                    _ => None,
                                }
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .into_iter(),
        )
        */
    }

    /// collect user defined variables from debug info in MIR
    fn collect_user_vars(&self) -> HashMap<Local, (Span, String)> {
        self.body
            .var_debug_info
            .iter()
            .filter_map(|debug| match &debug.value {
                VarDebugInfoContents::Place(place) => Some((
                    place.local,
                    (debug.source_info.span, debug.name.as_str().to_owned()),
                )),
                _ => None,
            })
            .collect()
    }
    /// collect declared variables in MIR body
    fn collect_decls(&self) -> Vec<MirDecl> {
        let user_vars = self.collect_user_vars();
        let lives = self.lives();
        let local_loan = self.local_loan();
        let must_live_at = self.local_must_lives_at();
        self.body
            .local_decls
            .iter_enumerated()
            .map(|(local, decl)| {
                let local_index = local.index();
                let ty = decl.ty.to_string();
                let lives = lives.get(&local).cloned().unwrap_or(Vec::new());
                let loan_live_at = local_loan.get(&local).cloned().unwrap_or(Vec::new());
                let must_live_at = must_live_at.get(&local).cloned().unwrap_or(Vec::new());
                if decl.is_user_variable() {
                    let (span, name) = user_vars.get(&local).cloned().unwrap();
                    MirDecl::User {
                        local_index,
                        name,
                        span: Range::from(span),
                        ty,
                        lives,
                        must_live_at,
                    }
                } else {
                    MirDecl::Other {
                        local_index,
                        ty,
                        lives,
                        must_live_at,
                    }
                }
            })
            .collect()
    }

    /// collect and translate basic blocks
    fn basic_blocks(&self) -> Vec<MirBasicBlock> {
        self.bb_map
            .iter()
            .map(|(_bb, bb_data)| {
                let statements = bb_data
                    .statements
                    .iter()
                    .filter_map(|statement| {
                        if !statement
                            .source_info
                            .span
                            .is_visible(self.compiler.sess.source_map())
                        {
                            return None;
                        }
                        match &statement.kind {
                            StatementKind::StorageLive(local) => Some(MirStatement::StorageLive {
                                target_local_index: local.index(),
                                range: Range::from(statement.source_info.span),
                            }),
                            StatementKind::StorageDead(local) => Some(MirStatement::StorageDead {
                                target_local_index: local.index(),
                                range: Range::from(statement.source_info.span),
                            }),
                            StatementKind::Assign(ref v) => {
                                let (place, rval) = &**v;
                                let target_local_index = place.local.index();
                                //place.local
                                let rv = match rval {
                                    Rvalue::Use(usage) => match usage {
                                        Operand::Move(p) => {
                                            let local = p.local;
                                            Some(MirRval::Move {
                                                target_local_index: local.index(),
                                                range: Range::from(statement.source_info.span),
                                            })
                                        }
                                        _ => None,
                                    },
                                    Rvalue::Ref(region, kind, place) => {
                                        let mutable = match kind {
                                            BorrowKind::Mut { .. } => true,
                                            _ => false,
                                        };
                                        let local = place.local;
                                        let outlive = None;
                                        Some(MirRval::Borrow {
                                            target_local_index: local.index(),
                                            range: Range::from(statement.source_info.span),
                                            mutable,
                                            outlive,
                                        })
                                    }
                                    _ => None,
                                };
                                Some(MirStatement::Assign {
                                    target_local_index,
                                    range: Range::from(statement.source_info.span),
                                    rval: rv,
                                })
                            }
                            _ => None,
                        }
                    })
                    .collect();
                let terminator =
                    bb_data
                        .terminator
                        .as_ref()
                        .map(|terminator| match &terminator.kind {
                            TerminatorKind::Drop { place, .. } => MirTerminator::Drop {
                                local_index: place.local.index(),
                                range: terminator.source_info.span.into(),
                            },
                            TerminatorKind::Call {
                                func,
                                args,
                                destination,
                                target,
                                unwind,
                                call_source,
                                fn_span,
                            } => MirTerminator::Call {
                                destination_local_index: destination.local.as_usize(),
                                fn_span: (*fn_span).into(),
                            },
                            _ => MirTerminator::Other,
                        });
                MirBasicBlock {
                    statements,
                    terminator,
                }
            })
            .collect()
    }

    fn local_loan(&self) -> HashMap<Local, Vec<Range>> {
        HashMap::from_iter(
            self.local_loan_live_at
                .iter()
                .map(|(local, rich)| (*local, self.rich_locations_to_ranges(&rich))),
        )
    }

    fn erase_superset(mut ranges: Vec<Range>, erase_subset: bool) -> Vec<Range> {
        let mut len = ranges.len();
        let mut i = 0;
        while i < len {
            let mut j = i + 1;
            while j < len {
                if !erase_subset
                    && ((ranges[j].from <= ranges[i].from && ranges[i].until < ranges[j].until)
                        || (ranges[j].from < ranges[i].from && ranges[i].until <= ranges[j].until))
                {
                    ranges.remove(j);
                } else if erase_subset
                    && ((ranges[i].from <= ranges[j].from && ranges[j].until < ranges[i].until)
                        || (ranges[i].from < ranges[j].from && ranges[j].until <= ranges[i].until))
                {
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
    fn local_must_lives_at(&self) -> HashMap<Local, Vec<Range>> {
        HashMap::from_iter(self.local_super_regions.iter().map(|(local, regions)| {
            (
                *local,
                Self::erase_superset(self.rich_locations_to_ranges(regions), false),
            )
        }))
    }
    /*
    fn local_can_lives_at(&self) -> HashMap<Local, Vec<Range>> {
        HashMap::from_iter(self.local_super_regions.iter().map(|(local, regions)| {
            (
                *local,
                Self::erase_superset(self.rich_locations_to_ranges(regions), true),
            )
        }))
    }
    */

    /// analyze MIR to get JSON-serializable, TypeScript friendly representation
    pub fn analyze<'a>(&mut self) -> AnalyzedMir {
        let decls = self.collect_decls();
        let basic_blocks = self.basic_blocks();
        //let mut lives = HashMap::new();

        //for (locidx, borrows) in output.errors.iter() {}
        AnalyzedMir {
            basic_blocks,
            decls,
        }
    }
}

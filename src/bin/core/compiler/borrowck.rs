use super::*;

use std::collections::{HashMap, HashSet};

impl_as_rustc!(
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    Point,
    <rustc_borrowck::consumers::RustcFacts as polonius_engine::FactTypes>::Point,
);

impl_as_rustc!(
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    Borrow,
    <rustc_borrowck::consumers::RustcFacts as polonius_engine::FactTypes>::Loan,
);

impl_as_rustc!(
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    Region,
    <rustc_borrowck::consumers::RustcFacts as polonius_engine::FactTypes>::Origin,
);

impl_as_rustc!(PoloniusInput, rustc_borrowck::consumers::PoloniusInput);
impl PoloniusInput {
    pub fn compute(&self) -> PoloniusOutput {
        AsRustc::from_rustc(rustc_borrowck::consumers::PoloniusOutput::compute(
            self.as_rustc(),
            polonius_engine::Algorithm::DatafrogOpt,
            true,
        ))
    }

    pub fn var_dropped_at(&self) -> Vec<(LocalId, Point)> {
        self.as_rustc()
            .var_dropped_at
            .iter()
            .map(|(v, p)| (AsRustc::from_rustc(*v), AsRustc::from_rustc(*p)))
            .collect()
    }
}

impl_as_rustc!(PoloniusOutput, rustc_borrowck::consumers::PoloniusOutput);

impl PoloniusOutput {
    pub fn var_live_on_entry(&self) -> HashMap<Point, Vec<LocalId>> {
        self.as_rustc()
            .var_live_on_entry
            .iter()
            .map(|(p, vs)| {
                (
                    AsRustc::from_rustc(*p),
                    vs.iter().map(|v| AsRustc::from_rustc(*v)).collect(),
                )
            })
            .collect()
    }
    pub fn var_drop_live_on_entry(&self) -> HashMap<Point, Vec<LocalId>> {
        self.as_rustc()
            .var_drop_live_on_entry
            .iter()
            .map(|(p, vs)| {
                (
                    AsRustc::from_rustc(*p),
                    vs.iter().map(|v| AsRustc::from_rustc(*v)).collect(),
                )
            })
            .collect()
    }
    pub fn loan_live_at(&self) -> HashMap<Point, Vec<Borrow>> {
        self.as_rustc()
            .loan_live_at
            .iter()
            .map(|(p, bs)| {
                (
                    AsRustc::from_rustc(*p),
                    bs.iter().map(|b| AsRustc::from_rustc(*b)).collect(),
                )
            })
            .collect()
    }
    pub fn origin_live_on_entry(&self) -> HashMap<Point, Vec<Region>> {
        self.as_rustc()
            .origin_live_on_entry
            .iter()
            .map(|(p, rs)| {
                (
                    AsRustc::from_rustc(*p),
                    rs.iter().map(|r| AsRustc::from_rustc(*r)).collect(),
                )
            })
            .collect()
    }
    pub fn subset(&self) -> HashMap<Point, HashMap<Region, HashSet<Region>>> {
        self.as_rustc()
            .subset
            .iter()
            .map(|(p, rrs)| {
                (
                    AsRustc::from_rustc(*p),
                    rrs.iter()
                        .map(|(r, rs)| {
                            (
                                AsRustc::from_rustc(*r),
                                rs.iter().map(|r| AsRustc::from_rustc(*r)).collect(),
                            )
                        })
                        .collect(),
                )
            })
            .collect()
    }
    pub fn origin_contains_loan_at(&self) -> HashMap<Point, HashMap<Region, HashSet<Borrow>>> {
        self.as_rustc()
            .origin_contains_loan_at
            .iter()
            .map(|(p, rbs)| {
                (
                    AsRustc::from_rustc(*p),
                    rbs.iter()
                        .map(|(r, bs)| {
                            (
                                AsRustc::from_rustc(*r),
                                bs.iter().map(|b| AsRustc::from_rustc(*b)).collect(),
                            )
                        })
                        .collect(),
                )
            })
            .collect()
    }
}

impl_as_rustc!(
    PoloniusLocationTable,
    rustc_borrowck::consumers::PoloniusLocationTable
);
impl PoloniusLocationTable {
    pub fn get_rich_location(&self, p: &Point) -> RichLocation {
        RustcRichLocation::from_rustc(self.as_rustc().to_rich_location(*p.as_rustc()))
            .rich_location()
    }
}

impl_as_rustc!(
    BorrowckFacts<'tcx>,
    rustc_borrowck::consumers::BodyWithBorrowckFacts<'tcx>
);
impl<'tcx> BorrowckFacts<'tcx> {
    pub fn body(&self) -> Body<'tcx> {
        AsRustc::from_rustc(self.as_rustc().body.clone())
    }
    pub fn borrow_map(&self) -> BorrowMap {
        BorrowMap::new(&self.as_rustc().borrow_set)
    }

    pub fn polonius_input(&mut self) -> PoloniusInput {
        AsRustc::from_rustc(*self.mut_rustc().input_facts.take().unwrap())
    }
    pub fn location_table(&mut self) -> PoloniusLocationTable {
        AsRustc::from_rustc(self.mut_rustc().location_table.take().unwrap())
    }
}

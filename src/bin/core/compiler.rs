use rayon::prelude::*;
use rustowl::models::*;
use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};

macro_rules! impl_as_rustc {
    (
        $(#[$outer:meta])*
        $n:ident $(<$($p:tt),*>)?,
        $c:ty$(,)?
    ) => {
        $(#[$outer])*
        pub struct $n$(<$($p),*>)?($c);
        impl$(<$($p),*>)? $crate::core::compiler::AsRustc for $n$(<$($p),*>)? {
            type Rustc = $c;
            fn as_rustc(&self) -> &Self::Rustc {
                &self.0
            }
            fn mut_rustc(&mut self) -> &mut Self::Rustc {
                &mut self.0
            }
            fn into_rustc(self) -> Self::Rustc {
                self.0
            }
            fn from_rustc(rustc: Self::Rustc) -> Self {
                Self(rustc)
            }
        }
    };
}

#[macro_use]
mod borrowck;
#[macro_use]
mod hash;
#[macro_use]
mod transform;

pub use borrowck::*;
pub use hash::Hasher;
pub use transform::*;

fn range_from_span(source: &str, span: Span, offset: u32) -> Option<Range> {
    let from = Loc::new(source, span.lo(), offset);
    let until = Loc::new(source, span.hi(), offset);
    Range::new(from, until)
}

pub struct SourceInfo {
    offset: u32,
    path: PathBuf,
    source: String,
}
impl SourceInfo {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
    pub fn source(&self) -> &str {
        &self.source
    }
}

pub trait AsRustc {
    type Rustc;
    fn as_rustc(&self) -> &Self::Rustc;
    fn mut_rustc(&mut self) -> &mut Self::Rustc;
    fn into_rustc(self) -> Self::Rustc;
    fn from_rustc(rustc: Self::Rustc) -> Self;
}

impl_as_rustc!(
    #[derive(Clone, Copy)]
    TyCtxt<'tcx>, rustc_middle::ty::TyCtxt<'tcx>,
);

impl<'tcx> TyCtxt<'tcx> {
    #[rustversion::since(1.90.0)]
    pub fn get_borrowck_facts(&self, def_id: DefId) -> HashMap<DefId, BorrowckFacts<'tcx>> {
        let facts = rustc_borrowck::consumers::get_bodies_with_borrowck_facts(
            *self.as_rustc(),
            *def_id.as_rustc(),
            rustc_borrowck::consumers::ConsumerOptions::PoloniusInputFacts,
        );
        facts
            .into_iter()
            .map(|(k, v)| (AsRustc::from_rustc(k), AsRustc::from_rustc(v)))
            .collect()
    }
    #[rustversion::before(1.90.0)]
    pub fn get_borrowck_facts(&self, def_id: DefId) -> HashMap<DefId, BorrowckFacts<'tcx>> {
        let mut result = HashMap::new();
        let facts = rustc_borrowck::consumers::get_body_with_borrowck_facts(
            *self.as_rustc(),
            *def_id.as_rustc(),
            rustc_borrowck::consumers::ConsumerOptions::PoloniusInputFacts,
        );
        result.insert(def_id, AsRustc::from_rustc(facts));
        for nested_def_id in self.as_rustc().nested_bodies_within(*def_id.as_rustc()) {
            let facts = rustc_borrowck::consumers::get_body_with_borrowck_facts(
                *self.as_rustc(),
                nested_def_id,
                rustc_borrowck::consumers::ConsumerOptions::PoloniusInputFacts,
            );
            result.insert(
                AsRustc::from_rustc(nested_def_id),
                AsRustc::from_rustc(facts),
            );
        }
        result
    }

    pub fn source_info_from_span(&self, span: Span) -> SourceInfo {
        let source_map = self.as_rustc().sess.source_map();
        let file_name = source_map.span_to_filename(*span.as_rustc());
        let source_file = source_map.get_source_file(&file_name).unwrap();
        let offset = source_file.start_pos.0;
        let file_name = source_map.path_mapping().to_embeddable_absolute_path(
            rustc_span::RealFileName::LocalPath(file_name.into_local_path().unwrap()),
            &rustc_span::RealFileName::LocalPath(std::env::current_dir().unwrap()),
        );
        let path = file_name
            .to_path(rustc_span::FileNameDisplayPreference::Local)
            .to_path_buf();
        let source = std::fs::read_to_string(&path).unwrap();
        SourceInfo {
            offset,
            path,
            source,
        }
    }

    pub fn crate_name(&self) -> String {
        self.as_rustc()
            .crate_name(rustc_hir::def_id::LOCAL_CRATE)
            .to_string()
    }

    pub fn def_name(&self, def_id: DefId) -> String {
        self.as_rustc().def_path_str(def_id.as_rustc().to_def_id())
    }
}

impl_as_rustc!(
    #[derive(Clone)]
    Body<'tcx>,
    rustc_middle::mir::Body<'tcx>,
);

impl<'tcx> Body<'tcx> {
    pub fn get_local_decls(&self) -> HashMap<LocalId, String> {
        self.0
            .local_decls
            .iter_enumerated()
            .map(|(local, decl)| (AsRustc::from_rustc(local), decl.ty.to_string()))
            .collect()
    }

    pub fn collect_user_variables(
        &self,
        source_info: &SourceInfo,
    ) -> HashMap<LocalId, (Range, String)> {
        self.0
            .var_debug_info
            // this cannot be par_iter since body cannot send
            .iter()
            .filter_map(|debug| match &debug.value {
                rustc_middle::mir::VarDebugInfoContents::Place(place) => {
                    let span = AsRustc::from_rustc(debug.source_info.span);
                    range_from_span(&source_info.source, span, source_info.offset).map(|range| {
                        (
                            AsRustc::from_rustc(place.local),
                            (range, debug.name.as_str().to_owned()),
                        )
                    })
                }
                _ => None,
            })
            .collect()
    }

    pub fn span(&self) -> Span {
        AsRustc::from_rustc(self.0.span)
    }

    /// Extract StorageLive and StorageDead information from MIR body.
    /// Returns a map from LocalId to (StorageLive ranges, StorageDead ranges).
    pub fn get_storage_info(
        &self,
        source_info: &SourceInfo,
    ) -> (HashMap<LocalId, Vec<Range>>, HashMap<LocalId, Vec<Range>>) {
        use rustc_middle::mir::*;

        let mut storage_live: HashMap<LocalId, Vec<Range>> = HashMap::new();
        let mut storage_dead: HashMap<LocalId, Vec<Range>> = HashMap::new();

        for bb_data in self.0.basic_blocks.iter() {
            for stmt in &bb_data.statements {
                let span = AsRustc::from_rustc(stmt.source_info.span);
                match &stmt.kind {
                    StatementKind::StorageLive(local) => {
                        if let Some(range) =
                            range_from_span(&source_info.source, span, source_info.offset)
                        {
                            storage_live
                                .entry(AsRustc::from_rustc(*local))
                                .or_default()
                                .push(range);
                        }
                    }
                    StatementKind::StorageDead(local) => {
                        if let Some(range) =
                            range_from_span(&source_info.source, span, source_info.offset)
                        {
                            storage_dead
                                .entry(AsRustc::from_rustc(*local))
                                .or_default()
                                .push(range);
                        }
                    }
                    _ => {}
                }
            }
        }

        (storage_live, storage_dead)
    }

    /// Compute storage ranges for each local variable based on StorageLive/StorageDead.
    /// Returns a map from LocalId to a list of ranges where the variable is valid.
    pub fn compute_storage_ranges(&self, source_info: &SourceInfo) -> HashMap<LocalId, Vec<Range>> {
        use rustowl::utils;

        let (storage_live, storage_dead) = self.get_storage_info(source_info);
        let mut result: HashMap<LocalId, Vec<Range>> = HashMap::new();

        for (local, live_ranges) in &storage_live {
            let dead_ranges = storage_dead.get(local);

            for live_range in live_ranges {
                // Find the corresponding StorageDead
                // We look for a StorageDead with position >= StorageLive position
                let end_pos = if let Some(dead_ranges) = dead_ranges {
                    dead_ranges
                        .iter()
                        .filter(|dr| dr.from() >= live_range.from())
                        .map(|dr| dr.until())
                        .min_by_key(|loc| *loc)
                } else {
                    None
                };

                if let Some(end) = end_pos
                    && let Some(range) = Range::new(live_range.from(), end)
                {
                    result.entry(*local).or_default().push(range);
                }
            }
        }

        // Eliminate overlapping ranges
        result
            .into_iter()
            .map(|(local, ranges)| (local, utils::eliminated_ranges(ranges)))
            .collect()
    }
}

impl_as_rustc!(
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    Span,
    rustc_span::Span,
);

impl Span {
    pub fn lo(&self) -> u32 {
        use rustc_span::Pos;
        self.0.lo().to_u32()
    }
    pub fn hi(&self) -> u32 {
        use rustc_span::Pos;
        self.0.hi().to_u32()
    }
}

impl_as_rustc!(
    /// Definition ID type
    /// corresponds to function definition ID
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    DefId,
    rustc_hir::def_id::LocalDefId,
);
impl DefId {
    pub fn as_u32(&self) -> u32 {
        self.as_rustc().local_def_index.as_u32()
    }
}

impl_as_rustc!(
    /// Local ID type
    /// corresponds to function local (variable) ID
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    LocalId,
    rustc_middle::mir::Local,
);
impl LocalId {
    pub fn as_u32(&self) -> u32 {
        self.as_rustc().as_u32()
    }
}

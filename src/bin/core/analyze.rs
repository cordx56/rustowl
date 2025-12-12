mod polonius_analyzer;
mod shared;
mod transform;

use super::cache;
use rustc_borrowck::consumers::{
    BodyWithBorrowckFacts, ConsumerOptions, PoloniusInput, PoloniusOutput,
    get_bodies_with_borrowck_facts,
};
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_middle::{mir::Local, ty::TyCtxt};
use rustowl::models::FoldIndexMap as HashMap;
use rustowl::models::range_vec_from_vec;
use rustowl::models::*;
use smallvec::SmallVec;
use std::future::Future;
use std::pin::Pin;

pub type MirAnalyzeFuture = Pin<Box<dyn Future<Output = MirAnalyzer> + Send + Sync>>;

#[derive(Clone, Debug)]
pub struct AnalyzeResult {
    pub file_name: String,
    pub file_hash: String,
    pub mir_hash: String,
    pub analyzed: Function,
}

pub enum MirAnalyzerInitResult {
    Cached(Box<AnalyzeResult>),
    Analyzer(MirAnalyzeFuture),
}

pub struct MirAnalyzer {
    file_name: String,
    local_decls: HashMap<Local, String>,
    user_vars: HashMap<Local, (Range, String)>,
    input: PoloniusInput,
    basic_blocks: SmallVec<[MirBasicBlock; 8]>,
    fn_id: LocalDefId,
    file_hash: String,
    mir_hash: String,
    accurate_live: HashMap<Local, Vec<Range>>,
    must_live: HashMap<Local, Vec<Range>>,
    shared_live: HashMap<Local, Vec<Range>>,
    mutable_live: HashMap<Local, Vec<Range>>,
    drop_range: HashMap<Local, Vec<Range>>,
}
impl MirAnalyzer {
    /// initialize analyzer for the function and all nested bodies (closures, async blocks)
    pub fn batch_init<'tcx>(tcx: TyCtxt<'tcx>, fn_id: LocalDefId) -> Vec<MirAnalyzerInitResult> {
        let bodies =
            get_bodies_with_borrowck_facts(tcx, fn_id, ConsumerOptions::PoloniusInputFacts);

        bodies
            .into_iter()
            .map(|(def_id, facts)| Self::init_one(tcx, def_id, facts))
            .collect()
    }

    fn init_one<'tcx>(
        tcx: TyCtxt<'tcx>,
        fn_id: LocalDefId,
        mut facts: BodyWithBorrowckFacts<'tcx>,
    ) -> MirAnalyzerInitResult {
        let input = *facts.input_facts.take().unwrap();
        let location_table = facts.location_table.take().unwrap();

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
        tracing::info!("facts of {fn_id:?} prepared; start analyze of {fn_id:?}");

        // collect local declared vars
        // this must be done in local thread
        let local_decls = facts
            .body
            .local_decls
            .iter_enumerated()
            .map(|(local, decl)| (local, decl.ty.to_string()))
            .collect();

        // region variables should not be hashed (it results an error)
        // so we erase region variables and set 'static as new region
        let mir_hash = cache::Hasher::get_hash(
            tcx,
            transform::erase_region_variables(tcx, facts.body.clone()),
        );
        let file_hash = cache::Hasher::get_hash(tcx, &source);
        let mut cache = cache::CACHE.lock().unwrap();

        // setup cache
        if cache.is_none() {
            *cache = cache::get_cache(&tcx.crate_name(LOCAL_CRATE).to_string());
        }
        if let Some(cache) = cache.as_mut()
            && let Some(analyzed) = cache.get_cache(&file_hash, &mir_hash, Some(&file_name))
        {
            tracing::info!("MIR cache hit: {fn_id:?}");
            return MirAnalyzerInitResult::Cached(Box::new(AnalyzeResult {
                file_name,
                file_hash,
                mir_hash,
                analyzed,
            }));
        }
        drop(cache);

        // collect user defined vars
        // this must be done in local thread
        let user_vars = transform::collect_user_vars(&source, offset, &facts.body);

        // build basic blocks map
        // this must be done in local thread
        let basic_blocks = transform::collect_basic_blocks(
            fn_id,
            &source,
            offset,
            &facts.body.basic_blocks,
            tcx.sess.source_map(),
        );

        // collect borrow data
        // this must be done in local thread
        let borrow_data = transform::BorrowMap::new(&facts.borrow_set);

        let analyzer = Box::pin(async move {
            tracing::info!("start re-computing borrow check with dump: true");
            // compute accurate region, which may eliminate invalid region
            let output_datafrog =
                PoloniusOutput::compute(&input, polonius_engine::Algorithm::DatafrogOpt, true);
            tracing::info!("borrow check finished");

            let accurate_live = polonius_analyzer::get_accurate_live(
                &output_datafrog,
                &location_table,
                &basic_blocks,
            );

            let must_live = polonius_analyzer::get_must_live(
                &output_datafrog,
                &location_table,
                &borrow_data,
                &basic_blocks,
            );

            let (shared_live, mutable_live) = polonius_analyzer::get_borrow_live(
                &output_datafrog,
                &location_table,
                &borrow_data,
                &basic_blocks,
            );

            let drop_range =
                polonius_analyzer::drop_range(&output_datafrog, &location_table, &basic_blocks);

            MirAnalyzer {
                file_name,
                local_decls,
                input,
                user_vars,
                basic_blocks,
                fn_id,
                file_hash,
                mir_hash,
                accurate_live,
                must_live,
                shared_live,
                mutable_live,
                drop_range,
            }
        });
        MirAnalyzerInitResult::Analyzer(analyzer)
    }

    /// collect declared variables in MIR body
    /// final step of analysis
    fn collect_decls(&self) -> DeclVec {
        let user_vars = &self.user_vars;
        let lives = &self.accurate_live;
        let must_live_at = &self.must_live;

        let drop_range = &self.drop_range;
        let mut result = DeclVec::with_capacity(self.local_decls.len());

        for (local, ty) in &self.local_decls {
            let ty = smol_str::SmolStr::from(ty.as_str());
            let must_live_at = must_live_at.get(local).cloned().unwrap_or_default();
            let lives = lives.get(local).cloned().unwrap_or_default();
            let shared_borrow = self.shared_live.get(local).cloned().unwrap_or_default();
            let mutable_borrow = self.mutable_live.get(local).cloned().unwrap_or_default();
            let drop = self.is_drop(*local);
            let drop_range = drop_range.get(local).cloned().unwrap_or_default();

            let fn_local = FnLocal::new(local.as_u32(), self.fn_id.local_def_index.as_u32());
            let decl = if let Some((span, name)) = user_vars.get(local).cloned() {
                MirDecl::User {
                    local: fn_local,
                    name: smol_str::SmolStr::from(name.as_str()),
                    span,
                    ty,
                    lives: range_vec_from_vec(lives),
                    shared_borrow: range_vec_from_vec(shared_borrow),
                    mutable_borrow: range_vec_from_vec(mutable_borrow),
                    must_live_at: range_vec_from_vec(must_live_at),
                    drop,
                    drop_range: range_vec_from_vec(drop_range),
                }
            } else {
                MirDecl::Other {
                    local: fn_local,
                    ty,
                    lives: range_vec_from_vec(lives),
                    shared_borrow: range_vec_from_vec(shared_borrow),
                    mutable_borrow: range_vec_from_vec(mutable_borrow),
                    drop,
                    drop_range: range_vec_from_vec(drop_range),
                    must_live_at: range_vec_from_vec(must_live_at),
                }
            };
            result.push(decl);
        }
        result
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

#[cfg(test)]
mod tests {
    use super::*;

    // Test AnalyzeResult structure creation
    #[test]
    fn test_analyze_result_creation() {
        let result = AnalyzeResult {
            file_name: "test.rs".to_string(),
            file_hash: "abc123".to_string(),
            mir_hash: "def456".to_string(),
            analyzed: Function {
                fn_id: 1,
                basic_blocks: SmallVec::new(),
                decls: DeclVec::new(),
            },
        };

        assert_eq!(result.file_name, "test.rs");
        assert_eq!(result.file_hash, "abc123");
        assert_eq!(result.mir_hash, "def456");
        assert_eq!(result.analyzed.fn_id, 1);
        assert!(result.analyzed.decls.is_empty());
        assert!(result.analyzed.basic_blocks.is_empty());
    }

    // Test MirAnalyzerInitResult enum variants
    #[test]
    fn test_mir_analyzer_init_result_cached() {
        let analyze_result = AnalyzeResult {
            file_name: "test.rs".to_string(),
            file_hash: "hash".to_string(),
            mir_hash: "mir_hash".to_string(),
            analyzed: Function {
                fn_id: 1,
                basic_blocks: SmallVec::new(),
                decls: DeclVec::new(),
            },
        };

        let result = MirAnalyzerInitResult::Cached(Box::new(analyze_result.clone()));
        match result {
            MirAnalyzerInitResult::Cached(cached) => {
                assert_eq!(cached.file_name, "test.rs");
                assert_eq!(cached.file_hash, "hash");
                assert_eq!(cached.mir_hash, "mir_hash");
            }
            _ => panic!("Expected Cached variant"),
        }
    }

    // Test AnalyzeResult with populated data
    #[test]
    fn test_analyze_result_with_data() {
        let mut decls = DeclVec::new();
        decls.push(MirDecl::Other {
            local: FnLocal { id: 1, fn_id: 50 },
            ty: "String".into(),
            lives: SmallVec::new(),
            shared_borrow: SmallVec::new(),
            mutable_borrow: SmallVec::new(),
            drop: true,
            drop_range: SmallVec::new(),
            must_live_at: SmallVec::new(),
        });

        let mut basic_blocks = SmallVec::new();
        basic_blocks.push(MirBasicBlock {
            statements: SmallVec::new(),
            terminator: None,
        });

        let result = AnalyzeResult {
            file_name: "complex.rs".to_string(),
            file_hash: "complex_hash".to_string(),
            mir_hash: "complex_mir".to_string(),
            analyzed: Function {
                fn_id: 42,
                basic_blocks,
                decls,
            },
        };

        assert_eq!(result.file_name, "complex.rs");
        assert_eq!(result.analyzed.fn_id, 42);
        assert_eq!(result.analyzed.decls.len(), 1);
        assert_eq!(result.analyzed.basic_blocks.len(), 1);
    }

    // Test AnalyzeResult with user variables (simplified)
    #[test]
    fn test_analyze_result_with_user_vars() {
        let mut decls = DeclVec::new();
        // Create a simple test without complex Range construction
        decls.push(MirDecl::Other {
            local: FnLocal { id: 1, fn_id: 42 },
            ty: "i32".into(),
            lives: SmallVec::new(),
            shared_borrow: SmallVec::new(),
            mutable_borrow: SmallVec::new(),
            drop: true,
            drop_range: SmallVec::new(),
            must_live_at: SmallVec::new(),
        });

        let result = AnalyzeResult {
            file_name: "user_vars.rs".to_string(),
            file_hash: "user_hash".to_string(),
            mir_hash: "user_mir".to_string(),
            analyzed: Function {
                fn_id: 50,
                basic_blocks: SmallVec::new(),
                decls,
            },
        };

        assert_eq!(result.analyzed.decls.len(), 1);
        match &result.analyzed.decls[0] {
            MirDecl::Other { drop, .. } => {
                assert!(*drop);
            }
            _ => panic!("Expected Other variant"),
        }
    }
}

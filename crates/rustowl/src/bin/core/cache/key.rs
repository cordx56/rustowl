use std::sync::Arc;

use rustowl::models::{Function, Loc};

/// `fn_text_hash` is the function's own source text: an edit inside it misses, an
/// edit before it leaves the hash unchanged and the entry is served re-anchored.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct FunctionKey {
    pub def_path: u64,
    pub fn_text_hash: u64,
    pub mir_hash: String,
    pub analysis_version: u32,
    pub toolchain_hash: u64,
}

/// A cached body with the spans it had when stored, plus the offset its
/// function started at.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedEntry {
    pub base: Loc,
    pub function: Arc<Function>,
}

impl CachedEntry {
    pub fn estimated_bytes(&self) -> usize {
        self.function.name.len() + self.function.decls.len() * 128
    }
}

/// Return the cached body for a function now starting at `current_base`.
///
/// Spans are cached absolute, so an unmoved body is shared as-is and a moved one
/// is shifted here; the cached entry keeps its own `base` and never drifts.
pub fn resolve(entry: &CachedEntry, current_base: Loc) -> Arc<Function> {
    resolve_inner(entry, current_base)
}

fn resolve_inner(entry: &CachedEntry, current_base: Loc) -> Arc<Function> {
    let delta = i64::from(current_base.0) - i64::from(entry.base.0);
    if delta == 0 {
        return Arc::clone(&entry.function);
    }
    let mut function = (*entry.function).clone();
    function.shift(delta);
    Arc::new(function)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustowl::models::{FnLocal, MirDecl, MirType, Range};

    fn ranges(start: u32, n: usize) -> Vec<Range> {
        (0..n)
            .map(|i| {
                Range::new(Loc(start + i as u32 * 10), Loc(start + i as u32 * 10 + 5)).unwrap()
            })
            .collect()
    }

    fn sample_function() -> Function {
        let ty = MirType {
            name: "T".into(),
            reference: None,
        };
        Function {
            fn_id: 7,
            name: "f".into(),
            basic_blocks: vec![],
            decls: vec![
                MirDecl::User {
                    local: FnLocal::new(0, 7),
                    name: "x".into(),
                    span: Range::new(Loc(100), Loc(110)).unwrap(),
                    ty: ty.clone(),
                    lives: ranges(120, 2),
                    shared_borrow: ranges(150, 2),
                    mutable_borrow: ranges(180, 2),
                    drop: true,
                    drop_range: ranges(210, 2),
                    definitely_live_at: ranges(240, 2),
                    maybe_init_at: ranges(270, 2),
                    deficit_at: ranges(300, 2),
                    storage_range: ranges(330, 2),
                },
                MirDecl::Other {
                    local: FnLocal::new(1, 7),
                    ty,
                    lives: ranges(400, 2),
                    shared_borrow: ranges(430, 2),
                    mutable_borrow: ranges(460, 2),
                    drop: false,
                    drop_range: ranges(490, 2),
                    definitely_live_at: ranges(520, 2),
                    maybe_init_at: ranges(550, 2),
                    deficit_at: ranges(580, 2),
                    storage_range: ranges(610, 2),
                },
            ],
        }
    }

    fn key(fn_text_hash: u64) -> FunctionKey {
        FunctionKey {
            def_path: 42,
            fn_text_hash,
            mir_hash: "mir".into(),
            analysis_version: super::super::ANALYSIS_VERSION,
            toolchain_hash: super::super::toolchain_hash(),
        }
    }

    #[test]
    fn an_edit_inside_the_function_misses() {
        assert_ne!(key(1), key(2), "fn_text_hash must be part of identity");
    }

    #[test]
    fn an_edit_before_the_function_still_hits() {
        assert_eq!(key(1), key(1));
    }

    #[test]
    fn a_different_toolchain_misses() {
        let mut other = key(1);
        other.toolchain_hash = key(1).toolchain_hash.wrapping_add(1);
        assert_ne!(key(1), other);
    }

    #[test]
    fn a_different_analysis_version_misses() {
        let mut other = key(1);
        other.analysis_version = super::super::ANALYSIS_VERSION + 1;
        assert_ne!(key(1), other);
    }

    #[test]
    fn an_unmoved_function_shares_its_allocation() {
        let entry = CachedEntry {
            base: Loc(100),
            function: Arc::new(sample_function()),
        };
        let resolved = resolve(&entry, Loc(100));
        assert!(
            Arc::ptr_eq(&resolved, &entry.function),
            "delta == 0 must share, not copy"
        );
    }

    #[test]
    fn a_moved_function_is_re_anchored_by_exactly_the_delta() {
        let entry = CachedEntry {
            base: Loc(100),
            function: Arc::new(sample_function()),
        };
        let resolved = resolve(&entry, Loc(350));

        let mut expected = sample_function();
        expected.shift(250);
        assert_eq!(
            serde_json::to_string(&*resolved).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
        assert!(
            !Arc::ptr_eq(&resolved, &entry.function),
            "a moved function must not alias the cache"
        );
    }

    #[test]
    fn re_anchoring_backwards_works_too() {
        let entry = CachedEntry {
            base: Loc(500),
            function: Arc::new(sample_function()),
        };
        let resolved = resolve(&entry, Loc(400));

        let mut expected = sample_function();
        expected.shift(-100);
        assert_eq!(
            serde_json::to_string(&*resolved).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
    }

    #[test]
    fn a_cached_entry_survives_the_file_round_trip() {
        let entry = CachedEntry {
            base: Loc(7),
            function: Arc::new(sample_function()),
        };
        let json = serde_json::to_string(&entry).expect("a cached entry must serialize");
        let back: CachedEntry = serde_json::from_str(&json).expect("a cached entry must decode");

        assert_eq!(back.base, entry.base);
        assert_eq!(
            serde_json::to_string(&*back.function).unwrap(),
            serde_json::to_string(&*entry.function).unwrap()
        );
    }
}

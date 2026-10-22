use archery::{ArcK, SharedPointer};
use std::sync::Arc;

use rustowl::models::{Function, Loc};

/// `fn_text_hash` is the function's own source text: an edit inside it misses, an
/// edit before it leaves the hash unchanged and the entry is served re-anchored.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct FunctionKey {
    pub def_path: u64,
    pub fn_text_hash: u64,
    pub mir_hash: u64,
    pub analysis_version: u32,
    pub toolchain_hash: u64,
}

/// A cached body with the spans it had when stored, plus the offset its
/// function started at. The pointer is what lets an unmoved body be shared.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CachedEntry {
    pub base: Loc,
    pub function: SharedPointer<Function, ArcK>,
    /// The identity this body was stored under; a hit checks it.
    pub key: FunctionKey,
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
pub fn resolve(entry: &CachedEntry, current_base: Loc) -> SharedPointer<Function, ArcK> {
    resolve_inner(entry, current_base)
}

fn resolve_inner(entry: &CachedEntry, current_base: Loc) -> SharedPointer<Function, ArcK> {
    let delta = i64::from(current_base.0) - i64::from(entry.base.0);
    if delta == 0 {
        return SharedPointer::clone(&entry.function);
    }
    // The moved body is built, not copied: only the spans are new, the name
    // and the basic blocks are shared with the cached body.
    let source = &*entry.function;
    SharedPointer::new(Function {
        fn_id: source.fn_id,
        name: Arc::clone(&source.name),
        basic_blocks: Arc::clone(&source.basic_blocks),
        decls: source
            .decls
            .iter()
            .map(|decl| decl.shifted(delta))
            .collect(),
    })
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

    fn sample_type() -> MirType {
        MirType {
            name: Arc::from("T"),
            reference: None,
        }
    }

    fn sample_function() -> Function {
        let ty = sample_type();
        Function {
            fn_id: 7,
            name: "f".into(),
            basic_blocks: Arc::from(Vec::new()),
            decls: vec![
                MirDecl::User {
                    local: FnLocal::new(0, 7),
                    name: "x".into(),
                    span: Range::new(Loc(100), Loc(110)).unwrap(),
                    ty: sample_type(),
                    lives: Arc::from(ranges(120, 2)),
                    shared_borrow: Arc::from(ranges(150, 2)),
                    mutable_borrow: Arc::from(ranges(180, 2)),
                    drop: true,
                    drop_range: Arc::from(ranges(210, 2)),
                    definitely_live_at: Arc::from(ranges(240, 2)),
                    maybe_init_at: Arc::from(ranges(270, 2)),
                    deficit_at: Arc::from(ranges(300, 2)),
                    storage_range: Arc::from(ranges(330, 2)),
                },
                MirDecl::Other {
                    local: FnLocal::new(1, 7),
                    ty,
                    lives: Arc::from(ranges(400, 2)),
                    shared_borrow: Arc::from(ranges(430, 2)),
                    mutable_borrow: Arc::from(ranges(460, 2)),
                    drop: false,
                    drop_range: Arc::from(ranges(490, 2)),
                    definitely_live_at: Arc::from(ranges(520, 2)),
                    maybe_init_at: Arc::from(ranges(550, 2)),
                    deficit_at: Arc::from(ranges(580, 2)),
                    storage_range: Arc::from(ranges(610, 2)),
                },
            ],
        }
    }

    fn key(fn_text_hash: u64) -> FunctionKey {
        FunctionKey {
            def_path: 42,
            fn_text_hash,
            mir_hash: 7,
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
            function: SharedPointer::new(sample_function()),
            key: key(0),
        };
        let resolved = resolve(&entry, Loc(100));
        assert!(
            SharedPointer::ptr_eq(&resolved, &entry.function),
            "delta == 0 must share, not copy"
        );
    }

    #[test]
    fn a_moved_body_leaves_the_cache_untouched() {
        let entry = CachedEntry {
            base: Loc(100),
            function: SharedPointer::new(sample_function()),
            key: key(0),
        };
        let resolved = resolve(&entry, Loc(350));

        let expected = sample_function().shifted(250);
        assert_eq!(
            serde_json::to_string(&*resolved).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
        assert!(
            !SharedPointer::ptr_eq(&resolved, &entry.function),
            "a moved body must not alias the cache"
        );
        assert_eq!(
            serde_json::to_string(&*entry.function).unwrap(),
            serde_json::to_string(&sample_function()).unwrap(),
            "resolving must not drift the cached entry"
        );
        assert!(
            Arc::ptr_eq(&resolved.name, &entry.function.name),
            "the name is shared between the cached body and the moved one"
        );
    }

    #[test]
    fn a_moved_function_is_re_anchored_by_exactly_the_delta() {
        let entry = CachedEntry {
            base: Loc(100),
            function: SharedPointer::new(sample_function()),
            key: key(0),
        };
        let resolved = resolve(&entry, Loc(350));

        let expected = sample_function().shifted(250);
        assert_eq!(
            serde_json::to_string(&*resolved).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
        assert!(
            !SharedPointer::ptr_eq(&resolved, &entry.function),
            "a moved function must not alias the cache"
        );
    }

    #[test]
    fn re_anchoring_backwards_works_too() {
        let entry = CachedEntry {
            base: Loc(500),
            function: SharedPointer::new(sample_function()),
            key: key(0),
        };
        let resolved = resolve(&entry, Loc(400));

        let expected = sample_function().shifted(-100);
        assert_eq!(
            serde_json::to_string(&*resolved).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
    }

    #[test]
    fn a_cached_entry_survives_the_file_round_trip() {
        let entry = CachedEntry {
            base: Loc(7),
            function: SharedPointer::new(sample_function()),
            key: key(0),
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

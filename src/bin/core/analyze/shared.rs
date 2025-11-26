//! Shared analysis helpers extracted from MIR analyze pipeline.
use rustc_middle::mir::BasicBlock;
use rustc_span::Span;
use rustowl::models::{Loc, Range};

/// Construct a `Range` from a rustc `Span` relative to file offset.
pub fn range_from_span(source: &str, span: Span, offset: u32) -> Option<Range> {
    let from = Loc::new(source, span.lo().0, offset);
    let until = Loc::new(source, span.hi().0, offset);
    Range::new(from, until)
}

/// Sort (BasicBlock, index) pairs by block then index.
pub fn sort_locs(v: &mut [(BasicBlock, usize)]) {
    v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
}

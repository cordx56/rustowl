//! Shared analysis helpers extracted from MIR analyze pipeline.
use rustowl::models::{Loc, Range};
use rustc_span::Span;
use rustc_middle::mir::BasicBlock;

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

/// Decide the effective lifetime set to visualize: if variable is dropped use drop range else lives.
pub fn effective_live(is_drop: bool, lives: Vec<Range>, drop_range: Vec<Range>) -> Vec<Range> {
    if is_drop { drop_range } else { lives }
}

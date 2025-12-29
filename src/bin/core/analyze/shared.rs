//! Shared analysis helpers extracted from MIR analyze pipeline.
use rustc_middle::mir::BasicBlock;
use rustc_span::Span;
use rustowl::models::Range;
use rustowl::utils::NormalizedByteCharIndex;

pub fn range_from_span_indexed(
    index: &NormalizedByteCharIndex,
    span: Span,
    offset: u32,
) -> Option<Range> {
    let from = index.loc_from_byte_pos(span.lo().0, offset);
    let until = index.loc_from_byte_pos(span.hi().0, offset);
    Range::new(from, until)
}

/// Sort (BasicBlock, index) pairs by block then index.
pub fn sort_locs(v: &mut [(BasicBlock, usize)]) {
    v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_locs_sorts_by_block_then_statement_index() {
        let mut locs = vec![
            (BasicBlock::from_u32(2), 1),
            (BasicBlock::from_u32(1), 99),
            (BasicBlock::from_u32(1), 3),
            (BasicBlock::from_u32(0), 5),
            (BasicBlock::from_u32(2), 0),
        ];

        sort_locs(&mut locs);

        assert_eq!(
            locs,
            vec![
                (BasicBlock::from_u32(0), 5),
                (BasicBlock::from_u32(1), 3),
                (BasicBlock::from_u32(1), 99),
                (BasicBlock::from_u32(2), 0),
                (BasicBlock::from_u32(2), 1),
            ]
        );
    }

    #[test]
    fn range_from_span_indexed_handles_offset_and_unicode() {
        use rustc_span::{BytePos, Span};

        // 'aé' => byte offsets: a(0..1), é(1..3), b(3..4)
        let src = "aéb";
        let index = NormalizedByteCharIndex::new(src);

        let span = Span::with_root_ctxt(BytePos(1), BytePos(3));
        let range = range_from_span_indexed(&index, span, 0).expect("valid range");
        assert_eq!(u32::from(range.from()), 1);
        assert_eq!(u32::from(range.until()), 2);

        let span_with_offset = Span::with_root_ctxt(BytePos(3), BytePos(4));
        let range = range_from_span_indexed(&index, span_with_offset, 1).expect("valid range");
        assert_eq!(u32::from(range.from()), 1);
        assert_eq!(u32::from(range.until()), 2);
    }
}

//! Utility functions for range manipulation and MIR analysis.
//!
//! This module provides core algorithms for working with source code ranges,
//! merging overlapping ranges, and providing visitor patterns for MIR traversal.

use crate::models::range_vec_into_vec;
use crate::models::*;

/// Determines if one range completely contains another range.
///
/// A range `r1` is a super range of `r2` if `r1` completely encompasses `r2`.
/// This means `r1` starts before or at the same position as `r2` and ends
/// after or at the same position as `r2`, with at least one strict inequality.
pub fn is_super_range(r1: Range, r2: Range) -> bool {
    (r1.from() < r2.from() && r2.until() <= r1.until())
        || (r1.from() <= r2.from() && r2.until() < r1.until())
}

/// Finds the overlapping portion of two ranges.
///
/// Returns the intersection of two ranges if they overlap, or `None` if
/// they don't intersect.
pub fn common_range(r1: Range, r2: Range) -> Option<Range> {
    if r2.from() < r1.from() {
        return common_range(r2, r1);
    }
    if r1.until() < r2.from() {
        return None;
    }
    let from = r2.from();
    let until = r1.until().min(r2.until());
    Range::new(from, until)
}

/// Finds all pairwise intersections among a collection of ranges.
///
/// Returns a vector of ranges representing all overlapping regions
/// between pairs of input ranges, with overlapping regions merged.
pub fn common_ranges(ranges: &[Range]) -> Vec<Range> {
    let mut common_ranges = Vec::new();
    for i in 0..ranges.len() {
        for j in i + 1..ranges.len() {
            if let Some(common) = common_range(ranges[i], ranges[j]) {
                common_ranges.push(common);
            }
        }
    }
    eliminated_ranges(common_ranges)
}

/// Merges two ranges into their superset if they overlap or are adjacent.
///
/// Returns a single range that encompasses both input ranges if they
/// overlap or are directly adjacent. Returns `None` if they are disjoint.
pub fn merge_ranges(r1: Range, r2: Range) -> Option<Range> {
    if common_range(r1, r2).is_some() || r1.until() == r2.from() || r2.until() == r1.from() {
        let from = r1.from().min(r2.from());
        let until = r1.until().max(r2.until());
        Range::new(from, until)
    } else {
        None
    }
}

/// Eliminates overlapping and adjacent ranges by merging them.
///
/// Optimized implementation: O(n log n) sort + linear merge instead of
/// the previous O(n^2) pairwise merging loop. Keeps behavior identical.
pub fn eliminated_ranges(mut ranges: Vec<Range>) -> Vec<Range> {
    if ranges.len() <= 1 {
        return ranges;
    }
    // Sort by start, then end
    ranges.sort_by_key(|r| (r.from().0, r.until().0));
    let mut merged: Vec<Range> = Vec::with_capacity(ranges.len());
    let mut current = ranges[0];
    for r in ranges.into_iter().skip(1) {
        if r.from().0 <= current.until().0 || r.from().0 == current.until().0 {
            // Overlapping or adjacent
            if r.until().0 > current.until().0 {
                current = Range::new(current.from(), r.until()).unwrap();
            }
        } else {
            merged.push(current);
            current = r;
        }
    }
    merged.push(current);
    merged
}

/// Version of [`eliminated_ranges`] that works with SmallVec.
pub fn eliminated_ranges_small(ranges: RangeVec) -> Vec<Range> {
    eliminated_ranges(range_vec_into_vec(ranges))
}

/// Subtracts exclude ranges from a set of ranges.
///
/// For each range in `from`, removes any portions that overlap with
/// ranges in `excludes`. If a range is partially excluded, it may be
/// split into multiple smaller ranges.
pub fn exclude_ranges(from: Vec<Range>, excludes: Vec<Range>) -> Vec<Range> {
    let mut from = from;
    let mut i = 0;
    'outer: while i < from.len() {
        let mut j = 0;
        while j < excludes.len() {
            if let Some(common) = common_range(from[i], excludes[j]) {
                if let Some(r) = Range::new(from[i].from(), common.from() - 1) {
                    from.push(r);
                }
                if let Some(r) = Range::new(common.until() + 1, from[i].until()) {
                    from.push(r);
                }
                from.remove(i);
                continue 'outer;
            }
            j += 1;
        }
        i += 1;
    }
    eliminated_ranges(from)
}

/// Version of [`exclude_ranges`] that works with SmallVec.
pub fn exclude_ranges_small(from: RangeVec, excludes: Vec<Range>) -> Vec<Range> {
    exclude_ranges(range_vec_into_vec(from), excludes)
}

/// Visitor trait for traversing MIR (Mid-level IR) structures.
///
/// Provides a flexible pattern for implementing analysis passes over
/// MIR functions by visiting different components in a structured way.
pub trait MirVisitor {
    /// Called when visiting a function.
    fn visit_func(&mut self, _func: &Function) {}
    /// Called when visiting a variable declaration.
    fn visit_decl(&mut self, _decl: &MirDecl) {}
    /// Called when visiting a statement.
    fn visit_stmt(&mut self, _stmt: &MirStatement) {}
    /// Called when visiting a terminator.
    fn visit_term(&mut self, _term: &MirTerminator) {}
}

/// Traverses a MIR function using the visitor pattern.
///
/// Calls the appropriate visitor methods for each component of the function
/// in a structured order: function, declarations, statements, terminators.
pub fn mir_visit(func: &Function, visitor: &mut impl MirVisitor) {
    visitor.visit_func(func);
    for decl in &func.decls {
        visitor.visit_decl(decl);
    }
    for bb in &func.basic_blocks {
        for stmt in &bb.statements {
            visitor.visit_stmt(stmt);
        }
        if let Some(term) = &bb.terminator {
            visitor.visit_term(term);
        }
    }
}

/// Converts a character index to line and column numbers.
///
/// Given a source string and character index, returns the corresponding
/// line and column position. Handles CR characters consistently with
/// the Rust compiler by ignoring them.
pub fn index_to_line_char(s: &str, idx: Loc) -> (u32, u32) {
    use memchr::memchr_iter;
    let target = idx.0;
    let mut line = 0u32;
    let mut col = 0u32;
    let mut logical_idx = 0u32; // counts chars excluding CR
    let mut seg_start = 0usize;

    // Scan newline boundaries quickly, counting chars inside each segment.
    for nl in memchr_iter(b'\n', s.as_bytes()) {
        for ch in s[seg_start..=nl].chars() {
            if ch == '\r' {
                continue;
            }
            if logical_idx == target {
                return (line, col);
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
            logical_idx += 1;
        }
        seg_start = nl + 1;
        if logical_idx > target {
            break;
        }
    }
    if logical_idx <= target {
        for ch in s[seg_start..].chars() {
            if ch == '\r' {
                continue;
            }
            if logical_idx == target {
                return (line, col);
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
            logical_idx += 1;
        }
    }
    (line, col)
}

/// Converts line and column numbers to a character index.
///
/// Given a source string, line number, and column number, returns the
/// corresponding character index. Handles CR characters consistently
/// with the Rust compiler by ignoring them.
pub fn line_char_to_index(s: &str, mut line: u32, char: u32) -> u32 {
    use memchr::memchr_iter;
    let mut consumed = 0u32; // logical chars excluding CR
    let mut seg_start = 0usize;

    for nl in memchr_iter(b'\n', s.as_bytes()) {
        if line == 0 {
            break;
        }
        for ch in s[seg_start..=nl].chars() {
            if ch == '\r' {
                continue;
            }
            consumed += 1;
        }
        seg_start = nl + 1;
        line -= 1;
    }

    if line > 0 {
        for ch in s[seg_start..].chars() {
            if ch == '\r' {
                continue;
            }
            consumed += 1;
        }
        return consumed; // best effort if line exceeds file
    }

    let mut col_count = 0u32;
    for ch in s[seg_start..].chars() {
        if ch == '\r' {
            continue;
        }
        if col_count == char {
            return consumed;
        }
        if ch == '\n' {
            return consumed;
        }
        consumed += 1;
        col_count += 1;
    }
    consumed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_super_range() {
        let r1 = Range::new(Loc(0), Loc(10)).unwrap();
        let r2 = Range::new(Loc(2), Loc(8)).unwrap();
        let r3 = Range::new(Loc(5), Loc(15)).unwrap();

        assert!(is_super_range(r1, r2)); // r1 contains r2
        assert!(!is_super_range(r2, r1)); // r2 doesn't contain r1
        assert!(!is_super_range(r1, r3)); // r1 doesn't fully contain r3
        assert!(!is_super_range(r3, r1)); // r3 doesn't contain r1
    }

    #[test]
    fn test_common_range() {
        let r1 = Range::new(Loc(0), Loc(10)).unwrap();
        let r2 = Range::new(Loc(5), Loc(15)).unwrap();
        let r3 = Range::new(Loc(20), Loc(30)).unwrap();

        // Overlapping ranges
        let common = common_range(r1, r2).unwrap();
        assert_eq!(common.from(), Loc(5));
        assert_eq!(common.until(), Loc(10));

        // Non-overlapping ranges
        assert!(common_range(r1, r3).is_none());

        // Order shouldn't matter
        let common2 = common_range(r2, r1).unwrap();
        assert_eq!(common, common2);
    }

    #[test]
    fn test_merge_ranges() {
        let r1 = Range::new(Loc(0), Loc(10)).unwrap();
        let r2 = Range::new(Loc(5), Loc(15)).unwrap();
        let r3 = Range::new(Loc(10), Loc(20)).unwrap(); // Adjacent
        let r4 = Range::new(Loc(25), Loc(30)).unwrap(); // Disjoint

        // Overlapping ranges should merge
        let merged = merge_ranges(r1, r2).unwrap();
        assert_eq!(merged.from(), Loc(0));
        assert_eq!(merged.until(), Loc(15));

        // Adjacent ranges should merge
        let merged = merge_ranges(r1, r3).unwrap();
        assert_eq!(merged.from(), Loc(0));
        assert_eq!(merged.until(), Loc(20));

        // Disjoint ranges shouldn't merge
        assert!(merge_ranges(r1, r4).is_none());
    }

    #[test]
    fn test_eliminated_ranges() {
        let ranges = vec![
            Range::new(Loc(0), Loc(10)).unwrap(),
            Range::new(Loc(5), Loc(15)).unwrap(),
            Range::new(Loc(12), Loc(20)).unwrap(),
            Range::new(Loc(25), Loc(30)).unwrap(),
        ];

        let eliminated = eliminated_ranges(ranges);
        assert_eq!(eliminated.len(), 2);

        // Should have merged the overlapping ranges
        assert!(
            eliminated
                .iter()
                .any(|r| r.from() == Loc(0) && r.until() == Loc(20))
        );
        assert!(
            eliminated
                .iter()
                .any(|r| r.from() == Loc(25) && r.until() == Loc(30))
        );
    }

    #[test]
    fn test_exclude_ranges() {
        let from = vec![Range::new(Loc(0), Loc(20)).unwrap()];
        let excludes = vec![Range::new(Loc(5), Loc(15)).unwrap()];

        let result = exclude_ranges(from, excludes);

        // Should split the original range around the exclusion
        assert_eq!(result.len(), 2);
        assert!(
            result
                .iter()
                .any(|r| r.from() == Loc(0) && r.until() == Loc(4))
        );
        assert!(
            result
                .iter()
                .any(|r| r.from() == Loc(16) && r.until() == Loc(20))
        );
    }

    #[test]
    fn test_index_to_line_char() {
        let source = "hello\nworld\ntest";

        assert_eq!(index_to_line_char(source, Loc(0)), (0, 0)); // 'h'
        assert_eq!(index_to_line_char(source, Loc(6)), (1, 0)); // 'w'
        assert_eq!(index_to_line_char(source, Loc(12)), (2, 0)); // 't'
    }

    #[test]
    fn test_line_char_to_index() {
        let source = "hello\nworld\ntest";

        assert_eq!(line_char_to_index(source, 0, 0), 0); // 'h'
        assert_eq!(line_char_to_index(source, 1, 0), 6); // 'w'  
        assert_eq!(line_char_to_index(source, 2, 0), 12); // 't'
    }

    #[test]
    fn test_index_line_char_roundtrip() {
        let source = "hello\nworld\ntest\nwith unicode: 🦀";

        for i in 0..source.chars().count() {
            let loc = Loc(i as u32);
            let (line, char) = index_to_line_char(source, loc);
            let back_to_index = line_char_to_index(source, line, char);
            assert_eq!(loc.0, back_to_index);
        }
    }
}

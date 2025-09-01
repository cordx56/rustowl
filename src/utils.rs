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
/// Takes a vector of ranges and repeatedly merges overlapping or adjacent
/// ranges until no more merges are possible, returning the minimal set
/// of non-overlapping ranges.
pub fn eliminated_ranges(ranges: Vec<Range>) -> Vec<Range> {
    let mut ranges = ranges;
    let mut i = 0;
    'outer: while i < ranges.len() {
        let mut j = 0;
        while j < ranges.len() {
            if i != j
                && let Some(merged) = merge_ranges(ranges[i], ranges[j])
            {
                ranges[i] = merged;
                ranges.remove(j);
                continue 'outer;
            }
            j += 1;
        }
        i += 1;
    }
    ranges
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
    let mut line = 0;
    let mut col = 0;
    // it seems that the compiler is ignoring CR
    for (i, c) in s.replace("\r", "").chars().enumerate() {
        if idx == Loc::from(i as u32) {
            return (line, col);
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else if c != '\r' {
            col += 1;
        }
    }
    (0, 0)
}

/// Converts line and column numbers to a character index.
///
/// Given a source string, line number, and column number, returns the
/// corresponding character index. Handles CR characters consistently
/// with the Rust compiler by ignoring them.
pub fn line_char_to_index(s: &str, mut line: u32, char: u32) -> u32 {
    let mut col = 0;
    // it seems that the compiler is ignoring CR
    for (i, c) in s.replace("\r", "").chars().enumerate() {
        if line == 0 && col == char {
            return i as u32;
        }
        if c == '\n' && 0 < line {
            line -= 1;
            col = 0;
        } else if c != '\r' {
            col += 1;
        }
    }
    0
}

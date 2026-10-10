use crate::models::*;

pub fn is_super_range(r1: Range, r2: Range) -> bool {
    (r1.from() < r2.from() && r2.until() <= r1.until())
        || (r1.from() <= r2.from() && r2.until() < r1.until())
}

pub fn common_range(r1: Range, r2: Range) -> Option<Range> {
    if r2.from() < r1.from() {
        return common_range(r2, r1);
    }
    if r1.until() <= r2.from() {
        return None;
    }
    let from = r2.from();
    let until = r1.until().min(r2.until());
    Range::new(from, until)
}

/// Intersections of every pair, flattened.
pub fn common_ranges(ranges: &[Range]) -> Vec<Range> {
    if ranges.len() < 2 {
        return Vec::new();
    }
    // (position, +1 at a range start, -1 at its end)
    let mut events: Vec<(u32, i8)> = Vec::with_capacity(ranges.len() * 2);
    for range in ranges {
        events.push((range.from().0, 1));
        events.push((range.until().0, -1));
    }
    events.sort_by_key(|(pos, delta)| (*pos, *delta));

    let mut result: Vec<Range> = Vec::new();
    let mut covering = 0i8;
    let mut start = 0u32;
    let mut open = false;
    for (pos, delta) in events {
        let before = covering;
        covering += delta;
        if before >= 2 && covering < 2 {
            if let Some(range) = Range::new(Loc(start), Loc(pos)) {
                result.push(range);
            }
            open = false;
        } else if covering >= 2 && !open {
            start = pos;
            open = true;
        }
    }
    eliminated_ranges(result)
}

/// merge two ranges, result is superset of two ranges
pub fn merge_ranges(r1: Range, r2: Range) -> Option<Range> {
    if common_range(r1, r2).is_some() || r1.until() == r2.from() || r2.until() == r1.from() {
        let from = r1.from().min(r2.from());
        let until = r1.until().max(r2.until());
        Range::new(from, until)
    } else {
        None
    }
}

/// eliminate common ranges and flatten ranges
///
/// Sorting by start position and sweeping once reaches the same partition the
/// pairwise merge did: any two ranges that overlap or abut land in the same
/// output group, so repeated merging collapses to a single linear pass.
pub fn eliminated_ranges(mut ranges: Vec<Range>) -> Vec<Range> {
    ranges.sort_by_key(|r| (r.from().0, r.until().0));
    let mut result: Vec<Range> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match result.last_mut() {
            // abutting ranges merge too, as merge_ranges did
            Some(last) if last.from() <= range.from() && range.from() <= last.until() => {
                if let Some(merged) = merge_ranges(*last, range) {
                    *last = merged;
                }
            }
            _ => result.push(range),
        }
    }
    result
}

/// Compute intersection of two range lists.
/// Returns ranges that are covered by both lists.
pub fn intersect_ranges(ranges1: Vec<Range>, ranges2: Vec<Range>) -> Vec<Range> {
    let mut result = Vec::new();
    for r1 in &ranges1 {
        for r2 in &ranges2 {
            if let Some(common) = common_range(*r1, *r2) {
                result.push(common);
            }
        }
    }
    eliminated_ranges(result)
}

/// Compute union of two range lists.
/// Returns ranges that are covered by either list.
pub fn union_ranges(ranges1: Vec<Range>, ranges2: Vec<Range>) -> Vec<Range> {
    let mut combined = ranges1;
    combined.extend(ranges2);
    eliminated_ranges(combined)
}

pub fn exclude_ranges(mut from: Vec<Range>, excludes: Vec<Range>) -> Vec<Range> {
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

pub trait MirVisitor {
    fn visit_func(&mut self, _: &Function) {}
    fn visit_decl(&mut self, _: &MirDecl) {}
    fn visit_stmt(&mut self, _: &MirStatement) {}
    fn visit_term(&mut self, _: &MirTerminator) {}
}
pub fn mir_visit(func: &Function, visitor: &mut impl MirVisitor) {
    visitor.visit_func(func);
    for decl in &func.decls {
        visitor.visit_decl(decl);
    }
    for bb in &func.basic_blocks {
        for stmt in &bb.statements {
            visitor.visit_stmt(stmt);
        }
        visitor.visit_term(&bb.terminator);
    }
}

pub fn is_source_clean(s: &str) -> bool {
    !s.contains('\r')
}
pub fn clean_source(s: &str) -> String {
    if is_source_clean(s) {
        s.to_string()
    } else {
        s.replace('\r', "")
    }
}

pub fn range_is_multiline(s: &str, range: Range) -> bool {
    SourceIndex::new(s).is_multiline(&range)
}

pub fn index_to_line_char(s: &str, idx: Loc) -> (u32, u32) {
    let mut cleaned = String::new();
    if !is_source_clean(s) {
        cleaned = clean_source(s);
    }
    let source_clean = if cleaned.is_empty() { s } else { &cleaned };

    let mut line = 0;
    let mut col = 0;
    for (i, c) in source_clean.chars().enumerate() {
        if idx == Loc::from(i as u32) {
            return (line, col);
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    // Return current position when idx equals the string length (end position)
    // or when idx is out of bounds
    (line, col)
}
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

#[cfg(test)]
mod tests {
    use super::*;

    fn r(a: u32, b: u32) -> Range {
        Range::new(Loc(a), Loc(b)).expect("test range must be non-empty")
    }

    #[test]
    fn range_new_rejects_empty_and_inverted() {
        assert!(Range::new(Loc(5), Loc(5)).is_none());
        assert!(Range::new(Loc(9), Loc(2)).is_none());
        assert!(Range::new(Loc(0), Loc(1)).is_some());
    }

    #[test]
    fn merge_ranges_merges_overlap_containment_and_adjacency() {
        assert_eq!(merge_ranges(r(0, 5), r(3, 8)), Some(r(0, 8)));
        assert_eq!(merge_ranges(r(0, 10), r(2, 4)), Some(r(0, 10)));
        assert_eq!(merge_ranges(r(0, 3), r(3, 6)), Some(r(0, 6)));
    }

    #[test]
    fn merge_ranges_rejects_disjoint() {
        assert_eq!(merge_ranges(r(0, 3), r(5, 8)), None);
        assert_eq!(merge_ranges(r(0, 3), r(4, 8)), None);
    }

    #[test]
    fn eliminated_ranges_passes_through_trivial_input() {
        assert!(eliminated_ranges(vec![]).is_empty());
        assert_eq!(eliminated_ranges(vec![r(4, 9)]), vec![r(4, 9)]);
    }

    #[test]
    fn eliminated_ranges_keeps_disjoint_ranges() {
        assert_eq!(
            eliminated_ranges(vec![r(0, 3), r(6, 9)]),
            vec![r(0, 3), r(6, 9)]
        );
    }

    #[test]
    fn eliminated_ranges_flattens_overlaps() {
        assert_eq!(eliminated_ranges(vec![r(0, 5), r(3, 8)]), vec![r(0, 8)]);
        assert_eq!(eliminated_ranges(vec![r(0, 10), r(2, 4)]), vec![r(0, 10)]);
        assert_eq!(eliminated_ranges(vec![r(0, 3), r(3, 6)]), vec![r(0, 6)]);
    }

    #[test]
    fn eliminated_ranges_cascades_transitively() {
        assert_eq!(
            eliminated_ranges(vec![r(0, 3), r(2, 5), r(4, 7)]),
            vec![r(0, 7)]
        );
    }

    #[test]
    fn clean_source_strips_cr() {
        assert!(is_source_clean("a\nb"));
        assert!(!is_source_clean("a\r\nb"));
        assert_eq!(clean_source("a\r\nb"), "a\nb");
        assert_eq!(clean_source("a\nb"), "a\nb");
    }
}

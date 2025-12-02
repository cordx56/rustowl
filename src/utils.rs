use crate::models::range_vec_into_vec;
use crate::models::*;

pub fn is_super_range(r1: Range, r2: Range) -> bool {
    (r1.from() < r2.from() && r2.until() <= r1.until())
        || (r1.from() <= r2.from() && r2.until() < r1.until())
}

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

/// Version of eliminated_ranges that works with SmallVec
pub fn eliminated_ranges_small(ranges: RangeVec) -> Vec<Range> {
    eliminated_ranges(range_vec_into_vec(ranges))
}

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

/// Version of exclude_ranges that works with SmallVec
pub fn exclude_ranges_small(from: RangeVec, excludes: Vec<Range>) -> Vec<Range> {
    exclude_ranges(range_vec_into_vec(from), excludes)
}

#[allow(unused)]
pub trait MirVisitor {
    fn visit_func(&mut self, func: &Function) {}
    fn visit_decl(&mut self, decl: &MirDecl) {}
    fn visit_stmt(&mut self, stmt: &MirStatement) {}
    fn visit_term(&mut self, term: &MirTerminator) {}
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
        if let Some(term) = &bb.terminator {
            visitor.visit_term(term);
        }
    }
}

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

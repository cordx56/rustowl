use crate::models::*;
use std::path::PathBuf;
use tower_lsp_server::gen_lsp_types::Uri;
use url::Url;

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
pub fn eliminated_ranges(mut ranges: Vec<Range>) -> Vec<Range> {
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
        visitor.visit_term(&bb.terminator);
    }
}

pub fn is_source_clean(s: &str) -> bool {
    !s.contains('\r')
}
pub fn clean_source(s: &str) -> String {
    if is_source_clean(s) {
        // it seems that the compiler is ignoring CR
        s.replace('\r', "")
    } else {
        s.to_string()
    }
}

pub fn range_is_multiline(s: &str, range: Range) -> bool {
    let mut cleaned = String::new();
    if !is_source_clean(s) {
        cleaned = clean_source(s);
    }
    let source_clean = if cleaned.is_empty() { s } else { &cleaned };

    let from = range.from().0 as usize;
    let until = range.until().0 as usize;
    source_clean
        .chars()
        .enumerate()
        .skip(from)
        .take(until - from)
        .any(|(_, c)| c == '\n')
}

/// Converts a `file:` URI from the client into a local path.
///
/// The `Uri` itself stays `fluent_uri::Uri`, so the protocol boundary keeps
/// RFC 3986 parsing and validation. Turning a URI into a filesystem path is an
/// OS concern rather than a protocol one, so that half is delegated to `url`,
/// which already handles percent-decoding, Windows drive letters and UNC hosts.
pub fn uri_to_file_path(uri: &Uri) -> Option<PathBuf> {
    // `Url::to_file_path` does not check the scheme itself, and editors address
    // local-looking paths under other schemes (`untitled:`, `git:`), which would
    // otherwise convert happily. A scheme is ASCII, so this matches the
    // ASCII-case-insensitive comparison `fluent_uri` itself would do.
    if !uri.scheme().as_str().eq_ignore_ascii_case("file") {
        return None;
    }
    let url = Url::parse(uri.as_str()).ok()?;
    if url
        .host_str()
        .is_some_and(|host| !host.eq_ignore_ascii_case("localhost"))
    {
        return None;
    }
    url.to_file_path().ok()
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
mod uri_tests {
    use super::*;

    fn parse(uri: &str) -> Uri {
        uri.parse()
            .expect("test URI should be a valid RFC 3986 URI")
    }

    #[test]
    fn converts_plain_file_uri() {
        assert_eq!(
            uri_to_file_path(&parse("file:///home/user/project")),
            Some(PathBuf::from("/home/user/project"))
        );
    }

    #[test]
    fn percent_decodes_the_path() {
        assert_eq!(
            uri_to_file_path(&parse("file:///home/user/my%20project/a%2Bb.rs")),
            Some(PathBuf::from("/home/user/my project/a+b.rs"))
        );
    }

    #[test]
    fn accepts_localhost_authority() {
        assert_eq!(
            uri_to_file_path(&parse("file://localhost/home/user")),
            Some(PathBuf::from("/home/user"))
        );
    }

    #[test]
    fn rejects_non_file_scheme() {
        // These have no host and an absolute path, so only an explicit scheme
        // check keeps them from being read as local directories.
        assert_eq!(uri_to_file_path(&parse("untitled:///home/user")), None);
        assert_eq!(uri_to_file_path(&parse("git:///home/user")), None);
        assert_eq!(uri_to_file_path(&parse("https://example.com/x")), None);
    }

    #[test]
    fn rejects_remote_authority() {
        // `file://host/share` is a share on another machine, not a local path,
        // so treating it as local would silently analyse the wrong directory.
        assert_eq!(
            uri_to_file_path(&parse("file://server/share/project")),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_requires_a_drive_letter() {
        assert_eq!(
            uri_to_file_path(&parse("file:///C:/Users/test")),
            Some(PathBuf::from(r"C:\Users\test"))
        );
        assert_eq!(uri_to_file_path(&parse("file:///home/user")), None);
    }
}

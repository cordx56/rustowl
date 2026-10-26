#![allow(unused)]

use archery::{ArcK, SharedPointer, SharedPointerKind};
use rpds::{HashTrieMap, Vector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FnLocal {
    pub id: u32,
    pub fn_id: u32,
}

impl FnLocal {
    pub fn new(id: u32, fn_id: u32) -> Self {
        Self { id, fn_id }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct Loc(pub u32);
impl Loc {
    /// Move by a signed delta, saturating so a bad delta cannot wrap into a
    /// huge offset that would silently misplace every span.
    fn translate(self, delta: i64) -> Self {
        let moved = i64::from(self.0) + delta;
        Self(moved.clamp(0, i64::from(u32::MAX)) as u32)
    }

    pub fn new(source: &str, byte_pos: u32, offset: u32) -> Self {
        let byte_pos = byte_pos.saturating_sub(offset) as usize;
        let byte_pos = if byte_pos >= source.len() {
            source.len()
        } else {
            let mut byte_pos = byte_pos;
            while !source.is_char_boundary(byte_pos) {
                byte_pos -= 1;
            }
            byte_pos
        };
        Self(source[..byte_pos].chars().filter(|&c| c != '\r').count() as u32)
    }
}

/// CR-cleaned source plus a line-start table, built once per file.
///
/// A line index over CR-stripped source, built once per file.
///
/// Note the offset contract: `Loc` is a count of characters in the **raw**
/// source with CRs skipped, not an index into `cleaned()`. Span offsets are
/// byte positions in the raw source. The two differ by one per preceding CR,
/// so converting a span needs a table built over the raw text, not this one.
pub struct SourceIndex {
    cleaned: String,
    /// char index of the first character of each line
    line_starts: Vec<usize>,
    /// byte offset in the **raw** source -> number of non-CR chars before it,
    /// which is what `Loc` holds
    raw_offsets: Vec<u32>,
}
impl SourceIndex {
    pub fn new(raw: &str) -> Self {
        let cleaned = if raw.contains('\r') {
            raw.replace('\r', "")
        } else {
            raw.to_string()
        };
        let mut line_starts = vec![0];
        for (index, c) in cleaned.chars().enumerate() {
            if c == '\n' {
                line_starts.push(index + 1);
            }
        }
        // Loc counts non-CR chars in the raw source, so the table for span
        // conversion is built over `raw`, not over `cleaned`
        let mut raw_offsets = vec![0u32; raw.len() + 1];
        let mut count = 0;
        for (byte, c) in raw.char_indices() {
            let len = c.len_utf8();
            for slot in &mut raw_offsets[byte..byte + len] {
                *slot = count;
            }
            if c != '\r' {
                count += 1;
            }
        }
        *raw_offsets.last_mut().unwrap() = count;
        Self {
            cleaned,
            line_starts,
            raw_offsets,
        }
    }

    /// The `Loc` for a byte offset into the raw source.
    ///
    /// Matches [`Loc::new`] with a zero offset, including its walk back to a
    /// character boundary: every byte inside a character maps to that
    /// character's start, which is what the walk-back lands on.
    pub fn loc_at(&self, byte: usize) -> Loc {
        match self.raw_offsets.get(byte) {
            Some(count) => Loc(*count),
            None => Loc(*self.raw_offsets.last().unwrap()),
        }
    }

    pub fn cleaned(&self) -> &str {
        &self.cleaned
    }

    /// `(line, char)` for a character offset into the cleaned source.
    pub fn locate(&self, char_pos: u32) -> (usize, usize) {
        let pos = char_pos as usize;
        let line = match self.line_starts.binary_search(&pos) {
            Ok(line) => line,
            Err(next) => next - 1,
        };
        (line, pos - self.line_starts[line])
    }

    pub fn line_text(&self, line: usize) -> &str {
        let Some(&from) = self.line_starts.get(line) else {
            return "";
        };
        let until = self
            .line_starts
            .get(line + 1)
            .map(|end| end - 1)
            .unwrap_or(self.cleaned.chars().count());
        let from = self
            .cleaned
            .char_indices()
            .nth(from)
            .map(|(i, _)| i)
            .unwrap_or(self.cleaned.len());
        let until = self
            .cleaned
            .char_indices()
            .nth(until)
            .map(|(i, _)| i)
            .unwrap_or(self.cleaned.len());
        if from >= until {
            return "";
        }
        self.cleaned[from..until].trim_end_matches('\n')
    }

    /// Whether the character range spans a line break.
    pub fn is_multiline(&self, range: &Range) -> bool {
        let (from, _) = self.locate(range.from().0);
        let (until, _) = self.locate(range.until().0);
        from != until
    }
}

impl std::ops::Add<i32> for Loc {
    type Output = Loc;
    fn add(self, rhs: i32) -> Self::Output {
        if rhs < 0 && (self.0 as i32) < -rhs {
            Loc(0)
        } else {
            Loc(self.0 + rhs as u32)
        }
    }
}

impl std::ops::Sub<i32> for Loc {
    type Output = Loc;
    fn sub(self, rhs: i32) -> Self::Output {
        if 0 < rhs && (self.0 as i32) < rhs {
            Loc(0)
        } else {
            Loc(self.0 - rhs as u32)
        }
    }
}

impl From<u32> for Loc {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Loc> for u32 {
    fn from(value: Loc) -> Self {
        value.0
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Range {
    from: Loc,
    until: Loc,
}

impl Range {
    pub fn new(from: Loc, until: Loc) -> Option<Self> {
        if until.0 <= from.0 {
            None
        } else {
            Some(Self { from, until })
        }
    }
    pub fn from(&self) -> Loc {
        self.from
    }
    pub fn until(&self) -> Loc {
        self.until
    }
    pub fn size(&self) -> u32 {
        self.until.0 - self.from.0
    }
    fn translate(&mut self, delta: i64) {
        self.from = self.from.translate(delta);
        self.until = self.until.translate(delta);
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirVariable {
    User {
        index: u32,
        live: Range,
        dead: Range,
    },
    Other {
        index: u32,
        live: Range,
        dead: Range,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(transparent)]
pub struct MirVariables(HashMap<u32, MirVariable>);

impl Default for MirVariables {
    fn default() -> Self {
        Self::new()
    }
}

impl MirVariables {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn push(&mut self, var: MirVariable) {
        match &var {
            MirVariable::User { index, .. } => {
                if !self.0.contains_key(index) {
                    self.0.insert(*index, var);
                }
            }
            MirVariable::Other { index, .. } => {
                if !self.0.contains_key(index) {
                    self.0.insert(*index, var);
                }
            }
        }
    }

    pub fn to_vec(self) -> Vec<MirVariable> {
        self.0.into_values().collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Item {
    Function { span: Range, mir: Function },
}

/// A file's analysed bodies, shared by pointer: one analysed body reaches the
/// cache, the wire and the renderer without a copy.
#[derive(Serialize, Deserialize, Debug)]
#[serde(bound = "")]
pub struct File<P: SharedPointerKind = ArcK> {
    pub items: Vector<SharedPointer<Function, P>, P>,
}

impl<P: SharedPointerKind> Default for File<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: SharedPointerKind> File<P> {
    pub fn new() -> Self {
        Self {
            items: Vector::default(),
        }
    }

    pub fn push(&self, function: SharedPointer<Function, P>) -> Self {
        Self {
            items: self.items.push_back(function),
        }
    }

    /// A new file with the body of this `fn_id` replaced, or appended when
    /// the file has no such body.
    pub fn replace(&self, fn_id: u32, function: SharedPointer<Function, P>) -> Self {
        let index = self.items.iter().position(|item| item.fn_id == fn_id);
        let items = match index {
            // The index came from this very vector, so `set` cannot fail.
            Some(index) => self
                .items
                .set(index, function)
                .expect("the index came from this vector"),
            None => self.items.push_back(function),
        };
        Self { items }
    }

    /// A new file with `other`'s bodies merged in: a body with the same
    /// `fn_id` replaces the one here, the rest are appended. `None` when
    /// `other` is empty.
    pub fn merged(&self, other: &Self) -> Option<Self> {
        if other.items.is_empty() {
            return None;
        }
        let mut merged: Option<Self> = None;
        for item in other.items.iter() {
            let next = match &merged {
                Some(file) => file.replace(item.fn_id, SharedPointer::clone(item)),
                None => self.replace(item.fn_id, SharedPointer::clone(item)),
            };
            merged = Some(next);
        }
        merged
    }
}

pub type CrateFiles<P> = HashTrieMap<Arc<str>, SharedPointer<File<P>, P>, P>;

/// The analysed files of one crate, keyed by path.
#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent, bound = "")]
pub struct Crate<P: SharedPointerKind = ArcK>(pub CrateFiles<P>);

impl<P: SharedPointerKind> Crate<P> {
    /// A crate with `other`'s files merged in: a body with the same `fn_id`
    /// replaces the one here, the rest are appended. `None` when `other` has
    /// no files.
    pub fn merged(&self, other: &Self) -> Option<Self> {
        if other.0.is_empty() {
            return None;
        }
        let mut files: Option<CrateFiles<P>> = None;
        for (file, incoming) in other.0.iter() {
            // The file as it stands now: the one already merged in, or the
            // one this crate already had.
            let current = match &files {
                Some(files) => files.get(file),
                None => self.0.get(file),
            };
            let merged = match current {
                Some(current) => match current.as_ref().merged(incoming.as_ref()) {
                    Some(merged) => SharedPointer::new(merged),
                    None => SharedPointer::clone(current),
                },
                None => SharedPointer::clone(incoming),
            };
            files = Some(match files {
                Some(files) => files.insert(Arc::clone(file), merged),
                None => self.0.insert(Arc::clone(file), merged),
            });
        }
        Some(Crate(files.unwrap_or_default()))
    }

    pub fn merge(&mut self, other: Self) {
        if other.0.is_empty() {
            return;
        }
        let merged = self
            .merged(&other)
            .expect("a crate with files has something to merge");
        *self = merged;
    }

    pub fn file(&self, path: &str) -> Option<&File<P>> {
        self.0.get(path).map(|file| file.as_ref())
    }

    pub fn without(&self, path: &str) -> Option<Self> {
        if !self.0.contains_key(path) {
            return None;
        }
        Some(Crate(self.0.remove(path)))
    }
}

/// The analysed crates of one workspace, keyed by crate name.
#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent, bound = "")]
pub struct Workspace<P: SharedPointerKind = ArcK>(
    pub HashTrieMap<Arc<str>, SharedPointer<Crate<P>, P>, P>,
);

impl<P: SharedPointerKind> Workspace<P> {
    pub fn merge(&mut self, other: Self) {
        for (name, incoming) in other.0.iter() {
            let merged = match self.0.get(name) {
                Some(existing) => match existing.as_ref().merged(incoming.as_ref()) {
                    Some(merged) => SharedPointer::new(merged),
                    None => SharedPointer::clone(existing),
                },
                None => SharedPointer::clone(incoming),
            };
            let next = self.0.insert(Arc::clone(name), merged);
            self.0 = next;
        }
    }

    pub fn get(&self, name: &str) -> Option<&Crate<P>> {
        self.0.get(name).map(|krate| krate.as_ref())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirProjectionElem {
    Deref,
    Field { index: usize },
    Index { local: FnLocal },
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MirPlace {
    pub local: FnLocal,
    pub projection: Vec<MirProjectionElem>,
}
impl MirPlace {
    pub fn root(local: FnLocal) -> Self {
        Self {
            local,
            projection: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirOperand {
    Copy { place: MirPlace },
    Move { place: MirPlace },
    // TODO: Constant, RuntimeChecks
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirRval {
    Use { operand: MirOperand },
    Repeat { operand: MirOperand },
    Ref { place: MirPlace, mutable: bool },
    Cast { operand: MirOperand },
    BinaryOp { left: MirOperand, right: MirOperand },
    UnaryOp { operand: MirOperand },
    Aggregate { fields: Vec<MirOperand> },
    // TODO: ThreadLocalRef, RawPtr, Discriminant, CopyForDeref, WrapUnsafeBinder, Reborrow
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct MirStatement {
    #[serde(flatten)]
    pub kind: MirStatementKind,
    pub range: Option<Range>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirStatementKind {
    Assign { place: MirPlace, rval: MirRval },
    StorageLive { local: FnLocal },
    StorageDead { local: FnLocal },
    Nop,
    // TODO: FakeRead, SetDiscriminant, PlaceMention, AscribeUserType, Coverage, ConstEvalCounter,
    // BackwardIncompatibleDropHint
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MirTerminator {
    #[serde(flatten)]
    pub kind: MirTerminatorKind,
    pub range: Option<Range>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirTerminatorKind {
    Goto {
        target: BasicBlockId,
    },
    SwitchInt {
        discr: MirOperand,
        targets: Vec<BasicBlockId>,
    },
    Return,
    Unreachable,
    Drop {
        place: MirPlace,
        target: BasicBlockId,
    },
    Call {
        func: MirOperand,
        args: Vec<MirOperand>,
        destination: MirPlace,
        target: Option<BasicBlockId>,
        fn_range: Option<Range>,
    },
    TailCall {
        func: MirOperand,
        args: Vec<MirOperand>,
        fn_range: Option<Range>,
    },
    Assert {
        cond: MirOperand,
        target: BasicBlockId,
    },
    // TODO: UnwindResume, UnwindTerminate, Yield, CoroutineDrop, FalseEdge, FalseUnwind, InlineAsm
    Other {
        successors: Vec<BasicBlockId>,
    },
}
impl MirTerminator {
    pub fn successors(&self) -> Vec<BasicBlockId> {
        match &self.kind {
            MirTerminatorKind::Goto { target } => vec![*target],
            MirTerminatorKind::SwitchInt { targets, .. } => targets.clone(),
            MirTerminatorKind::Drop { target, .. } => vec![*target],
            MirTerminatorKind::Call { target, .. } => (*target).into_iter().collect(),
            MirTerminatorKind::Assert { target, .. } => vec![*target],
            MirTerminatorKind::Other { successors } => successors.clone(),
            MirTerminatorKind::TailCall { .. }
            | MirTerminatorKind::Return
            | MirTerminatorKind::Unreachable => Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
#[serde(transparent)]
pub struct BasicBlockId(pub usize);
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MirBasicBlock {
    pub statements: Vec<MirStatement>,
    pub terminator: MirTerminator,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct MirRefType {
    pub refer_to: MirType,
    pub mutable: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct MirType {
    pub name: String,
    pub reference: Option<Box<MirRefType>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MirDecl {
    User {
        local: FnLocal,
        name: String,
        span: Range,
        ty: MirType,
        lives: Vec<Range>,
        shared_borrow: Vec<Range>,
        mutable_borrow: Vec<Range>,
        drop: bool,
        drop_range: Vec<Range>,
        definitely_live_at: Vec<Range>,
        maybe_init_at: Vec<Range>,
        deficit_at: Vec<Range>,
        /// Range from StorageLive to StorageDead for this variable
        storage_range: Vec<Range>,
    },
    Other {
        local: FnLocal,
        ty: MirType,
        lives: Vec<Range>,
        shared_borrow: Vec<Range>,
        mutable_borrow: Vec<Range>,
        drop: bool,
        drop_range: Vec<Range>,
        definitely_live_at: Vec<Range>,
        maybe_init_at: Vec<Range>,
        deficit_at: Vec<Range>,
        /// Range from StorageLive to StorageDead for this variable
        storage_range: Vec<Range>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Function {
    pub fn_id: u32,
    pub name: String,
    pub basic_blocks: Vec<MirBasicBlock>,
    pub decls: Vec<MirDecl>,
}

impl Function {
    /// Cached entries keep absolute spans: an unmoved function needs no shift and
    /// its allocation can be shared, a moved one is shifted by the offset its
    /// function travelled. `basic_blocks` carries no ranges.
    pub fn shift(&mut self, delta: i64) {
        if delta == 0 {
            return;
        }
        for decl in &mut self.decls {
            decl.translate(delta);
        }
    }
}

impl MirDecl {
    fn translate(&mut self, delta: i64) {
        // Matched, not shared by field: a new variant must be a compile error
        // here rather than a silently unshifted set of spans.
        match self {
            MirDecl::User {
                span,
                lives,
                shared_borrow,
                mutable_borrow,
                drop_range,
                definitely_live_at,
                maybe_init_at,
                deficit_at,
                storage_range,
                ..
            } => {
                span.translate(delta);
                translate_all_ranges(
                    delta,
                    &mut [
                        lives,
                        shared_borrow,
                        mutable_borrow,
                        drop_range,
                        definitely_live_at,
                        maybe_init_at,
                        deficit_at,
                        storage_range,
                    ],
                );
            }
            MirDecl::Other {
                lives,
                shared_borrow,
                mutable_borrow,
                drop_range,
                definitely_live_at,
                maybe_init_at,
                deficit_at,
                storage_range,
                ..
            } => translate_all_ranges(
                delta,
                &mut [
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    definitely_live_at,
                    maybe_init_at,
                    deficit_at,
                    storage_range,
                ],
            ),
        }
    }
}

fn translate_all_ranges(delta: i64, sets: &mut [&mut Vec<Range>]) {
    for set in sets {
        for range in set.iter_mut() {
            range.translate(delta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Crate, File, Workspace};
    use super::{FnLocal, Function, Loc, MirDecl, MirType, Range, SourceIndex};
    use archery::{ArcK, SharedPointer};
    use std::sync::Arc;

    /// The file is rebuilt, not copied: the other bodies keep their allocation.
    #[test]
    fn replacing_a_function_shares_the_rest() {
        let shared: SharedPointer<Function, ArcK> = SharedPointer::new(sample_function());
        let other = {
            let mut f = sample_function();
            f.fn_id = 8;
            SharedPointer::new(f)
        };
        let file: File = File::new()
            .push(SharedPointer::clone(&shared))
            .push(SharedPointer::clone(&other));

        let replaced = file.replace(7, SharedPointer::new(sample_function()));

        assert_eq!(replaced.items.len(), 2);
        assert!(
            SharedPointer::ptr_eq(&replaced.items[1], &other),
            "the untouched function must keep its allocation"
        );
    }

    /// Only the delivered file is rebuilt; the others keep their allocation.
    #[test]
    fn merging_a_crate_shares_the_untouched_file() {
        let kept: SharedPointer<File, ArcK> =
            SharedPointer::new(File::new().push(SharedPointer::new(sample_function())));
        let key: Arc<str> = Arc::from("a.rs");
        let mut krate: Crate = Crate([(key, SharedPointer::clone(&kept))].into_iter().collect());

        let mut incoming: Crate = Crate(Default::default());
        let other: Arc<str> = Arc::from("b.rs");
        incoming.0 = incoming.0.insert(other, SharedPointer::new(File::new()));
        krate.merge(incoming);

        assert!(
            SharedPointer::ptr_eq(krate.0.get("a.rs").expect("a.rs must survive"), &kept),
            "merging must not rebuild the file it did not deliver"
        );
    }

    /// The wire shape is what the receiver parses: it must not change.
    #[test]
    fn the_workspace_wire_shape_is_unchanged() {
        let file: SharedPointer<File, ArcK> = SharedPointer::new(File {
            items: rpds::Vector::default(),
        });
        let path: Arc<str> = Arc::from("lib.rs");
        let krate: SharedPointer<Crate, ArcK> =
            SharedPointer::new(Crate([(path, file)].into_iter().collect()));
        let name: Arc<str> = Arc::from("krate");
        let workspace: Workspace = Workspace([(name, krate)].into_iter().collect());

        assert_eq!(
            serde_json::to_string(&workspace).unwrap(),
            r#"{"krate":{"lib.rs":{"items":[]}}}"#
        );
    }

    #[test]
    fn loc_new_subtracts_offset() {
        assert_eq!(Loc::new("hello", 3, 1), Loc(2));
        assert_eq!(Loc::new("hello", 3, 0), Loc(3));
    }

    #[test]
    fn loc_new_offset_subtraction_saturates_at_zero() {
        assert_eq!(Loc::new("hello", 1, 5), Loc(0));
        assert_eq!(Loc::new("hello", 0, 9), Loc(0));
    }

    #[test]
    fn loc_new_clamps_past_end_to_char_count() {
        assert_eq!(Loc::new("hello", 99, 0), Loc(5));
        assert_eq!(Loc::new("hello", 5, 0), Loc(5));
    }

    #[test]
    fn loc_new_counts_chars_not_bytes() {
        // a=0, e-acute=1..2, kanji=3..5
        assert_eq!(Loc::new("aé漢", 3, 0), Loc(2));
        assert_eq!(Loc::new("aé漢", 6, 0), Loc(3));
        assert_eq!(Loc::new("aé漢", 99, 0), Loc(3));
    }

    #[test]
    fn loc_new_ignores_cr_ahead_of_the_position() {
        assert_eq!(Loc::new("a\r\nb", 3, 0), Loc(2));
        assert_eq!(Loc::new("a\nb", 2, 0), Loc(2));
    }

    #[test]
    fn source_index_locates_within_a_crlf_file() {
        // the CR is stripped, so the third line starts at char 4
        let index = SourceIndex::new("a\r\nb\r\ncc");
        assert_eq!(index.locate(0), (0, 0));
        assert_eq!(index.locate(1), (0, 1));
        assert_eq!(index.locate(2), (1, 0));
        assert_eq!(index.locate(4), (2, 0));
        assert_eq!(index.locate(5), (2, 1));
        // the column is unaffected by the stripped CRs: byte 8 of the
        // cleaned source is char 6 counting CRs, but the table
        // already dropped them
        assert_eq!(index.locate(5), (2, 1));
        assert_eq!(index.cleaned(), "a\nb\ncc");
    }

    #[test]
    fn source_index_handles_mixed_line_endings() {
        let index = SourceIndex::new("a\r\nb\nc");
        assert_eq!(index.locate(0), (0, 0));
        assert_eq!(index.locate(2), (1, 0));
        assert_eq!(index.locate(4), (2, 0));
    }

    #[test]
    fn source_index_counts_chars_not_bytes() {
        // 漢 is 3 bytes, 😀 is 4; each is one char. Char 1 is the
        // newline itself, so line 1 starts at char 2.
        let index = SourceIndex::new("漢\n😀x");
        assert_eq!(index.locate(0), (0, 0));
        assert_eq!(index.locate(1), (0, 1));
        assert_eq!(index.locate(2), (1, 0));
        assert_eq!(index.locate(3), (1, 1));
    }

    #[test]
    fn source_index_bounds() {
        let index = SourceIndex::new("ab\ncd");
        assert_eq!(index.locate(0), (0, 0));
        // past the end stays on the last line rather than panicking
        assert_eq!(index.locate(5), (1, 2));
        assert_eq!(index.locate(99), (1, 96));
    }

    #[test]
    fn source_index_line_text() {
        let index = SourceIndex::new("ab\ncd\n");
        assert_eq!(index.line_text(0), "ab");
        assert_eq!(index.line_text(1), "cd");
    }

    #[test]
    fn source_index_loc_at_matches_loc_new() {
        // loc_at must agree with Loc::new at every byte offset, for CRLF and
        // multi-byte text alike; this is the equivalence the span path needs
        for raw in [
            "a\r\nb\r\ncc",
            "a\nb\nc",
            "漢\r\n😀x\r\nend",
            "no newline at all",
            "",
            "\r\n\r\n",
        ] {
            let index = SourceIndex::new(raw);
            for byte in 0..=raw.len() {
                assert_eq!(
                    index.loc_at(byte),
                    Loc::new(raw, byte as u32, 0),
                    "raw {raw:?} byte {byte}"
                );
            }
        }
    }

    #[test]
    fn source_index_is_multiline() {
        let index = SourceIndex::new("ab\ncd\nef");
        assert!(index.is_multiline(&Range::new(Loc(1), Loc(4)).unwrap()));
        assert!(!index.is_multiline(&Range::new(Loc(0), Loc(2)).unwrap()));
    }

    fn ranges(start: u32, n: usize) -> Vec<Range> {
        (0..n)
            .map(|i| {
                Range::new(Loc(start + i as u32 * 10), Loc(start + i as u32 * 10 + 5)).unwrap()
            })
            .collect()
    }

    fn sample_function() -> Function {
        let ty = MirType {
            name: "T".into(),
            reference: None,
        };
        Function {
            fn_id: 7,
            name: "f".into(),
            basic_blocks: vec![],
            decls: vec![
                MirDecl::User {
                    local: FnLocal::new(0, 7),
                    name: "x".into(),
                    span: Range::new(Loc(100), Loc(110)).unwrap(),
                    ty: ty.clone(),
                    lives: ranges(120, 2),
                    shared_borrow: ranges(150, 2),
                    mutable_borrow: ranges(180, 2),
                    drop: true,
                    drop_range: ranges(210, 2),
                    definitely_live_at: ranges(240, 2),
                    maybe_init_at: ranges(270, 2),
                    deficit_at: ranges(300, 2),
                    storage_range: ranges(330, 2),
                },
                MirDecl::Other {
                    local: FnLocal::new(1, 7),
                    ty,
                    lives: ranges(400, 2),
                    shared_borrow: ranges(430, 2),
                    mutable_borrow: ranges(460, 2),
                    drop: false,
                    drop_range: ranges(490, 2),
                    definitely_live_at: ranges(520, 2),
                    maybe_init_at: ranges(550, 2),
                    deficit_at: ranges(580, 2),
                    storage_range: ranges(610, 2),
                },
            ],
        }
    }

    /// Walks every range independently of the implementation under test, so a
    /// site the walk forgets shows up as a length mismatch, not a silent gap.
    fn collect_ranges(f: &Function) -> Vec<Range> {
        let mut out = Vec::new();
        for decl in &f.decls {
            match decl {
                MirDecl::User {
                    span,
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    definitely_live_at,
                    maybe_init_at,
                    deficit_at,
                    storage_range,
                    ..
                } => {
                    out.push(*span);
                    for v in [
                        lives,
                        shared_borrow,
                        mutable_borrow,
                        drop_range,
                        definitely_live_at,
                        maybe_init_at,
                        deficit_at,
                        storage_range,
                    ] {
                        out.extend(v.iter().copied());
                    }
                }
                MirDecl::Other {
                    lives,
                    shared_borrow,
                    mutable_borrow,
                    drop_range,
                    definitely_live_at,
                    maybe_init_at,
                    deficit_at,
                    storage_range,
                    ..
                } => {
                    for v in [
                        lives,
                        shared_borrow,
                        mutable_borrow,
                        drop_range,
                        definitely_live_at,
                        maybe_init_at,
                        deficit_at,
                        storage_range,
                    ] {
                        out.extend(v.iter().copied());
                    }
                }
            }
        }
        out
    }

    #[test]
    fn span_walk_covers_every_range_site() {
        // User: span + 8 lists x 2 = 17. Other: 8 lists x 2 = 16.
        assert_eq!(collect_ranges(&sample_function()).len(), 33);
    }

    #[test]
    fn shift_moves_every_range_by_the_delta() {
        let before = collect_ranges(&sample_function());
        let mut moved = sample_function();
        moved.shift(7);
        let after = collect_ranges(&moved);

        assert_eq!(before.len(), after.len());
        for (b, a) in before.iter().zip(after.iter()) {
            assert_eq!(a.from().0, b.from().0 + 7);
            assert_eq!(a.until().0, b.until().0 + 7);
        }
    }

    /// A re-delivered function replaces the earlier one: runs overlap, so the newer body wins.
    #[test]
    fn a_second_delivery_of_a_function_replaces_it() {
        use super::{Crate, File, Workspace};
        use archery::{ArcK, SharedPointer};
        use std::sync::Arc;

        let sibling: SharedPointer<Function, ArcK> = {
            let mut f = sample_function();
            f.fn_id = 8;
            f.name = "g".into();
            SharedPointer::new(f)
        };
        let stale: SharedPointer<Function, ArcK> = SharedPointer::new(sample_function());
        let fresh: SharedPointer<Function, ArcK> = {
            // the same function after an edit above it: every span moved
            let mut f = sample_function();
            f.shift(25);
            SharedPointer::new(f)
        };
        let deliver = |items: Vec<SharedPointer<Function, ArcK>>| {
            let file: SharedPointer<File, ArcK> = SharedPointer::new(
                File::new()
                    .push(SharedPointer::clone(&items[0]))
                    .push(SharedPointer::clone(&items[1])),
            );
            let path: Arc<str> = Arc::from("lib.rs");
            let krate: SharedPointer<Crate, ArcK> =
                SharedPointer::new(Crate([(path, file)].into_iter().collect()));
            let name: Arc<str> = Arc::from("krate");
            Workspace([(name, krate)].into_iter().collect())
        };

        let mut published = deliver(vec![
            SharedPointer::clone(&stale),
            SharedPointer::clone(&sibling),
        ]);
        published.merge(deliver(vec![
            SharedPointer::clone(&fresh),
            SharedPointer::clone(&sibling),
        ]));

        let krate = published.get("krate").expect("the crate must be there");
        let file = krate.file("lib.rs").expect("the file must be there");
        let items = &file.items;
        assert_eq!(
            items.len(),
            2,
            "a re-delivered function must replace, not duplicate"
        );
        let found = items.iter().find(|f| f.fn_id == 7).unwrap();
        assert!(
            SharedPointer::ptr_eq(found, &fresh),
            "the newer delivery must win"
        );
    }
}

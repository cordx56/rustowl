#![allow(unused)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct File {
    pub items: Vec<Function>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct Workspace(pub HashMap<String, Crate>);

impl Workspace {
    pub fn merge(&mut self, other: Self) {
        let Workspace(crates) = other;
        for (name, krate) in crates {
            if let Some(insert) = self.0.get_mut(&name) {
                insert.merge(krate);
            } else {
                self.0.insert(name, krate);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct Crate(pub HashMap<String, File>);

impl Crate {
    pub fn merge(&mut self, other: Self) {
        let Crate(files) = other;
        for (file, mir) in files {
            if let Some(insert) = self.0.get_mut(&file) {
                insert.items.extend_from_slice(&mir.items);
                insert.items.dedup_by(|a, b| a.fn_id == b.fn_id);
            } else {
                self.0.insert(file, mir);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::Loc;

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
}

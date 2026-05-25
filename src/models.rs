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
        let byte_pos = byte_pos.saturating_sub(offset);
        // it seems that the compiler is ignoring CR
        let source_clean = source.replace("\r", "");

        // Convert byte position to character position safely
        if source_clean.len() < byte_pos as usize {
            return Self(source_clean.chars().count() as u32);
        }

        // Find the character index corresponding to the byte position
        match source_clean
            .char_indices()
            .position(|(byte_idx, _)| (byte_pos as usize) <= byte_idx)
        {
            Some(char_idx) => Self(char_idx as u32),
            None => Self(source_clean.chars().count() as u32),
        }
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirProjectionElem {
    Deref,
    Field { index: usize },
    Index { local: FnLocal },
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MirPlace {
    pub local: FnLocal,
    pub projection: Vec<MirProjectionElem>,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MirDecl {
    User {
        local: FnLocal,
        name: String,
        span: Range,
        ty: String,
        lives: Vec<Range>,
        shared_borrow: Vec<Range>,
        mutable_borrow: Vec<Range>,
        drop: bool,
        drop_range: Vec<Range>,
        definitely_live_at: Vec<Range>,
        maybe_init_at: Vec<Range>,
        must_live_at: Vec<Range>,
        /// Range from StorageLive to StorageDead for this variable
        storage_range: Vec<Range>,
    },
    Other {
        local: FnLocal,
        ty: String,
        lives: Vec<Range>,
        shared_borrow: Vec<Range>,
        mutable_borrow: Vec<Range>,
        drop: bool,
        drop_range: Vec<Range>,
        definitely_live_at: Vec<Range>,
        maybe_init_at: Vec<Range>,
        must_live_at: Vec<Range>,
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

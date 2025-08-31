#![allow(unused)]

use serde::{Deserialize, Serialize};
use smallvec::{SmallVec, smallvec};
use indexmap::IndexMap;
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
        let byte_pos = byte_pos as usize;

        // Convert byte position to character position efficiently
        // Skip CR characters without allocating a new string
        let mut char_count = 0u32;
        let mut byte_count = 0usize;
        
        for ch in source.chars() {
            if byte_count >= byte_pos {
                break;
            }
            
            // Skip CR characters (compiler ignores them)
            if ch != '\r' {
                byte_count += ch.len_utf8();
                if byte_count <= byte_pos {
                    char_count += 1;
                }
            } else {
                byte_count += ch.len_utf8();
            }
        }
        
        Self(char_count)
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
pub struct MirVariables(IndexMap<u32, MirVariable>);

impl Default for MirVariables {
    fn default() -> Self {
        Self::new()
    }
}

impl MirVariables {
    pub fn new() -> Self {
        Self(IndexMap::with_capacity(8))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(IndexMap::with_capacity(capacity))
    }

    pub fn push(&mut self, var: MirVariable) {
        let index = match &var {
            MirVariable::User { index, .. } | MirVariable::Other { index, .. } => *index,
        };
        self.0.entry(index).or_insert(var);
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
    pub items: SmallVec<[Function; 4]>, // Most files have few functions
}

impl Default for File {
    fn default() -> Self {
        Self::new()
    }
}

impl File {
    pub fn new() -> Self {
        Self {
            items: SmallVec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: SmallVec::with_capacity(capacity),
        }
    }
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
        for (file, mut mir) in files {
            match self.0.get_mut(&file) {
                Some(existing) => {
                    // Pre-allocate capacity for better performance
                    let new_size = existing.items.len() + mir.items.len();
                    if existing.items.capacity() < new_size {
                        existing.items.reserve(mir.items.len());
                    }
                    
                    // Use a HashSet for O(1) lookup instead of dedup_by
                    let mut seen_ids = std::collections::HashSet::with_capacity(existing.items.len());
                    for item in &existing.items {
                        seen_ids.insert(item.fn_id);
                    }
                    
                    mir.items.retain(|item| seen_ids.insert(item.fn_id));
                    existing.items.append(&mut mir.items);
                }
                None => {
                    self.0.insert(file, mir);
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirRval {
    Move {
        target_local: FnLocal,
        range: Range,
    },
    Borrow {
        target_local: FnLocal,
        range: Range,
        mutable: bool,
        outlive: Option<Range>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirStatement {
    StorageLive {
        target_local: FnLocal,
        range: Range,
    },
    StorageDead {
        target_local: FnLocal,
        range: Range,
    },
    Assign {
        target_local: FnLocal,
        range: Range,
        rval: Option<MirRval>,
    },
    Other {
        range: Range,
    },
}
impl MirStatement {
    pub fn range(&self) -> Range {
        match self {
            Self::StorageLive { range, .. } => *range,
            Self::StorageDead { range, .. } => *range,
            Self::Assign { range, .. } => *range,
            Self::Other { range } => *range,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirTerminator {
    Drop {
        local: FnLocal,
        range: Range,
    },
    Call {
        destination_local: FnLocal,
        fn_span: Range,
    },
    Other {
        range: Range,
    },
}
impl MirTerminator {
    pub fn range(&self) -> Range {
        match self {
            Self::Drop { range, .. } => *range,
            Self::Call { fn_span, .. } => *fn_span,
            Self::Other { range } => *range,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MirBasicBlock {
    pub statements: StatementVec,
    pub terminator: Option<MirTerminator>,
}

impl Default for MirBasicBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl MirBasicBlock {
    pub fn new() -> Self {
        Self {
            statements: StatementVec::new(),
            terminator: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            statements: StatementVec::with_capacity(capacity),
            terminator: None,
        }
    }
}

// Type aliases for commonly small collections
pub type RangeVec = SmallVec<[Range; 4]>; // Most variables have few ranges
pub type StatementVec = SmallVec<[MirStatement; 8]>; // Most basic blocks have few statements
pub type DeclVec = SmallVec<[MirDecl; 16]>; // Most functions have moderate number of declarations

// Helper functions for conversions since we can't impl traits on type aliases
pub fn range_vec_into_vec(ranges: RangeVec) -> Vec<Range> {
    smallvec::SmallVec::into_vec(ranges)
}

pub fn range_vec_from_vec(vec: Vec<Range>) -> RangeVec {
    SmallVec::from_vec(vec)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MirDecl {
    User {
        local: FnLocal,
        name: String,
        span: Range,
        ty: String,
        lives: RangeVec,
        shared_borrow: RangeVec,
        mutable_borrow: RangeVec,
        drop: bool,
        drop_range: RangeVec,
        must_live_at: RangeVec,
    },
    Other {
        local: FnLocal,
        ty: String,
        lives: RangeVec,
        shared_borrow: RangeVec,
        mutable_borrow: RangeVec,
        drop: bool,
        drop_range: RangeVec,
        must_live_at: RangeVec,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Function {
    pub fn_id: u32,
    pub basic_blocks: SmallVec<[MirBasicBlock; 8]>, // Most functions have few basic blocks
    pub decls: DeclVec,
}

impl Function {
    pub fn new(fn_id: u32) -> Self {
        Self {
            fn_id,
            basic_blocks: SmallVec::new(),
            decls: DeclVec::new(),
        }
    }

    pub fn with_capacity(fn_id: u32, bb_capacity: usize, decl_capacity: usize) -> Self {
        Self {
            fn_id,
            basic_blocks: SmallVec::with_capacity(bb_capacity),
            decls: DeclVec::with_capacity(decl_capacity),
        }
    }
}

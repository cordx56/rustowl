use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Error {
    SyntaxError,
    UnknownError,
    LocalDeclareNotFound,
    LocalIsNotUserVariable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Local {
    pub id: u32,
    pub fn_id: u32,
}

impl Local {
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
        for (i, (byte, _)) in source.char_indices().enumerate() {
            if byte_pos <= byte as u32 {
                return Self(i as u32);
            }
        }
        Self(0)
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Range {
    pub from: Loc,
    pub until: Loc,
}
impl Range {
    pub fn new(from: Loc, until: Loc) -> Self {
        Self { from, until }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
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
pub struct Workspace(pub HashMap<String, File>);
impl Workspace {
    pub fn merge(mut self, other: Self) -> Self {
        let Workspace(files) = other;
        for (file, mir) in files {
            if let Some(insert) = self.0.get_mut(&file) {
                insert.items.extend_from_slice(&mir.items);
            } else {
                self.0.insert(file, mir);
            }
        }
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirRval {
    Move {
        target_local_index: u32,
        range: Range,
    },
    Borrow {
        target_local_index: u32,
        range: Range,
        mutable: bool,
        outlive: Option<Range>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirStatement {
    StorageLive {
        target_local_index: u32,
        range: Range,
    },
    StorageDead {
        target_local_index: u32,
        range: Range,
    },
    Assign {
        target_local_index: u32,
        range: Range,
        rval: Option<MirRval>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirTerminator {
    Drop {
        local_index: u32,
        range: Range,
    },
    Call {
        destination_local_index: u32,
        fn_span: Range,
    },
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MirBasicBlock {
    pub statements: Vec<MirStatement>,
    pub terminator: Option<MirTerminator>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirDecl {
    User {
        local_index: u32,
        fn_id: u32,
        name: String,
        span: Range,
        ty: String,
        lives: Vec<Range>,
        drop: bool,
        drop_range: Vec<Range>,
        must_live_at: Vec<Range>,
    },
    Other {
        local_index: u32,
        fn_id: u32,
        ty: String,
        lives: Vec<Range>,
        drop: bool,
        drop_range: Vec<Range>,
        must_live_at: Vec<Range>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Function {
    pub fn_id: u32,
    pub basic_blocks: Vec<MirBasicBlock>,
    pub decls: Vec<MirDecl>,
}

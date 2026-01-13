//! Data models for RustOwl ownership and lifetime analysis.
//!
//! This module contains the core data structures used to represent
//! ownership information, lifetimes, and analysis results extracted
//! from Rust code via compiler integration.

use ecow::{EcoString, EcoVec};
use foldhash::quality::RandomState as FoldHasher;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

/// An IndexMap with FoldHasher for fast + high-quality hashing.
pub type FoldIndexMap<K, V> = IndexMap<K, V, FoldHasher>;

/// An IndexSet with FoldHasher for fast + high-quality hashing.
pub type FoldIndexSet<K> = IndexSet<K, FoldHasher>;

/// Represents a local variable within a function scope.
///
/// This structure uniquely identifies a local variable by combining
/// its local ID within the function and the function ID itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FnLocal {
    /// Local variable ID within the function
    pub id: u32,
    /// Function ID this local belongs to
    pub fn_id: u32,
}

impl FnLocal {
    /// Creates a new function-local variable identifier.
    ///
    /// # Arguments
    /// * `id` - The local variable ID within the function
    /// * `fn_id` - The function ID this local belongs to
    pub fn new(id: u32, fn_id: u32) -> Self {
        Self { id, fn_id }
    }
}

/// Represents a character position in source code.
///
/// This is a character-based position that handles Unicode correctly
/// and automatically filters out carriage return characters to match
/// compiler behavior.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(transparent)]
pub struct Loc(pub u32);

impl Loc {
    /// Creates a new location from source text and byte position.
    ///
    /// Converts a byte position to a character position, handling Unicode
    /// correctly and filtering out CR characters as the compiler does.
    ///
    /// # Arguments
    /// * `source` - The source code text
    /// * `byte_pos` - Byte position in the source
    /// * `offset` - Offset to subtract from byte position
    pub fn new(source: &str, byte_pos: u32, offset: u32) -> Self {
        let byte_pos = byte_pos.saturating_sub(offset);
        let byte_pos = byte_pos as usize;

        // This method is intentionally allocation-free. Hot paths should prefer
        // `utils::NormalizedByteCharIndex` to avoid repeatedly scanning `source`.
        //
        // Note: rustc byte positions are reported as if `\r` doesn't exist.
        // So our byte counting must ignore CR too.
        let mut char_count = 0u32;
        let mut normalized_byte_count = 0usize;

        for ch in source.chars() {
            if ch == '\r' {
                continue;
            }
            if normalized_byte_count >= byte_pos {
                break;
            }

            normalized_byte_count += ch.len_utf8();
            if normalized_byte_count <= byte_pos {
                char_count += 1;
            }
        }

        Self(char_count)
    }
}

impl std::ops::Add<i32> for Loc {
    type Output = Loc;
    /// Adds a signed offset to this `Loc`, saturating to avoid underflow or overflow.
    ///
    /// For non-negative offsets, the location is increased with saturation at `u32::MAX`.
    /// For negative offsets, the absolute value is subtracted with saturation at `0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rustowl::models::Loc;
    /// let a = Loc(5);
    /// assert_eq!(a + 3, Loc(8));
    ///
    /// let b = Loc(0);
    /// assert_eq!(b + -10, Loc(0)); // saturates at zero, does not underflow
    ///
    /// let c = Loc(u32::MAX - 1);
    /// assert_eq!(c + 10, Loc(u32::MAX)); // saturates at u32::MAX, does not overflow
    /// ```
    fn add(self, rhs: i32) -> Self::Output {
        if rhs >= 0 {
            // Use saturating_add to prevent overflow
            Loc(self.0.saturating_add(rhs as u32))
        } else {
            // rhs is negative, so subtract the absolute value
            let abs_rhs = (-rhs) as u32;
            Loc(self.0.saturating_sub(abs_rhs))
        }
    }
}

impl std::ops::Sub<i32> for Loc {
    type Output = Loc;
    /// Subtracts a signed offset from this `Loc`, using saturating arithmetic.
    ///
    /// For non-negative `rhs` the function subtracts `rhs` (saturating at 0 to prevent underflow).
    /// If `rhs` is negative the absolute value is added (saturating on overflow).
    ///
    /// # Examples
    ///
    /// ```
    /// # use rustowl::models::Loc;
    /// let a = Loc(10);
    /// assert_eq!(a - 3, Loc(7));   // normal subtraction
    /// assert_eq!(a - (-2), Loc(12)); // negative rhs -> addition
    /// let zero = Loc(0);
    /// assert_eq!(zero - 1, Loc(0)); // saturates at 0, no underflow
    /// let max = Loc(u32::MAX);
    /// assert_eq!(max - (-1), Loc(u32::MAX)); // saturating add prevents overflow
    /// ```
    fn sub(self, rhs: i32) -> Self::Output {
        if rhs >= 0 {
            Loc(self.0.saturating_sub(rhs as u32))
        } else {
            // rhs is negative, so we're actually adding the absolute value
            let abs_rhs = (-rhs) as u32;
            Loc(self.0.saturating_add(abs_rhs))
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

/// Represents a character range in source code.
///
/// A range is defined by a starting and ending location, where the
/// ending location is exclusive (half-open interval).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Range {
    from: Loc,
    until: Loc,
}

impl Range {
    /// Creates a new range if the end position is after the start position.
    ///
    /// # Arguments
    /// * `from` - Starting location (inclusive)
    /// * `until` - Ending location (exclusive)
    ///
    /// # Returns
    /// `Some(Range)` if valid, `None` if `until <= from`
    pub fn new(from: Loc, until: Loc) -> Option<Self> {
        if until.0 <= from.0 {
            None
        } else {
            Some(Self { from, until })
        }
    }

    /// Returns the starting location of the range.
    pub fn from(&self) -> Loc {
        self.from
    }

    /// Returns the ending location of the range.
    pub fn until(&self) -> Loc {
        self.until
    }

    /// Returns the size of the range in characters.
    pub fn size(&self) -> u32 {
        self.until.0 - self.from.0
    }
}

/// Represents a MIR (Mid-level IR) variable with lifetime information.
///
/// MIR variables can be either user-defined variables or compiler-generated
/// temporaries, each with their own live and dead ranges.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum MirVariable {
    /// A user-defined variable
    User {
        /// Variable index within the function
        index: u32,
        /// Range where the variable is live
        live: Range,
        /// Range where the variable is dead/dropped
        dead: Range,
    },
    /// A compiler-generated temporary or other variable
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
pub struct File {
    pub items: EcoVec<Function>,
}

impl Default for File {
    fn default() -> Self {
        Self::new()
    }
}

impl File {
    pub fn new() -> Self {
        Self {
            items: EcoVec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: EcoVec::with_capacity(capacity),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct Workspace(pub FoldIndexMap<String, Crate>);

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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(transparent)]
pub struct Crate(pub FoldIndexMap<String, File>);

impl Crate {
    pub fn merge(&mut self, other: Self) {
        let Crate(files) = other;
        for (file, mir) in files {
            match self.0.get_mut(&file) {
                Some(existing) => {
                    let mut seen_ids = FoldIndexSet::with_capacity_and_hasher(
                        existing.items.len(),
                        FoldHasher::default(),
                    );
                    seen_ids.extend(existing.items.iter().map(|i| i.fn_id));

                    // `EcoVec` doesn't offer `retain`/`append`, so rebuild the delta.
                    let new_items: EcoVec<Function> = mir
                        .items
                        .iter()
                        .filter(|&item| seen_ids.insert(item.fn_id))
                        .cloned()
                        .collect();

                    if !new_items.is_empty() {
                        let mut merged =
                            EcoVec::with_capacity(existing.items.len() + new_items.len());
                        merged.extend(existing.items.iter().cloned());
                        merged.extend(new_items);
                        existing.items = merged;
                    }
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

// Type aliases for commonly cloned collections.
//
// These were previously `SmallVec` to optimize for small inline sizes.
// We now use `EcoVec` to make cloning across the LSP boundary cheap.
pub type RangeVec = EcoVec<Range>;
pub type StatementVec = EcoVec<MirStatement>;
pub type DeclVec = EcoVec<MirDecl>;

pub fn range_vec_into_vec(ranges: RangeVec) -> Vec<Range> {
    ranges.into_iter().collect()
}

pub fn range_vec_from_vec(vec: Vec<Range>) -> RangeVec {
    vec.into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MirDecl {
    User {
        local: FnLocal,
        name: EcoString,
        span: Range,
        ty: EcoString,
        lives: RangeVec,
        shared_borrow: RangeVec,
        mutable_borrow: RangeVec,
        drop: bool,
        drop_range: RangeVec,
        must_live_at: RangeVec,
    },
    Other {
        local: FnLocal,
        ty: EcoString,
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
    pub basic_blocks: EcoVec<MirBasicBlock>,
    pub decls: DeclVec,
}

impl Function {
    pub fn new(fn_id: u32) -> Self {
        Self {
            fn_id,
            basic_blocks: EcoVec::new(),
            decls: DeclVec::new(),
        }
    }

    /// Creates a `Function` with preallocated capacity for basic blocks and declarations.
    ///
    /// `fn_id` is the function identifier. `bb_capacity` is the initial capacity reserved
    /// for the function's basic block list. `decl_capacity` is the initial capacity reserved
    /// for the function's declarations.
    ///
    /// # Examples
    ///
    /// ```
    /// use rustowl::models::Function;
    /// let f = Function::with_capacity(42, 8, 16);
    /// assert_eq!(f.fn_id, 42);
    /// assert!(f.basic_blocks.capacity() >= 8);
    /// assert!(f.decls.capacity() >= 16);
    /// ```
    pub fn with_capacity(fn_id: u32, bb_capacity: usize, decl_capacity: usize) -> Self {
        Self {
            fn_id,
            basic_blocks: EcoVec::with_capacity(bb_capacity),
            decls: DeclVec::with_capacity(decl_capacity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loc_creation_with_unicode() {
        let source = "hello 🦀 world\r\ngoodbye 🌍 world";
        // Test character position conversion
        let _loc = Loc::new(source, 8, 0); // Should point to space before 🦀

        // Verify that CR characters are filtered out.
        // rustc byte positions are reported as if `\r` doesn't exist, so the same
        // `byte_pos` should map to the same `Loc`.
        let source_with_cr = "hello\r\n world";
        let loc_with_cr = Loc::new(source_with_cr, 7, 0);
        let loc_without_cr = Loc::new("hello\n world", 7, 0);
        assert_eq!(loc_with_cr.0, loc_without_cr.0);
    }

    #[test]
    fn test_workspace_merge_operations() {
        let mut workspace1 = Workspace(FoldIndexMap::default());
        let mut workspace2 = Workspace(FoldIndexMap::default());

        // Setup workspace1 with a crate
        let mut crate1 = Crate(FoldIndexMap::default());
        crate1.0.insert("lib.rs".to_string(), File::new());
        workspace1.0.insert("my_crate".to_string(), crate1);

        // Setup workspace2 with the same crate name but different file
        let mut crate2 = Crate(FoldIndexMap::default());
        crate2.0.insert("main.rs".to_string(), File::new());
        workspace2.0.insert("my_crate".to_string(), crate2);

        // Setup workspace2 with a different crate
        let crate3 = Crate(FoldIndexMap::default());
        workspace2.0.insert("other_crate".to_string(), crate3);

        workspace1.merge(workspace2);

        // Should have 2 crates total
        assert_eq!(workspace1.0.len(), 2);
        assert!(workspace1.0.contains_key("my_crate"));
        assert!(workspace1.0.contains_key("other_crate"));

        // my_crate should have both files after merge
        let merged_crate = &workspace1.0["my_crate"];
        assert_eq!(merged_crate.0.len(), 2);
        assert!(merged_crate.0.contains_key("lib.rs"));
        assert!(merged_crate.0.contains_key("main.rs"));
    }

    #[test]
    fn test_crate_merge_with_duplicate_functions() {
        let mut crate1 = Crate(FoldIndexMap::default());
        let mut crate2 = Crate(FoldIndexMap::default());

        // Create files with functions
        let mut file1 = File::new();
        file1.items.push(Function::new(1));
        file1.items.push(Function::new(2));

        let mut file2 = File::new();
        file2.items.push(Function::new(2)); // Duplicate fn_id
        file2.items.push(Function::new(3));

        crate1.0.insert("test.rs".to_string(), file1);
        crate2.0.insert("test.rs".to_string(), file2);

        crate1.merge(crate2);

        let merged_file = &crate1.0["test.rs"];
        // Should have 3 unique functions (1, 2, 3) with duplicate 2 filtered out
        assert_eq!(merged_file.items.len(), 3);

        // Check that function IDs are unique
        let mut ids: Vec<u32> = merged_file.items.iter().map(|f| f.fn_id).collect();
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn test_mir_statement_range_extraction() {
        let range = Range::new(Loc(10), Loc(20)).unwrap();
        let fn_local = FnLocal::new(1, 42);

        let storage_live = MirStatement::StorageLive {
            target_local: fn_local,
            range,
        };
        assert_eq!(storage_live.range(), range);

        let storage_dead = MirStatement::StorageDead {
            target_local: fn_local,
            range,
        };
        assert_eq!(storage_dead.range(), range);

        let assign = MirStatement::Assign {
            target_local: fn_local,
            range,
            rval: None,
        };
        assert_eq!(assign.range(), range);

        let other = MirStatement::Other { range };
        assert_eq!(other.range(), range);
    }

    #[test]
    fn test_range_vec_conversions() {
        let ranges = vec![
            Range::new(Loc(0), Loc(5)).unwrap(),
            Range::new(Loc(10), Loc(15)).unwrap(),
        ];

        let range_vec = range_vec_from_vec(ranges.clone());
        let converted_back = range_vec_into_vec(range_vec);

        assert_eq!(ranges, converted_back);
    }

    #[test]
    fn test_mir_variable_comprehensive_scenarios() {
        // Test comprehensive MirVariable scenarios
        let base_range = Range::new(Loc(10), Loc(50)).unwrap();
        let live_range = Range::new(Loc(15), Loc(40)).unwrap();
        let dead_range = Range::new(Loc(40), Loc(45)).unwrap();

        let variables = vec![
            MirVariable::User {
                index: 0,
                live: live_range,
                dead: dead_range,
            },
            MirVariable::User {
                index: u32::MAX,
                live: base_range,
                dead: Range::new(Loc(50), Loc(60)).unwrap(),
            },
            MirVariable::Other {
                index: 0,
                live: live_range,
                dead: dead_range,
            },
            MirVariable::Other {
                index: 12345,
                live: base_range,
                dead: live_range,
            },
            MirVariable::Other {
                index: 999,
                live: Range::new(Loc(0), Loc(10)).unwrap(),
                dead: Range::new(Loc(10), Loc(20)).unwrap(),
            },
        ];

        for variable in variables {
            // Test serialization roundtrip
            let json = serde_json::to_string(&variable).unwrap();
            let deserialized: MirVariable = serde_json::from_str(&json).unwrap();

            // Extract and compare components
            let (orig_index, orig_live, orig_dead) = match &variable {
                MirVariable::User { index, live, dead } => (index, live, dead),
                MirVariable::Other { index, live, dead } => (index, live, dead),
            };

            let (deser_index, deser_live, deser_dead) = match &deserialized {
                MirVariable::User { index, live, dead } => (index, live, dead),
                MirVariable::Other { index, live, dead } => (index, live, dead),
            };

            assert_eq!(orig_index, deser_index);
            assert_eq!(orig_live, deser_live);
            assert_eq!(orig_dead, deser_dead);

            // Verify ranges are valid
            assert!(orig_live.from() < orig_live.until());
            assert!(orig_dead.from() < orig_dead.until());
        }
    }

    #[test]
    fn test_serialization_format_consistency() {
        // Test that serialization format is consistent and predictable
        let function = Function::new(42);
        let range = Range::new(Loc(10), Loc(20)).unwrap();
        let fn_local = FnLocal::new(1, 2);

        let variable = MirVariable::User {
            index: 5,
            live: range,
            dead: Range::new(Loc(20), Loc(30)).unwrap(),
        };

        let statement = MirStatement::Assign {
            target_local: fn_local,
            range,
            rval: None,
        };

        let terminator = MirTerminator::Other { range };

        // Test multiple serialization rounds produce same result
        for _ in 0..3 {
            let json1 = serde_json::to_string(&function).unwrap();
            let json2 = serde_json::to_string(&function).unwrap();
            assert_eq!(json1, json2, "Serialization should be deterministic");

            let json1 = serde_json::to_string(&variable).unwrap();
            let json2 = serde_json::to_string(&variable).unwrap();
            assert_eq!(
                json1, json2,
                "Variable serialization should be deterministic"
            );

            let json1 = serde_json::to_string(&statement).unwrap();
            let json2 = serde_json::to_string(&statement).unwrap();
            assert_eq!(
                json1, json2,
                "Statement serialization should be deterministic"
            );

            let json1 = serde_json::to_string(&terminator).unwrap();
            let json2 = serde_json::to_string(&terminator).unwrap();
            assert_eq!(
                json1, json2,
                "Terminator serialization should be deterministic"
            );
        }
    }

    #[test]
    fn test_memory_usage_optimization() {
        // Test memory usage optimization for data structures
        use std::mem;

        // Test that core types have reasonable memory footprint
        let function = Function::new(0);
        let function_size = mem::size_of_val(&function);
        assert!(
            function_size <= 8192,
            "Function should be compact: {function_size} bytes"
        );

        let range = Range::new(Loc(0), Loc(100)).unwrap();
        let range_size = mem::size_of_val(&range);
        assert!(
            range_size <= 16,
            "Range should be compact: {range_size} bytes"
        );

        let fn_local = FnLocal::new(0, 0);
        let fn_local_size = mem::size_of_val(&fn_local);
        assert!(
            fn_local_size <= 16,
            "FnLocal should be compact: {fn_local_size} bytes"
        );

        // Spot-check `EcoVec` remains a compact container.
        let vec = EcoVec::<Function>::new();
        let vec_size = mem::size_of_val(&vec);
        assert!(vec_size > 0);

        let mut vec = EcoVec::<Function>::new();
        for i in 0..4 {
            vec.push(Function::new(i));
        }
        assert_eq!(vec.len(), 4);
    }
}

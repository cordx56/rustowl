//! # Miri Memory Safety Tests
//!
//! This module contains tests specifically designed to run under Miri
//! to validate memory safety and undefined behavior detection in RustOwl's core functionality.
//!
//! These tests avoid external dependencies and process spawning that Miri doesn't support,
//! focusing on pure Rust code paths that can be fully analyzed for memory safety.
//!
//! ## What These Tests Cover:
//!
//! ### Core Data Models & Memory Safety:
//! - **Loc arithmetic**: Position tracking with overflow/underflow protection
//! - **Range validation**: Bounds checking and edge case handling
//! - **FnLocal operations**: Hash map usage and equality checks
//! - **File model**: Vector operations and memory management
//! - **Workspace/Crate hierarchy**: Complex nested HashMap operations
//! - **MirVariable variants**: Enum handling and pattern matching
//! - **Function structures**: Complex nested data structure operations
//!
//! ### Memory Management Patterns:
//! - **String handling**: Unicode support and concatenation safety
//! - **Collection operations**: HashMap/Vector operations with complex nesting
//! - **Clone operations**: Deep copying of complex structures
//! - **Serialization structures**: Data integrity for serde-compatible types
//! - **Capacity management**: Pre-allocation and memory growth patterns
//!
//! ### What Miri Validates:
//! - No use-after-free bugs
//! - No buffer overflows/underflows
//! - No uninitialized memory access
//! - No data races (in single-threaded context)
//! - Proper pointer provenance
//! - Memory leak detection
//! - Undefined behavior in arithmetic operations
//!
//! ## Limitations:
//! These tests cannot cover RustOwl functionality that requires:
//! - Process spawning (cargo metadata calls)
//! - File system operations
//! - Network operations
//! - External tool integration
//!
//! However, they thoroughly validate the core algorithms and data structures
//! that form the foundation of RustOwl's analysis capabilities.
//!
//! ## Usage:
//! ```bash
//! MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-permissive-provenance" cargo miri test --lib
//! ```

#[cfg(test)]
mod miri_memory_safety_tests {
    use crate::models::FoldIndexMap as HashMap;
    use crate::models::*;

    #[test]
    fn test_loc_arithmetic_memory_safety() {
        // Test Loc model creation and arithmetic operations for memory safety
        let loc = Loc::new("test string with unicode 🦀", 5, 0);
        let loc2 = loc + 2;
        let loc3 = loc2 - 1;

        // Test arithmetic operations don't cause memory issues
        assert_eq!(loc3.0, loc.0 + 1);

        // Test boundary conditions
        let loc_zero = Loc(0);
        let loc_underflow = loc_zero - 10; // Should saturate to 0
        assert_eq!(loc_underflow.0, 0);

        // Test large values (but avoid overflow)
        let loc_large = Loc(u32::MAX - 10);
        let loc_add = loc_large + 5; // Safe addition
        assert_eq!(loc_add.0, u32::MAX - 5);
    }

    #[test]
    fn test_range_creation_and_validation() {
        // Test Range creation with various scenarios
        let valid_range = Range::new(Loc(0), Loc(10)).unwrap();
        assert_eq!(valid_range.from().0, 0);
        assert_eq!(valid_range.until().0, 10);
        assert_eq!(valid_range.size(), 10);

        // Test invalid range (until <= from)
        let invalid_range = Range::new(Loc(10), Loc(5));
        assert!(invalid_range.is_none());

        // Test edge case: same positions
        let same_pos_range = Range::new(Loc(5), Loc(5));
        assert!(same_pos_range.is_none());

        // Test large ranges
        let large_range = Range::new(Loc(0), Loc(u32::MAX)).unwrap();
        assert_eq!(large_range.size(), u32::MAX);
    }

    #[test]
    fn test_fn_local_operations() {
        // Test FnLocal model creation and operations
        let fn_local1 = FnLocal::new(42, 100);
        let fn_local2 = FnLocal::new(43, 100);
        let fn_local3 = FnLocal::new(42, 100);

        // Test equality and inequality
        assert_eq!(fn_local1, fn_local3);
        assert_ne!(fn_local1, fn_local2);

        // Test hashing (via HashMap insertion)
        let mut map = HashMap::default();
        map.insert(fn_local1, "first");
        map.insert(fn_local2, "second");
        map.insert(fn_local3, "third"); // Should overwrite first

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&fn_local1), Some(&"third"));
        assert_eq!(map.get(&fn_local2), Some(&"second"));
    }

    #[test]
    fn test_file_model_operations() {
        // Test File model with various operations
        let mut file = File::new();

        // Test vector operations
        assert_eq!(file.items.len(), 0);
        assert!(file.items.is_empty());

        // Test vector capacity and memory management
        file.items.reserve(1000);
        assert!(file.items.capacity() >= 1000);

        // Test cloning (deep copy)
        let file_clone = file.clone();
        assert_eq!(file.items.len(), file_clone.items.len());
    }

    #[test]
    fn test_workspace_operations() {
        // Test Workspace and Crate models
        let mut workspace = Workspace(HashMap::default());
        let mut crate1 = Crate(HashMap::default());
        let mut crate2 = Crate(HashMap::default());

        // Add some files to crates
        crate1.0.insert("lib.rs".to_string(), File::new());
        crate1.0.insert("main.rs".to_string(), File::new());

        crate2.0.insert("helper.rs".to_string(), File::new());

        // Add crates to workspace
        workspace.0.insert("crate1".to_string(), crate1);
        workspace.0.insert("crate2".to_string(), crate2);

        assert_eq!(workspace.0.len(), 2);
        assert!(workspace.0.contains_key("crate1"));
        assert!(workspace.0.contains_key("crate2"));

        // Test workspace merging
        let mut other_workspace = Workspace(HashMap::default());
        let crate3 = Crate(HashMap::default());
        other_workspace.0.insert("crate3".to_string(), crate3);

        workspace.merge(other_workspace);
        assert_eq!(workspace.0.len(), 3);
        assert!(workspace.0.contains_key("crate3"));
    }

    #[test]
    fn test_mir_variables_operations() {
        // Test MirVariables collection operations
        let mut mir_vars = MirVariables::new();

        // Test creation of MirVariable variants
        let user_var = MirVariable::User {
            index: 1,
            live: Range::new(Loc(0), Loc(10)).unwrap(),
            dead: Range::new(Loc(10), Loc(20)).unwrap(),
        };

        let other_var = MirVariable::Other {
            index: 2,
            live: Range::new(Loc(5), Loc(15)).unwrap(),
            dead: Range::new(Loc(15), Loc(25)).unwrap(),
        };

        // Test insertion using push method
        mir_vars.push(user_var);
        mir_vars.push(other_var);

        // Test converting to vector
        let vars_vec = mir_vars.clone().to_vec();
        assert_eq!(vars_vec.len(), 2);

        // Test that we can find our variables
        let has_user_var = vars_vec
            .iter()
            .any(|v| matches!(v, MirVariable::User { index: 1, .. }));
        let has_other_var = vars_vec
            .iter()
            .any(|v| matches!(v, MirVariable::Other { index: 2, .. }));

        assert!(has_user_var);
        assert!(has_other_var);

        // Test duplicate insertion (should not duplicate)
        mir_vars.push(user_var);
        let final_vec = mir_vars.to_vec();
        assert_eq!(final_vec.len(), 2); // Still 2, not 3
    }

    #[test]
    fn test_function_model_complex_operations() {
        // Test Function model with complex nested structures
        let function = Function::new(42);

        // Test cloning of complex nested structures
        let function_clone = function.clone();
        assert_eq!(function.fn_id, function_clone.fn_id);
        assert_eq!(
            function.basic_blocks.len(),
            function_clone.basic_blocks.len()
        );
        assert_eq!(function.decls.len(), function_clone.decls.len());

        // Test memory layout and alignment
        let function_size = std::mem::size_of::<Function>();
        assert!(function_size > 0);

        // Test that we can create multiple instances without memory issues
        let mut functions = Vec::new();
        for i in 0..100 {
            functions.push(Function::new(i));
        }

        assert_eq!(functions.len(), 100);
        assert_eq!(functions[50].fn_id, 50);

        // Test vector capacity management
        let large_function = Function::with_capacity(999, 1000, 500);

        assert!(large_function.basic_blocks.capacity() >= 1000);
        assert!(large_function.decls.capacity() >= 500);
    }

    #[test]
    fn test_string_handling_memory_safety() {
        // Test string operations that could cause memory issues
        let mut strings = Vec::new();

        // Test various string operations
        for i in 0..50 {
            let s = format!("test_string_{i}");
            strings.push(s);
        }

        // Test string concatenation
        let mut concatenated = String::new();
        for s in &strings {
            concatenated.push_str(s);
            concatenated.push(' ');
        }

        assert!(!concatenated.is_empty());

        // Test unicode handling
        let unicode_string = "🦀 Rust 🔥 Memory Safety 🛡️".to_string();

        // Ensure unicode doesn't cause memory issues
        assert!(unicode_string.len() > unicode_string.chars().count());
    }

    #[test]
    fn test_collections_memory_safety() {
        // Test various collection operations for memory safety
        let mut map: HashMap<String, Vec<FnLocal>> = HashMap::default();

        // Insert data with complex nesting
        for i in 0..20 {
            let key = format!("key_{i}");
            let mut vec = Vec::new();

            for j in 0..5 {
                vec.push(FnLocal::new(j, i));
            }

            map.insert(key, vec);
        }

        assert_eq!(map.len(), 20);

        // Test iteration and borrowing
        for (key, vec) in &map {
            assert!(key.starts_with("key_"));
            assert_eq!(vec.len(), 5);

            for fn_local in vec {
                assert!(fn_local.id < 5);
                assert!(fn_local.fn_id < 20);
            }
        }

        // Test modification during iteration (using drain)
        let mut keys_to_remove = Vec::new();
        for key in map.keys() {
            if key.ends_with("_1") || key.ends_with("_2") {
                keys_to_remove.push(key.clone());
            }
        }

        for key in keys_to_remove {
            map.swap_remove(&key);
        }

        assert_eq!(map.len(), 18); // 20 - 2
    }

    #[test]
    fn test_serialization_structures() {
        // Test that our serializable structures don't have memory issues
        // when working with the underlying data (without actual serialization)

        let range = Range::new(Loc(10), Loc(20)).unwrap();
        let fn_local = FnLocal::new(1, 2);

        // Test that Clone and PartialEq work correctly
        let range_clone = range;
        let fn_local_clone = fn_local;

        assert_eq!(range, range_clone);
        assert_eq!(fn_local, fn_local_clone);

        // Test Debug formatting (without actually printing)
        let debug_string = format!("{range:?}");
        assert!(debug_string.contains("Range"));

        let debug_fn_local = format!("{fn_local:?}");
        assert!(debug_fn_local.contains("FnLocal"));
    }

    /// Exercises complex string creation, mutation, searching, slicing, and deduplication to help detect memory-safety issues.
    ///
    /// Builds patterned strings, prepends a prefix and appends a suffix to each, verifies prefix/suffix invariants and
    /// that slicing via `find` yields expected substrings, then deduplicates with a `HashSet` and asserts the deduplicated
    /// count does not exceed the number of original distinct bases.
    ///
    /// # Examples
    ///
    /// ```
    /// // construct and mutate a few patterned strings, then dedupe
    /// let mut v = Vec::new();
    /// for i in 0..3 { v.push(format!("test_{}", i)); }
    /// for s in &mut v { s.insert_str(0, "prefix_"); s.push_str("_suffix"); }
    /// for s in &v { assert!(s.starts_with("prefix_") && s.ends_with("_suffix")); }
    /// if let Some(pos) = v[0].find("test_") { let slice = &v[0][pos..]; assert!(slice.starts_with("test_")); }
    /// let set: std::collections::HashSet<_> = v.into_iter().collect();
    /// assert!(set.len() <= 3);
    /// ```
    #[test]
    fn test_advanced_string_operations() {
        // Test more complex string operations for memory safety
        let mut strings = Vec::with_capacity(100);

        // Test string creation with various patterns
        for i in 0..50 {
            let s = format!("test_{i}");
            strings.push(s);
        }

        // Test string manipulation
        for s in &mut strings {
            s.push_str("_suffix");
            s.insert_str(0, "prefix_");
        }

        // Test string searching and slicing
        for s in &strings {
            assert!(s.starts_with("prefix_"));
            assert!(s.ends_with("_suffix"));

            if let Some(pos) = s.find("test_") {
                let slice = &s[pos..];
                assert!(slice.starts_with("test_"));
            }
        }

        // Test string deduplication
        let mut unique_strings = std::collections::HashSet::new();
        for s in strings {
            unique_strings.insert(s);
        }
        assert_eq!(unique_strings.len(), 50);
    }

    #[test]
    fn test_complex_nested_structures() {
        // Test deeply nested data structures for memory safety
        let mut workspace = Workspace(HashMap::default());
        for crate_idx in 0..10 {
            let mut crate_data = Crate(HashMap::default());
            for file_idx in 0..5 {
                let mut file = File::new();

                for func_idx in 0..3 {
                    let mut function = Function::new(func_idx + file_idx * 3 + crate_idx * 15);

                    // Add basic blocks
                    for bb_idx in 0..4 {
                        let mut basic_block = MirBasicBlock::new();

                        // Add statements
                        for stmt_idx in 0..6 {
                            let range =
                                Range::new(Loc(stmt_idx * 10), Loc(stmt_idx * 10 + 5)).unwrap();

                            basic_block.statements.push(MirStatement::Other { range });
                        }

                        // Add terminator
                        if bb_idx % 2 == 0 {
                            basic_block.terminator = Some(MirTerminator::Other {
                                range: Range::new(Loc(60), Loc(65)).unwrap(),
                            });
                        }

                        function.basic_blocks.push(basic_block);
                    }

                    file.items.push(function);
                }

                crate_data.0.insert(format!("file_{file_idx}.rs"), file);
            }

            workspace.0.insert(format!("crate_{crate_idx}"), crate_data);
        }

        // Verify structure
        assert_eq!(workspace.0.len(), 10);

        for (crate_name, crate_data) in &workspace.0 {
            assert!(crate_name.starts_with("crate_"));
            assert_eq!(crate_data.0.len(), 5);

            for (file_name, file_data) in &crate_data.0 {
                assert!(file_name.starts_with("file_"));
                assert_eq!(file_data.items.len(), 3);

                for function in &file_data.items {
                    assert_eq!(function.basic_blocks.len(), 4);

                    for (bb_idx, basic_block) in function.basic_blocks.iter().enumerate() {
                        assert_eq!(basic_block.statements.len(), 6);
                        if bb_idx % 2 == 0 {
                            assert!(basic_block.terminator.is_some());
                        } else {
                            assert!(basic_block.terminator.is_none());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_memory_intensive_range_operations() {
        // Test range operations with many ranges for memory safety
        let mut ranges = Vec::with_capacity(1000);

        // Create overlapping ranges
        for i in 0..500 {
            let start = i * 2;
            let end = start + 10;
            if let Some(range) = Range::new(Loc(start), Loc(end)) {
                ranges.push(range);
            }
        }

        // Test range merging and elimination
        let eliminated = crate::utils::eliminated_ranges(ranges.clone());
        assert!(eliminated.len() < ranges.len()); // Should merge some ranges
        // Ensure eliminated ranges are non-overlapping
        assert!(
            eliminated
                .windows(2)
                .all(|w| crate::utils::common_range(w[0], w[1]).is_none())
        );
        // Test range exclusion
        let excludes = vec![
            Range::new(Loc(50), Loc(100)).unwrap(),
            Range::new(Loc(200), Loc(250)).unwrap(),
        ];

        let excluded = crate::utils::exclude_ranges(ranges, excludes.clone());
        assert!(!excluded.is_empty());

        // Verify no excluded ranges overlap with exclude regions
        for range in &excluded {
            for exclude in &excludes {
                assert!(crate::utils::common_range(*range, *exclude).is_none());
            }
        }
    }

    #[test]
    fn test_mir_variable_enum_exhaustive() {
        // Test all MirVariable enum variants and operations
        let user_vars = (0..20)
            .map(|i| MirVariable::User {
                index: i,
                live: Range::new(Loc(i * 10), Loc(i * 10 + 5)).unwrap(),
                dead: Range::new(Loc(i * 10 + 5), Loc(i * 10 + 10)).unwrap(),
            })
            .collect::<Vec<_>>();

        let other_vars = (0..20)
            .map(|i| MirVariable::Other {
                index: i + 100,
                live: Range::new(Loc(i * 15), Loc(i * 15 + 7)).unwrap(),
                dead: Range::new(Loc(i * 15 + 7), Loc(i * 15 + 14)).unwrap(),
            })
            .collect::<Vec<_>>();

        // Test pattern matching and extraction
        for var in &user_vars {
            match var {
                MirVariable::User { index, live, dead } => {
                    assert!(*index < 20);
                    assert!(live.size() == 5);
                    assert!(dead.size() == 5);
                    assert_eq!(live.until(), dead.from());
                }
                _ => panic!("Expected User variant"),
            }
        }

        for var in &other_vars {
            match var {
                MirVariable::Other { index, live, dead } => {
                    assert!(*index >= 100);
                    assert!(live.size() == 7);
                    assert!(dead.size() == 7);
                    assert_eq!(live.until(), dead.from());
                }
                _ => panic!("Expected Other variant"),
            }
        }

        // Test collection operations
        let mut all_vars = MirVariables::with_capacity(40);
        for var in user_vars.into_iter().chain(other_vars.into_iter()) {
            all_vars.push(var);
        }

        let final_vars = all_vars.to_vec();
        assert_eq!(final_vars.len(), 40);
    }

    #[test]
    fn test_cache_config_memory_safety() {
        // Test cache configuration structures for memory safety
        use crate::cache::CacheConfig;

        let mut configs = Vec::new();

        // Create configurations with various settings
        for i in 0..50 {
            let config = CacheConfig {
                max_entries: 1000 + i,
                max_memory_bytes: (100 + i) * 1024 * 1024,
                use_lru_eviction: i % 2 == 0,
                validate_file_mtime: i % 3 == 0,
                enable_compression: i % 4 == 0,
            };
            configs.push(config);
        }

        // Test cloning and manipulation
        for config in &configs {
            let cloned = config.clone();
            assert_eq!(config.max_entries, cloned.max_entries);
            assert_eq!(config.max_memory_bytes, cloned.max_memory_bytes);
            assert_eq!(config.use_lru_eviction, cloned.use_lru_eviction);
            assert_eq!(config.validate_file_mtime, cloned.validate_file_mtime);
            assert_eq!(config.enable_compression, cloned.enable_compression);
        }

        // Test debug formatting
        for config in &configs {
            let debug_str = format!("{config:?}");
            assert!(debug_str.contains("CacheConfig"));
            assert!(debug_str.contains(&config.max_entries.to_string()));
        }
    }

    /// Verifies Loc arithmetic is safe around integer boundaries (no wrapping; saturates at zero).
    ///
    /// Tests addition and subtraction on extreme and intermediate Loc values to ensure operations
    /// do not wrap on overflow and underflow and that subtraction saturates at zero where appropriate.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use crate::models::Loc; // adjust path as needed
    /// let max = Loc(u32::MAX);
    /// let min = Loc(0);
    /// assert!((max + 1).0 >= max.0);
    /// assert_eq!((min - 1).0, 0);
    /// ```
    #[test]
    fn test_advanced_arithmetic_safety() {
        // Test arithmetic operations for overflow/underflow safety

        // Test Loc arithmetic with extreme values
        let max_loc = Loc(u32::MAX);
        let min_loc = Loc(0);

        // Test addition near overflow
        let result = max_loc + 1;
        assert_eq!(result.0, max_loc.0); // Saturates at max
        let result = max_loc + (-1);
        assert_eq!(result.0, u32::MAX - 1); // Should subtract correctly

        // Test subtraction near underflow
        let result = min_loc - 1;
        assert_eq!(result.0, 0); // Should saturate at 0

        let result = min_loc + (-10);
        assert_eq!(result.0, 0); // Should saturate at 0

        // Test with intermediate values
        let mid_loc = Loc(u32::MAX / 2);
        let result = mid_loc + (u32::MAX / 2) as i32;
        assert_eq!(result.0, u32::MAX - 1); // Exact expected value
        let result = mid_loc - (u32::MAX / 2) as i32;
        assert_eq!(result.0, 0); // Exact expected value
    }

    #[test]
    fn test_concurrent_like_operations() {
        // Test operations that might be used in concurrent contexts
        // (single-threaded but stress-testing for memory safety)

        use std::sync::Arc;

        let workspace = Arc::new(Workspace(FoldIndexMap::default()));
        let mut handles = Vec::new();

        // Simulate concurrent-like access patterns
        for i in 0..10 {
            let workspace_clone = Arc::clone(&workspace);

            // Create some work that would be done in different "threads"
            let work = move || {
                let _crate_name = format!("crate_{i}");
                let _workspace_ref = &*workspace_clone;

                // Simulate reading from workspace
                for j in 0..5 {
                    let _key = format!("key_{j}");
                    // Would normally do workspace_ref.0.get(&key)
                }
            };

            handles.push(work);
        }

        // Execute all "work" sequentially (since this is single-threaded)
        for work in handles {
            work();
        }

        // Test that Arc and reference counting works correctly
        assert_eq!(Arc::strong_count(&workspace), 1); // Only our reference remains
    }
}

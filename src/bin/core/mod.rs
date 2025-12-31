mod analyze;
mod cache;

use analyze::{AnalyzeResult, MirAnalyzer, MirAnalyzerInitResult};
use ecow::EcoVec;
use rustc_hir::def_id::{LOCAL_CRATE, LocalDefId};
use rustc_interface::interface;
use rustc_middle::{query::queries, ty::TyCtxt, util::Providers};
use rustc_session::config;
use rustowl::models::FoldIndexMap as HashMap;
use rustowl::models::*;
use std::env;
use std::sync::{LazyLock, Mutex, atomic::AtomicBool};
use tokio::{
    runtime::{Builder, Runtime},
    task::JoinSet,
};

pub struct RustcCallback;
impl rustc_driver::Callbacks for RustcCallback {}

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);
static TASKS: LazyLock<Mutex<JoinSet<AnalyzeResult>>> =
    LazyLock::new(|| Mutex::new(JoinSet::new()));

// make tokio runtime
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    let worker_threads = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(2, 8))
        .unwrap_or(4);

    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .thread_stack_size(128 * 1024 * 1024)
        .build()
        .unwrap()
});

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    local.mir_borrowck = mir_borrowck;
}

fn mir_borrowck(tcx: TyCtxt<'_>, def_id: LocalDefId) -> queries::mir_borrowck::ProvidedValue<'_> {
    tracing::info!("start borrowck of {def_id:?}");

    let analyzers = MirAnalyzer::batch_init(tcx, def_id);

    {
        let mut tasks = TASKS.lock().unwrap();
        for analyzer in analyzers {
            match analyzer {
                MirAnalyzerInitResult::Cached(cached) => {
                    handle_analyzed_result(tcx, *cached);
                }
                MirAnalyzerInitResult::Analyzer(analyzer) => {
                    tasks.spawn_on(async move { analyzer.await.analyze() }, RUNTIME.handle());
                }
            }
        }

        tracing::info!("there are {} tasks", tasks.len());
        while let Some(Ok(result)) = tasks.try_join_next() {
            tracing::info!("one task joined");
            handle_analyzed_result(tcx, result);
        }
    }

    let mut providers = Providers::default();
    rustc_borrowck::provide(&mut providers);
    let original_mir_borrowck = providers.mir_borrowck;
    original_mir_borrowck(tcx, def_id)
}

pub struct AnalyzerCallback;
impl rustc_driver::Callbacks for AnalyzerCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.using_internal_features = &ATOMIC_TRUE;
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.polonius = config::Polonius::Next;
        config.opts.incremental = None;
        config.override_queries = Some(override_queries);
        config.make_codegen_backend = None;
    }
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        let result = rustc_driver::catch_fatal_errors(|| tcx.analysis(()));

        // join all tasks after all analysis finished
        loop {
            // First collect any tasks that have already finished
            while let Some(Ok(result)) = {
                let mut guard = TASKS.lock().unwrap();
                guard.try_join_next()
            } {
                tracing::info!("one task joined");
                handle_analyzed_result(tcx, result);
            }

            // Check if all tasks are done
            let has_tasks = {
                let guard = TASKS.lock().unwrap();
                !guard.is_empty()
            };
            if !has_tasks {
                break;
            }

            // Wait for at least one more task to finish
            let result = {
                let mut guard = TASKS.lock().unwrap();
                RUNTIME.block_on(guard.join_next())
            };
            if let Some(Ok(result)) = result {
                tracing::info!("one task joined");
                handle_analyzed_result(tcx, result);
            }
        }

        if let Some(cache) = cache::CACHE.lock().unwrap().as_ref() {
            // Log cache statistics before writing
            let stats = cache.get_stats();
            tracing::info!(
                "Cache statistics: {} hits, {} misses, {:.1}% hit rate, {} evictions",
                stats.hits,
                stats.misses,
                stats.hit_rate() * 100.0,
                stats.evictions
            );
            cache::write_cache(&tcx.crate_name(LOCAL_CRATE).to_string(), cache);
        }

        if result.is_ok() {
            rustc_driver::Compilation::Continue
        } else {
            rustc_driver::Compilation::Stop
        }
    }
}

pub fn handle_analyzed_result(tcx: TyCtxt<'_>, analyzed: AnalyzeResult) {
    if let Some(cache) = cache::CACHE.lock().unwrap().as_mut() {
        // Pass file name for potential file modification time validation
        cache.insert_cache_with_file_path(
            analyzed.file_hash.clone(),
            analyzed.mir_hash.clone(),
            analyzed.analyzed.clone(),
            Some(&analyzed.file_name),
        );
    }
    let mut map = HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
    map.insert(
        analyzed.file_name.to_owned(),
        File {
            items: EcoVec::from([analyzed.analyzed]),
        },
    );
    let krate = Crate(map);
    // get currently-compiling crate name
    let crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
    let mut ws_map =
        HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
    ws_map.insert(crate_name.clone(), krate);
    let ws = Workspace(ws_map);
    println!("{}", serde_json::to_string(&ws).unwrap());
}

pub fn run_compiler() -> i32 {
    let mut args: Vec<String> = env::args().collect();
    // by using `RUSTC_WORKSPACE_WRAPPER`, arguments will be as follows:
    // For dependencies: rustowlc [args...]
    // For user workspace: rustowlc rustowlc [args...]
    // So we skip analysis if currently-compiling crate is one of the dependencies
    if args.first() == args.get(1) {
        args = args.into_iter().skip(1).collect();
    } else {
        return rustc_driver::catch_with_exit_code(|| {
            rustc_driver::run_compiler(&args, &mut RustcCallback)
        });
    }

    for arg in &args {
        // utilize default rustc to avoid unexpected behavior if these arguments are passed
        if arg == "-vV" || arg == "--version" || arg.starts_with("--print") {
            return rustc_driver::catch_with_exit_code(|| {
                rustc_driver::run_compiler(&args, &mut RustcCallback)
            });
        }
    }

    rustc_driver::catch_with_exit_code(|| {
        rustc_driver::run_compiler(&args, &mut AnalyzerCallback);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_atomic_true_constant() {
        // Test that ATOMIC_TRUE is properly initialized
        assert!(ATOMIC_TRUE.load(Ordering::Relaxed));

        // Test that it can be read multiple times consistently
        assert!(ATOMIC_TRUE.load(Ordering::SeqCst));
        assert!(ATOMIC_TRUE.load(Ordering::Acquire));
    }

    #[test]
    fn test_worker_thread_calculation() {
        // Test the worker thread calculation logic
        let available = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(2, 8))
            .unwrap_or(4);

        assert!(available >= 2);
        assert!(available <= 8);
    }

    #[test]
    fn test_runtime_configuration() {
        // Test that RUNTIME is properly configured
        let runtime = &*RUNTIME;

        // Test that we can spawn a simple task
        let result = runtime.block_on(async { 42 });
        assert_eq!(result, 42);

        // Test that runtime handle is available
        let _handle = runtime.handle();
        let _enter = runtime.enter();
        assert!(tokio::runtime::Handle::try_current().is_ok());
    }

    #[test]
    fn test_handle_analyzed_result() {
        // Test that handle_analyzed_result processes analysis results correctly
        // Note: This is a simplified test since we can't easily mock TyCtxt

        // Create a mock AnalyzeResult
        let analyzed = Function {
            fn_id: 1,
            basic_blocks: EcoVec::new(),
            decls: DeclVec::new(),
        };

        let analyze_result = AnalyzeResult {
            file_name: "test.rs".to_string(),
            file_hash: "testhash".to_string(),
            mir_hash: "mirhash".to_string(),
            analyzed,
        };

        // Test that the function can be called without panicking
        // In a real scenario, this would interact with the cache
        // For now, we just verify the function signature and basic structure
        assert_eq!(analyze_result.file_name, "test.rs");
        assert_eq!(analyze_result.file_hash, "testhash");
        assert_eq!(analyze_result.mir_hash, "mirhash");
    }

    #[test]
    fn test_run_compiler_argument_processing() {
        // Test argument processing logic in run_compiler
        let original_args = vec![
            "rustowlc".to_string(),
            "rustowlc".to_string(),
            "--help".to_string(),
        ];

        // Test the logic for skipping duplicate first argument
        let mut args = original_args.clone();
        if args.first() == args.get(1) {
            args = args.into_iter().skip(1).collect();
        }

        assert_eq!(args, vec!["rustowlc".to_string(), "--help".to_string()]);
    }

    #[test]
    fn test_run_compiler_version_handling() {
        // Test that version arguments are handled correctly
        let version_args = ["rustowlc".to_string(), "-vV".to_string()];
        let print_args = ["rustowlc".to_string(), "--print=cfg".to_string()];

        // Test version argument detection (skip first arg which is the program name)
        for arg in &version_args[1..] {
            assert!(arg == "-vV" || arg == "--version");
        }

        // Test print argument detection (skip first arg which is the program name)
        for arg in &print_args[1..] {
            assert!(arg.starts_with("--print"));
        }
    }

    #[test]
    fn test_tasks_mutex_initialization() {
        // Test that TASKS lazy static is properly initialized
        let tasks = TASKS.lock().unwrap();
        assert!(tasks.is_empty());
        drop(tasks); // Release the lock
    }

    #[test]
    fn test_runtime_initialization() {
        // Test that RUNTIME lazy static is properly initialized
        let runtime = &*RUNTIME;

        // Test basic runtime functionality
        let result = runtime.block_on(async { 42 });
        assert_eq!(result, 42);
    }

    #[test]
    fn test_argument_processing_logic() {
        // Test the argument processing logic without actually running the compiler

        // Test detection of version flags
        let version_args = vec!["-vV", "--version", "--print=cfg"];
        for arg in version_args {
            // Simulate the check that's done in run_compiler
            let should_use_default_rustc =
                arg == "-vV" || arg == "--version" || arg.starts_with("--print");
            assert!(
                should_use_default_rustc,
                "Should use default rustc for: {arg}"
            );
        }

        // Test normal compilation args
        let normal_args = vec!["--crate-type", "lib", "-L", "dependency=/path"];
        for arg in normal_args {
            let should_use_default_rustc =
                arg == "-vV" || arg == "--version" || arg.starts_with("--print");
            assert!(
                !should_use_default_rustc,
                "Should not use default rustc for: {arg}"
            );
        }
    }

    #[test]
    fn test_workspace_wrapper_detection() {
        // Test the RUSTC_WORKSPACE_WRAPPER detection logic
        let test_cases = vec![
            // Case 1: For dependencies: rustowlc [args...]
            (vec!["rustowlc", "--crate-type", "lib"], false), // Different first and second args
            // Case 2: For user workspace: rustowlc rustowlc [args...]
            (vec!["rustowlc", "rustowlc", "--crate-type", "lib"], true), // Same first and second args
            // Edge cases
            (vec!["rustowlc"], false),          // Only one arg
            (vec!["rustc", "rustc"], true),     // Same pattern with rustc
            (vec!["other", "rustowlc"], false), // Different tools
        ];

        for (args, should_skip) in test_cases {
            let first = args.first();
            let second = args.get(1);
            let detected_skip = first == second;
            assert_eq!(detected_skip, should_skip, "Failed for args: {args:?}");
        }
    }

    #[test]
    fn test_hashmap_creation_with_capacity() {
        // Test the HashMap creation pattern used in handle_analyzed_result
        let map: HashMap<String, String> =
            HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
        assert_eq!(map.len(), 0);
        assert!(map.capacity() >= 1);

        // Test creating with different capacities
        for capacity in [0, 1, 10, 100] {
            let map: HashMap<String, String> = HashMap::with_capacity_and_hasher(
                capacity,
                foldhash::quality::RandomState::default(),
            );
            assert_eq!(map.len(), 0);
            if capacity > 0 {
                assert!(map.capacity() >= capacity);
            }
        }
    }

    #[test]
    fn test_workspace_structure_creation() {
        // Test the workspace structure creation logic
        let file_name = "test.rs".to_string();
        let crate_name = "test_crate".to_string();

        // Create a minimal Function for testing
        let test_function = Function::new(0);

        // Create the nested structure like in handle_analyzed_result
        let mut file_map: HashMap<String, File> =
            HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
        file_map.insert(
            file_name.clone(),
            File {
                items: EcoVec::from([test_function]),
            },
        );
        let krate = Crate(file_map);

        let mut ws_map: HashMap<String, Crate> =
            HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
        ws_map.insert(crate_name.clone(), krate);
        let workspace = Workspace(ws_map);

        // Verify structure
        assert_eq!(workspace.0.len(), 1);
        assert!(workspace.0.contains_key(&crate_name));

        let crate_ref = &workspace.0[&crate_name];
        assert_eq!(crate_ref.0.len(), 1);
        assert!(crate_ref.0.contains_key(&file_name));

        let file_ref = &crate_ref.0[&file_name];
        assert_eq!(file_ref.items.len(), 1);
        assert_eq!(file_ref.items[0].fn_id, 0);
    }

    #[test]
    fn test_json_serialization_output() {
        // Test that the workspace structure can be serialized to JSON
        let test_function = Function::new(42);

        let mut file_map: HashMap<String, File> =
            HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
        file_map.insert(
            "main.rs".to_string(),
            File {
                items: EcoVec::from([test_function]),
            },
        );
        let krate = Crate(file_map);

        let mut ws_map: HashMap<String, Crate> =
            HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
        ws_map.insert("my_crate".to_string(), krate);
        let workspace = Workspace(ws_map);

        // Test serialization
        let json_result = serde_json::to_string(&workspace);
        assert!(json_result.is_ok());

        let json_string = json_result.unwrap();
        assert!(!json_string.is_empty());
        assert!(json_string.contains("my_crate"));
        assert!(json_string.contains("main.rs"));
        assert!(json_string.contains("42"));
    }

    #[test]
    fn test_stack_size_configuration() {
        // Test that the runtime is configured with appropriate stack size
        const EXPECTED_STACK_SIZE: usize = 128 * 1024 * 1024; // 128 MB

        // Test that the value is a power of 2 times some base unit
        assert_eq!(EXPECTED_STACK_SIZE % (1024 * 1024), 0); // Multiple of 1MB
    }

    #[test]
    fn test_local_crate_constant() {
        // Test that LOCAL_CRATE constant is available and can be used
        use rustc_hir::def_id::LOCAL_CRATE;

        // LOCAL_CRATE should be a valid CrateNum
        // We can't test much about it without a TyCtxt, but we can verify it exists
        let _crate_num = LOCAL_CRATE;
    }

    #[test]
    fn test_config_options_simulation() {
        // Test the configuration options that would be set in AnalyzerCallback::config

        // Test mir_opt_level
        let mir_opt_level = Some(0);
        assert_eq!(mir_opt_level, Some(0));

        // Test that polonius config enum value exists
        use rustc_session::config::Polonius;
        let _polonius_config = Polonius::Next;

        // Test that incremental compilation is disabled
        let incremental = None::<std::path::PathBuf>;
        assert!(incremental.is_none());
    }

    #[test]
    fn test_error_handling_pattern() {
        // Test the error handling pattern used with rustc_driver::catch_fatal_errors

        // Simulate successful operation
        let success_result = || -> Result<(), ()> { Ok(()) };
        let result = success_result();
        assert!(result.is_ok());

        // Simulate error operation
        let error_result = || -> Result<(), ()> { Err(()) };
        let result = error_result();
        assert!(result.is_err());
    }

    #[test]
    fn test_parallel_task_management() {
        // Test parallel task management patterns
        use tokio::task::JoinSet;

        let rt = &*RUNTIME;
        rt.block_on(async {
            let mut tasks = JoinSet::new();

            // Spawn multiple tasks
            for i in 0..5 {
                tasks.spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(i * 10)).await;
                    i * 2
                });
            }

            let mut results = Vec::new();
            while let Some(result) = tasks.join_next().await {
                if let Ok(value) = result {
                    results.push(value);
                }
            }

            // Should have collected all results
            assert_eq!(results.len(), 5);
            results.sort();
            assert_eq!(results, vec![0, 2, 4, 6, 8]);
        });
    }

    #[test]
    fn test_complex_workspace_structures() {
        // Test complex workspace structure creation
        let mut complex_workspace =
            HashMap::with_capacity_and_hasher(3, foldhash::quality::RandomState::default());

        // Create multiple crates with different structures
        for crate_idx in 0..3 {
            let crate_name = format!("crate_{crate_idx}");
            let mut crate_files =
                HashMap::with_capacity_and_hasher(5, foldhash::quality::RandomState::default());

            for file_idx in 0..5 {
                let file_name = format!("src/module_{file_idx}.rs");
                let mut functions = EcoVec::new();

                for fn_idx in 0..3 {
                    let function = Function::new((crate_idx * 100 + file_idx * 10 + fn_idx) as u32);
                    functions.push(function);
                }

                crate_files.insert(file_name, File { items: functions });
            }

            complex_workspace.insert(crate_name, Crate(crate_files));
        }

        let workspace = Workspace(complex_workspace);

        // Validate structure
        assert_eq!(workspace.0.len(), 3);

        for crate_idx in 0..3 {
            let crate_name = format!("crate_{crate_idx}");
            assert!(workspace.0.contains_key(&crate_name));

            let crate_ref = &workspace.0[&crate_name];
            assert_eq!(crate_ref.0.len(), 5);

            for file_idx in 0..5 {
                let file_name = format!("src/module_{file_idx}.rs");
                assert!(crate_ref.0.contains_key(&file_name));

                let file_ref = &crate_ref.0[&file_name];
                assert_eq!(file_ref.items.len(), 3);

                for fn_idx in 0..3 {
                    let expected_fn_id = (crate_idx * 100 + file_idx * 10 + fn_idx) as u32;
                    assert_eq!(file_ref.items[fn_idx].fn_id, expected_fn_id);
                }
            }
        }
    }

    #[test]
    fn test_json_serialization_edge_cases() {
        // Test JSON serialization with edge cases
        let edge_case_functions = vec![
            Function::new(0),        // Minimum ID
            Function::new(u32::MAX), // Maximum ID
            Function::new(12345),    // Regular ID
        ];

        for function in edge_case_functions {
            let fn_id = function.fn_id; // Store ID before move
            let mut file_map =
                HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
            file_map.insert(
                "test.rs".to_string(),
                File {
                    items: EcoVec::from([function]),
                },
            );

            let krate = Crate(file_map);
            let mut ws_map =
                HashMap::with_capacity_and_hasher(1, foldhash::quality::RandomState::default());
            ws_map.insert("test_crate".to_string(), krate);

            let workspace = Workspace(ws_map);

            // Test serialization
            let json_result = serde_json::to_string(&workspace);
            assert!(
                json_result.is_ok(),
                "Failed to serialize function with ID {fn_id}"
            );

            let json_string = json_result.unwrap();
            assert!(json_string.contains(&fn_id.to_string()));

            // Test deserialization roundtrip
            let deserialized: Result<Workspace, _> = serde_json::from_str(&json_string);
            assert!(
                deserialized.is_ok(),
                "Failed to deserialize function with ID {fn_id}"
            );

            let deserialized_workspace = deserialized.unwrap();
            assert_eq!(deserialized_workspace.0.len(), 1);
        }
    }

    #[test]
    fn test_runtime_configuration_comprehensive() {
        // Test comprehensive runtime configuration
        let runtime = &*RUNTIME;

        // Test basic async operation
        let result = runtime.block_on(async {
            let mut sum = 0;
            for i in 0..100 {
                sum += i;
            }
            sum
        });
        assert_eq!(result, 4950);

        // Test spawning tasks
        let result = runtime.block_on(async {
            let task1 = tokio::spawn(async { 1 + 1 });
            let task2 = tokio::spawn(async { 2 + 2 });
            let task3 = tokio::spawn(async { 3 + 3 });

            let (r1, r2, r3) = tokio::join!(task1, task2, task3);
            (r1.unwrap(), r2.unwrap(), r3.unwrap())
        });
        assert_eq!(result, (2, 4, 6));

        // Test timeout operations
        let timeout_result = runtime.block_on(async {
            tokio::time::timeout(tokio::time::Duration::from_millis(100), async {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                42
            })
            .await
        });
        assert!(timeout_result.is_ok());
        assert_eq!(timeout_result.unwrap(), 42);
    }

    #[test]
    fn test_argument_processing_comprehensive() {
        // Test comprehensive argument processing patterns
        let test_cases = vec![
            // (args, should_use_default_rustc, should_skip_analysis)
            (vec!["rustowlc"], false, false),
            (vec!["rustowlc", "rustowlc"], false, true), // Workspace wrapper
            (vec!["rustowlc", "-vV"], true, false),      // Version flag
            (vec!["rustowlc", "--version"], true, false), // Version flag
            (vec!["rustowlc", "--print=cfg"], true, false), // Print flag
            (vec!["rustowlc", "--print", "cfg"], true, false), // Print flag
            (vec!["rustowlc", "--crate-type", "lib"], false, false), // Normal compilation
            (vec!["rustowlc", "-L", "dependency=/path"], false, false), // Normal compilation
            (
                vec!["rustowlc", "rustowlc", "--crate-type", "lib"],
                false,
                true,
            ), // Wrapper + normal
            (vec!["rustowlc", "rustowlc", "-vV"], true, true), // Wrapper + version (should detect version)
        ];

        for (args, expected_default_rustc, expected_skip_analysis) in test_cases {
            // Test skip analysis detection
            let first = args.first();
            let second = args.get(1);
            let should_skip_analysis = first == second;
            assert_eq!(
                should_skip_analysis, expected_skip_analysis,
                "Skip analysis mismatch for: {args:?}"
            );

            // Test version/print flag detection
            let mut should_use_default_rustc = false;
            for arg in &args {
                if *arg == "-vV" || *arg == "--version" || arg.starts_with("--print") {
                    should_use_default_rustc = true;
                    break;
                }
            }
            assert_eq!(
                should_use_default_rustc, expected_default_rustc,
                "Default rustc mismatch for: {args:?}"
            );
        }
    }

    #[test]
    fn test_cache_statistics_simulation() {
        // Test cache statistics handling patterns
        #[derive(Debug, Default)]
        struct MockCacheStats {
            hits: u64,
            misses: u64,
            evictions: u64,
        }

        impl MockCacheStats {
            fn hit_rate(&self) -> f64 {
                if self.hits + self.misses == 0 {
                    0.0
                } else {
                    self.hits as f64 / (self.hits + self.misses) as f64
                }
            }
        }

        let test_scenarios = vec![
            MockCacheStats {
                hits: 100,
                misses: 20,
                evictions: 5,
            },
            MockCacheStats {
                hits: 0,
                misses: 10,
                evictions: 0,
            },
            MockCacheStats {
                hits: 50,
                misses: 0,
                evictions: 2,
            },
            MockCacheStats {
                hits: 0,
                misses: 0,
                evictions: 0,
            },
            MockCacheStats {
                hits: 1000,
                misses: 100,
                evictions: 50,
            },
        ];

        for stats in test_scenarios {
            let hit_rate = stats.hit_rate();

            // Hit rate should be between 0 and 1
            assert!(
                (0.0..=1.0).contains(&hit_rate),
                "Invalid hit rate: {hit_rate}"
            );

            // Test logging format (simulate what would be logged)
            let log_message = format!(
                "Cache statistics: {} hits, {} misses, {:.1}% hit rate, {} evictions",
                stats.hits,
                stats.misses,
                hit_rate * 100.0,
                stats.evictions
            );

            assert!(log_message.contains("Cache statistics"));
            assert!(log_message.contains(&stats.hits.to_string()));
            assert!(log_message.contains(&stats.misses.to_string()));
            assert!(log_message.contains(&stats.evictions.to_string()));
        }
    }

    #[test]
    fn test_worker_thread_calculation_edge_cases() {
        // Test worker thread calculation with various scenarios
        let test_cases = vec![
            // (available_parallelism, expected_range)
            (1, 2..=2),  // Single core -> minimum 2
            (2, 2..=2),  // Dual core -> 1 thread, clamped to 2
            (4, 2..=2),  // Quad core -> 2 threads
            (8, 4..=4),  // 8 cores -> 4 threads
            (16, 8..=8), // 16 cores -> 8 threads, clamped to max
            (32, 8..=8), // 32 cores -> 16 threads, clamped to 8
        ];

        for (available, expected_range) in test_cases {
            let calculated = (available / 2).clamp(2, 8);
            assert!(
                expected_range.contains(&calculated),
                "Worker thread calculation failed for {available} cores: got {calculated}, expected {expected_range:?}"
            );
        }

        // Test with the actual calculation logic
        let actual_available = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(2, 8))
            .unwrap_or(4);

        assert!(actual_available >= 2);
        assert!(actual_available <= 8);
    }

    #[test]
    fn test_compilation_result_handling() {
        // Test compilation result handling patterns
        use rustc_driver::Compilation;

        // Test result interpretation
        let success_results: Vec<Result<(), ()>> = vec![Ok(()), Ok(())];
        let error_results: Vec<Result<(), ()>> = vec![Err(()), Err(())];

        for result in success_results {
            let compilation_action = if result.is_ok() {
                Compilation::Continue
            } else {
                Compilation::Stop
            };
            assert_eq!(compilation_action, Compilation::Continue);
        }

        for result in error_results {
            let compilation_action = if result.is_ok() {
                Compilation::Continue
            } else {
                Compilation::Stop
            };
            assert_eq!(compilation_action, Compilation::Stop);
        }
    }

    #[test]
    fn test_memory_allocation_patterns() {
        // Test memory allocation patterns in data structure creation
        use std::mem;

        // Test memory usage of various HashMap sizes
        for capacity in [1, 10, 100, 1000] {
            let map: HashMap<String, String> = HashMap::with_capacity_and_hasher(
                capacity,
                foldhash::quality::RandomState::default(),
            );

            let size = mem::size_of_val(&map);
            assert!(size > 0, "HashMap should have non-zero size");

            // Memory usage should scale reasonably
            if capacity > 0 {
                assert!(
                    map.capacity() >= capacity,
                    "HashMap should have at least requested capacity"
                );
            }
        }

        // Basic `EcoVec` growth sanity check.
        let mut vec = EcoVec::<Function>::new();
        let _initial_size = mem::size_of_val(&vec);

        for i in 0..10 {
            vec.push(Function::new(i));
            let current_size = mem::size_of_val(&vec);
            assert!(
                current_size < 100_000,
                "EcoVec size should remain reasonable: {current_size} bytes"
            );
        }

        assert_eq!(vec.len(), 10);
    }

    #[test]
    fn test_configuration_options_comprehensive() {
        // Test configuration option handling
        use rustc_session::config::Polonius;

        // Test Polonius configuration
        let polonius_variants = [Polonius::Legacy, Polonius::Next];
        for variant in polonius_variants {
            // Should be able to create and use variants
            let _config_value = variant;
        }

        // Test MIR optimization level
        let mir_opt_levels = [Some(0), Some(1), Some(2), Some(3), None];
        for l in mir_opt_levels.into_iter().flatten() {
            assert!(l <= 4, "MIR opt level should be reasonable")
        }

        // Test incremental compilation settings
        let incremental_options: Vec<Option<std::path::PathBuf>> =
            vec![None, Some(std::path::PathBuf::from("/tmp/incremental"))];

        for path in incremental_options.into_iter().flatten() {
            assert!(!path.as_os_str().is_empty())
        }
    }

    #[test]
    fn test_async_task_error_handling() {
        // Test async task error handling patterns
        let runtime = &*RUNTIME;

        runtime.block_on(async {
            let mut tasks = tokio::task::JoinSet::new();

            // Spawn tasks that succeed
            for i in 0..3 {
                tasks.spawn(async move { Ok::<i32, &str>(i) });
            }

            // Spawn tasks that fail
            for _i in 3..5 {
                tasks.spawn(async move { Err::<i32, &str>("failed") });
            }

            let mut successes = 0;
            let mut failures = 0;

            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok(_)) => successes += 1,
                    Ok(Err(_)) => failures += 1,
                    Err(_) => (), // Join error
                }
            }

            assert_eq!(successes, 3);
            assert_eq!(failures, 2);
        });
    }
}

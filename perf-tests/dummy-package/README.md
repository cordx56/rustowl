# RustOwl Performance Test Dummy Package

This is a dummy Rust package specifically designed for performance testing with RustOwl. The package contains various Rust patterns and constructs that RustOwl can analyze, including:

## Features Tested

### Ownership and Borrowing Issues

- Methods returning references to internal data
- Potential lifetime issues
- Clone operations that might be unnecessary

### Error Handling Patterns

- Use of `unwrap()` that might panic
- Proper error handling with `Result` and `Context`
- Missing error propagation

### Resource Management

- File handling and potential resource leaks
- Thread safety and potential deadlocks
- Memory allocation patterns

### Async/Concurrency Patterns

- Async operations with tokio
- Thread-based parallel processing
- Shared state with `Arc<Mutex<T>>`

### Performance Patterns

- Memory-intensive operations
- CPU-intensive computations
- I/O operations

## Usage

The dummy package can be built and run to provide realistic workload for RustOwl analysis:

```bash
cargo build --release
cargo run -- --help
```

## Operations

The dummy application supports several operations:

- `data` - Run data structure operations with potential ownership issues
- `network` - Run network operations (may fail without internet)
- `files` - Run file I/O operations
- `compute` - Run CPU-intensive computations
- `all` - Run all operations

## Performance Testing

This package is used by the RustOwl performance testing script (`perf-tests.sh`) to provide a consistent, realistic codebase for performance analysis.

The code intentionally contains patterns that RustOwl can analyze and potentially flag, making it an ideal test case for measuring RustOwl's analysis performance on real-world-like Rust code.

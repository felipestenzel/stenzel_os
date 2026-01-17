//! Kernel Test Framework
//!
//! A lightweight testing framework for no_std kernel testing.
//! Supports unit tests, integration tests, and benchmarks.

extern crate alloc;

use alloc::string::String;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};

pub mod memory;
pub mod scheduler;
pub mod filesystem;
pub mod network;
pub mod syscall;
pub mod integration;
pub mod stress;
pub mod hardware;
pub mod automation;

/// Test result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestResult {
    Pass,
    Fail,
    Skip,
    Timeout,
}

/// Test outcome with details
#[derive(Debug, Clone)]
pub struct TestOutcome {
    pub name: String,
    pub result: TestResult,
    pub message: Option<String>,
    pub duration_us: u64,
}

/// Test function type
pub type TestFn = fn() -> TestResult;

/// Test definition
pub struct TestDef {
    pub name: &'static str,
    pub func: TestFn,
    pub category: &'static str,
    pub timeout_ms: u32,
}

/// Test statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct TestStats {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub timeout: u32,
}

/// Global test state
static TESTS_RUNNING: AtomicBool = AtomicBool::new(false);
static CURRENT_TEST: AtomicU32 = AtomicU32::new(0);

/// Test runner
pub struct TestRunner {
    tests: Vec<TestDef>,
    outcomes: Vec<TestOutcome>,
    stats: TestStats,
    verbose: bool,
}

impl TestRunner {
    /// Create a new test runner
    pub fn new() -> Self {
        Self {
            tests: Vec::new(),
            outcomes: Vec::new(),
            stats: TestStats::default(),
            verbose: true,
        }
    }

    /// Set verbose mode
    pub fn verbose(mut self, v: bool) -> Self {
        self.verbose = v;
        self
    }

    /// Register a test
    pub fn add_test(&mut self, name: &'static str, func: TestFn, category: &'static str) {
        self.tests.push(TestDef {
            name,
            func,
            category,
            timeout_ms: 5000, // 5 second default timeout
        });
    }

    /// Register a test with custom timeout
    pub fn add_test_with_timeout(&mut self, name: &'static str, func: TestFn, category: &'static str, timeout_ms: u32) {
        self.tests.push(TestDef {
            name,
            func,
            category,
            timeout_ms,
        });
    }

    /// Run all tests
    pub fn run_all(&mut self) -> &TestStats {
        TESTS_RUNNING.store(true, Ordering::SeqCst);

        crate::kprintln!("\n========================================");
        crate::kprintln!("     Stenzel OS Kernel Test Suite");
        crate::kprintln!("========================================\n");
        crate::kprintln!("Running {} tests...\n", self.tests.len());

        self.stats.total = self.tests.len() as u32;

        for (i, test) in self.tests.iter().enumerate() {
            CURRENT_TEST.store(i as u32, Ordering::SeqCst);

            if self.verbose {
                crate::kprint!("  [{:3}/{}] {} ... ", i + 1, self.tests.len(), test.name);
            }

            let start = get_time_us();
            let result = self.run_single_test(test);
            let duration = get_time_us() - start;

            let outcome = TestOutcome {
                name: String::from(test.name),
                result,
                message: None,
                duration_us: duration,
            };

            match result {
                TestResult::Pass => {
                    self.stats.passed += 1;
                    if self.verbose {
                        crate::kprintln!("\x1b[32mPASS\x1b[0m ({} us)", duration);
                    }
                }
                TestResult::Fail => {
                    self.stats.failed += 1;
                    if self.verbose {
                        crate::kprintln!("\x1b[31mFAIL\x1b[0m ({} us)", duration);
                    }
                }
                TestResult::Skip => {
                    self.stats.skipped += 1;
                    if self.verbose {
                        crate::kprintln!("\x1b[33mSKIP\x1b[0m");
                    }
                }
                TestResult::Timeout => {
                    self.stats.timeout += 1;
                    if self.verbose {
                        crate::kprintln!("\x1b[31mTIMEOUT\x1b[0m ({}ms limit)", test.timeout_ms);
                    }
                }
            }

            self.outcomes.push(outcome);
        }

        TESTS_RUNNING.store(false, Ordering::SeqCst);

        self.print_summary();

        &self.stats
    }

    /// Run tests in a specific category
    pub fn run_category(&mut self, category: &str) -> &TestStats {
        let tests: Vec<_> = self.tests.iter()
            .filter(|t| t.category == category)
            .cloned()
            .collect();

        // ... similar to run_all but filtered
        &self.stats
    }

    /// Run a single test
    fn run_single_test(&self, test: &TestDef) -> TestResult {
        // In a real implementation, we would:
        // 1. Set up a timeout timer
        // 2. Catch panics
        // 3. Run the test function
        // 4. Return the result

        // For now, just run the test directly
        (test.func)()
    }

    /// Print test summary
    fn print_summary(&self) {
        crate::kprintln!("\n========================================");
        crate::kprintln!("             Test Summary");
        crate::kprintln!("========================================");
        crate::kprintln!("  Total:   {}", self.stats.total);
        crate::kprintln!("  \x1b[32mPassed:\x1b[0m  {}", self.stats.passed);
        crate::kprintln!("  \x1b[31mFailed:\x1b[0m  {}", self.stats.failed);
        crate::kprintln!("  \x1b[33mSkipped:\x1b[0m {}", self.stats.skipped);
        crate::kprintln!("  Timeout: {}", self.stats.timeout);
        crate::kprintln!("========================================\n");

        if self.stats.failed == 0 && self.stats.timeout == 0 {
            crate::kprintln!("\x1b[32mAll tests passed!\x1b[0m\n");
        } else {
            crate::kprintln!("\x1b[31mSome tests failed!\x1b[0m\n");

            // Print failed tests
            crate::kprintln!("Failed tests:");
            for outcome in &self.outcomes {
                if outcome.result == TestResult::Fail || outcome.result == TestResult::Timeout {
                    crate::kprintln!("  - {}", outcome.name);
                }
            }
        }
    }

    /// Get test outcomes
    pub fn outcomes(&self) -> &[TestOutcome] {
        &self.outcomes
    }
}

impl Clone for TestDef {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            func: self.func,
            category: self.category,
            timeout_ms: self.timeout_ms,
        }
    }
}

// ============================================================================
// Assertion macros
// ============================================================================

/// Assert that a condition is true
#[macro_export]
macro_rules! test_assert {
    ($cond:expr) => {
        if !$cond {
            crate::kprintln!("    Assertion failed: {}", stringify!($cond));
            return $crate::tests::TestResult::Fail;
        }
    };
    ($cond:expr, $msg:expr) => {
        if !$cond {
            crate::kprintln!("    Assertion failed: {} - {}", stringify!($cond), $msg);
            return $crate::tests::TestResult::Fail;
        }
    };
}

/// Assert equality
#[macro_export]
macro_rules! test_assert_eq {
    ($left:expr, $right:expr) => {
        if $left != $right {
            crate::kprintln!("    Assertion failed: {} == {}", stringify!($left), stringify!($right));
            crate::kprintln!("    Left:  {:?}", $left);
            crate::kprintln!("    Right: {:?}", $right);
            return $crate::tests::TestResult::Fail;
        }
    };
}

/// Assert inequality
#[macro_export]
macro_rules! test_assert_ne {
    ($left:expr, $right:expr) => {
        if $left == $right {
            crate::kprintln!("    Assertion failed: {} != {}", stringify!($left), stringify!($right));
            crate::kprintln!("    Both equal: {:?}", $left);
            return $crate::tests::TestResult::Fail;
        }
    };
}

/// Assert that an option is Some
#[macro_export]
macro_rules! test_assert_some {
    ($opt:expr) => {
        if $opt.is_none() {
            crate::kprintln!("    Assertion failed: {} is None, expected Some", stringify!($opt));
            return $crate::tests::TestResult::Fail;
        }
    };
}

/// Assert that a result is Ok
#[macro_export]
macro_rules! test_assert_ok {
    ($result:expr) => {
        if $result.is_err() {
            crate::kprintln!("    Assertion failed: {} is Err, expected Ok", stringify!($result));
            return $crate::tests::TestResult::Fail;
        }
    };
}

/// Assert that a result is Err
#[macro_export]
macro_rules! test_assert_err {
    ($result:expr) => {
        if $result.is_ok() {
            crate::kprintln!("    Assertion failed: {} is Ok, expected Err", stringify!($result));
            return $crate::tests::TestResult::Fail;
        }
    };
}

// ============================================================================
// Test registration macro
// ============================================================================

/// Register tests in a module
#[macro_export]
macro_rules! register_tests {
    ($runner:expr, $category:expr, $($name:ident),* $(,)?) => {
        $(
            $runner.add_test(stringify!($name), $name, $category);
        )*
    };
}

// ============================================================================
// Built-in tests
// ============================================================================

/// Run all kernel tests
pub fn run_all_tests() -> TestStats {
    let mut runner = TestRunner::new();

    // Register core tests
    register_core_tests(&mut runner);

    // Register memory tests
    memory::register_tests(&mut runner);

    // Register scheduler tests
    scheduler::register_tests(&mut runner);

    // Register filesystem tests
    filesystem::register_tests(&mut runner);

    // Register network tests
    network::register_tests(&mut runner);

    // Register syscall tests
    syscall::register_tests(&mut runner);

    // Register integration tests
    integration::register_tests(&mut runner);

    // Register stress tests
    stress::register_tests(&mut runner);

    // Register hardware tests
    hardware::register_tests(&mut runner);

    // Run all tests
    runner.run_all().clone()
}

/// Register core kernel tests
fn register_core_tests(runner: &mut TestRunner) {
    runner.add_test("core::basic_arithmetic", test_basic_arithmetic, "core");
    runner.add_test("core::string_operations", test_string_operations, "core");
    runner.add_test("core::vec_operations", test_vec_operations, "core");
    runner.add_test("core::option_operations", test_option_operations, "core");
    runner.add_test("core::result_operations", test_result_operations, "core");
}

fn test_basic_arithmetic() -> TestResult {
    test_assert_eq!(2 + 2, 4);
    test_assert_eq!(10 - 3, 7);
    test_assert_eq!(6 * 7, 42);
    test_assert_eq!(100 / 10, 10);
    test_assert_eq!(17 % 5, 2);
    TestResult::Pass
}

fn test_string_operations() -> TestResult {
    let s1 = String::from("Hello");
    let s2 = String::from(", World!");
    let s3 = format!("{}{}", s1, s2);

    test_assert_eq!(s3, "Hello, World!");
    test_assert_eq!(s3.len(), 13);
    test_assert!(s3.contains("World"));
    test_assert!(s3.starts_with("Hello"));
    test_assert!(s3.ends_with("!"));

    TestResult::Pass
}

fn test_vec_operations() -> TestResult {
    let mut v: Vec<i32> = Vec::new();

    test_assert!(v.is_empty());

    v.push(1);
    v.push(2);
    v.push(3);

    test_assert_eq!(v.len(), 3);
    test_assert_eq!(v[0], 1);
    test_assert_eq!(v[2], 3);

    let sum: i32 = v.iter().sum();
    test_assert_eq!(sum, 6);

    v.pop();
    test_assert_eq!(v.len(), 2);

    TestResult::Pass
}

fn test_option_operations() -> TestResult {
    let some_val: Option<i32> = Some(42);
    let none_val: Option<i32> = None;

    test_assert!(some_val.is_some());
    test_assert!(none_val.is_none());
    test_assert_eq!(some_val.unwrap(), 42);
    test_assert_eq!(none_val.unwrap_or(0), 0);

    let mapped = some_val.map(|x| x * 2);
    test_assert_eq!(mapped, Some(84));

    TestResult::Pass
}

fn test_result_operations() -> TestResult {
    let ok_val: Result<i32, &str> = Ok(42);
    let err_val: Result<i32, &str> = Err("error");

    test_assert!(ok_val.is_ok());
    test_assert!(err_val.is_err());
    test_assert_eq!(ok_val.unwrap(), 42);
    test_assert_eq!(err_val.unwrap_or(0), 0);

    TestResult::Pass
}

// ============================================================================
// Helper functions
// ============================================================================

fn get_time_us() -> u64 {
    // Would use TSC or HPET
    0
}

/// Check if tests are currently running
pub fn tests_running() -> bool {
    TESTS_RUNNING.load(Ordering::Relaxed)
}

/// Initialize the test framework
pub fn init() {
    crate::kprintln!("Test framework initialized");
}

//! Automated Testing Infrastructure
//!
//! Provides support for CI/CD integration and automated test execution.

use super::{TestRunner, TestResult, TestStats, TestOutcome};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

/// Test report format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// Human-readable text output
    Text,
    /// JSON format for CI tools
    Json,
    /// JUnit XML format for CI/CD systems
    JUnit,
    /// TAP (Test Anything Protocol)
    Tap,
}

/// Test filter configuration
#[derive(Debug, Clone)]
pub struct TestFilter {
    /// Categories to include (empty = all)
    pub categories: Vec<String>,
    /// Test name patterns to include
    pub include_patterns: Vec<String>,
    /// Test name patterns to exclude
    pub exclude_patterns: Vec<String>,
    /// Only run tests that failed last time
    pub failed_only: bool,
}

impl Default for TestFilter {
    fn default() -> Self {
        Self {
            categories: Vec::new(),
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            failed_only: false,
        }
    }
}

/// CI/CD configuration
#[derive(Debug, Clone)]
pub struct CiConfig {
    /// Exit code on failure
    pub fail_fast: bool,
    /// Maximum parallel test execution
    pub parallelism: u32,
    /// Timeout multiplier for CI
    pub timeout_multiplier: f32,
    /// Report format
    pub format: ReportFormat,
    /// Retry failed tests
    pub retry_count: u32,
}

impl Default for CiConfig {
    fn default() -> Self {
        Self {
            fail_fast: false,
            parallelism: 1,
            timeout_multiplier: 2.0,
            format: ReportFormat::Text,
            retry_count: 0,
        }
    }
}

/// Test execution result for CI
#[derive(Debug)]
pub struct TestExecutionResult {
    pub stats: TestStats,
    pub outcomes: Vec<TestOutcome>,
    pub duration_ms: u64,
    pub success: bool,
}

/// Automated test executor
pub struct AutomatedTestRunner {
    config: CiConfig,
    filter: TestFilter,
}

impl AutomatedTestRunner {
    pub fn new() -> Self {
        Self {
            config: CiConfig::default(),
            filter: TestFilter::default(),
        }
    }

    pub fn with_config(mut self, config: CiConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_filter(mut self, filter: TestFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Run all tests with automation support
    pub fn run(&self) -> TestExecutionResult {
        let mut runner = TestRunner::new();

        // Register all test categories
        super::register_core_tests(&mut runner);
        super::memory::register_tests(&mut runner);
        super::scheduler::register_tests(&mut runner);
        super::filesystem::register_tests(&mut runner);
        super::network::register_tests(&mut runner);
        super::syscall::register_tests(&mut runner);
        super::integration::register_tests(&mut runner);
        super::stress::register_tests(&mut runner);
        super::hardware::register_tests(&mut runner);

        // Run tests
        let stats = runner.run_all().clone();

        TestExecutionResult {
            success: stats.failed == 0 && stats.timeout == 0,
            stats,
            outcomes: runner.outcomes().to_vec(),
            duration_ms: 0, // Would be set by actual timing
        }
    }

    /// Generate test report
    pub fn generate_report(&self, result: &TestExecutionResult) -> String {
        match self.config.format {
            ReportFormat::Text => self.generate_text_report(result),
            ReportFormat::Json => self.generate_json_report(result),
            ReportFormat::JUnit => self.generate_junit_report(result),
            ReportFormat::Tap => self.generate_tap_report(result),
        }
    }

    fn generate_text_report(&self, result: &TestExecutionResult) -> String {
        let mut report = String::new();

        report.push_str("========================================\n");
        report.push_str("     Stenzel OS Test Report\n");
        report.push_str("========================================\n\n");

        report.push_str(&format!("Total:   {}\n", result.stats.total));
        report.push_str(&format!("Passed:  {}\n", result.stats.passed));
        report.push_str(&format!("Failed:  {}\n", result.stats.failed));
        report.push_str(&format!("Skipped: {}\n", result.stats.skipped));
        report.push_str(&format!("Timeout: {}\n\n", result.stats.timeout));

        if result.success {
            report.push_str("Result: PASS\n");
        } else {
            report.push_str("Result: FAIL\n\n");
            report.push_str("Failed tests:\n");
            for outcome in &result.outcomes {
                if outcome.result == TestResult::Fail || outcome.result == TestResult::Timeout {
                    report.push_str(&format!("  - {}\n", outcome.name));
                }
            }
        }

        report
    }

    fn generate_json_report(&self, result: &TestExecutionResult) -> String {
        let mut json = String::from("{\n");

        json.push_str(&format!("  \"total\": {},\n", result.stats.total));
        json.push_str(&format!("  \"passed\": {},\n", result.stats.passed));
        json.push_str(&format!("  \"failed\": {},\n", result.stats.failed));
        json.push_str(&format!("  \"skipped\": {},\n", result.stats.skipped));
        json.push_str(&format!("  \"timeout\": {},\n", result.stats.timeout));
        json.push_str(&format!("  \"success\": {},\n", result.success));
        json.push_str("  \"tests\": [\n");

        for (i, outcome) in result.outcomes.iter().enumerate() {
            let result_str = match outcome.result {
                TestResult::Pass => "pass",
                TestResult::Fail => "fail",
                TestResult::Skip => "skip",
                TestResult::Timeout => "timeout",
            };
            json.push_str(&format!(
                "    {{\"name\": \"{}\", \"result\": \"{}\", \"duration_us\": {}}}",
                outcome.name, result_str, outcome.duration_us
            ));
            if i < result.outcomes.len() - 1 {
                json.push_str(",");
            }
            json.push_str("\n");
        }

        json.push_str("  ]\n");
        json.push_str("}\n");

        json
    }

    fn generate_junit_report(&self, result: &TestExecutionResult) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");

        xml.push_str(&format!(
            "<testsuite name=\"stenzel_os\" tests=\"{}\" failures=\"{}\" skipped=\"{}\">\n",
            result.stats.total, result.stats.failed + result.stats.timeout, result.stats.skipped
        ));

        for outcome in &result.outcomes {
            let time_sec = outcome.duration_us as f64 / 1_000_000.0;
            xml.push_str(&format!(
                "  <testcase name=\"{}\" time=\"{:.6}\"",
                outcome.name, time_sec
            ));

            match outcome.result {
                TestResult::Pass => {
                    xml.push_str("/>\n");
                }
                TestResult::Fail => {
                    xml.push_str(">\n");
                    xml.push_str("    <failure message=\"Test failed\"/>\n");
                    xml.push_str("  </testcase>\n");
                }
                TestResult::Skip => {
                    xml.push_str(">\n");
                    xml.push_str("    <skipped/>\n");
                    xml.push_str("  </testcase>\n");
                }
                TestResult::Timeout => {
                    xml.push_str(">\n");
                    xml.push_str("    <failure message=\"Test timed out\"/>\n");
                    xml.push_str("  </testcase>\n");
                }
            }
        }

        xml.push_str("</testsuite>\n");
        xml
    }

    fn generate_tap_report(&self, result: &TestExecutionResult) -> String {
        let mut tap = format!("TAP version 13\n1..{}\n", result.stats.total);

        for (i, outcome) in result.outcomes.iter().enumerate() {
            let test_num = i + 1;
            match outcome.result {
                TestResult::Pass => {
                    tap.push_str(&format!("ok {} - {}\n", test_num, outcome.name));
                }
                TestResult::Fail => {
                    tap.push_str(&format!("not ok {} - {}\n", test_num, outcome.name));
                }
                TestResult::Skip => {
                    tap.push_str(&format!("ok {} - {} # SKIP\n", test_num, outcome.name));
                }
                TestResult::Timeout => {
                    tap.push_str(&format!("not ok {} - {} # TIMEOUT\n", test_num, outcome.name));
                }
            }
        }

        tap
    }
}

/// Run tests for CI/CD
pub fn run_ci_tests() -> TestExecutionResult {
    let runner = AutomatedTestRunner::new()
        .with_config(CiConfig {
            fail_fast: false,
            format: ReportFormat::Text,
            ..CiConfig::default()
        });

    runner.run()
}

/// Run tests with specific format
pub fn run_tests_with_format(format: ReportFormat) -> (TestExecutionResult, String) {
    let runner = AutomatedTestRunner::new()
        .with_config(CiConfig {
            format,
            ..CiConfig::default()
        });

    let result = runner.run();
    let report = runner.generate_report(&result);

    (result, report)
}

/// QEMU exit code for CI
pub fn exit_qemu(success: bool) {
    // QEMU isa-debug-exit device
    // Port 0x501: exit code = (value << 1) | 1
    // success=true: 0x10 -> exit code 0x21 (33)
    // success=false: 0x11 -> exit code 0x23 (35)
    let exit_code: u32 = if success { 0x10 } else { 0x11 };

    unsafe {
        // Would write to QEMU debug exit port
        core::arch::asm!(
            "out dx, eax",
            in("dx") 0x501u16,
            in("eax") exit_code,
            options(nomem, nostack, preserves_flags)
        );
    }
}

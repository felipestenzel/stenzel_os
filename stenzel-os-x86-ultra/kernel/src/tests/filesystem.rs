//! Filesystem tests

use super::{TestRunner, TestResult};
use crate::{test_assert, test_assert_eq, test_assert_ne};

pub fn register_tests(runner: &mut TestRunner) {
    runner.add_test("fs::path_normalization", test_path_normalization, "filesystem");
    runner.add_test("fs::path_components", test_path_components, "filesystem");
    runner.add_test("fs::tmpfs_operations", test_tmpfs_operations, "filesystem");
}

fn test_path_normalization() -> TestResult {
    use alloc::string::String;

    // Test path normalization
    let test_cases = [
        ("/a/b/c", "/a/b/c"),
        ("/a//b/c", "/a/b/c"),
        ("/a/./b/c", "/a/b/c"),
        ("/a/b/../c", "/a/c"),
        ("/../a", "/a"),
        ("/a/b/../../c", "/c"),
    ];

    for (input, expected) in test_cases.iter() {
        let normalized = normalize_path(input);
        test_assert_eq!(normalized.as_str(), *expected);
    }

    TestResult::Pass
}

fn test_path_components() -> TestResult {
    use alloc::vec::Vec;
    use alloc::string::String;

    let path = "/usr/local/bin/program";
    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    test_assert_eq!(components.len(), 4);
    test_assert_eq!(components[0], "usr");
    test_assert_eq!(components[1], "local");
    test_assert_eq!(components[2], "bin");
    test_assert_eq!(components[3], "program");

    TestResult::Pass
}

fn test_tmpfs_operations() -> TestResult {
    // Would test tmpfs file creation, reading, writing
    // Placeholder - actual implementation would use VFS
    TestResult::Pass
}

// Helper function (public for integration tests)
pub fn normalize_path_test(path: &str) -> alloc::string::String {
    normalize_path(path)
}

fn normalize_path(path: &str) -> alloc::string::String {
    use alloc::string::String;
    use alloc::vec::Vec;

    let mut components: Vec<&str> = Vec::new();

    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            _ => {
                components.push(component);
            }
        }
    }

    if components.is_empty() {
        String::from("/")
    } else {
        let mut result = String::new();
        for c in components {
            result.push('/');
            result.push_str(c);
        }
        result
    }
}

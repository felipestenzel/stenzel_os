//! Syscall tests

use super::{TestRunner, TestResult};
use crate::{test_assert, test_assert_eq, test_assert_ne};

pub fn register_tests(runner: &mut TestRunner) {
    runner.add_test("syscall::syscall_number_range", test_syscall_number_range, "syscall");
    runner.add_test("syscall::error_codes", test_error_codes, "syscall");
}

fn test_syscall_number_range() -> TestResult {
    // Test that syscall numbers are within valid range
    // Common Linux syscall numbers for reference

    const SYS_READ: u64 = 0;
    const SYS_WRITE: u64 = 1;
    const SYS_OPEN: u64 = 2;
    const SYS_CLOSE: u64 = 3;
    const SYS_EXIT: u64 = 60;
    const SYS_FORK: u64 = 57;

    // Verify common syscall numbers
    test_assert!(SYS_READ < 1000);
    test_assert!(SYS_WRITE < 1000);
    test_assert!(SYS_OPEN < 1000);
    test_assert!(SYS_CLOSE < 1000);
    test_assert!(SYS_EXIT < 1000);
    test_assert!(SYS_FORK < 1000);

    TestResult::Pass
}

fn test_error_codes() -> TestResult {
    // Test common errno values
    const EPERM: i32 = 1;
    const ENOENT: i32 = 2;
    const ESRCH: i32 = 3;
    const EINTR: i32 = 4;
    const EIO: i32 = 5;
    const ENXIO: i32 = 6;
    const E2BIG: i32 = 7;
    const ENOEXEC: i32 = 8;
    const EBADF: i32 = 9;
    const ECHILD: i32 = 10;
    const EAGAIN: i32 = 11;
    const ENOMEM: i32 = 12;
    const EACCES: i32 = 13;
    const EFAULT: i32 = 14;
    const EBUSY: i32 = 16;
    const EEXIST: i32 = 17;
    const ENODEV: i32 = 19;
    const ENOTDIR: i32 = 20;
    const EISDIR: i32 = 21;
    const EINVAL: i32 = 22;

    // Verify error codes are distinct positive integers
    let errors = [EPERM, ENOENT, ESRCH, EINTR, EIO, ENXIO, E2BIG, ENOEXEC,
                  EBADF, ECHILD, EAGAIN, ENOMEM, EACCES, EFAULT, EBUSY,
                  EEXIST, ENODEV, ENOTDIR, EISDIR, EINVAL];

    for &e in &errors {
        test_assert!(e > 0);
        test_assert!(e < 1000);
    }

    // Check that all error codes are unique
    for (i, &e1) in errors.iter().enumerate() {
        for &e2 in &errors[i+1..] {
            test_assert_ne!(e1, e2);
        }
    }

    TestResult::Pass
}

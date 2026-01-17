//! Memory subsystem tests

use super::{TestRunner, TestResult};
use crate::{test_assert, test_assert_eq, test_assert_ne};

pub fn register_tests(runner: &mut TestRunner) {
    runner.add_test("memory::heap_allocation", test_heap_allocation, "memory");
    runner.add_test("memory::heap_reallocation", test_heap_reallocation, "memory");
    runner.add_test("memory::large_allocation", test_large_allocation, "memory");
    runner.add_test("memory::many_small_allocations", test_many_small_allocations, "memory");
    runner.add_test("memory::box_allocation", test_box_allocation, "memory");
}

fn test_heap_allocation() -> TestResult {
    use alloc::vec::Vec;

    let mut v: Vec<u64> = Vec::new();
    for i in 0..100 {
        v.push(i);
    }

    test_assert_eq!(v.len(), 100);
    test_assert_eq!(v[50], 50);

    TestResult::Pass
}

fn test_heap_reallocation() -> TestResult {
    use alloc::vec::Vec;

    let mut v: Vec<u64> = Vec::with_capacity(10);
    let initial_ptr = v.as_ptr();

    // Fill to capacity
    for i in 0..10 {
        v.push(i);
    }

    // Push more to trigger reallocation
    for i in 10..100 {
        v.push(i);
    }

    // Verify data integrity after reallocation
    for (i, val) in v.iter().enumerate() {
        test_assert_eq!(*val, i as u64);
    }

    TestResult::Pass
}

fn test_large_allocation() -> TestResult {
    use alloc::vec::Vec;

    // Allocate 1MB
    let size = 1024 * 1024;
    let mut v: Vec<u8> = Vec::with_capacity(size);

    for i in 0..size {
        v.push((i & 0xFF) as u8);
    }

    test_assert_eq!(v.len(), size);

    // Verify some values
    test_assert_eq!(v[0], 0);
    test_assert_eq!(v[255], 255);
    test_assert_eq!(v[256], 0);

    TestResult::Pass
}

fn test_many_small_allocations() -> TestResult {
    use alloc::vec::Vec;
    use alloc::boxed::Box;

    let mut boxes: Vec<Box<u64>> = Vec::new();

    // Create many small allocations
    for i in 0..1000 {
        boxes.push(Box::new(i as u64));
    }

    // Verify all allocations
    for (i, b) in boxes.iter().enumerate() {
        test_assert_eq!(**b, i as u64);
    }

    // Free in reverse order
    while !boxes.is_empty() {
        boxes.pop();
    }

    TestResult::Pass
}

fn test_box_allocation() -> TestResult {
    use alloc::boxed::Box;

    let boxed_val = Box::new(42u64);
    test_assert_eq!(*boxed_val, 42);

    let boxed_array = Box::new([1, 2, 3, 4, 5]);
    test_assert_eq!(boxed_array[2], 3);

    #[derive(Debug, PartialEq)]
    struct TestStruct {
        a: u32,
        b: u64,
        c: [u8; 16],
    }

    let boxed_struct = Box::new(TestStruct {
        a: 123,
        b: 456789,
        c: [0; 16],
    });

    test_assert_eq!(boxed_struct.a, 123);
    test_assert_eq!(boxed_struct.b, 456789);

    TestResult::Pass
}

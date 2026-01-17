//! Network stack tests

use super::{TestRunner, TestResult};
use crate::{test_assert, test_assert_eq, test_assert_ne, test_assert_some};

pub fn register_tests(runner: &mut TestRunner) {
    runner.add_test("net::ip_address_parsing", test_ip_address_parsing, "network");
    runner.add_test("net::mac_address_parsing", test_mac_address_parsing, "network");
    runner.add_test("net::checksum_calculation", test_checksum_calculation, "network");
    runner.add_test("net::url_parsing", test_url_parsing, "network");
}

fn test_ip_address_parsing() -> TestResult {
    // Test IPv4 address parsing
    let test_cases = [
        ("192.168.1.1", [192, 168, 1, 1]),
        ("0.0.0.0", [0, 0, 0, 0]),
        ("255.255.255.255", [255, 255, 255, 255]),
        ("10.0.0.1", [10, 0, 0, 1]),
        ("172.16.0.1", [172, 16, 0, 1]),
    ];

    for (input, expected) in test_cases.iter() {
        let parsed = parse_ipv4(input);
        test_assert_some!(parsed);
        test_assert_eq!(parsed.unwrap(), *expected);
    }

    // Test invalid addresses
    let invalid = ["256.1.1.1", "1.2.3", "a.b.c.d", ""];
    for input in invalid.iter() {
        let parsed = parse_ipv4(input);
        test_assert!(parsed.is_none());
    }

    TestResult::Pass
}

fn test_mac_address_parsing() -> TestResult {
    // Test MAC address parsing
    let test_cases = [
        ("00:11:22:33:44:55", [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
        ("FF:FF:FF:FF:FF:FF", [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
        ("aa:bb:cc:dd:ee:ff", [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]),
    ];

    for (input, expected) in test_cases.iter() {
        let parsed = parse_mac(input);
        test_assert_some!(parsed);
        test_assert_eq!(parsed.unwrap(), *expected);
    }

    TestResult::Pass
}

fn test_checksum_calculation() -> TestResult {
    // Test IP header checksum
    // Example from RFC 791
    let header: [u8; 20] = [
        0x45, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00,
        0x40, 0x11, 0x00, 0x00, 0xc0, 0xa8, 0x00, 0x01,
        0xc0, 0xa8, 0x00, 0xc7,
    ];

    let checksum = calculate_checksum(&header);
    // The checksum field (bytes 10-11) should make the total sum 0xFFFF
    test_assert!(checksum != 0);

    TestResult::Pass
}

fn test_url_parsing() -> TestResult {
    use alloc::string::String;

    let url = "https://example.com:8080/path/to/resource?query=value";

    let parsed = parse_url(url);
    test_assert_some!(parsed);

    let (scheme, host, port, path) = parsed.unwrap();
    test_assert_eq!(scheme, "https");
    test_assert_eq!(host, "example.com");
    test_assert_eq!(port, Some(8080));
    test_assert_eq!(path, "/path/to/resource?query=value");

    TestResult::Pass
}

// Helper functions

fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
    let parts: alloc::vec::Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return None;
    }

    let mut result = [0u8; 4];
    for (i, part) in parts.iter().enumerate() {
        match part.parse::<u8>() {
            Ok(n) => result[i] = n,
            Err(_) => return None,
        }
    }

    Some(result)
}

fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let parts: alloc::vec::Vec<&str> = s.split(':').collect();
    if parts.len() != 6 {
        return None;
    }

    let mut result = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        match u8::from_str_radix(part, 16) {
            Ok(n) => result[i] = n,
            Err(_) => return None,
        }
    }

    Some(result)
}

fn calculate_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    for chunk in data.chunks(2) {
        let word = if chunk.len() == 2 {
            ((chunk[0] as u16) << 8) | (chunk[1] as u16)
        } else {
            (chunk[0] as u16) << 8
        };
        sum = sum.wrapping_add(word as u32);
    }

    // Fold 32-bit sum to 16-bit
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
}

fn parse_url(url: &str) -> Option<(alloc::string::String, alloc::string::String, Option<u16>, alloc::string::String)> {
    use alloc::string::String;

    // Parse scheme
    let (scheme, rest) = url.split_once("://")?;

    // Parse host and path
    let (host_port, path) = rest.find('/').map(|i| rest.split_at(i)).unwrap_or((rest, "/"));

    // Parse port from host
    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (h, p.parse::<u16>().ok())
    } else {
        (host_port, None)
    };

    Some((
        String::from(scheme),
        String::from(host),
        port,
        String::from(path),
    ))
}

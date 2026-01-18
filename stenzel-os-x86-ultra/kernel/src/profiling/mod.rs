//! Profiling and Performance Monitoring for Stenzel OS.
//!
//! This module provides various profiling capabilities:
//! - Hardware Performance Counters (perf)
//! - eBPF (Extended Berkeley Packet Filter)
//! - Tracing (ftrace-like)
//! - CPU Profiling

#![allow(dead_code)]

pub mod perf;

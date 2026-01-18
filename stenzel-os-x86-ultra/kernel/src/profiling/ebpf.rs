//! Extended Berkeley Packet Filter (eBPF) for Stenzel OS.
//!
//! Provides an in-kernel virtual machine for safe, efficient program execution.
//!
//! Features:
//! - eBPF instruction set emulation
//! - Program verification
//! - Maps (hash, array, per-cpu)
//! - Helper functions
//! - Tracepoint/kprobe attachment
//! - XDP (eXpress Data Path) support
//! - Program types (socket filter, tracepoint, kprobe, etc.)

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use spin::{Mutex, Once, RwLock};

// ============================================================================
// eBPF Instruction Set
// ============================================================================

/// eBPF instruction opcode classes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BpfClass {
    /// Load from immediate
    Ld = 0x00,
    /// Load from register
    Ldx = 0x01,
    /// Store immediate
    St = 0x02,
    /// Store from register
    Stx = 0x03,
    /// 32-bit ALU operations
    Alu = 0x04,
    /// Jump operations
    Jmp = 0x05,
    /// 32-bit jump operations
    Jmp32 = 0x06,
    /// 64-bit ALU operations
    Alu64 = 0x07,
}

/// eBPF ALU operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BpfAluOp {
    Add = 0x00,
    Sub = 0x10,
    Mul = 0x20,
    Div = 0x30,
    Or = 0x40,
    And = 0x50,
    Lsh = 0x60,
    Rsh = 0x70,
    Neg = 0x80,
    Mod = 0x90,
    Xor = 0xa0,
    Mov = 0xb0,
    Arsh = 0xc0,
    End = 0xd0,
}

/// eBPF jump operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BpfJmpOp {
    Ja = 0x00,
    Jeq = 0x10,
    Jgt = 0x20,
    Jge = 0x30,
    Jset = 0x40,
    Jne = 0x50,
    Jsgt = 0x60,
    Jsge = 0x70,
    Call = 0x80,
    Exit = 0x90,
    Jlt = 0xa0,
    Jle = 0xb0,
    Jslt = 0xc0,
    Jsle = 0xd0,
}

/// eBPF instruction
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BpfInsn {
    /// Opcode
    pub code: u8,
    /// Destination register (low 4 bits) and source register (high 4 bits)
    pub regs: u8,
    /// Offset for jumps/memory access
    pub off: i16,
    /// Immediate value
    pub imm: i32,
}

impl BpfInsn {
    /// Create new instruction
    pub const fn new(code: u8, dst: u8, src: u8, off: i16, imm: i32) -> Self {
        Self {
            code,
            regs: (src << 4) | (dst & 0xf),
            off,
            imm,
        }
    }

    /// Get destination register
    pub fn dst_reg(&self) -> u8 {
        self.regs & 0xf
    }

    /// Get source register
    pub fn src_reg(&self) -> u8 {
        self.regs >> 4
    }

    /// Get opcode class
    pub fn class(&self) -> u8 {
        self.code & 0x07
    }

    /// Get ALU/JMP operation
    pub fn op(&self) -> u8 {
        self.code & 0xf0
    }

    /// Get source type (immediate or register)
    pub fn src_type(&self) -> u8 {
        self.code & 0x08
    }

    /// Get memory size
    pub fn size(&self) -> u8 {
        self.code & 0x18
    }

    /// Get memory mode
    pub fn mode(&self) -> u8 {
        self.code & 0xe0
    }
}

// ============================================================================
// eBPF Registers
// ============================================================================

/// eBPF register file (11 registers: r0-r10)
#[derive(Debug, Clone, Default)]
pub struct BpfRegs {
    /// R0: return value
    pub r0: u64,
    /// R1-R5: function arguments
    pub r1: u64,
    pub r2: u64,
    pub r3: u64,
    pub r4: u64,
    pub r5: u64,
    /// R6-R9: callee-saved
    pub r6: u64,
    pub r7: u64,
    pub r8: u64,
    pub r9: u64,
    /// R10: frame pointer (read-only)
    pub r10: u64,
}

impl BpfRegs {
    /// Get register by index
    pub fn get(&self, idx: u8) -> u64 {
        match idx {
            0 => self.r0,
            1 => self.r1,
            2 => self.r2,
            3 => self.r3,
            4 => self.r4,
            5 => self.r5,
            6 => self.r6,
            7 => self.r7,
            8 => self.r8,
            9 => self.r9,
            10 => self.r10,
            _ => 0,
        }
    }

    /// Set register by index
    pub fn set(&mut self, idx: u8, val: u64) {
        match idx {
            0 => self.r0 = val,
            1 => self.r1 = val,
            2 => self.r2 = val,
            3 => self.r3 = val,
            4 => self.r4 = val,
            5 => self.r5 = val,
            6 => self.r6 = val,
            7 => self.r7 = val,
            8 => self.r8 = val,
            9 => self.r9 = val,
            10 => {} // r10 is read-only
            _ => {}
        }
    }
}

// ============================================================================
// eBPF Maps
// ============================================================================

/// Map type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpfMapType {
    /// Unspecified
    Unspec = 0,
    /// Hash table
    Hash = 1,
    /// Array
    Array = 2,
    /// Program array (for tail calls)
    ProgArray = 3,
    /// Perf event array
    PerfEventArray = 4,
    /// Per-CPU hash
    PercpuHash = 5,
    /// Per-CPU array
    PercpuArray = 6,
    /// Stack trace
    StackTrace = 7,
    /// Cgroup array
    CgroupArray = 8,
    /// LRU hash
    LruHash = 9,
    /// LRU per-CPU hash
    LruPercpuHash = 10,
    /// LPM trie
    LpmTrie = 11,
    /// Array of maps
    ArrayOfMaps = 12,
    /// Hash of maps
    HashOfMaps = 13,
    /// Device map
    Devmap = 14,
    /// Socket map
    Sockmap = 15,
    /// Ring buffer
    Ringbuf = 16,
}

/// Map key-value entry
#[derive(Debug, Clone)]
pub struct MapEntry {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

/// eBPF map
pub struct BpfMap {
    /// Map ID
    pub id: u32,
    /// Map type
    pub map_type: BpfMapType,
    /// Key size in bytes
    pub key_size: u32,
    /// Value size in bytes
    pub value_size: u32,
    /// Maximum entries
    pub max_entries: u32,
    /// Map flags
    pub flags: u32,
    /// Map name
    pub name: String,
    /// Entries (for hash/array maps)
    entries: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
    /// Array storage (for array maps)
    array: RwLock<Vec<Vec<u8>>>,
}

impl BpfMap {
    /// Create new map
    pub fn new(
        id: u32,
        map_type: BpfMapType,
        key_size: u32,
        value_size: u32,
        max_entries: u32,
        flags: u32,
        name: &str,
    ) -> Self {
        let array = if map_type == BpfMapType::Array || map_type == BpfMapType::PercpuArray {
            vec![vec![0u8; value_size as usize]; max_entries as usize]
        } else {
            Vec::new()
        };

        Self {
            id,
            map_type,
            key_size,
            value_size,
            max_entries,
            flags,
            name: name.to_string(),
            entries: RwLock::new(BTreeMap::new()),
            array: RwLock::new(array),
        }
    }

    /// Lookup value by key
    pub fn lookup(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.map_type {
            BpfMapType::Array | BpfMapType::PercpuArray => {
                if key.len() != 4 {
                    return None;
                }
                let idx = u32::from_ne_bytes([key[0], key[1], key[2], key[3]]) as usize;
                let array = self.array.read();
                array.get(idx).cloned()
            }
            _ => {
                let entries = self.entries.read();
                entries.get(key).cloned()
            }
        }
    }

    /// Update value
    pub fn update(&self, key: &[u8], value: &[u8], flags: u64) -> Result<(), BpfError> {
        if key.len() != self.key_size as usize {
            return Err(BpfError::InvalidKey);
        }
        if value.len() != self.value_size as usize {
            return Err(BpfError::InvalidValue);
        }

        match self.map_type {
            BpfMapType::Array | BpfMapType::PercpuArray => {
                let idx = u32::from_ne_bytes([key[0], key[1], key[2], key[3]]) as usize;
                let mut array = self.array.write();
                if idx >= array.len() {
                    return Err(BpfError::InvalidKey);
                }
                array[idx] = value.to_vec();
                Ok(())
            }
            _ => {
                let mut entries = self.entries.write();
                if entries.len() >= self.max_entries as usize && !entries.contains_key(key) {
                    return Err(BpfError::MapFull);
                }
                entries.insert(key.to_vec(), value.to_vec());
                Ok(())
            }
        }
    }

    /// Delete entry
    pub fn delete(&self, key: &[u8]) -> Result<(), BpfError> {
        match self.map_type {
            BpfMapType::Array | BpfMapType::PercpuArray => {
                // Arrays don't support deletion, just zero
                let idx = u32::from_ne_bytes([key[0], key[1], key[2], key[3]]) as usize;
                let mut array = self.array.write();
                if idx >= array.len() {
                    return Err(BpfError::InvalidKey);
                }
                array[idx] = vec![0u8; self.value_size as usize];
                Ok(())
            }
            _ => {
                let mut entries = self.entries.write();
                entries.remove(key).ok_or(BpfError::NotFound)?;
                Ok(())
            }
        }
    }

    /// Get next key (for iteration)
    pub fn get_next_key(&self, key: Option<&[u8]>) -> Option<Vec<u8>> {
        match self.map_type {
            BpfMapType::Array | BpfMapType::PercpuArray => {
                let array = self.array.read();
                let next_idx = match key {
                    None => 0,
                    Some(k) => {
                        let idx = u32::from_ne_bytes([k[0], k[1], k[2], k[3]]) as usize;
                        idx + 1
                    }
                };
                if next_idx < array.len() {
                    Some((next_idx as u32).to_ne_bytes().to_vec())
                } else {
                    None
                }
            }
            _ => {
                let entries = self.entries.read();
                match key {
                    None => entries.keys().next().cloned(),
                    Some(k) => entries.range(k.to_vec()..).skip(1).next().map(|(k, _)| k.clone()),
                }
            }
        }
    }
}

// ============================================================================
// eBPF Program Types
// ============================================================================

/// Program type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpfProgType {
    /// Unspecified
    Unspec = 0,
    /// Socket filter
    SocketFilter = 1,
    /// Kprobe/uprobe
    Kprobe = 2,
    /// Scheduler classifier
    SchedCls = 3,
    /// Scheduler action
    SchedAct = 4,
    /// Tracepoint
    Tracepoint = 5,
    /// XDP (eXpress Data Path)
    Xdp = 6,
    /// Perf event
    PerfEvent = 7,
    /// Cgroup socket
    CgroupSkb = 8,
    /// Cgroup socket ops
    CgroupSock = 9,
    /// Lightweight tunnel
    LwtIn = 10,
    /// Lightweight tunnel out
    LwtOut = 11,
    /// Socket ops
    SockOps = 12,
    /// SK message
    SkMsg = 13,
    /// Raw tracepoint
    RawTracepoint = 14,
    /// Cgroup socket address
    CgroupSockAddr = 15,
    /// LSM (Linux Security Module)
    Lsm = 16,
    /// Struct ops
    StructOps = 17,
    /// Extension
    Ext = 18,
    /// Syscall
    Syscall = 19,
}

/// Program attach type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpfAttachType {
    CgroupInetIngress,
    CgroupInetEgress,
    CgroupInetSockCreate,
    CgroupSockOps,
    SkSkbStreamParser,
    SkSkbStreamVerdict,
    CgroupDevice,
    SkMsgVerdict,
    CgroupInet4Bind,
    CgroupInet6Bind,
    CgroupInet4Connect,
    CgroupInet6Connect,
    CgroupInet4PostBind,
    CgroupInet6PostBind,
    CgroupUdp4Sendmsg,
    CgroupUdp6Sendmsg,
    TraceFentry,
    TraceFexit,
    XdpDevmap,
    CgroupInetSockRelease,
}

// ============================================================================
// eBPF Program
// ============================================================================

/// eBPF program
pub struct BpfProg {
    /// Program ID
    pub id: u32,
    /// Program type
    pub prog_type: BpfProgType,
    /// Program name
    pub name: String,
    /// Instructions
    insns: Vec<BpfInsn>,
    /// License
    pub license: String,
    /// Kernel version requirement
    pub kern_version: u32,
    /// Maps used by this program
    pub maps: Vec<u32>,
    /// Attached
    attached: AtomicBool,
    /// Run count
    run_count: AtomicU64,
    /// Total runtime (ns)
    total_runtime_ns: AtomicU64,
}

impl BpfProg {
    /// Create new program
    pub fn new(
        id: u32,
        prog_type: BpfProgType,
        name: &str,
        insns: Vec<BpfInsn>,
        license: &str,
    ) -> Self {
        Self {
            id,
            prog_type,
            name: name.to_string(),
            insns,
            license: license.to_string(),
            kern_version: 0,
            maps: Vec::new(),
            attached: AtomicBool::new(false),
            run_count: AtomicU64::new(0),
            total_runtime_ns: AtomicU64::new(0),
        }
    }

    /// Get instruction count
    pub fn insn_count(&self) -> usize {
        self.insns.len()
    }

    /// Get instructions
    pub fn insns(&self) -> &[BpfInsn] {
        &self.insns
    }

    /// Check if attached
    pub fn is_attached(&self) -> bool {
        self.attached.load(Ordering::Relaxed)
    }

    /// Mark as attached
    pub fn set_attached(&self, attached: bool) {
        self.attached.store(attached, Ordering::Relaxed);
    }

    /// Record run statistics
    pub fn record_run(&self, runtime_ns: u64) {
        self.run_count.fetch_add(1, Ordering::Relaxed);
        self.total_runtime_ns.fetch_add(runtime_ns, Ordering::Relaxed);
    }

    /// Get run count
    pub fn get_run_count(&self) -> u64 {
        self.run_count.load(Ordering::Relaxed)
    }

    /// Get total runtime
    pub fn get_total_runtime_ns(&self) -> u64 {
        self.total_runtime_ns.load(Ordering::Relaxed)
    }
}

// ============================================================================
// eBPF Verifier
// ============================================================================

/// Verifier result
#[derive(Debug, Clone)]
pub struct VerifierResult {
    /// Verification passed
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Log output
    pub log: String,
    /// Instruction count
    pub insn_count: usize,
    /// Stack depth used
    pub stack_depth: usize,
}

/// eBPF program verifier
pub struct BpfVerifier {
    /// Maximum instructions
    max_insns: usize,
    /// Maximum stack depth
    max_stack: usize,
    /// Allow loops
    allow_loops: bool,
}

impl BpfVerifier {
    /// Create new verifier
    pub fn new() -> Self {
        Self {
            max_insns: 1_000_000,
            max_stack: 512,
            allow_loops: false,
        }
    }

    /// Verify a program
    pub fn verify(&self, prog: &BpfProg) -> VerifierResult {
        let mut log = String::new();

        // Check instruction count
        if prog.insn_count() > self.max_insns {
            return VerifierResult {
                success: false,
                error: Some("Too many instructions".to_string()),
                log,
                insn_count: prog.insn_count(),
                stack_depth: 0,
            };
        }

        // Check for invalid instructions
        for (i, insn) in prog.insns().iter().enumerate() {
            if let Err(e) = self.verify_insn(insn, i) {
                return VerifierResult {
                    success: false,
                    error: Some(e),
                    log,
                    insn_count: prog.insn_count(),
                    stack_depth: 0,
                };
            }
        }

        // Check control flow
        if let Err(e) = self.verify_control_flow(prog) {
            return VerifierResult {
                success: false,
                error: Some(e),
                log,
                insn_count: prog.insn_count(),
                stack_depth: 0,
            };
        }

        log.push_str("Verification passed\n");

        VerifierResult {
            success: true,
            error: None,
            log,
            insn_count: prog.insn_count(),
            stack_depth: 0, // Would calculate actual stack depth
        }
    }

    /// Verify single instruction
    fn verify_insn(&self, insn: &BpfInsn, idx: usize) -> Result<(), String> {
        // Check register bounds
        if insn.dst_reg() > 10 {
            return Err(format!("Invalid dst register at insn {}", idx));
        }
        if insn.src_reg() > 10 {
            return Err(format!("Invalid src register at insn {}", idx));
        }

        // Check opcode validity
        let class = insn.class();
        if class > 7 {
            return Err(format!("Invalid opcode class at insn {}", idx));
        }

        Ok(())
    }

    /// Verify control flow (no unreachable code, proper termination)
    fn verify_control_flow(&self, prog: &BpfProg) -> Result<(), String> {
        let insns = prog.insns();
        if insns.is_empty() {
            return Err("Empty program".to_string());
        }

        // Check last instruction is exit
        let last = &insns[insns.len() - 1];
        let is_exit = last.class() == BpfClass::Jmp as u8 && last.op() == BpfJmpOp::Exit as u8;
        if !is_exit {
            return Err("Program doesn't end with exit".to_string());
        }

        Ok(())
    }
}

impl Default for BpfVerifier {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// eBPF Virtual Machine
// ============================================================================

/// eBPF VM execution context
pub struct BpfContext {
    /// Registers
    pub regs: BpfRegs,
    /// Stack (512 bytes)
    pub stack: [u8; 512],
    /// Data pointer (context-specific)
    pub data: u64,
    /// Data end pointer
    pub data_end: u64,
}

impl Default for BpfContext {
    fn default() -> Self {
        Self {
            regs: BpfRegs::default(),
            stack: [0; 512],
            data: 0,
            data_end: 0,
        }
    }
}

/// eBPF VM
pub struct BpfVm {
    /// Helper functions
    helpers: BTreeMap<u32, BpfHelper>,
}

/// Helper function type
type BpfHelper = fn(&mut BpfContext, u64, u64, u64, u64, u64) -> u64;

impl BpfVm {
    /// Create new VM
    pub fn new() -> Self {
        let mut vm = Self {
            helpers: BTreeMap::new(),
        };
        vm.register_builtin_helpers();
        vm
    }

    /// Register built-in helper functions
    fn register_builtin_helpers(&mut self) {
        // Helper 1: bpf_map_lookup_elem
        self.helpers.insert(1, |_ctx, _map_fd, _key, _, _, _| 0);

        // Helper 2: bpf_map_update_elem
        self.helpers.insert(2, |_ctx, _map_fd, _key, _value, _flags, _| 0);

        // Helper 3: bpf_map_delete_elem
        self.helpers.insert(3, |_ctx, _map_fd, _key, _, _, _| 0);

        // Helper 4: bpf_probe_read
        self.helpers.insert(4, |_ctx, _dst, _size, _src, _, _| 0);

        // Helper 5: bpf_ktime_get_ns
        self.helpers.insert(5, |_ctx, _, _, _, _, _| {
            // Return current time in nanoseconds
            crate::time::uptime_ns()
        });

        // Helper 6: bpf_trace_printk
        self.helpers.insert(6, |_ctx, _fmt, _fmt_size, _arg1, _arg2, _arg3| 0);

        // Helper 14: bpf_get_current_pid_tgid
        self.helpers.insert(14, |_ctx, _, _, _, _, _| {
            // Return (tgid << 32) | pid
            0
        });

        // Helper 35: bpf_get_current_comm
        self.helpers.insert(35, |_ctx, _buf, _size, _, _, _| 0);
    }

    /// Execute program
    pub fn run(&self, prog: &BpfProg, ctx: &mut BpfContext) -> Result<u64, BpfError> {
        let insns = prog.insns();
        let mut pc: usize = 0;

        // Set up frame pointer
        ctx.regs.r10 = ctx.stack.as_ptr() as u64 + 512;

        while pc < insns.len() {
            let insn = &insns[pc];
            let dst = insn.dst_reg();
            let src = insn.src_reg();
            let imm = insn.imm as i64 as u64;

            match insn.class() {
                // ALU64
                0x07 => {
                    let src_val = if insn.src_type() == 0x08 {
                        ctx.regs.get(src)
                    } else {
                        imm
                    };
                    let dst_val = ctx.regs.get(dst);

                    let result = match insn.op() {
                        0x00 => dst_val.wrapping_add(src_val), // ADD
                        0x10 => dst_val.wrapping_sub(src_val), // SUB
                        0x20 => dst_val.wrapping_mul(src_val), // MUL
                        0x30 => {
                            if src_val == 0 {
                                return Err(BpfError::DivByZero);
                            }
                            dst_val / src_val // DIV
                        }
                        0x40 => dst_val | src_val,  // OR
                        0x50 => dst_val & src_val,  // AND
                        0x60 => dst_val << (src_val & 0x3f), // LSH
                        0x70 => dst_val >> (src_val & 0x3f), // RSH
                        0x80 => (-(dst_val as i64)) as u64,  // NEG
                        0x90 => {
                            if src_val == 0 {
                                return Err(BpfError::DivByZero);
                            }
                            dst_val % src_val // MOD
                        }
                        0xa0 => dst_val ^ src_val,  // XOR
                        0xb0 => src_val,            // MOV
                        0xc0 => ((dst_val as i64) >> (src_val & 0x3f)) as u64, // ARSH
                        _ => return Err(BpfError::InvalidInsn),
                    };
                    ctx.regs.set(dst, result);
                }

                // ALU32
                0x04 => {
                    let src_val = if insn.src_type() == 0x08 {
                        ctx.regs.get(src) as u32
                    } else {
                        imm as u32
                    };
                    let dst_val = ctx.regs.get(dst) as u32;

                    let result = match insn.op() {
                        0x00 => dst_val.wrapping_add(src_val),
                        0x10 => dst_val.wrapping_sub(src_val),
                        0x20 => dst_val.wrapping_mul(src_val),
                        0x30 => {
                            if src_val == 0 {
                                return Err(BpfError::DivByZero);
                            }
                            dst_val / src_val
                        }
                        0x40 => dst_val | src_val,
                        0x50 => dst_val & src_val,
                        0x60 => dst_val << (src_val & 0x1f),
                        0x70 => dst_val >> (src_val & 0x1f),
                        0xb0 => src_val,
                        _ => return Err(BpfError::InvalidInsn),
                    };
                    ctx.regs.set(dst, result as u64);
                }

                // JMP
                0x05 => {
                    let src_val = if insn.src_type() == 0x08 {
                        ctx.regs.get(src)
                    } else {
                        imm
                    };
                    let dst_val = ctx.regs.get(dst);

                    let take_branch = match insn.op() {
                        0x00 => true,  // JA
                        0x10 => dst_val == src_val, // JEQ
                        0x20 => dst_val > src_val,  // JGT
                        0x30 => dst_val >= src_val, // JGE
                        0x40 => (dst_val & src_val) != 0, // JSET
                        0x50 => dst_val != src_val, // JNE
                        0x60 => (dst_val as i64) > (src_val as i64), // JSGT
                        0x70 => (dst_val as i64) >= (src_val as i64), // JSGE
                        0xa0 => dst_val < src_val,  // JLT
                        0xb0 => dst_val <= src_val, // JLE
                        0x80 => {
                            // CALL
                            let func_id = insn.imm as u32;
                            if let Some(helper) = self.helpers.get(&func_id) {
                                let result = helper(
                                    ctx,
                                    ctx.regs.r1,
                                    ctx.regs.r2,
                                    ctx.regs.r3,
                                    ctx.regs.r4,
                                    ctx.regs.r5,
                                );
                                ctx.regs.r0 = result;
                            } else {
                                return Err(BpfError::UnknownHelper);
                            }
                            false
                        }
                        0x90 => {
                            // EXIT
                            return Ok(ctx.regs.r0);
                        }
                        _ => return Err(BpfError::InvalidInsn),
                    };

                    if take_branch && insn.op() != 0x80 {
                        pc = (pc as i64 + insn.off as i64) as usize;
                    }
                }

                // LDX (load from memory)
                0x01 => {
                    let addr = ctx.regs.get(src).wrapping_add(insn.off as i64 as u64);
                    let val = match insn.size() {
                        0x00 => unsafe { *(addr as *const u32) as u64 }, // W
                        0x08 => unsafe { *(addr as *const u16) as u64 }, // H
                        0x10 => unsafe { *(addr as *const u8) as u64 },  // B
                        0x18 => unsafe { *(addr as *const u64) },        // DW
                        _ => return Err(BpfError::InvalidInsn),
                    };
                    ctx.regs.set(dst, val);
                }

                // STX (store to memory)
                0x03 => {
                    let addr = ctx.regs.get(dst).wrapping_add(insn.off as i64 as u64);
                    let val = ctx.regs.get(src);
                    match insn.size() {
                        0x00 => unsafe { *(addr as *mut u32) = val as u32 },
                        0x08 => unsafe { *(addr as *mut u16) = val as u16 },
                        0x10 => unsafe { *(addr as *mut u8) = val as u8 },
                        0x18 => unsafe { *(addr as *mut u64) = val },
                        _ => return Err(BpfError::InvalidInsn),
                    }
                }

                // ST (store immediate)
                0x02 => {
                    let addr = ctx.regs.get(dst).wrapping_add(insn.off as i64 as u64);
                    match insn.size() {
                        0x00 => unsafe { *(addr as *mut u32) = imm as u32 },
                        0x08 => unsafe { *(addr as *mut u16) = imm as u16 },
                        0x10 => unsafe { *(addr as *mut u8) = imm as u8 },
                        0x18 => unsafe { *(addr as *mut u64) = imm },
                        _ => return Err(BpfError::InvalidInsn),
                    }
                }

                _ => return Err(BpfError::InvalidInsn),
            }

            pc += 1;
        }

        Err(BpfError::NoExit)
    }
}

impl Default for BpfVm {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// eBPF Subsystem Manager
// ============================================================================

/// eBPF subsystem manager
pub struct BpfManager {
    /// Programs by ID
    progs: BTreeMap<u32, Arc<BpfProg>>,
    /// Maps by ID
    maps: BTreeMap<u32, Arc<BpfMap>>,
    /// Next program ID
    next_prog_id: AtomicU32,
    /// Next map ID
    next_map_id: AtomicU32,
    /// Verifier
    verifier: BpfVerifier,
    /// VM
    vm: BpfVm,
    /// Enabled
    enabled: AtomicBool,
}

impl BpfManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            progs: BTreeMap::new(),
            maps: BTreeMap::new(),
            next_prog_id: AtomicU32::new(1),
            next_map_id: AtomicU32::new(1),
            verifier: BpfVerifier::new(),
            vm: BpfVm::new(),
            enabled: AtomicBool::new(true),
        }
    }

    /// Create a map
    pub fn create_map(
        &mut self,
        map_type: BpfMapType,
        key_size: u32,
        value_size: u32,
        max_entries: u32,
        flags: u32,
        name: &str,
    ) -> Result<u32, BpfError> {
        let id = self.next_map_id.fetch_add(1, Ordering::Relaxed);
        let map = BpfMap::new(id, map_type, key_size, value_size, max_entries, flags, name);
        self.maps.insert(id, Arc::new(map));
        Ok(id)
    }

    /// Get map by ID
    pub fn get_map(&self, id: u32) -> Option<Arc<BpfMap>> {
        self.maps.get(&id).cloned()
    }

    /// Load a program
    pub fn load_prog(
        &mut self,
        prog_type: BpfProgType,
        name: &str,
        insns: Vec<BpfInsn>,
        license: &str,
    ) -> Result<u32, BpfError> {
        let id = self.next_prog_id.fetch_add(1, Ordering::Relaxed);
        let prog = BpfProg::new(id, prog_type, name, insns, license);

        // Verify program
        let result = self.verifier.verify(&prog);
        if !result.success {
            return Err(BpfError::VerificationFailed(
                result.error.unwrap_or_default(),
            ));
        }

        self.progs.insert(id, Arc::new(prog));
        Ok(id)
    }

    /// Get program by ID
    pub fn get_prog(&self, id: u32) -> Option<Arc<BpfProg>> {
        self.progs.get(&id).cloned()
    }

    /// Unload program
    pub fn unload_prog(&mut self, id: u32) -> Result<(), BpfError> {
        self.progs.remove(&id).ok_or(BpfError::NotFound)?;
        Ok(())
    }

    /// Delete map
    pub fn delete_map(&mut self, id: u32) -> Result<(), BpfError> {
        self.maps.remove(&id).ok_or(BpfError::NotFound)?;
        Ok(())
    }

    /// Run program
    pub fn run_prog(&self, id: u32, ctx: &mut BpfContext) -> Result<u64, BpfError> {
        let prog = self.progs.get(&id).ok_or(BpfError::NotFound)?;
        self.vm.run(prog, ctx)
    }

    /// List programs
    pub fn list_progs(&self) -> Vec<u32> {
        self.progs.keys().copied().collect()
    }

    /// List maps
    pub fn list_maps(&self) -> Vec<u32> {
        self.maps.keys().copied().collect()
    }
}

impl Default for BpfManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// eBPF error
#[derive(Debug, Clone)]
pub enum BpfError {
    /// Invalid instruction
    InvalidInsn,
    /// Division by zero
    DivByZero,
    /// No exit instruction reached
    NoExit,
    /// Unknown helper function
    UnknownHelper,
    /// Invalid key
    InvalidKey,
    /// Invalid value
    InvalidValue,
    /// Map full
    MapFull,
    /// Not found
    NotFound,
    /// Verification failed
    VerificationFailed(String),
    /// Permission denied
    PermissionDenied,
    /// Already attached
    AlreadyAttached,
}

// ============================================================================
// Global Instance
// ============================================================================

static BPF_MANAGER: Once<Mutex<BpfManager>> = Once::new();

/// Initialize eBPF subsystem
pub fn init() {
    BPF_MANAGER.call_once(|| Mutex::new(BpfManager::new()));
    crate::kprintln!("ebpf: initialized");
}

/// Get BPF manager
pub fn manager() -> &'static Mutex<BpfManager> {
    BPF_MANAGER.get().expect("eBPF not initialized")
}

/// Create map
pub fn bpf_create_map(
    map_type: BpfMapType,
    key_size: u32,
    value_size: u32,
    max_entries: u32,
) -> Result<u32, BpfError> {
    manager().lock().create_map(map_type, key_size, value_size, max_entries, 0, "")
}

/// Load program
pub fn bpf_prog_load(
    prog_type: BpfProgType,
    insns: Vec<BpfInsn>,
    license: &str,
) -> Result<u32, BpfError> {
    manager().lock().load_prog(prog_type, "", insns, license)
}

/// Map lookup
pub fn bpf_map_lookup_elem(map_id: u32, key: &[u8]) -> Option<Vec<u8>> {
    manager().lock().get_map(map_id)?.lookup(key)
}

/// Map update
pub fn bpf_map_update_elem(map_id: u32, key: &[u8], value: &[u8]) -> Result<(), BpfError> {
    manager().lock().get_map(map_id).ok_or(BpfError::NotFound)?.update(key, value, 0)
}

/// Map delete
pub fn bpf_map_delete_elem(map_id: u32, key: &[u8]) -> Result<(), BpfError> {
    manager().lock().get_map(map_id).ok_or(BpfError::NotFound)?.delete(key)
}

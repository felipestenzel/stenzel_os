//! SMP (Symmetric Multi-Processing) support for x86_64.
//!
//! This module handles CPU detection, per-CPU data structures, and AP (Application Processor)
//! startup.

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use spin::Mutex;

/// Maximum number of supported CPUs
pub const MAX_CPUS: usize = 256;

/// CPU information from CPUID
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// APIC ID
    pub apic_id: u8,
    /// Whether this is the BSP (Bootstrap Processor)
    pub is_bsp: bool,
    /// Whether this CPU is online
    pub online: bool,
    /// CPU vendor string (e.g., "GenuineIntel", "AuthenticAMD")
    pub vendor: String,
    /// CPU brand string (e.g., "Intel Core i7...")
    pub brand: String,
    /// CPU family
    pub family: u8,
    /// CPU model
    pub model: u8,
    /// CPU stepping
    pub stepping: u8,
    /// Feature flags
    pub features: CpuFeatures,
}

/// CPU features detected via CPUID
#[derive(Debug, Clone, Default)]
pub struct CpuFeatures {
    // Basic features (CPUID.01H:EDX)
    pub fpu: bool,           // x87 FPU
    pub vme: bool,           // Virtual 8086 extensions
    pub de: bool,            // Debugging extensions
    pub pse: bool,           // Page Size Extension
    pub tsc: bool,           // Time Stamp Counter
    pub msr: bool,           // Model-Specific Registers
    pub pae: bool,           // Physical Address Extension
    pub mce: bool,           // Machine Check Exception
    pub cx8: bool,           // CMPXCHG8B
    pub apic: bool,          // On-chip APIC
    pub sep: bool,           // SYSENTER/SYSEXIT
    pub mtrr: bool,          // Memory Type Range Registers
    pub pge: bool,           // Page Global Enable
    pub mca: bool,           // Machine Check Architecture
    pub cmov: bool,          // Conditional Move
    pub pat: bool,           // Page Attribute Table
    pub pse36: bool,         // 36-bit Page Size Extension
    pub psn: bool,           // Processor Serial Number
    pub clfsh: bool,         // CLFLUSH
    pub ds: bool,            // Debug Store
    pub acpi: bool,          // ACPI thermal/power control
    pub mmx: bool,           // MMX
    pub fxsr: bool,          // FXSAVE/FXRSTOR
    pub sse: bool,           // SSE
    pub sse2: bool,          // SSE2
    pub ss: bool,            // Self Snoop
    pub htt: bool,           // Hyper-Threading
    pub tm: bool,            // Thermal Monitor
    pub ia64: bool,          // IA-64 processor emulating x86
    pub pbe: bool,           // Pending Break Enable

    // Extended features (CPUID.01H:ECX)
    pub sse3: bool,          // SSE3
    pub pclmulqdq: bool,     // PCLMULQDQ
    pub dtes64: bool,        // 64-bit Debug Store
    pub monitor: bool,       // MONITOR/MWAIT
    pub ds_cpl: bool,        // CPL Qualified Debug Store
    pub vmx: bool,           // Virtual Machine Extensions
    pub smx: bool,           // Safer Mode Extensions
    pub eist: bool,          // Enhanced SpeedStep
    pub tm2: bool,           // Thermal Monitor 2
    pub ssse3: bool,         // SSSE3
    pub cnxt_id: bool,       // L1 Context ID
    pub sdbg: bool,          // Silicon Debug
    pub fma: bool,           // FMA
    pub cx16: bool,          // CMPXCHG16B
    pub xtpr: bool,          // xTPR Update Control
    pub pdcm: bool,          // Performance/Debug Capability MSR
    pub pcid: bool,          // Process Context Identifiers
    pub dca: bool,           // Direct Cache Access
    pub sse4_1: bool,        // SSE4.1
    pub sse4_2: bool,        // SSE4.2
    pub x2apic: bool,        // x2APIC
    pub movbe: bool,         // MOVBE
    pub popcnt: bool,        // POPCNT
    pub tsc_deadline: bool,  // TSC-Deadline
    pub aes: bool,           // AES
    pub xsave: bool,         // XSAVE
    pub osxsave: bool,       // OSXSAVE
    pub avx: bool,           // AVX
    pub f16c: bool,          // F16C
    pub rdrand: bool,        // RDRAND
    pub hypervisor: bool,    // Hypervisor present

    // Extended features (CPUID.07H:EBX)
    pub fsgsbase: bool,      // FSGSBASE
    pub bmi1: bool,          // BMI1
    pub hle: bool,           // Hardware Lock Elision
    pub avx2: bool,          // AVX2
    pub smep: bool,          // Supervisor Mode Execution Prevention
    pub bmi2: bool,          // BMI2
    pub erms: bool,          // Enhanced REP MOVSB/STOSB
    pub invpcid: bool,       // INVPCID
    pub rtm: bool,           // Restricted Transactional Memory
    pub pqm: bool,           // Platform Quality of Service Monitoring
    pub mpx: bool,           // Memory Protection Extensions
    pub pqe: bool,           // Platform Quality of Service Enforcement
    pub avx512f: bool,       // AVX-512 Foundation
    pub avx512dq: bool,      // AVX-512 Doubleword/Quadword
    pub rdseed: bool,        // RDSEED
    pub adx: bool,           // ADX (ADCX/ADOX)
    pub smap: bool,          // Supervisor Mode Access Prevention
    pub avx512ifma: bool,    // AVX-512 Integer FMA
    pub pcommit: bool,       // PCOMMIT (deprecated)
    pub clflushopt: bool,    // CLFLUSHOPT
    pub clwb: bool,          // CLWB
    pub intel_pt: bool,      // Intel Processor Trace
    pub avx512pf: bool,      // AVX-512 Prefetch
    pub avx512er: bool,      // AVX-512 Exponential/Reciprocal
    pub avx512cd: bool,      // AVX-512 Conflict Detection
    pub sha: bool,           // SHA extensions
    pub avx512bw: bool,      // AVX-512 Byte/Word
    pub avx512vl: bool,      // AVX-512 Vector Length Extensions

    // Extended AMD features (CPUID.80000001H:EDX/ECX)
    pub syscall: bool,       // SYSCALL/SYSRET
    pub nx: bool,            // No-Execute bit
    pub mmxext: bool,        // AMD MMX extensions
    pub fxsr_opt: bool,      // FXSAVE/FXRSTOR optimizations
    pub pdpe1gb: bool,       // 1GB pages
    pub rdtscp: bool,        // RDTSCP
    pub lm: bool,            // Long Mode (64-bit)
    pub _3dnowext: bool,     // AMD 3DNow! extensions
    pub _3dnow: bool,        // AMD 3DNow!
    pub lahf_lm: bool,       // LAHF/SAHF in Long Mode
    pub cmp_legacy: bool,    // Hyperthreading not valid
    pub svm: bool,           // Secure Virtual Machine (AMD-V)
    pub extapic: bool,       // Extended APIC space
    pub cr8_legacy: bool,    // CR8 in 32-bit mode
    pub abm: bool,           // Advanced bit manipulation
    pub sse4a: bool,         // SSE4a
    pub misalignsse: bool,   // Misaligned SSE
    pub _3dnow_prefetch: bool, // 3DNow! prefetch
    pub osvw: bool,          // OS Visible Workaround
    pub ibs: bool,           // Instruction Based Sampling
    pub xop: bool,           // XOP
    pub skinit: bool,        // SKINIT/STGI
    pub wdt: bool,           // Watchdog Timer
    pub lwp: bool,           // Lightweight Profiling
    pub fma4: bool,          // FMA4
    pub tce: bool,           // Translation Cache Extension
    pub nodeid_msr: bool,    // NodeId MSR
    pub tbm: bool,           // Trailing Bit Manipulation
    pub topoext: bool,       // Topology Extensions
    pub perfctr_core: bool,  // Core Performance Counter Extensions
    pub perfctr_nb: bool,    // NB Performance Counter Extensions
}

impl CpuInfo {
    pub fn new(apic_id: u8, is_bsp: bool) -> Self {
        Self {
            apic_id,
            is_bsp,
            online: false,
            vendor: String::new(),
            brand: String::new(),
            family: 0,
            model: 0,
            stepping: 0,
            features: CpuFeatures::default(),
        }
    }
}

/// Global CPU information
static CPU_INFO: Mutex<Vec<CpuInfo>> = Mutex::new(Vec::new());
static CPU_COUNT: AtomicU32 = AtomicU32::new(0);
static BSP_APIC_ID: AtomicU32 = AtomicU32::new(0);
static SMP_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Detect current CPU features using CPUID
pub fn detect_cpu_features() -> (String, String, u8, u8, u8, CpuFeatures) {
    let mut features = CpuFeatures::default();
    let mut vendor = String::new();
    let mut brand = String::new();
    let mut family: u8 = 0;
    let mut model: u8 = 0;
    let mut stepping: u8 = 0;

    unsafe {
        // Get vendor string (CPUID.00H)
        let cpuid = core::arch::x86_64::__cpuid(0);
        let max_basic = cpuid.eax;

        // Vendor string is in EBX, EDX, ECX (in that order)
        let vendor_bytes: [u8; 12] = [
            (cpuid.ebx & 0xFF) as u8,
            ((cpuid.ebx >> 8) & 0xFF) as u8,
            ((cpuid.ebx >> 16) & 0xFF) as u8,
            ((cpuid.ebx >> 24) & 0xFF) as u8,
            (cpuid.edx & 0xFF) as u8,
            ((cpuid.edx >> 8) & 0xFF) as u8,
            ((cpuid.edx >> 16) & 0xFF) as u8,
            ((cpuid.edx >> 24) & 0xFF) as u8,
            (cpuid.ecx & 0xFF) as u8,
            ((cpuid.ecx >> 8) & 0xFF) as u8,
            ((cpuid.ecx >> 16) & 0xFF) as u8,
            ((cpuid.ecx >> 24) & 0xFF) as u8,
        ];
        vendor = core::str::from_utf8(&vendor_bytes).unwrap_or("").to_string();

        // Get processor info and features (CPUID.01H)
        if max_basic >= 1 {
            let cpuid = core::arch::x86_64::__cpuid(1);

            // Extract family, model, stepping
            stepping = (cpuid.eax & 0xF) as u8;
            let base_model = ((cpuid.eax >> 4) & 0xF) as u8;
            let base_family = ((cpuid.eax >> 8) & 0xF) as u8;
            let ext_model = ((cpuid.eax >> 16) & 0xF) as u8;
            let ext_family = ((cpuid.eax >> 20) & 0xFF) as u8;

            if base_family == 0xF {
                family = base_family + ext_family;
                model = (ext_model << 4) | base_model;
            } else if base_family == 0x6 {
                family = base_family;
                model = (ext_model << 4) | base_model;
            } else {
                family = base_family;
                model = base_model;
            }

            // EDX features
            let edx = cpuid.edx;
            features.fpu = (edx & (1 << 0)) != 0;
            features.vme = (edx & (1 << 1)) != 0;
            features.de = (edx & (1 << 2)) != 0;
            features.pse = (edx & (1 << 3)) != 0;
            features.tsc = (edx & (1 << 4)) != 0;
            features.msr = (edx & (1 << 5)) != 0;
            features.pae = (edx & (1 << 6)) != 0;
            features.mce = (edx & (1 << 7)) != 0;
            features.cx8 = (edx & (1 << 8)) != 0;
            features.apic = (edx & (1 << 9)) != 0;
            features.sep = (edx & (1 << 11)) != 0;
            features.mtrr = (edx & (1 << 12)) != 0;
            features.pge = (edx & (1 << 13)) != 0;
            features.mca = (edx & (1 << 14)) != 0;
            features.cmov = (edx & (1 << 15)) != 0;
            features.pat = (edx & (1 << 16)) != 0;
            features.pse36 = (edx & (1 << 17)) != 0;
            features.psn = (edx & (1 << 18)) != 0;
            features.clfsh = (edx & (1 << 19)) != 0;
            features.ds = (edx & (1 << 21)) != 0;
            features.acpi = (edx & (1 << 22)) != 0;
            features.mmx = (edx & (1 << 23)) != 0;
            features.fxsr = (edx & (1 << 24)) != 0;
            features.sse = (edx & (1 << 25)) != 0;
            features.sse2 = (edx & (1 << 26)) != 0;
            features.ss = (edx & (1 << 27)) != 0;
            features.htt = (edx & (1 << 28)) != 0;
            features.tm = (edx & (1 << 29)) != 0;
            features.ia64 = (edx & (1 << 30)) != 0;
            features.pbe = (edx & (1 << 31)) != 0;

            // ECX features
            let ecx = cpuid.ecx;
            features.sse3 = (ecx & (1 << 0)) != 0;
            features.pclmulqdq = (ecx & (1 << 1)) != 0;
            features.dtes64 = (ecx & (1 << 2)) != 0;
            features.monitor = (ecx & (1 << 3)) != 0;
            features.ds_cpl = (ecx & (1 << 4)) != 0;
            features.vmx = (ecx & (1 << 5)) != 0;
            features.smx = (ecx & (1 << 6)) != 0;
            features.eist = (ecx & (1 << 7)) != 0;
            features.tm2 = (ecx & (1 << 8)) != 0;
            features.ssse3 = (ecx & (1 << 9)) != 0;
            features.cnxt_id = (ecx & (1 << 10)) != 0;
            features.sdbg = (ecx & (1 << 11)) != 0;
            features.fma = (ecx & (1 << 12)) != 0;
            features.cx16 = (ecx & (1 << 13)) != 0;
            features.xtpr = (ecx & (1 << 14)) != 0;
            features.pdcm = (ecx & (1 << 15)) != 0;
            features.pcid = (ecx & (1 << 17)) != 0;
            features.dca = (ecx & (1 << 18)) != 0;
            features.sse4_1 = (ecx & (1 << 19)) != 0;
            features.sse4_2 = (ecx & (1 << 20)) != 0;
            features.x2apic = (ecx & (1 << 21)) != 0;
            features.movbe = (ecx & (1 << 22)) != 0;
            features.popcnt = (ecx & (1 << 23)) != 0;
            features.tsc_deadline = (ecx & (1 << 24)) != 0;
            features.aes = (ecx & (1 << 25)) != 0;
            features.xsave = (ecx & (1 << 26)) != 0;
            features.osxsave = (ecx & (1 << 27)) != 0;
            features.avx = (ecx & (1 << 28)) != 0;
            features.f16c = (ecx & (1 << 29)) != 0;
            features.rdrand = (ecx & (1 << 30)) != 0;
            features.hypervisor = (ecx & (1 << 31)) != 0;
        }

        // Get extended features (CPUID.07H)
        if max_basic >= 7 {
            let cpuid = core::arch::x86_64::__cpuid_count(7, 0);

            let ebx = cpuid.ebx;
            features.fsgsbase = (ebx & (1 << 0)) != 0;
            features.bmi1 = (ebx & (1 << 3)) != 0;
            features.hle = (ebx & (1 << 4)) != 0;
            features.avx2 = (ebx & (1 << 5)) != 0;
            features.smep = (ebx & (1 << 7)) != 0;
            features.bmi2 = (ebx & (1 << 8)) != 0;
            features.erms = (ebx & (1 << 9)) != 0;
            features.invpcid = (ebx & (1 << 10)) != 0;
            features.rtm = (ebx & (1 << 11)) != 0;
            features.pqm = (ebx & (1 << 12)) != 0;
            features.mpx = (ebx & (1 << 14)) != 0;
            features.pqe = (ebx & (1 << 15)) != 0;
            features.avx512f = (ebx & (1 << 16)) != 0;
            features.avx512dq = (ebx & (1 << 17)) != 0;
            features.rdseed = (ebx & (1 << 18)) != 0;
            features.adx = (ebx & (1 << 19)) != 0;
            features.smap = (ebx & (1 << 20)) != 0;
            features.avx512ifma = (ebx & (1 << 21)) != 0;
            features.pcommit = (ebx & (1 << 22)) != 0;
            features.clflushopt = (ebx & (1 << 23)) != 0;
            features.clwb = (ebx & (1 << 24)) != 0;
            features.intel_pt = (ebx & (1 << 25)) != 0;
            features.avx512pf = (ebx & (1 << 26)) != 0;
            features.avx512er = (ebx & (1 << 27)) != 0;
            features.avx512cd = (ebx & (1 << 28)) != 0;
            features.sha = (ebx & (1 << 29)) != 0;
            features.avx512bw = (ebx & (1 << 30)) != 0;
            features.avx512vl = (ebx & (1 << 31)) != 0;
        }

        // Get extended function info (CPUID.80000000H)
        let cpuid = core::arch::x86_64::__cpuid(0x80000000);
        let max_extended = cpuid.eax;

        // Get extended features (CPUID.80000001H)
        if max_extended >= 0x80000001 {
            let cpuid = core::arch::x86_64::__cpuid(0x80000001);

            let edx = cpuid.edx;
            features.syscall = (edx & (1 << 11)) != 0;
            features.nx = (edx & (1 << 20)) != 0;
            features.mmxext = (edx & (1 << 22)) != 0;
            features.fxsr_opt = (edx & (1 << 25)) != 0;
            features.pdpe1gb = (edx & (1 << 26)) != 0;
            features.rdtscp = (edx & (1 << 27)) != 0;
            features.lm = (edx & (1 << 29)) != 0;
            features._3dnowext = (edx & (1 << 30)) != 0;
            features._3dnow = (edx & (1 << 31)) != 0;

            let ecx = cpuid.ecx;
            features.lahf_lm = (ecx & (1 << 0)) != 0;
            features.cmp_legacy = (ecx & (1 << 1)) != 0;
            features.svm = (ecx & (1 << 2)) != 0;
            features.extapic = (ecx & (1 << 3)) != 0;
            features.cr8_legacy = (ecx & (1 << 4)) != 0;
            features.abm = (ecx & (1 << 5)) != 0;
            features.sse4a = (ecx & (1 << 6)) != 0;
            features.misalignsse = (ecx & (1 << 7)) != 0;
            features._3dnow_prefetch = (ecx & (1 << 8)) != 0;
            features.osvw = (ecx & (1 << 9)) != 0;
            features.ibs = (ecx & (1 << 10)) != 0;
            features.xop = (ecx & (1 << 11)) != 0;
            features.skinit = (ecx & (1 << 12)) != 0;
            features.wdt = (ecx & (1 << 13)) != 0;
            features.lwp = (ecx & (1 << 15)) != 0;
            features.fma4 = (ecx & (1 << 16)) != 0;
            features.tce = (ecx & (1 << 17)) != 0;
            features.nodeid_msr = (ecx & (1 << 19)) != 0;
            features.tbm = (ecx & (1 << 21)) != 0;
            features.topoext = (ecx & (1 << 22)) != 0;
            features.perfctr_core = (ecx & (1 << 23)) != 0;
            features.perfctr_nb = (ecx & (1 << 24)) != 0;
        }

        // Get brand string (CPUID.80000002H-80000004H)
        if max_extended >= 0x80000004 {
            let mut brand_bytes = [0u8; 48];

            for i in 0..3 {
                let cpuid = core::arch::x86_64::__cpuid(0x80000002 + i);
                let offset = i as usize * 16;

                brand_bytes[offset..offset + 4].copy_from_slice(&cpuid.eax.to_le_bytes());
                brand_bytes[offset + 4..offset + 8].copy_from_slice(&cpuid.ebx.to_le_bytes());
                brand_bytes[offset + 8..offset + 12].copy_from_slice(&cpuid.ecx.to_le_bytes());
                brand_bytes[offset + 12..offset + 16].copy_from_slice(&cpuid.edx.to_le_bytes());
            }

            brand = core::str::from_utf8(&brand_bytes)
                .unwrap_or("")
                .trim_end_matches('\0')
                .trim()
                .to_string();
        }
    }

    (vendor, brand, family, model, stepping, features)
}

/// Detect all CPUs from MADT
pub fn detect_cpus() {
    // Get BSP APIC ID
    let bsp_id = super::apic::lapic_id() as u8;
    BSP_APIC_ID.store(bsp_id as u32, Ordering::Relaxed);

    // Get MADT info
    let madt_info = crate::drivers::acpi::parse_madt();

    let (vendor, brand, family, model, stepping, features) = detect_cpu_features();

    let mut cpus = CPU_INFO.lock();

    if let Some(ref info) = madt_info {
        for (i, lapic) in info.local_apics.iter().enumerate() {
            let is_bsp = lapic.apic_id == bsp_id;

            let mut cpu = CpuInfo::new(lapic.apic_id, is_bsp);
            cpu.online = is_bsp; // Only BSP is online initially
            cpu.vendor = vendor.clone();
            cpu.brand = brand.clone();
            cpu.family = family;
            cpu.model = model;
            cpu.stepping = stepping;
            cpu.features = features.clone();

            cpus.push(cpu);

            if i == 0 || is_bsp {
                crate::kprintln!(
                    "smp: CPU {} (APIC ID {}) {}",
                    i,
                    lapic.apic_id,
                    if is_bsp { "[BSP]" } else { "[AP]" }
                );
            }
        }

        CPU_COUNT.store(info.local_apics.len() as u32, Ordering::Relaxed);
    } else {
        // No MADT, assume single CPU
        let mut cpu = CpuInfo::new(bsp_id, true);
        cpu.online = true;
        cpu.vendor = vendor.clone();
        cpu.brand = brand.clone();
        cpu.family = family;
        cpu.model = model;
        cpu.stepping = stepping;
        cpu.features = features.clone();

        cpus.push(cpu);
        CPU_COUNT.store(1, Ordering::Relaxed);

        crate::kprintln!("smp: single CPU detected (no MADT)");
    }

    // Print CPU info
    if !brand.is_empty() {
        crate::kprintln!("smp: {}", brand);
    } else {
        crate::kprintln!("smp: {} Family {} Model {} Stepping {}", vendor, family, model, stepping);
    }

    // Print key features
    let mut feat_str = String::new();
    if features.sse { feat_str.push_str("SSE "); }
    if features.sse2 { feat_str.push_str("SSE2 "); }
    if features.sse3 { feat_str.push_str("SSE3 "); }
    if features.ssse3 { feat_str.push_str("SSSE3 "); }
    if features.sse4_1 { feat_str.push_str("SSE4.1 "); }
    if features.sse4_2 { feat_str.push_str("SSE4.2 "); }
    if features.avx { feat_str.push_str("AVX "); }
    if features.avx2 { feat_str.push_str("AVX2 "); }
    if features.aes { feat_str.push_str("AES "); }
    if features.vmx { feat_str.push_str("VMX "); }
    if features.svm { feat_str.push_str("SVM "); }
    if features.nx { feat_str.push_str("NX "); }
    if features.x2apic { feat_str.push_str("x2APIC "); }
    if features.hypervisor { feat_str.push_str("HV "); }

    if !feat_str.is_empty() {
        crate::kprintln!("smp: features: {}", feat_str.trim());
    }

    SMP_INITIALIZED.store(true, Ordering::Relaxed);
}

/// Get the number of detected CPUs
pub fn cpu_count() -> u32 {
    CPU_COUNT.load(Ordering::Relaxed)
}

/// Get the BSP's APIC ID
pub fn bsp_apic_id() -> u8 {
    BSP_APIC_ID.load(Ordering::Relaxed) as u8
}

/// Check if SMP is initialized
pub fn is_initialized() -> bool {
    SMP_INITIALIZED.load(Ordering::Relaxed)
}

/// Get current CPU's APIC ID
pub fn current_cpu_id() -> u8 {
    super::apic::lapic_id() as u8
}

/// Check if we're running on the BSP
pub fn is_bsp() -> bool {
    current_cpu_id() == bsp_apic_id()
}

/// Initialize SMP subsystem
pub fn init() {
    crate::kprintln!("smp: detecting CPUs...");
    detect_cpus();
}

/// Get info for a specific CPU by APIC ID
pub fn get_cpu_info(apic_id: u8) -> Option<CpuInfo> {
    let cpus = CPU_INFO.lock();
    cpus.iter().find(|c| c.apic_id == apic_id).cloned()
}

/// Get all CPU info
pub fn get_all_cpus() -> Vec<CpuInfo> {
    let cpus = CPU_INFO.lock();
    cpus.clone()
}

// ============================================================================
// AP (Application Processor) Startup
// ============================================================================

/// Trampoline code address (must be below 1MB, page-aligned)
const AP_TRAMPOLINE_ADDR: u64 = 0x8000;

/// Stack size per AP
const AP_STACK_SIZE: usize = 16384; // 16KB per AP

/// Wrapper for raw pointer to make it Send+Sync
struct ApStack(*mut u8);
unsafe impl Send for ApStack {}
unsafe impl Sync for ApStack {}

/// AP stacks (allocated during init)
static AP_STACKS: Mutex<Vec<ApStack>> = Mutex::new(Vec::new());

/// Number of APs that have started
static APS_STARTED: AtomicU32 = AtomicU32::new(0);

/// Flag for AP to signal it has started
static AP_STARTED_FLAG: AtomicBool = AtomicBool::new(false);

/// Current AP being started (APIC ID)
static CURRENT_AP_APIC_ID: AtomicU32 = AtomicU32::new(0);

/// Raw trampoline code bytes for AP startup
/// This is hand-assembled code that transitions from real mode to long mode
/// Offsets for data:
///   0xF0: CR3 value (8 bytes)
///   0xF8: Stack pointer (8 bytes)
///   0x100: Entry point (8 bytes)
static AP_TRAMPOLINE_CODE: &[u8] = &[
    // 16-bit real mode code (offset 0x00)
    0xFA,                   // cli
    0xFC,                   // cld
    0x31, 0xC0,             // xor ax, ax
    0x8E, 0xD8,             // mov ds, ax
    0x8E, 0xC0,             // mov es, ax
    0x8E, 0xD0,             // mov ss, ax
    0xBC, 0x00, 0x7C,       // mov sp, 0x7c00

    // Enable A20 (fast A20 gate)
    0xE4, 0x92,             // in al, 0x92
    0x0C, 0x02,             // or al, 2
    0xE6, 0x92,             // out 0x92, al

    // Load GDT pointer at 0x80C0
    0x0F, 0x01, 0x16, 0xC0, 0x80, // lgdt [0x80C0]

    // Enable protected mode (CR0.PE = 1)
    0x0F, 0x20, 0xC0,       // mov eax, cr0
    0x0C, 0x01,             // or al, 1
    0x0F, 0x22, 0xC0,       // mov cr0, eax

    // Far jump to protected mode (0x08:0x8030)
    0xEA, 0x30, 0x80, 0x00, 0x00, 0x08, 0x00, // ljmp 0x08, 0x8030

    // Padding to offset 0x30 (32-bit protected mode)
    0x90, 0x90, 0x90,

    // 32-bit protected mode code (offset 0x30)
    0x66, 0xB8, 0x10, 0x00, // mov ax, 0x10
    0x8E, 0xD8,             // mov ds, ax
    0x8E, 0xC0,             // mov es, ax
    0x8E, 0xD0,             // mov ss, ax

    // Enable PAE (CR4.PAE = 1)
    0x0F, 0x20, 0xE0,       // mov eax, cr4
    0x0C, 0x20,             // or al, 0x20
    0x0F, 0x22, 0xE0,       // mov cr4, eax

    // Load CR3 from data area (offset 0xF0)
    0xA1, 0xF0, 0x80, 0x00, 0x00, // mov eax, [0x80F0]
    0x0F, 0x22, 0xD8,       // mov cr3, eax

    // Enable long mode (EFER.LME = 1)
    0xB9, 0x80, 0x00, 0x00, 0xC0, // mov ecx, 0xC0000080
    0x0F, 0x32,             // rdmsr
    0x0D, 0x00, 0x01, 0x00, 0x00, // or eax, 0x100
    0x0F, 0x30,             // wrmsr

    // Enable paging (CR0.PG = 1)
    0x0F, 0x20, 0xC0,       // mov eax, cr0
    0x0D, 0x00, 0x00, 0x00, 0x80, // or eax, 0x80000000
    0x0F, 0x22, 0xC0,       // mov cr0, eax

    // Load 64-bit GDT pointer at 0x80D0
    0x0F, 0x01, 0x15, 0xD0, 0x80, 0x00, 0x00, // lgdt [0x80D0]

    // Far jump to long mode (0x08:0x8080) - using retf trick
    0x68, 0x08, 0x00, 0x00, 0x00, // push 0x08 (code segment)
    0x68, 0x80, 0x80, 0x00, 0x00, // push 0x8080 (offset)
    0xCB,                   // retf

    // Padding to offset 0x80 (64-bit long mode)
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,

    // 64-bit long mode code (offset 0x80)
    // Set data segments
    0x66, 0xB8, 0x10, 0x00, // mov ax, 0x10
    0x8E, 0xD8,             // mov ds, ax
    0x8E, 0xC0,             // mov es, ax
    0x8E, 0xD0,             // mov ss, ax
    0x8E, 0xE0,             // mov fs, ax
    0x8E, 0xE8,             // mov gs, ax

    // Load stack from data area (offset 0xF8)
    0x48, 0x8B, 0x24, 0x25, 0xF8, 0x80, 0x00, 0x00, // mov rsp, [0x80F8]

    // Jump to entry point (offset 0x100)
    0x48, 0x8B, 0x04, 0x25, 0x00, 0x81, 0x00, 0x00, // mov rax, [0x8100]
    0xFF, 0xE0,             // jmp rax

    // Padding to GDT at offset 0xB0
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
    0x90, 0x90,

    // 32-bit GDT at offset 0xB0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Null descriptor
    0xFF, 0xFF, 0x00, 0x00, 0x00, 0x9A, 0xCF, 0x00, // 32-bit code
    0xFF, 0xFF, 0x00, 0x00, 0x00, 0x92, 0xCF, 0x00, // 32-bit data

    // GDT pointer at offset 0xC8 (pointing to 0x80B0)
    0x17, 0x00,             // limit = 24 - 1
    0xB0, 0x80, 0x00, 0x00, // base = 0x80B0
    0x00, 0x00,             // padding

    // 64-bit GDT at offset 0xD0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Null descriptor
    0xFF, 0xFF, 0x00, 0x00, 0x00, 0x9A, 0xAF, 0x00, // 64-bit code
    0xFF, 0xFF, 0x00, 0x00, 0x00, 0x92, 0xAF, 0x00, // 64-bit data

    // GDT64 pointer at offset 0xE8 (pointing to 0x80D0) - this should be at 0xD0
    0x17, 0x00,             // limit = 24 - 1
    0xD0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // base = 0x80D0
];

/// GDT pointer offset in trampoline
const GDT_PTR_OFFSET: usize = 0xC0;
/// GDT64 pointer offset in trampoline
const GDT64_PTR_OFFSET: usize = 0xD0;

/// Size of the trampoline code
const AP_TRAMPOLINE_SIZE: usize = 512;

/// Offsets for data in trampoline
const AP_CR3_OFFSET: usize = 0x100;
const AP_STACK_OFFSET: usize = 0x108;
const AP_ENTRY_OFFSET: usize = 0x110;

/// AP entry point (called from trampoline in long mode)
#[no_mangle]
pub extern "C" fn ap_entry() -> ! {
    // Get our APIC ID
    let apic_id = super::apic::lapic_id() as u8;

    // Get CPU number (APs started + 1, since BSP is 0)
    let cpu_num = APS_STARTED.load(Ordering::SeqCst) as usize + 1;

    // Initialize per-CPU data for this AP
    super::percpu::init_ap(cpu_num, apic_id as u32);

    // Mark this AP as started
    {
        let mut cpus = CPU_INFO.lock();
        if let Some(cpu) = cpus.iter_mut().find(|c| c.apic_id == apic_id) {
            cpu.online = true;
        }
    }

    // Signal that we've started
    AP_STARTED_FLAG.store(true, Ordering::SeqCst);
    APS_STARTED.fetch_add(1, Ordering::SeqCst);

    crate::kprintln!("smp: AP {} (CPU {}) online", apic_id, cpu_num);

    // Initialize Local APIC for this AP
    unsafe {
        init_ap_lapic();
    }

    // Enable interrupts on this AP
    x86_64::instructions::interrupts::enable();

    // Enter AP idle loop
    ap_idle_loop()
}

/// Initialize Local APIC for an AP
unsafe fn init_ap_lapic() {
    use core::ptr::write_volatile;

    let lapic_base = crate::mm::phys_to_virt(x86_64::PhysAddr::new(0xFEE00000)).as_u64();

    // Enable APIC in spurious vector register
    let svr_addr = (lapic_base + 0xF0) as *mut u32;
    write_volatile(svr_addr, (1 << 8) | 0xFF);

    // Set task priority to 0 (accept all interrupts)
    let tpr_addr = (lapic_base + 0x80) as *mut u32;
    write_volatile(tpr_addr, 0);
}

/// AP idle loop
fn ap_idle_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

/// Start all Application Processors
pub fn start_aps() {
    if !SMP_INITIALIZED.load(Ordering::Relaxed) {
        crate::kprintln!("smp: must call detect_cpus() before start_aps()");
        return;
    }

    let cpu_count = CPU_COUNT.load(Ordering::Relaxed);
    if cpu_count <= 1 {
        crate::kprintln!("smp: no APs to start (single CPU system)");
        return;
    }

    crate::kprintln!("smp: starting {} APs...", cpu_count - 1);

    // Allocate stacks for APs
    allocate_ap_stacks(cpu_count as usize - 1);

    // Copy trampoline code to low memory
    setup_trampoline();

    // Get list of AP APIC IDs
    let ap_ids: Vec<u8> = {
        let cpus = CPU_INFO.lock();
        cpus.iter()
            .filter(|c| !c.is_bsp)
            .map(|c| c.apic_id)
            .collect()
    };

    // Start each AP
    for (idx, &apic_id) in ap_ids.iter().enumerate() {
        start_ap(apic_id, idx);
    }

    let started = APS_STARTED.load(Ordering::Relaxed);
    crate::kprintln!("smp: {} APs started successfully", started);
}

/// Allocate stacks for APs
fn allocate_ap_stacks(count: usize) {
    let mut stacks = AP_STACKS.lock();

    for _ in 0..count {
        // Allocate stack using kernel heap
        let stack = alloc::vec![0u8; AP_STACK_SIZE].into_boxed_slice();
        let stack_ptr = Box::into_raw(stack) as *mut u8;
        stacks.push(ApStack(stack_ptr));
    }

    crate::kprintln!("smp: allocated {} AP stacks ({} KB each)", count, AP_STACK_SIZE / 1024);
}

/// Copy trampoline code to low memory and set up data
fn setup_trampoline() {
    unsafe {
        // Get virtual address of trampoline location
        let trampoline_virt = crate::mm::phys_to_virt(
            x86_64::PhysAddr::new(AP_TRAMPOLINE_ADDR)
        ).as_u64() as *mut u8;

        // Clear the trampoline area first
        core::ptr::write_bytes(trampoline_virt, 0, AP_TRAMPOLINE_SIZE);

        // Copy trampoline code
        let code_len = AP_TRAMPOLINE_CODE.len();
        core::ptr::copy_nonoverlapping(
            AP_TRAMPOLINE_CODE.as_ptr(),
            trampoline_virt,
            code_len
        );

        // Set CR3 (page table) at offset 0xF0
        let cr3: u64;
        core::arch::asm!("mov {}, cr3", out(reg) cr3);
        let cr3_ptr = (trampoline_virt as u64 + AP_CR3_OFFSET as u64) as *mut u64;
        *cr3_ptr = cr3;

        // Set entry point at offset 0x100
        let entry_ptr = (trampoline_virt as u64 + AP_ENTRY_OFFSET as u64) as *mut u64;
        *entry_ptr = ap_entry as *const () as u64;

        crate::kprintln!("smp: trampoline copied to {:#x} ({} bytes), CR3={:#x}",
                         AP_TRAMPOLINE_ADDR, code_len, cr3);
    }
}

/// Start a single AP
fn start_ap(apic_id: u8, stack_idx: usize) {
    // Set stack pointer for this AP
    let stack_ptr = {
        let stacks = AP_STACKS.lock();
        if stack_idx >= stacks.len() {
            crate::kprintln!("smp: no stack for AP {}", apic_id);
            return;
        }
        // Stack grows down, so point to the end
        unsafe { stacks[stack_idx].0.add(AP_STACK_SIZE) }
    };

    unsafe {
        // Write stack pointer to trampoline data area
        let trampoline_virt = crate::mm::phys_to_virt(
            x86_64::PhysAddr::new(AP_TRAMPOLINE_ADDR)
        ).as_u64();
        let stack_data_ptr = (trampoline_virt + AP_STACK_OFFSET as u64) as *mut u64;
        *stack_data_ptr = stack_ptr as u64;
    }

    // Clear started flag
    AP_STARTED_FLAG.store(false, Ordering::SeqCst);
    CURRENT_AP_APIC_ID.store(apic_id as u32, Ordering::Relaxed);

    // Send INIT IPI
    super::apic::send_init_ipi(apic_id);

    // Wait 10ms
    delay_ms(10);

    // Send SIPI (Startup IPI) - vector is page number of trampoline
    let vector = (AP_TRAMPOLINE_ADDR / 0x1000) as u8;
    super::apic::send_sipi(apic_id, vector);

    // Wait 200us
    delay_us(200);

    // If AP didn't start, send SIPI again
    if !AP_STARTED_FLAG.load(Ordering::SeqCst) {
        super::apic::send_sipi(apic_id, vector);
        delay_ms(100); // Wait longer for second attempt
    }

    // Check if AP started
    if AP_STARTED_FLAG.load(Ordering::SeqCst) {
        crate::kprintln!("smp: AP {} (APIC ID {}) started", stack_idx + 1, apic_id);
    } else {
        crate::kprintln!("smp: AP {} (APIC ID {}) failed to start", stack_idx + 1, apic_id);
    }
}

/// Simple delay in milliseconds (using busy loop)
fn delay_ms(ms: u32) {
    for _ in 0..(ms * 1000) {
        delay_us(1);
    }
}

/// Simple delay in microseconds (using busy loop)
fn delay_us(us: u32) {
    // Rough estimate: ~1000 iterations per microsecond on modern CPUs
    for _ in 0..(us * 100) {
        core::hint::spin_loop();
    }
}

/// Get number of online CPUs
pub fn online_cpu_count() -> u32 {
    let cpus = CPU_INFO.lock();
    cpus.iter().filter(|c| c.online).count() as u32
}

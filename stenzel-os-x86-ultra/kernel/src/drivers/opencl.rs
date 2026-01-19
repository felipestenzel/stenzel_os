//! OpenCL GPU Compute Support
//!
//! Provides OpenCL-compatible GPU compute interface:
//! - Device enumeration and selection
//! - Context and command queue management
//! - Memory buffer allocation
//! - Kernel compilation and execution
//! - Work-group dispatching

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::{KResult, KError};

/// OpenCL device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClDeviceType {
    /// CPU compute device
    Cpu,
    /// GPU compute device
    Gpu,
    /// Accelerator (e.g., FPGA, DSP)
    Accelerator,
    /// Custom device
    Custom,
    /// All devices
    All,
}

/// OpenCL vendor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClVendor {
    /// Intel
    Intel,
    /// AMD
    Amd,
    /// NVIDIA
    Nvidia,
    /// Other
    Other,
}

/// OpenCL memory type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClMemType {
    /// Global memory
    Global,
    /// Local memory (work-group shared)
    Local,
    /// Constant memory
    Constant,
    /// Private memory (work-item)
    Private,
}

/// OpenCL memory flags
#[derive(Debug, Clone, Copy, Default)]
pub struct ClMemFlags {
    /// Read-only buffer
    pub read_only: bool,
    /// Write-only buffer
    pub write_only: bool,
    /// Read-write buffer
    pub read_write: bool,
    /// Use host pointer
    pub use_host_ptr: bool,
    /// Allocate host pointer
    pub alloc_host_ptr: bool,
    /// Copy host pointer
    pub copy_host_ptr: bool,
}

/// OpenCL command type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClCommandType {
    /// Read buffer
    ReadBuffer,
    /// Write buffer
    WriteBuffer,
    /// Copy buffer
    CopyBuffer,
    /// Map buffer
    MapBuffer,
    /// Unmap buffer
    UnmapBuffer,
    /// Execute kernel
    NdRangeKernel,
    /// Task (single work-item)
    Task,
    /// Native kernel
    NativeKernel,
    /// Barrier
    Barrier,
    /// Marker
    Marker,
}

/// OpenCL execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClExecStatus {
    /// Command queued
    Queued,
    /// Command submitted
    Submitted,
    /// Command running
    Running,
    /// Command complete
    Complete,
    /// Command error
    Error,
}

/// OpenCL platform info
#[derive(Debug, Clone)]
pub struct ClPlatformInfo {
    /// Platform ID
    pub id: u32,
    /// Platform name
    pub name: String,
    /// Vendor name
    pub vendor: String,
    /// Version string
    pub version: String,
    /// Profile (FULL_PROFILE or EMBEDDED_PROFILE)
    pub profile: String,
    /// Extensions
    pub extensions: Vec<String>,
}

impl Default for ClPlatformInfo {
    fn default() -> Self {
        ClPlatformInfo {
            id: 0,
            name: String::from("Stenzel OS OpenCL"),
            vendor: String::from("Stenzel"),
            version: String::from("OpenCL 1.2"),
            profile: String::from("FULL_PROFILE"),
            extensions: Vec::new(),
        }
    }
}

/// OpenCL device info
#[derive(Debug, Clone)]
pub struct ClDeviceInfo {
    /// Device ID
    pub id: u32,
    /// Device name
    pub name: String,
    /// Device vendor
    pub vendor: ClVendor,
    /// Device type
    pub device_type: ClDeviceType,
    /// Max compute units
    pub max_compute_units: u32,
    /// Max work-group size
    pub max_work_group_size: u64,
    /// Max work-item dimensions
    pub max_work_item_dimensions: u32,
    /// Max work-item sizes
    pub max_work_item_sizes: [u64; 3],
    /// Global memory size
    pub global_mem_size: u64,
    /// Local memory size
    pub local_mem_size: u64,
    /// Constant buffer size
    pub max_constant_buffer_size: u64,
    /// Max memory allocation size
    pub max_mem_alloc_size: u64,
    /// Image support
    pub image_support: bool,
    /// Max 2D image width
    pub image2d_max_width: u64,
    /// Max 2D image height
    pub image2d_max_height: u64,
    /// Double precision support
    pub double_fp_support: bool,
    /// OpenCL version
    pub opencl_version: String,
    /// Driver version
    pub driver_version: String,
    /// Extensions
    pub extensions: Vec<String>,
    /// Available
    pub available: bool,
}

impl Default for ClDeviceInfo {
    fn default() -> Self {
        ClDeviceInfo {
            id: 0,
            name: String::new(),
            vendor: ClVendor::Other,
            device_type: ClDeviceType::Gpu,
            max_compute_units: 1,
            max_work_group_size: 256,
            max_work_item_dimensions: 3,
            max_work_item_sizes: [256, 256, 256],
            global_mem_size: 0,
            local_mem_size: 32768,
            max_constant_buffer_size: 65536,
            max_mem_alloc_size: 0,
            image_support: false,
            image2d_max_width: 0,
            image2d_max_height: 0,
            double_fp_support: false,
            opencl_version: String::from("OpenCL 1.2"),
            driver_version: String::new(),
            extensions: Vec::new(),
            available: false,
        }
    }
}

/// OpenCL context
#[derive(Debug)]
pub struct ClContext {
    /// Context ID
    pub id: u32,
    /// Associated devices
    pub devices: Vec<u32>,
    /// Reference count
    ref_count: AtomicU32,
    /// Valid
    valid: AtomicBool,
}

/// OpenCL command queue
#[derive(Debug)]
pub struct ClCommandQueue {
    /// Queue ID
    pub id: u32,
    /// Context ID
    pub context_id: u32,
    /// Device ID
    pub device_id: u32,
    /// Out-of-order execution
    pub out_of_order: bool,
    /// Profiling enabled
    pub profiling: bool,
    /// Commands pending
    commands_pending: AtomicU32,
    /// Commands completed
    commands_completed: AtomicU64,
}

/// OpenCL memory buffer
#[derive(Debug)]
pub struct ClBuffer {
    /// Buffer ID
    pub id: u32,
    /// Context ID
    pub context_id: u32,
    /// Size in bytes
    pub size: u64,
    /// Flags
    pub flags: ClMemFlags,
    /// Device memory address
    pub device_ptr: u64,
    /// Host pointer (if mapped)
    pub host_ptr: Option<u64>,
    /// Reference count
    ref_count: AtomicU32,
}

/// OpenCL program (compiled kernels)
#[derive(Debug)]
pub struct ClProgram {
    /// Program ID
    pub id: u32,
    /// Context ID
    pub context_id: u32,
    /// Source code
    pub source: String,
    /// Compiled binary
    pub binary: Vec<u8>,
    /// Build status
    pub build_status: ClBuildStatus,
    /// Build log
    pub build_log: String,
    /// Kernel names
    pub kernel_names: Vec<String>,
}

/// OpenCL build status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClBuildStatus {
    /// Not built
    None,
    /// Build in progress
    InProgress,
    /// Build successful
    Success,
    /// Build error
    Error,
}

/// OpenCL kernel
#[derive(Debug)]
pub struct ClKernel {
    /// Kernel ID
    pub id: u32,
    /// Program ID
    pub program_id: u32,
    /// Kernel name
    pub name: String,
    /// Number of arguments
    pub num_args: u32,
    /// Work-group size
    pub work_group_size: u64,
    /// Local memory size
    pub local_mem_size: u64,
    /// Preferred work-group size multiple
    pub preferred_work_group_size_multiple: u64,
    /// Arguments set
    args: Vec<ClKernelArg>,
}

/// Kernel argument
#[derive(Debug, Clone)]
pub struct ClKernelArg {
    /// Argument index
    pub index: u32,
    /// Argument size
    pub size: u64,
    /// Value (buffer ID or raw data)
    pub value: ClArgValue,
}

/// Kernel argument value
#[derive(Debug, Clone)]
pub enum ClArgValue {
    /// Buffer reference
    Buffer(u32),
    /// Scalar value
    Scalar(Vec<u8>),
    /// Local memory size
    Local(u64),
    /// None (not set)
    None,
}

/// OpenCL event
#[derive(Debug)]
pub struct ClEvent {
    /// Event ID
    pub id: u32,
    /// Command type
    pub command_type: ClCommandType,
    /// Execution status
    pub status: ClExecStatus,
    /// Queued timestamp
    pub queued_time: u64,
    /// Submit timestamp
    pub submit_time: u64,
    /// Start timestamp
    pub start_time: u64,
    /// End timestamp
    pub end_time: u64,
}

/// OpenCL statistics
#[derive(Debug, Default)]
pub struct ClStats {
    /// Contexts created
    pub contexts_created: AtomicU64,
    /// Buffers allocated
    pub buffers_allocated: AtomicU64,
    /// Total buffer memory
    pub buffer_memory: AtomicU64,
    /// Kernels executed
    pub kernels_executed: AtomicU64,
    /// Kernel execution time
    pub kernel_time_ns: AtomicU64,
    /// Buffer transfers
    pub buffer_transfers: AtomicU64,
    /// Bytes transferred
    pub bytes_transferred: AtomicU64,
}

/// OpenCL manager
pub struct OpenClManager {
    /// Platform info
    platform: ClPlatformInfo,
    /// Available devices
    devices: Vec<ClDeviceInfo>,
    /// Active contexts
    contexts: Vec<ClContext>,
    /// Command queues
    queues: Vec<ClCommandQueue>,
    /// Memory buffers
    buffers: Vec<ClBuffer>,
    /// Programs
    programs: Vec<ClProgram>,
    /// Kernels
    kernels: Vec<ClKernel>,
    /// Events
    events: Vec<ClEvent>,
    /// Next IDs
    next_context_id: u32,
    next_queue_id: u32,
    next_buffer_id: u32,
    next_program_id: u32,
    next_kernel_id: u32,
    next_event_id: u32,
    /// Initialized
    initialized: bool,
    /// Statistics
    stats: ClStats,
}

pub static OPENCL_MANAGER: IrqSafeMutex<OpenClManager> = IrqSafeMutex::new(OpenClManager::new());

impl OpenClManager {
    pub const fn new() -> Self {
        OpenClManager {
            platform: ClPlatformInfo {
                id: 1,
                name: String::new(),
                vendor: String::new(),
                version: String::new(),
                profile: String::new(),
                extensions: Vec::new(),
            },
            devices: Vec::new(),
            contexts: Vec::new(),
            queues: Vec::new(),
            buffers: Vec::new(),
            programs: Vec::new(),
            kernels: Vec::new(),
            events: Vec::new(),
            next_context_id: 1,
            next_queue_id: 1,
            next_buffer_id: 1,
            next_program_id: 1,
            next_kernel_id: 1,
            next_event_id: 1,
            initialized: false,
            stats: ClStats {
                contexts_created: AtomicU64::new(0),
                buffers_allocated: AtomicU64::new(0),
                buffer_memory: AtomicU64::new(0),
                kernels_executed: AtomicU64::new(0),
                kernel_time_ns: AtomicU64::new(0),
                buffer_transfers: AtomicU64::new(0),
                bytes_transferred: AtomicU64::new(0),
            },
        }
    }

    /// Initialize OpenCL runtime
    pub fn init(&mut self) -> KResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Initialize platform info
        self.platform = ClPlatformInfo::default();
        self.platform.extensions.push(String::from("cl_khr_global_int32_base_atomics"));
        self.platform.extensions.push(String::from("cl_khr_global_int32_extended_atomics"));
        self.platform.extensions.push(String::from("cl_khr_byte_addressable_store"));

        // Enumerate GPU devices
        self.enumerate_devices()?;

        self.initialized = true;
        crate::kprintln!("opencl: initialized with {} device(s)", self.devices.len());
        Ok(())
    }

    /// Enumerate compute devices
    fn enumerate_devices(&mut self) -> KResult<()> {
        let pci_devices = crate::drivers::pci::scan();

        for device in pci_devices {
            // Check for GPU (class 0x03)
            if device.class.class_code != 0x03 {
                continue;
            }

            let mut cl_device = ClDeviceInfo::default();
            cl_device.id = self.devices.len() as u32 + 1;

            // Determine vendor
            match device.id.vendor_id {
                0x8086 => {
                    cl_device.vendor = ClVendor::Intel;
                    cl_device.name = String::from("Intel GPU");
                    self.configure_intel_device(&mut cl_device, &device);
                }
                0x1002 => {
                    cl_device.vendor = ClVendor::Amd;
                    cl_device.name = String::from("AMD GPU");
                    self.configure_amd_device(&mut cl_device, &device);
                }
                0x10DE => {
                    cl_device.vendor = ClVendor::Nvidia;
                    cl_device.name = String::from("NVIDIA GPU");
                    self.configure_nvidia_device(&mut cl_device, &device);
                }
                _ => continue,
            }

            cl_device.device_type = ClDeviceType::Gpu;
            cl_device.available = true;
            cl_device.extensions.push(String::from("cl_khr_global_int32_base_atomics"));

            self.devices.push(cl_device);
        }

        // Also add CPU as compute device
        let mut cpu_device = ClDeviceInfo::default();
        cpu_device.id = self.devices.len() as u32 + 1;
        cpu_device.device_type = ClDeviceType::Cpu;
        cpu_device.vendor = ClVendor::Other;
        cpu_device.name = String::from("CPU");
        cpu_device.available = true;
        cpu_device.max_compute_units = crate::arch::cpu_count() as u32;
        cpu_device.max_work_group_size = 1024;
        let (total_frames, _, _) = crate::mm::memory_stats();
        cpu_device.global_mem_size = (total_frames * 4096) as u64;
        cpu_device.local_mem_size = 32768;
        cpu_device.double_fp_support = true;
        self.devices.push(cpu_device);

        Ok(())
    }

    /// Configure Intel GPU device
    fn configure_intel_device(&self, device: &mut ClDeviceInfo, _pci: &crate::drivers::pci::PciDevice) {
        device.max_compute_units = 24;
        device.max_work_group_size = 256;
        device.local_mem_size = 65536;
        device.image_support = true;
        device.image2d_max_width = 16384;
        device.image2d_max_height = 16384;
        device.double_fp_support = true;
        device.extensions.push(String::from("cl_intel_subgroups"));
    }

    /// Configure AMD GPU device
    fn configure_amd_device(&self, device: &mut ClDeviceInfo, _pci: &crate::drivers::pci::PciDevice) {
        device.max_compute_units = 36;
        device.max_work_group_size = 256;
        device.local_mem_size = 65536;
        device.image_support = true;
        device.image2d_max_width = 16384;
        device.image2d_max_height = 16384;
        device.double_fp_support = true;
        device.extensions.push(String::from("cl_amd_device_attribute_query"));
    }

    /// Configure NVIDIA GPU device
    fn configure_nvidia_device(&self, device: &mut ClDeviceInfo, _pci: &crate::drivers::pci::PciDevice) {
        device.max_compute_units = 28;
        device.max_work_group_size = 1024;
        device.local_mem_size = 49152;
        device.image_support = true;
        device.image2d_max_width = 32768;
        device.image2d_max_height = 32768;
        device.double_fp_support = true;
        device.extensions.push(String::from("cl_nv_device_attribute_query"));
    }

    /// Get platform info
    pub fn get_platform_info(&self) -> &ClPlatformInfo {
        &self.platform
    }

    /// Get devices
    pub fn get_devices(&self, device_type: ClDeviceType) -> Vec<&ClDeviceInfo> {
        self.devices.iter()
            .filter(|d| device_type == ClDeviceType::All || d.device_type == device_type)
            .collect()
    }

    /// Get device by ID
    pub fn get_device(&self, id: u32) -> Option<&ClDeviceInfo> {
        self.devices.iter().find(|d| d.id == id)
    }

    /// Create context
    pub fn create_context(&mut self, device_ids: &[u32]) -> KResult<u32> {
        // Validate devices
        for &id in device_ids {
            if self.devices.iter().find(|d| d.id == id).is_none() {
                return Err(KError::NotFound);
            }
        }

        let context = ClContext {
            id: self.next_context_id,
            devices: device_ids.to_vec(),
            ref_count: AtomicU32::new(1),
            valid: AtomicBool::new(true),
        };

        self.next_context_id += 1;
        let id = context.id;
        self.contexts.push(context);

        self.stats.contexts_created.fetch_add(1, Ordering::Relaxed);
        Ok(id)
    }

    /// Release context
    pub fn release_context(&mut self, context_id: u32) -> KResult<()> {
        if let Some(ctx) = self.contexts.iter().find(|c| c.id == context_id) {
            if ctx.ref_count.fetch_sub(1, Ordering::SeqCst) == 1 {
                ctx.valid.store(false, Ordering::SeqCst);
            }
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Create command queue
    pub fn create_command_queue(&mut self, context_id: u32, device_id: u32, profiling: bool) -> KResult<u32> {
        // Validate context
        if !self.contexts.iter().any(|c| c.id == context_id && c.valid.load(Ordering::SeqCst)) {
            return Err(KError::Invalid);
        }

        let queue = ClCommandQueue {
            id: self.next_queue_id,
            context_id,
            device_id,
            out_of_order: false,
            profiling,
            commands_pending: AtomicU32::new(0),
            commands_completed: AtomicU64::new(0),
        };

        self.next_queue_id += 1;
        let id = queue.id;
        self.queues.push(queue);

        Ok(id)
    }

    /// Create buffer
    pub fn create_buffer(&mut self, context_id: u32, flags: ClMemFlags, size: u64) -> KResult<u32> {
        // Validate context
        if !self.contexts.iter().any(|c| c.id == context_id && c.valid.load(Ordering::SeqCst)) {
            return Err(KError::Invalid);
        }

        // Allocate device memory (placeholder - would use GPU memory allocator)
        let device_ptr = 0; // Would be actual GPU address

        let buffer = ClBuffer {
            id: self.next_buffer_id,
            context_id,
            size,
            flags,
            device_ptr,
            host_ptr: None,
            ref_count: AtomicU32::new(1),
        };

        self.next_buffer_id += 1;
        let id = buffer.id;
        self.buffers.push(buffer);

        self.stats.buffers_allocated.fetch_add(1, Ordering::Relaxed);
        self.stats.buffer_memory.fetch_add(size, Ordering::Relaxed);

        Ok(id)
    }

    /// Release buffer
    pub fn release_buffer(&mut self, buffer_id: u32) -> KResult<()> {
        let pos = self.buffers.iter().position(|b| b.id == buffer_id);
        if let Some(idx) = pos {
            let size = self.buffers[idx].size;
            self.buffers.remove(idx);
            self.stats.buffer_memory.fetch_sub(size, Ordering::Relaxed);
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    /// Write buffer
    pub fn enqueue_write_buffer(&mut self, queue_id: u32, buffer_id: u32, _data: &[u8]) -> KResult<u32> {
        let _queue = self.queues.iter().find(|q| q.id == queue_id)
            .ok_or(KError::NotFound)?;

        let buffer = self.buffers.iter().find(|b| b.id == buffer_id)
            .ok_or(KError::NotFound)?;

        // Create event
        let event = ClEvent {
            id: self.next_event_id,
            command_type: ClCommandType::WriteBuffer,
            status: ClExecStatus::Complete,
            queued_time: crate::time::uptime_ns(),
            submit_time: crate::time::uptime_ns(),
            start_time: crate::time::uptime_ns(),
            end_time: crate::time::uptime_ns(),
        };

        self.next_event_id += 1;
        let id = event.id;
        self.events.push(event);

        self.stats.buffer_transfers.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_transferred.fetch_add(buffer.size, Ordering::Relaxed);

        Ok(id)
    }

    /// Read buffer
    pub fn enqueue_read_buffer(&mut self, queue_id: u32, buffer_id: u32, _data: &mut [u8]) -> KResult<u32> {
        let _queue = self.queues.iter().find(|q| q.id == queue_id)
            .ok_or(KError::NotFound)?;

        let buffer = self.buffers.iter().find(|b| b.id == buffer_id)
            .ok_or(KError::NotFound)?;

        // Create event
        let event = ClEvent {
            id: self.next_event_id,
            command_type: ClCommandType::ReadBuffer,
            status: ClExecStatus::Complete,
            queued_time: crate::time::uptime_ns(),
            submit_time: crate::time::uptime_ns(),
            start_time: crate::time::uptime_ns(),
            end_time: crate::time::uptime_ns(),
        };

        self.next_event_id += 1;
        let id = event.id;
        self.events.push(event);

        self.stats.buffer_transfers.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_transferred.fetch_add(buffer.size, Ordering::Relaxed);

        Ok(id)
    }

    /// Create program from source
    pub fn create_program_with_source(&mut self, context_id: u32, source: &str) -> KResult<u32> {
        // Validate context
        if !self.contexts.iter().any(|c| c.id == context_id && c.valid.load(Ordering::SeqCst)) {
            return Err(KError::Invalid);
        }

        let program = ClProgram {
            id: self.next_program_id,
            context_id,
            source: String::from(source),
            binary: Vec::new(),
            build_status: ClBuildStatus::None,
            build_log: String::new(),
            kernel_names: Vec::new(),
        };

        self.next_program_id += 1;
        let id = program.id;
        self.programs.push(program);

        Ok(id)
    }

    /// Build program
    pub fn build_program(&mut self, program_id: u32, _options: &str) -> KResult<()> {
        let program = self.programs.iter_mut().find(|p| p.id == program_id)
            .ok_or(KError::NotFound)?;

        program.build_status = ClBuildStatus::InProgress;

        // Parse kernel names from source (simplified)
        let source = program.source.clone();
        for line in source.lines() {
            if line.contains("__kernel") || line.contains("kernel void") {
                // Extract kernel name (simplified parsing)
                if let Some(start) = line.find("void ") {
                    let rest = &line[start + 5..];
                    if let Some(end) = rest.find('(') {
                        let name = rest[..end].trim();
                        program.kernel_names.push(String::from(name));
                    }
                }
            }
        }

        program.build_status = ClBuildStatus::Success;
        program.build_log = String::from("Build successful");

        Ok(())
    }

    /// Create kernel
    pub fn create_kernel(&mut self, program_id: u32, kernel_name: &str) -> KResult<u32> {
        let program = self.programs.iter().find(|p| p.id == program_id)
            .ok_or(KError::NotFound)?;

        if program.build_status != ClBuildStatus::Success {
            return Err(KError::Invalid);
        }

        let kernel = ClKernel {
            id: self.next_kernel_id,
            program_id,
            name: String::from(kernel_name),
            num_args: 0,
            work_group_size: 256,
            local_mem_size: 0,
            preferred_work_group_size_multiple: 32,
            args: Vec::new(),
        };

        self.next_kernel_id += 1;
        let id = kernel.id;
        self.kernels.push(kernel);

        Ok(id)
    }

    /// Set kernel argument
    pub fn set_kernel_arg(&mut self, kernel_id: u32, arg_index: u32, value: ClArgValue) -> KResult<()> {
        let kernel = self.kernels.iter_mut().find(|k| k.id == kernel_id)
            .ok_or(KError::NotFound)?;

        let arg = ClKernelArg {
            index: arg_index,
            size: 8, // Placeholder
            value,
        };

        // Replace or add argument
        if let Some(existing) = kernel.args.iter_mut().find(|a| a.index == arg_index) {
            *existing = arg;
        } else {
            kernel.args.push(arg);
            kernel.num_args = kernel.args.len() as u32;
        }

        Ok(())
    }

    /// Enqueue kernel execution
    pub fn enqueue_ndrange_kernel(
        &mut self,
        queue_id: u32,
        kernel_id: u32,
        _work_dim: u32,
        _global_work_size: &[u64],
        _local_work_size: Option<&[u64]>,
    ) -> KResult<u32> {
        let _queue = self.queues.iter().find(|q| q.id == queue_id)
            .ok_or(KError::NotFound)?;

        let _kernel = self.kernels.iter().find(|k| k.id == kernel_id)
            .ok_or(KError::NotFound)?;

        let now = crate::time::uptime_ns();

        // Create event
        let event = ClEvent {
            id: self.next_event_id,
            command_type: ClCommandType::NdRangeKernel,
            status: ClExecStatus::Complete,
            queued_time: now,
            submit_time: now,
            start_time: now,
            end_time: now + 1000, // Placeholder execution time
        };

        self.next_event_id += 1;
        let id = event.id;
        self.events.push(event);

        self.stats.kernels_executed.fetch_add(1, Ordering::Relaxed);
        self.stats.kernel_time_ns.fetch_add(1000, Ordering::Relaxed);

        Ok(id)
    }

    /// Flush command queue
    pub fn flush(&mut self, queue_id: u32) -> KResult<()> {
        let _queue = self.queues.iter().find(|q| q.id == queue_id)
            .ok_or(KError::NotFound)?;
        Ok(())
    }

    /// Finish command queue (wait for completion)
    pub fn finish(&mut self, queue_id: u32) -> KResult<()> {
        let _queue = self.queues.iter().find(|q| q.id == queue_id)
            .ok_or(KError::NotFound)?;
        Ok(())
    }

    /// Wait for events
    pub fn wait_for_events(&self, _event_ids: &[u32]) -> KResult<()> {
        // All events are synchronous in this implementation
        Ok(())
    }

    /// Get event info
    pub fn get_event_info(&self, event_id: u32) -> Option<&ClEvent> {
        self.events.iter().find(|e| e.id == event_id)
    }

    /// Get statistics
    pub fn stats(&self) -> &ClStats {
        &self.stats
    }

    /// Get device count
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

/// Initialize OpenCL runtime
pub fn init() -> KResult<()> {
    OPENCL_MANAGER.lock().init()
}

/// Get device count
pub fn device_count() -> usize {
    OPENCL_MANAGER.lock().device_count()
}

/// Get platform info
pub fn platform_name() -> String {
    OPENCL_MANAGER.lock().get_platform_info().name.clone()
}

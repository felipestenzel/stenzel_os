//! Network drivers.
//!
//! Supports:
//! - VirtIO-net (QEMU, KVM)
//! - Intel E1000/E1000e (QEMU, VMware, real hardware)
//! - Intel WiFi (iwlwifi - various Intel wireless adapters)

pub mod virtio_net;
pub mod e1000;
pub mod iwlwifi;

use alloc::vec::Vec;
use crate::util::KResult;

/// Which driver is active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveDriver {
    None,
    VirtioNet,
    E1000,
}

static mut ACTIVE_DRIVER: ActiveDriver = ActiveDriver::None;

/// Initialize network drivers.
/// Tries VirtIO-net first (for QEMU), then E1000.
pub fn init() {
    // Try VirtIO-net first
    virtio_net::init();
    if virtio_net::get_mac().is_some() {
        unsafe { ACTIVE_DRIVER = ActiveDriver::VirtioNet; }
        return;
    }

    // Try E1000
    e1000::init();
    if e1000::get_mac().is_some() {
        unsafe { ACTIVE_DRIVER = ActiveDriver::E1000; }
        return;
    }

    // Try Intel WiFi
    iwlwifi::init();

    unsafe { ACTIVE_DRIVER = ActiveDriver::None; }
}

/// Get the MAC address of the active network interface.
pub fn get_mac() -> Option<[u8; 6]> {
    match unsafe { ACTIVE_DRIVER } {
        ActiveDriver::VirtioNet => virtio_net::get_mac(),
        ActiveDriver::E1000 => e1000::get_mac(),
        ActiveDriver::None => None,
    }
}

/// Send a packet.
pub fn send(data: &[u8]) -> KResult<()> {
    match unsafe { ACTIVE_DRIVER } {
        ActiveDriver::VirtioNet => virtio_net::send(data),
        ActiveDriver::E1000 => e1000::send(data),
        ActiveDriver::None => Err(crate::util::KError::NotSupported),
    }
}

/// Receive a packet.
pub fn recv() -> Option<Vec<u8>> {
    match unsafe { ACTIVE_DRIVER } {
        ActiveDriver::VirtioNet => virtio_net::recv(),
        ActiveDriver::E1000 => e1000::recv(),
        ActiveDriver::None => None,
    }
}

/// Check which driver is active.
pub fn driver_name() -> &'static str {
    match unsafe { ACTIVE_DRIVER } {
        ActiveDriver::VirtioNet => "virtio-net",
        ActiveDriver::E1000 => "e1000",
        ActiveDriver::None => "none",
    }
}

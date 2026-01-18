//! Network drivers.
//!
//! Supports:
//! - VirtIO-net (QEMU, KVM)
//! - Intel E1000/E1000e (QEMU, VMware, real hardware)
//! - Intel I210/I211/I217/I218/I219 (Gigabit Ethernet, real hardware)
//! - Realtek RTL8139 (10/100 Mbps, QEMU, real hardware)
//! - Realtek RTL8169/8168/8111 (Gigabit Ethernet, real hardware)
//! - Intel WiFi (iwlwifi - various Intel wireless adapters)
//! - Intel WiFi 7 (iwlwifi_be - BE200/BE202 802.11be adapters)
//! - Atheros WiFi (ath9k - AR9xxx/QCA series wireless adapters)
//! - Broadcom WiFi (brcmfmac - BCM43xx series wireless adapters)

pub mod virtio_net;
pub mod e1000;
pub mod igb;
pub mod rtl8139;
pub mod rtl8169;
pub mod iwlwifi;
pub mod iwlwifi_be;
pub mod ath9k;
pub mod brcm;
pub mod rtl8xxxu;
pub mod rtl8xxxu_wifi;
pub mod mt7921;
pub mod brcmfmac;
pub mod ath11k;

use alloc::vec::Vec;
use crate::util::KResult;

/// Which driver is active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveDriver {
    None,
    VirtioNet,
    E1000,
    Igb,
    Rtl8139,
    Rtl8169,
}

static mut ACTIVE_DRIVER: ActiveDriver = ActiveDriver::None;

/// Initialize network drivers.
/// Tries VirtIO-net first (for QEMU), then Intel, then Realtek.
pub fn init() {
    // Try VirtIO-net first (QEMU)
    virtio_net::init();
    if virtio_net::get_mac().is_some() {
        unsafe { ACTIVE_DRIVER = ActiveDriver::VirtioNet; }
        return;
    }

    // Try E1000 (QEMU, older Intel)
    e1000::init();
    if e1000::get_mac().is_some() {
        unsafe { ACTIVE_DRIVER = ActiveDriver::E1000; }
        return;
    }

    // Try IGB (Intel I210/I211/I219)
    igb::init();
    if igb::get_mac().is_some() {
        unsafe { ACTIVE_DRIVER = ActiveDriver::Igb; }
        return;
    }

    // Try RTL8139
    rtl8139::init();
    if rtl8139::get_mac().is_some() {
        unsafe { ACTIVE_DRIVER = ActiveDriver::Rtl8139; }
        return;
    }

    // Try RTL8169 (Gigabit)
    rtl8169::init();
    if rtl8169::get_mac().is_some() {
        unsafe { ACTIVE_DRIVER = ActiveDriver::Rtl8169; }
        return;
    }

    // Try Intel WiFi (AX200/AX201/AX210/AX211)
    iwlwifi::init();

    // Try Intel WiFi 7 (BE200/BE202)
    iwlwifi_be::init();

    // Try Realtek WiFi (USB adapters)
    rtl8xxxu::init();

    // Try Realtek WiFi (PCIe adapters)
    rtl8xxxu_wifi::init();

    // Try MediaTek WiFi (MT7921/MT7922/MT7925)
    mt7921::init();

    // Try Broadcom WiFi (BCM43xx)
    brcmfmac::init();

    // Try Atheros WiFi 6 (ath11k - QCA6390/WCN6855)
    ath11k::init();

    unsafe { ACTIVE_DRIVER = ActiveDriver::None; }
}

/// Get the MAC address of the active network interface.
pub fn get_mac() -> Option<[u8; 6]> {
    match unsafe { ACTIVE_DRIVER } {
        ActiveDriver::VirtioNet => virtio_net::get_mac(),
        ActiveDriver::E1000 => e1000::get_mac(),
        ActiveDriver::Igb => igb::get_mac(),
        ActiveDriver::Rtl8139 => rtl8139::get_mac(),
        ActiveDriver::Rtl8169 => rtl8169::get_mac(),
        ActiveDriver::None => None,
    }
}

/// Send a packet.
pub fn send(data: &[u8]) -> KResult<()> {
    match unsafe { ACTIVE_DRIVER } {
        ActiveDriver::VirtioNet => virtio_net::send(data),
        ActiveDriver::E1000 => e1000::send(data),
        ActiveDriver::Igb => igb::send(data),
        ActiveDriver::Rtl8139 => rtl8139::send(data),
        ActiveDriver::Rtl8169 => rtl8169::send(data),
        ActiveDriver::None => Err(crate::util::KError::NotSupported),
    }
}

/// Receive a packet.
pub fn recv() -> Option<Vec<u8>> {
    match unsafe { ACTIVE_DRIVER } {
        ActiveDriver::VirtioNet => virtio_net::recv(),
        ActiveDriver::E1000 => e1000::recv(),
        ActiveDriver::Igb => igb::recv(),
        ActiveDriver::Rtl8139 => rtl8139::recv(),
        ActiveDriver::Rtl8169 => rtl8169::recv(),
        ActiveDriver::None => None,
    }
}

/// Check which driver is active.
pub fn driver_name() -> &'static str {
    match unsafe { ACTIVE_DRIVER } {
        ActiveDriver::VirtioNet => "virtio-net",
        ActiveDriver::E1000 => "e1000",
        ActiveDriver::Igb => "igb",
        ActiveDriver::Rtl8139 => "rtl8139",
        ActiveDriver::Rtl8169 => "rtl8169",
        ActiveDriver::None => "none",
    }
}

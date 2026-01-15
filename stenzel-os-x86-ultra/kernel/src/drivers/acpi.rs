#![allow(dead_code)]

/// ACPI é a base para enumerar hardware moderno (APIC/IOAPIC, power mgmt, etc.).
///
/// Implementação completa exige:
/// - localizar RSDP (via UEFI ou scanning EBDA)
/// - parse RSDT/XSDT
/// - ler MADT (APIC), HPET, etc.
///
/// Aqui fica como placeholder de arquitetura.
pub fn init() {
    // TODO
}

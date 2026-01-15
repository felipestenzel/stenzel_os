# Arquitetura (x86_64)

## Camadas
- Firmware (UEFI/BIOS)
- Bootloader (carrega kernel, fornece mapa de memória e offset de memória física)
- Kernel (ring0)
  - arch/x86_64: GDT/IDT/TSS, interrupções, timer, syscalls (futuro)
  - mm: paging, alocador físico (bitmap), heap, mapeamento MMIO
  - task: threads + scheduler (base para preempção)
  - fs: VFS + tmpfs + permissões
  - security: usuários/credenciais
  - drivers: PCI, storage (AHCI/NVMe/virtio), etc.

## Layout de endereçamento virtual (proposto)
- 0x0000_0000_0000_0000 .. 0x0000_7FFF_FFFF_FFFF : user space (futuro)
- 0xFFFF_8000_0000_0000 .. 0xFFFF_FFFF_FFFF_FFFF : kernel space
  - mapa físico (physmem window) via `physical_memory_offset` do bootloader
  - heap do kernel em uma faixa fixa
  - MMIO em uma faixa fixa (mapeada sob demanda)
  - stacks por thread em faixas fixas ou alocadas

## Política de concorrência
- Locks do tipo spinlock (com IRQ-off em seções críticas pequenas)
- Estruturas com RwLock em dados compartilhados (VFS)
- Caminho para per-CPU no futuro

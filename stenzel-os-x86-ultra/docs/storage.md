# Armazenamento

## Objetivo
- Camada de bloco: `BlockDevice` (read/write por LBA)
- Buffer cache (LRU)
- Particionamento: GPT
- Drivers:
  - AHCI (SATA) via PCI + MMIO
  - NVMe via PCI + MMIO + queues
  - virtio-blk para virtualização (QEMU)

## Estratégia
- Inicialmente: ramdisk + GPT parser para validar pipeline.
- Depois: virtio-blk no QEMU (mais rápido de validar).
- Depois: AHCI real.
- NVMe por último (complexo mas performance alta).

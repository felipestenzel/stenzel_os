# Stenzel OS (x86_64 ultra – virtio-blk + preempção real + ring3)

Este repositório é um **esqueleto avançado** (mas ainda em evolução) do Stenzel OS para PCs x86_64
(máquinas comuns que hoje rodam Windows/Linux).

## O que já está implementado (neste zip)
- **virtio-blk (PCI legacy I/O)** funcional no QEMU:
  - virtqueue 0, 1 request síncrono por vez
  - leitura de capacidade e leitura/escrita de LBAs
  - leitura de GPT (header + partições) para validar o caminho de I/O
- **Preempção real**:
  - IRQ0 (timer) com stub em assembly cria `TrapFrame` e chama o scheduler
  - o scheduler pode devolver uma *outra* TrapFrame, e o stub retorna via `iretq`
- **Ring3 mínimo**:
  - criação de address space por processo (novo CR3)
  - `iretq` para entrar em ring3
  - syscalls via instrução `syscall` e retorno via `sysretq` (IA32_LSTAR/STAR/FM...)
  - dois programas user-mode embutidos imprimindo no serial para demonstrar alternância

## Onde estão os códigos em assembly
- IRQ stubs (TrapFrame + `iretq`): `kernel/src/arch/x86_64_arch/interrupts.rs` (`global_asm!`)
- Entrada de syscall (swapgs + troca de stack + `sysretq`): `kernel/src/arch/x86_64_arch/syscall.rs`
- Troca direta para uma TrapFrame (usado em `exit`): `kernel/src/arch/x86_64_arch/switch.rs`
- User programs em ring3 (blobs): `kernel/src/sched/userprog.rs`

## Requisitos
- Rust nightly + `rust-src` + `llvm-tools-preview`
- QEMU (`qemu-system-x86_64`)

## Rodar no QEMU
```bash
cargo run -p stenzel
```

### Flags úteis
- `cargo run -p stenzel -- --uefi` (requer OVMF instalado; veja `os/src/main.rs`)

## Notas importantes
- O kernel ainda está em estágio "bring-up". Os componentes principais foram estruturados para
  evoluir (SMP, ELF loader, VFS em cima do virtio, etc.).
- Se você quiser que eu avance para:
  - virtio 1.0 (modern, MMIO/capabilities),
  - ELF loader + initramfs,
  - userland com libc minimal,
  - scheduler mais avançado (prioridades/MLFQ, sleepers),
  eu já deixei os ganchos mais importantes no código.

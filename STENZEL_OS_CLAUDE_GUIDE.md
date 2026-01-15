# Stenzel OS — Guia Mestre (para Claude)  
**Alvo atual:** PC **x86_64** (máquinas que hoje rodam Windows/Linux)  
**Stack:** Rust `no_std` + assembly x86_64 (apenas onde necessário)  
**Foco do produto:** simplicidade + **controle por voz** (voz vira texto → *intent* → ação), mantendo núcleo pequeno e eficiente.

> Este documento consolida **100% do que foi produzido nesta conversa**: visão, roadmap, arquitetura, organização do projeto, etapas, e todos os trechos de código já apresentados (incluindo os stubs/asm relevantes).  
> Use este markdown como “brief” e “checklist” para continuar o desenvolvimento com o Claude.

---

## 0) Como usar este guia com o Claude (passo a passo)

1) **Envie para o Claude** este arquivo markdown **e os zips** gerados:
   - `stenzel-os-inicial.zip`
   - `stenzel-os-x86-inicial.zip`
   - `stenzel-os-x86-advanced.zip`
   - `stenzel-os-x86-ultra.zip`

2) Peça explicitamente para o Claude:
   - tratar **`stenzel-os-x86-ultra.zip` como baseline principal** (é o mais completo),
   - usar os zips anteriores como referência (principalmente para a parte de **voz/intents** e para histórico das estruturas),
   - **verificar consistência e compilar** (corrigir inconsistências de tipos/módulos, se existirem),
   - manter o kernel em Rust `no_std` e só usar assembly onde for essencial (interrupts/syscalls/context switch).

3) Resultado esperado ao rodar no QEMU (baseline ultra):
   - log de boot no serial
   - detecção de PCI e virtio-blk
   - tentativa de leitura GPT (e listagem de partições se presente)
   - alternância entre **dois “user programs” em ring3**, imprimindo no serial via syscall

---

## 1) Visão do Stenzel OS (o “porquê”)

### 1.1 Objetivo do produto
- **Sistema operacional simples**, mas **muito eficiente**, com foco em:
  - fluxo “voz-first” para ações comuns (“abrir pasta X”, “executar Y”, etc.)
  - fallback por texto/teclado
  - segurança e permissões (usuários, owners, modos)
  - organização de arquivos/pastas consistente

### 1.2 Multi-hardware (visão de longo prazo)
Rodar em “qualquer hardware” significa, na prática:
- suportar múltiplas arquiteturas **ao longo do tempo** (x86_64, aarch64, riscv64…),
- implementar boot chain e drivers específicos por plataforma.

**Neste guia o foco é x86_64** (PC), pois é onde o bring-up e os drivers são mais acessíveis e testáveis (QEMU).

---

## 2) Pipeline de controle por voz (design — para integrar depois)

A regra do Stenzel OS: **voz → texto → Intent → ação**.

1) `audiod`: captura áudio (driver/serviço)  
2) Wake word (opcional)  
3) ASR offline: áudio → texto  
4) NLU simples: texto → `Intent` (gramática + sinônimos)  
5) Router de intents: `Intent` → IPC para serviço alvo (FS/appmgr/ui)  
6) Feedback: TTS e/ou visual

> Importante: **NÃO** colocar reconhecimento de voz dentro do kernel.  
> Kernel deve expor syscalls/IPC; a “voz” deve viver em userland (serviço `voiced`).

---

## 3) Roadmap geral (etapas — do bring-up ao “OS de verdade”)

### Fase 0 — Bring-up (QEMU)
- Boot x86_64
- log serial
- parse de intents (texto) para validar comandos “voz-first” mesmo sem áudio

### Fase 1 — Kernel básico
- interrupções + timer
- memória (paging + heap)
- threads/scheduler (primeiro cooperativo, depois preemptivo)
- syscalls mínimas

### Fase 2 — Processos e FS
- ring3 + syscalls estáveis
- VFS + FS persistente (virtio-blk → FAT32/ext2/ext4)
- mount points, `/dev`, `/proc`, `/sys` como pseudo-fs

### Fase 3 — Áudio e voz
- driver áudio + serviço
- ASR offline + intents
- permissões para ações disparadas por voz

### Fase 4 — UI minimalista
- UI simples, voz-first, com fallback touch/teclado

### Fase 5 — Portabilidade
- aarch64 (QEMU virt / Raspberry Pi)
- riscv64
- “celular/tablet”: escolher alvos com boot menos fechado

---

## 4) Conjunto de repositórios/ZIPs gerados (o que cada um contém)

### 4.1 `stenzel-os-inicial.zip` (protótipo mínimo)
**Objetivo:** boot + serial + parser de intents (base para voz).  
**Tree:**
```
stenzel-os-inicial/
  Cargo.toml
  rust-toolchain.toml
  .cargo/config.toml
  kernel/
    src/main.rs
    src/serial.rs
    src/commands.rs
  os/
    src/main.rs
    build.rs
```

### 4.2 `stenzel-os-x86-inicial.zip` (x86 skeleton com MM/VFS/Security/Storage stubs)
**Objetivo:** estrutura de OS x86 (memória/heap, ramfs, usuários, storage stubs).  
**Tree (resumo):**
```
kernel/src/
  arch/x86_64_arch/{gdt.rs, interrupts.rs}
  mm/{mod.rs, paging.rs, frame_allocator.rs, heap.rs}
  vfs/{mod.rs, ramfs.rs}
  security/mod.rs
  storage/{block.rs, gpt.rs, ahci.rs, nvme.rs}
  process/mod.rs
  syscall/mod.rs
os/{build.rs, src/main.rs}
docs/{architecture-x86.md, memory.md, storage.md}
```

### 4.3 `stenzel-os-x86-advanced.zip` (mais subsistemas + shell e base de drivers)
**Objetivo:** kernel mais “OS”, com scheduler/threads base, tmpfs/vfs, storage cache, PCI/virtio stubs.  
(Serviu como ponte para o “ultra”.)

### 4.4 `stenzel-os-x86-ultra.zip` (baseline principal)
**Objetivo:** **virtio-blk funcional + preempção real + ring3 + syscalls** com assembly necessário.  
**Tree (resumo):**
```
kernel/src/
  arch/x86_64_arch/
    gdt.rs
    interrupts.rs   (stubs asm + TrapFrame + dispatcher)
    syscall.rs      (syscall entry asm + MSRs + sysretq)
    switch.rs       (iretq via TrapFrame - asm)
    pic.rs, pit.rs
  mm/               (paging + heap + bitmap frame allocator)
  sched/            (preemptivo + ring3 tasks + address spaces)
  drivers/pci.rs    (scan PCI config 0xCF8/0xCFC)
  drivers/storage/virtio_blk.rs (driver real virtio legacy I/O)
  storage/          (BlockDevice + GPT + cache + ramdisk fallback)
  fs/               (VFS + tmpfs + permissões)
  security/         (UserDb + Cred)
os/
  build.rs          (gera BIOS/UEFI image + cria disco virtio com GPT mínimo)
  src/main.rs       (roda QEMU e conecta virtio-blk legacy)
```

---

## 5) Arquitetura (x86_64) — visão em camadas

```
UEFI/BIOS
  ↓
bootloader (carrega kernel + fornece BootInfo)
  ↓
Kernel (ring0)
  ├─ arch/x86_64: GDT/TSS, IDT, PIC/PIT, syscall MSRs, stubs ASM
  ├─ mm: paging + heap + frame alloc (bitmap)
  ├─ sched: preemptivo (IRQ0) + tasks ring3
  ├─ fs: VFS + tmpfs + permissões (UID/GID + mode)
  ├─ storage: BlockDevice + GPT + cache + virtio-blk
  └─ (futuro) processos/ELF + userland de serviços
```

**Nota de design (eficiência):**
- Kernel tende a “monólito modular” no começo (mais fácil bring-up).
- Voz/áudio/UI preferencialmente em **userland** para reduzir TCB.

---

## 6) Build e execução (baseline ultra)

### 6.1 Requisitos
- Rust nightly + componentes:
  - `rust-src`
  - `llvm-tools-preview`
- Target: `x86_64-unknown-none`
- QEMU: `qemu-system-x86_64`

O repo já inclui:
- `rust-toolchain.toml` (nightly + target + components)
- `.cargo/config.toml` habilitando `bindeps=true` e flags de kernel

### 6.2 Rodar BIOS (padrão)
```bash
cargo run -p stenzel
```

### 6.3 Rodar UEFI (opcional)
```bash
cargo run -p stenzel -- --uefi
```
Para UEFI, ajuste `OVMF_CODE` conforme sua distro (veja `os/src/main.rs`).

### 6.4 Como o QEMU é chamado (ultra)
O runner conecta o disco virtio e força modo legacy para compatibilidade com o driver:
- `-device virtio-blk-pci,drive=vdisk,disable-modern=on`
- `-serial stdio -display none`

---

## 7) Componentes principais (ultra) — explicação + onde mexer

## 7.1 Boot & init do kernel
Arquivo: `kernel/src/main.rs`

- `BOOTLOADER_CONFIG`:
  - `physical_memory = Some(Mapping::Dynamic)` → habilita `physical_memory_offset`
  - `kernel_stack_size = 256 * 1024`

- Ordem do boot:
  1) `arch::init()` (GDT/IDT/PIC/PIT/SYSCALL)
  2) `mm::init(boot_info)`
  3) `security::init()`
  4) `fs::init()` + `fs::bootstrap_filesystem()`
  5) `storage::init()` (PCI scan + virtio-blk)
  6) `sched::init_and_launch_userspace()` (cria tasks ring3)
  7) enable interrupts e entra em `hlt` (idle)

---

## 7.2 Memória (paging + heap + frame allocator bitmap)
Arquivos:
- `kernel/src/mm/mod.rs`
- `kernel/src/mm/paging.rs`
- `kernel/src/mm/heap.rs`
- `kernel/src/mm/phys.rs`

### Estratégia
- Bootstrap:
  - `BootInfoFrameAllocator` (linear) usa `boot_info.memory_regions`.
  - Mapeia heap do kernel e inicializa `linked_list_allocator`.
- Definitivo:
  - `BitmapFrameAllocator` (rápido e previsível) construído a partir do mapa de memória.

### APIs importantes (mm/mod.rs)
- `phys_to_virt(pa) -> VirtAddr` (via `physical_memory_offset`)
- `virt_to_phys(va) -> Option<PhysAddr>` (via translate do mapper)
- `alloc_contiguous_pages(pages)` → crucial para virtqueue do virtio

---

## 7.3 Interrupções com TrapFrame + preempção real (assembly)
Arquivo: `kernel/src/arch/x86_64_arch/interrupts.rs`

### Conceito
- IRQ0 (timer) e IRQ1 (kbd) usam **stubs ASM** que:
  - empilham regs em layout `TrapFrame`
  - chamam `stenzel_interrupt_dispatch(tf)`
  - **recebem de volta um ponteiro de TrapFrame** (pode ser de outra task!)
  - restauram regs daquele frame e dão `iretq`

### Código (trecho consolidado do ultra)
```rust
#[repr(C)]
pub struct TrapFrame {
    pub rax: u64, pub rbx: u64, pub rcx: u64, pub rdx: u64,
    pub rsi: u64, pub rdi: u64, pub rbp: u64,
    pub r8: u64, pub r9: u64, pub r10: u64, pub r11: u64,
    pub r12: u64, pub r13: u64, pub r14: u64, pub r15: u64,
    pub vector: u64, pub error: u64,
    pub rip: u64, pub cs: u64, pub rflags: u64, pub rsp: u64, pub ss: u64,
}
```

```asm
// IRQ0 timer
stenzel_isr32:
    pushq $0
    pushq $32
    jmp stenzel_isr_common

stenzel_isr_common:
    push %r15
    ...
    push %rax

    mov %rsp, %rdi
    mov %rsp, %rax
    and $-16, %rsp
    call stenzel_interrupt_dispatch

    mov %rax, %rsp

    pop %rax
    ...
    pop %r15
    add $16, %rsp
    iretq
```

Dispatcher:
- `IRQ_TIMER` → `crate::sched::on_timer_tick(tf)` retorna próximo frame.
- envia EOI para PIC.

---

## 7.4 Scheduler preemptivo + tasks ring3
Arquivo: `kernel/src/sched/mod.rs`

### Design
- Round-robin com `quantum` em ticks.
- `on_timer_tick(tf)`:
  - salva `tf` no task atual,
  - decrementa quantum,
  - decide troca,
  - escolhe próximo `Ready`,
  - troca CR3 (address space), atualiza stacks de kernel (TSS + syscall),
  - retorna `TrapFrame*` do próximo.

### Entrada em ring3
- Cria `TrapFrame` inicial na stack do kernel com:
  - `cs`/`ss` user (DPL=3)
  - `rip` = `USER_BASE`
  - `rsp` = topo do user stack
  - `rflags` com IF=1
- A troca acontece por `iretq` do stub (timer ou switch_to).

### Address space por processo (CR3 próprio)
- Aloca novo P4 frame
- Copia metade alta do P4 ativo (entradas 256..511) para mapear kernel no high-half
- Mapeia:
  - user code em `USER_BASE = 0x0040_0000`
  - user stack no range terminando em `USER_STACK_TOP = 0x0080_0000`

---

## 7.5 Syscalls reais via SYSCALL/SYSRET (assembly)
Arquivo: `kernel/src/arch/x86_64_arch/syscall.rs`

### Conceito
- MSRs:
  - IA32_EFER.SCE habilita SYSCALL
  - IA32_STAR define seletores CS/SS (kernel e user)
  - IA32_LSTAR aponta para `stenzel_syscall_entry`
  - IA32_FMASK limpa IF no entry
  - IA32_KERNEL_GS_BASE aponta para `CpuLocal` (usado pelo `swapgs`)
- Entrada em syscall:
  - `swapgs`
  - salva `user rsp` em `CpuLocal.user_rsp_tmp`
  - muda `rsp` para `CpuLocal.kernel_stack_top`
  - empilha regs em layout `SyscallFrame`
  - chama `syscall_dispatch`
  - restaura regs e volta com `sysretq`

### Syscalls implementadas (mínimo funcional)
- `write(fd, buf, len)` → escreve no serial
- `exit(status)` → mata task e troca imediatamente para outro (não retorna)

Trecho asm consolidado (ultra):
```asm
stenzel_syscall_entry:
    swapgs
    mov %rsp, 8(%gs)
    mov 0(%gs), %rsp

    pushq $0
    push %rax
    push %rbx
    push %rcx
    ...
    push %r15

    mov %rsp, %rdi
    call syscall_dispatch

    pop %r15
    ...
    pop %rbx

    add $16, %rsp

    mov 8(%gs), %rdx
    swapgs
    mov %rdx, %rsp
    sysretq
```

---

## 7.6 Switch direto para TrapFrame (assembly — usado por exit)
Arquivo: `kernel/src/arch/x86_64_arch/switch.rs`

```asm
stenzel_switch_to:
    mov %rdi, %rsp
    pop %rax
    pop %rbx
    ...
    pop %r15
    add $16, %rsp
    iretq
```

---

## 7.7 Storage (BlockDevice + GPT + virtio-blk real)
Arquivos:
- `kernel/src/storage/*`
- `kernel/src/drivers/pci.rs`
- `kernel/src/drivers/storage/virtio_blk.rs`
- `os/build.rs` (cria disco virtio com GPT mínimo)
- `os/src/main.rs` (QEMU args)

### Estratégia
- `storage::init()`:
  - faz PCI scan
  - tenta probe/init virtio-blk
  - fallback para RamDisk
  - tenta ler GPT e imprime partições

### virtio-blk (legacy I/O)
- detecta vendor `0x1AF4`
- negocia features minimalistas (aceita 0)
- configura virtqueue 0 com memória contígua (`alloc_contiguous_pages`)
- implementa request síncrono: header + data segs + status

> Runner força `disable-modern=on` para ficar no legacy I/O.

---

## 7.8 VFS + tmpfs + permissões
Arquivos:
- `kernel/src/fs/vfs.rs`
- `kernel/src/fs/tmpfs.rs`
- `kernel/src/fs/perm.rs`
- `kernel/src/fs/mod.rs` (bootstrap)

### Layout inicial de diretórios (bootstrap)
Cria:
- `/bin`, `/sbin`
- `/etc`, `/etc/stenzel`
- `/home`, `/home/user`, `/root`
- `/var/log`, `/tmp`, `/dev`, `/proc`, `/sys`

Escreve:
- `/etc/passwd` (gerado pelo UserDb)
- `/etc/stenzel/system.conf`
- `/var/log/boot.log`

---

## 7.9 Usuários (UserDb) e credenciais
Arquivo: `kernel/src/security/mod.rs`

- Users: `root` (uid 0) e `user` (uid 1000)
- `Cred` (uid/gid/grupos)
- `passwd_text()` exporta /etc/passwd-like

---

## 8) Parte “voz-first” já criada (protótipo de intents)

Esta parte vem do `stenzel-os-inicial.zip` e deve ser reutilizada futuramente
no userland (`voiced`) — mas o parser já existe e é útil para CLI/serial.

Arquivo: `kernel/src/commands.rs` (do zip inicial)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent<'a> {
    OpenFolder(&'a str),
    Run(&'a str),
    Help,
    Unknown,
}

pub fn parse_command(input: &str) -> Intent<'_> {
    let s = input.trim();

    if s.eq_ignore_ascii_case("ajuda") || s.eq_ignore_ascii_case("help") {
        return Intent::Help;
    }

    if let Some(rest) = strip_prefix_ignore_ascii_case(s, "abrir pasta ") {
        let arg = rest.trim();
        if !arg.is_empty() { return Intent::OpenFolder(arg); }
    }

    if let Some(rest) = strip_prefix_ignore_ascii_case(s, "abrir ") {
        let arg = rest.trim();
        if !arg.is_empty() { return Intent::OpenFolder(arg); }
    }

    if let Some(rest) = strip_prefix_ignore_ascii_case(s, "executar ") {
        let arg = rest.trim();
        if !arg.is_empty() { return Intent::Run(arg); }
    }

    if let Some(rest) = strip_prefix_ignore_ascii_case(s, "rodar ") {
        let arg = rest.trim();
        if !arg.is_empty() { return Intent::Run(arg); }
    }

    Intent::Unknown
}
```

---

## 9) Pontos de atenção para o Claude (consistência/compilação)

### 9.1 “Baseline ultra” pode precisar de ajustes finos
Este guia descreve a intenção arquitetural e o código do zip ultra.
Peça para o Claude:

- **compilar** e corrigir erros de tipos/módulos se existirem;
- manter o design “eficiente e complexo” (não regredir para simplificações).

### 9.2 Recomendação de hardening imediato (sem simplificar)
1) **Capacidades (Caps) e credenciais**
   - Nas mensagens anteriores foi sugerido modelo de permissões/capabilities.
   - Se algum módulo do scheduler referir `caps`, implemente `Caps` em `security` via `bitflags`:
     - `CAP_FS`, `CAP_PROC`, `CAP_NET`, `CAP_ADMIN`, etc.
   - Faça `Cred` carregar `caps` e ajuste checks em FS/syscalls.

2) **`arch/mod.rs`**
   - Se existir reexport incorreto (ex.: `restore`) ajuste para refletir funções reais (`disable/enable` ou `are_enabled`).

3) **Syscall safety**
   - Validar ponteiros user (`buf_ptr`) antes de ler/escrever:
     - checar canonical address
     - checar se está em range user
     - checar páginas presentes e user-accessible (via page table walk)

4) **Preempção**
   - Confirmar alinhamento de stack e layout exato de `TrapFrame`.
   - Confirmar que o PIC EOI está correto antes do scheduling.

---

## 10) Próximos upgrades “nível OS excelente” (planejamento)

### 10.1 Interrupções e tempo modernos
- Migrar de PIC/PIT para APIC/IOAPIC (via ACPI MADT)
- Timer via APIC timer ou HPET
- Ganho: performance, SMP, latência melhor

### 10.2 virtio modern + MSI-X
- Implementar virtio 1.0 (capabilities PCI)
- MSI-X para interrupções por queue
- Melhor throughput e menor overhead

### 10.3 Loader ELF + initramfs
- Trocar blobs ring3 por ELF64
- initramfs para trazer `/bin/init`, configs e assets

### 10.4 FS persistente
- montar partição GPT do virtio-disk
- começar com FAT32 (boot/ESP) ou ext2 (simples)
- evoluir para ext4 + page cache integrado

### 10.5 Userland “de verdade”
Serviços:
- `init`
- `fsd` (filesystem service)
- `appmgr`
- `voiced` (ASR + intents)
- `audiod`
- UI minimal

Apps:
- considerar WASM como formato portável (futuro)

---

## 11) Apêndice: comandos e outputs esperados (sanidade)

### 11.1 Boot log (serial)
Você deve ver algo como:
- banner do kernel
- init mm (stats de frames)
- vfs bootstrap concluído
- PCI devices encontrados
- virtio-blk inicializado
- GPT partições (se parser reconhecer)
- “userspace pronto (2 tasks)”
- e mensagens alternadas dos programas ring3 (via syscall write)

### 11.2 Se não aparecer alternância dos user tasks
Checklist para o Claude:
- IDT carregada?
- PIT programado e IRQs habilitadas?
- PIC remapeado e IRQ0 desmascarado?
- `on_timer_tick` retornando TrapFrame correta?
- CR3 troca corretamente para user task?
- `syscall::init()` programou MSRs corretamente?
- Seletores user na GDT/TSS ok?

---

## 12) Referência rápida: onde está cada coisa (ultra)

- Boot + init: `kernel/src/main.rs`
- Serial/log: `kernel/src/serial.rs`, `kernel/src/util.rs`
- GDT/TSS: `kernel/src/arch/x86_64_arch/gdt.rs`
- PIC/PIT: `kernel/src/arch/x86_64_arch/pic.rs`, `pit.rs`
- IDT + stubs asm + TrapFrame: `kernel/src/arch/x86_64_arch/interrupts.rs`
- Syscalls (`syscall/sysretq`) + asm: `kernel/src/arch/x86_64_arch/syscall.rs`
- Switch TrapFrame → `iretq`: `kernel/src/arch/x86_64_arch/switch.rs`
- Memória: `kernel/src/mm/*`
- Scheduler + ring3 tasks: `kernel/src/sched/*`
- PCI scan: `kernel/src/drivers/pci.rs`
- virtio-blk: `kernel/src/drivers/storage/virtio_blk.rs`
- GPT: `kernel/src/storage/gpt.rs`
- VFS/tmpfs: `kernel/src/fs/*`
- Usuários: `kernel/src/security/mod.rs`
- Runner + imagem + disco GPT: `os/build.rs`, `os/src/main.rs`

---

## 13) Histórico de decisões (do começo da conversa)
- Linguagem preferida para o kernel: **Rust** (`no_std`) + assembly apenas para:
  - stubs de IRQ + retorno `iretq`
  - entry syscall `syscall/sysretq`
  - troca direta para TrapFrame (`switch_to`)
- “Voz-first” é parte do produto, mas **não entra no kernel**.
- Primeiro alvo realista: **PC x86_64** (QEMU + virtio).
- Depois: portabilidade para outras arquiteturas.

---

**Fim do guia.**

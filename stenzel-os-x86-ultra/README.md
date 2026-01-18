# Stenzel OS

Um sistema operacional moderno escrito do zero em Rust para arquitetura x86_64.

## Visão Geral

Stenzel OS é um projeto ambicioso de criar um sistema operacional completo, capaz de rodar em hardware real com interface gráfica, rede, WiFi e gerenciamento de pacotes.

## Status do Projeto

| Componente | Progresso |
|------------|-----------|
| Boot (BIOS/UEFI) | 70% |
| Kernel Core | 80% |
| Memory Management | 90% |
| Scheduler | 95% |
| Syscalls | 80% |
| Filesystem | 75% |
| Drivers | 30% |
| Rede | 70% |
| GUI | 65% |
| Package Manager | 85% |

## Funcionalidades Implementadas

### Kernel
- Boot via Legacy BIOS e UEFI com suporte Multiboot2
- Gerenciamento de memória completo (paging 4-level, heap, CoW, mmap, swap, huge pages, NUMA)
- Scheduler preemptivo com CFS, real-time scheduling e load balancing SMP
- Suporte completo a processos (fork, execve, threads, signals)
- IPC (pipes, Unix sockets, shared memory, semaphores, message queues)
- ACPI parser completo

### Sistemas de Arquivos
- VFS com path resolution, mount system e caches
- **ext2/ext4** - leitura e escrita
- **FAT32/exFAT** - para dispositivos removíveis
- **NTFS** - leitura de partições Windows
- **ISO9660** - para CD/DVD
- **tmpfs, procfs, sysfs, devfs**

### Drivers de Hardware
- **Storage**: AHCI (SATA), NVMe, VirtIO-blk, USB Mass Storage, SD/MMC, IDE
- **Input**: Teclado/Mouse PS/2 e USB, Touchpad, múltiplos layouts de teclado
- **Display**: Framebuffer, VBE/VESA, GOP (UEFI), drivers Intel/AMD/NVIDIA básicos
- **USB**: xHCI (3.x), EHCI (2.0), OHCI/UHCI (1.x), HID, Audio, Video
- **Audio**: Intel HDA, AC'97, mixer
- **Outros**: HPET, MSI/MSI-X, Thermal, Battery, Backlight, Bluetooth

### Rede
- Stack TCP/IP completo (IPv4, IPv6, ICMP, UDP, TCP)
- DHCP, DNS, NTP
- HTTP/HTTPS com TLS
- SSH e FTP
- **Drivers Ethernet**: VirtIO-net, E1000, RTL8139, RTL8169, Intel I210
- **WiFi**: 802.11 com WPA/WPA2/WPA3, drivers Intel, Atheros, Broadcom, Realtek

### Interface Gráfica
- Compositor e Window Manager
- Suporte a transparência, animações e multi-monitor
- Widgets completos (buttons, textbox, menus, dialogs, file picker)
- Desktop com taskbar, start menu, system tray, notificações
- **Aplicativos incluídos**: Terminal, File Manager, Text Editor, Image Viewer, Calculator, Settings, Task Manager, Web Browser

### Gerenciamento de Pacotes
- Formato de pacotes próprio com compressão e assinatura
- Repositório de pacotes com metadados
- Resolução de dependências
- Instalação, atualização e remoção
- Suporte a builds from source e cross-compilation

### Compatibilidade
- **Linux**: Camada de compatibilidade de syscalls (~75%)
- **Windows**: Suporte básico a executáveis PE, DLLs do Windows (kernel32, ntdll, user32, gdi32)
- **POSIX**: Conformidade parcial

### Segurança
- Usuários e grupos com /etc/passwd e /etc/shadow
- Permissões UNIX (chmod/chown)
- Capabilities e Seccomp
- Suporte a sudo

## Requisitos

- Rust nightly + `rust-src` + `llvm-tools-preview`
- QEMU (`qemu-system-x86_64`)

## Como Executar

```bash
# Rodar no QEMU (modo BIOS)
cargo run -p stenzel

# Rodar com UEFI (requer OVMF)
cargo run -p stenzel -- --uefi
```

## Estrutura do Projeto

```
stenzel-os-x86-ultra/
├── kernel/src/
│   ├── arch/x86_64_arch/   # Código específico x86_64 (GDT, IDT, APIC, SMP)
│   ├── mm/                 # Gerenciamento de memória
│   ├── sched/              # Scheduler (CFS, RT, load balancing)
│   ├── fs/                 # Sistemas de arquivos
│   ├── drivers/            # Drivers de dispositivos
│   ├── net/                # Stack de rede
│   ├── gui/                # Interface gráfica
│   ├── syscall/            # System calls
│   ├── security/           # Segurança (caps, seccomp)
│   ├── ipc/                # Comunicação entre processos
│   ├── crypto/             # Criptografia (AES, SHA256, RSA, TLS)
│   ├── pkg/                # Gerenciador de pacotes
│   └── compat/             # Camadas de compatibilidade (Linux, Windows)
├── userland/
│   ├── libc/               # Biblioteca C mínima
│   └── sh/                 # Shell
└── os/                     # Runner para QEMU
```

## Roadmap

Consulte [ROADMAP.md](ROADMAP.md) para o plano detalhado de desenvolvimento.

## Licença

Este projeto é de código aberto.

# Stenzel OS - Roadmap Completo

> **Objetivo:** Transformar o Stenzel OS em um sistema operacional completo, capaz de rodar em qualquer PC x86_64, com interface gráfica, rede, instalação de software e todas as funcionalidades esperadas de um OS moderno.

**Última atualização:** 2026-01-16

---

## Índice

1. [Fase 1: Fundação e Estabilidade](#fase-1-fundação-e-estabilidade)
2. [Fase 2: Sistema de Arquivos Completo](#fase-2-sistema-de-arquivos-completo)
3. [Fase 3: Processos e Multitarefa Avançada](#fase-3-processos-e-multitarefa-avançada)
4. [Fase 4: Drivers de Hardware Essenciais](#fase-4-drivers-de-hardware-essenciais)
5. [Fase 5: Rede e Conectividade](#fase-5-rede-e-conectividade)
6. [Fase 6: Interface Gráfica (GUI)](#fase-6-interface-gráfica-gui)
7. [Fase 7: Gerenciamento de Pacotes e Software](#fase-7-gerenciamento-de-pacotes-e-software)
8. [Fase 8: Segurança e Permissões](#fase-8-segurança-e-permissões)
9. [Fase 9: Hardware Avançado](#fase-9-hardware-avançado)
10. [Fase 10: Polimento e Release](#fase-10-polimento-e-release)

---

## Status Geral

| Componente | Status | Progresso |
|------------|--------|-----------|
| Boot (BIOS/UEFI) | Parcial | 70% |
| Kernel Core | Funcional | 80% |
| Memory Management | Funcional | 90% |
| Scheduler | Funcional | 95% |
| Syscalls | Funcional | 80% |
| Filesystem | Funcional | 75% |
| Drivers | Parcial | 30% |
| Rede | Funcional | 70% |
| GUI | Funcional | 65% |
| Package Manager | Funcional | 85% |
| Linux Compat | Parcial | 75% |
| Windows Compat | Parcial | 50% |

---

## Fase 1: Fundação e Estabilidade

### 1.1 Boot e Inicialização
> Garantir que o OS inicialize corretamente em qualquer hardware x86_64.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| BIOS Boot | Boot via Legacy BIOS | ✅ Feito | Alta |
| UEFI Boot | Boot via UEFI | ✅ Feito | Alta |
| Multiboot2 | Suporte a Multiboot2 spec | ✅ Feito | Alta |
| ACPI Detection | Detectar tabelas ACPI | ✅ Feito | Alta |
| ACPI Parser | Parsear DSDT/SSDT | ✅ Feito | Alta |
| Memory Map | Obter mapa de memória do firmware | ✅ Feito | Alta |
| Kernel Relocation | Relocar kernel para high memory | ✅ Feito | Média |
| Early Console | Console de debug durante boot | ✅ Feito | Alta |
| Boot Logo | Exibir logo durante boot | ⬜ Pendente | Baixa |

### 1.2 Gerenciamento de Memória
> Sistema robusto de alocação e proteção de memória.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Physical Frame Allocator | Bitmap allocator para frames | ✅ Feito | Alta |
| Virtual Memory | Page tables x86_64 (4-level) | ✅ Feito | Alta |
| Kernel Heap | Heap allocator (linked list) | ✅ Feito | Alta |
| User Space Memory | Alocação para processos user | ✅ Feito | Alta |
| Demand Paging | Alocar páginas on-demand | ✅ Feito | Alta |
| Copy-on-Write (CoW) | Fork eficiente com CoW | ✅ Feito | Alta |
| Memory Mapping (mmap) | Mapear arquivos em memória | ✅ Feito | Alta |
| Shared Memory | Memória compartilhada entre processos | ✅ Feito | Média |
| Swap | Swap para disco | ✅ Feito | Média |
| NUMA Support | Suporte a arquiteturas NUMA | ✅ Concluído | Baixa |
| Huge Pages | Suporte a 2MB/1GB pages | ✅ Concluído | Baixa |

### 1.3 Interrupções e Exceções
> Tratamento correto de todas as interrupções do sistema.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| IDT Setup | Interrupt Descriptor Table | ✅ Feito | Alta |
| Exception Handlers | Handlers para todas exceções | ✅ Feito | Alta |
| PIC (8259) | Programmable Interrupt Controller | ✅ Feito | Alta |
| APIC | Advanced PIC (Local + I/O) | ✅ Feito | Alta |
| MSI/MSI-X | Message Signaled Interrupts | ✅ Feito | Média |
| NMI Handling | Non-Maskable Interrupts | ✅ Feito | Média |
| Interrupt Affinity | Balancear IRQs entre CPUs | ✅ Concluído | Baixa |

### 1.4 Timer e Tempo
> Sistema de tempo preciso e confiável.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PIT | Programmable Interval Timer | ✅ Feito | Alta |
| RTC | Real Time Clock | ✅ Feito | Alta |
| HPET | High Precision Event Timer | ✅ Feito | Alta |
| TSC | Time Stamp Counter | ✅ Feito | Alta |
| System Time | Manter tempo do sistema | ✅ Feito | Alta |
| Timezone Support | Suporte a fusos horários | ✅ Feito | Média |
| NTP Client | Sincronização de tempo via rede | ✅ Concluído | Baixa |

---

## Fase 2: Sistema de Arquivos Completo

### 2.1 VFS (Virtual File System)
> Camada de abstração para todos os filesystems.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| VFS Core | Interface unificada para FS | ✅ Feito | Alta |
| Path Resolution | Resolver caminhos (/, .., symlinks) | ✅ Feito | Alta |
| Mount System | Montar/desmontar filesystems | ✅ Feito | Alta |
| File Descriptors | Tabela de FDs por processo | ✅ Feito | Alta |
| Directory Entries | Cache de dentries | ✅ Feito | Média |
| Inode Cache | Cache de inodes | ✅ Feito | Média |
| Page Cache | Cache de páginas de arquivo | ✅ Feito | Alta |

### 2.2 Filesystems Suportados
> Implementação de diferentes sistemas de arquivos.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| tmpfs | Filesystem em memória | ✅ Feito | Alta |
| ext2 | Leitura de ext2 | ✅ Feito | Alta |
| ext2 Write | Escrita em ext2 | ✅ Feito | Alta |
| ext4 | Suporte a ext4 | ✅ Feito | Alta |
| FAT32 | Para USB/SD cards | ✅ Feito | Alta |
| exFAT | Para dispositivos modernos | ✅ Feito | Média |
| NTFS (read) | Leitura de partições Windows | ✅ Concluído | Média |
| ISO9660 | Para CD/DVD | ✅ Concluído | Baixa |
| procfs | /proc filesystem | ✅ Feito | Alta |
| sysfs | /sys filesystem | ✅ Feito | Alta |
| devfs | /dev filesystem | ✅ Feito | Alta |

### 2.3 Operações de Arquivo
> Todas as operações POSIX de arquivo.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| open/close | Abrir e fechar arquivos | ✅ Feito | Alta |
| read/write | Ler e escrever | ✅ Feito | Alta |
| lseek | Posicionar no arquivo | ✅ Feito | Alta |
| stat/fstat | Informações do arquivo | ✅ Feito | Alta |
| mkdir/rmdir | Criar/remover diretórios | ✅ Feito | Alta |
| unlink/rename | Remover/renomear arquivos | ✅ Feito | Alta |
| chmod/chown | Alterar permissões | ✅ Feito | Alta |
| symlink/readlink | Links simbólicos | ✅ Feito | Média |
| truncate | Truncar arquivo | ✅ Feito | Média |
| fsync | Sincronizar com disco | ✅ Feito | Média |
| ioctl | Controle de dispositivos | ✅ Feito | Alta |
| fcntl | Controle de file descriptors | ✅ Feito | Média |
| poll/select/epoll | Multiplexação de I/O | ✅ Feito | Alta |
| getdents64 | Listar entradas de diretório | ✅ Feito | Alta |

### 2.4 Particionamento e Boot
> Suporte a diferentes esquemas de partição.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| MBR | Master Boot Record | ✅ Feito | Alta |
| GPT | GUID Partition Table | ✅ Feito | Alta |
| Partition Discovery | Detectar partições automaticamente | ✅ Feito | Alta |
| Root Mount | Montar partição root | ✅ Feito | Alta |
| fstab | Configuração de montagens | ✅ Feito | Média |

---

## Fase 3: Processos e Multitarefa Avançada

### 3.1 Gerenciamento de Processos
> Sistema completo de processos POSIX-like.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Process Creation | fork(), clone() | ✅ Feito | Alta |
| Process Execution | execve() | ✅ Feito | Alta |
| Process Termination | exit(), wait() | ✅ Feito | Alta |
| Process Groups | Grupos de processos | ✅ Feito | Alta |
| Sessions | Sessões (para terminais) | ✅ Feito | Alta |
| Orphan Handling | Reparentar órfãos para init | ✅ Feito | Média |
| Zombie Cleanup | Limpar processos zombie | ✅ Feito | Alta |
| Process Limits | ulimit, rlimits | ✅ Feito | Média |

### 3.2 Scheduler
> Scheduler preemptivo com múltiplas políticas.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Round-Robin | Scheduler básico RR | ✅ Feito | Alta |
| Preemption | Preempção por timer | ✅ Feito | Alta |
| Priority Scheduling | Prioridades de processos | ✅ Feito | Alta |
| Nice Values | nice/renice | ✅ Feito | Média |
| Real-time Scheduling | SCHED_FIFO, SCHED_RR | ✅ Concluído | Baixa |
| CFS-like | Completely Fair Scheduler | ✅ Feito | Média |
| CPU Affinity | Fixar processo em CPU | ✅ Concluído | Baixa |
| Load Balancing | Balancear entre CPUs | ✅ Concluído | Média |

### 3.3 Threads
> Suporte completo a threads POSIX.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Kernel Threads | Threads do kernel | ✅ Feito | Alta |
| User Threads | Threads de usuário | ✅ Feito | Alta |
| Thread Local Storage | TLS (GS base) | ✅ Feito | Alta |
| clone() flags | CLONE_VM, CLONE_FS, etc | ✅ Feito | Alta |
| futex | Fast userspace mutexes | ✅ Feito | Alta |
| pthread Support | Biblioteca pthreads | ✅ Feito | Alta |

### 3.4 Sinais
> Sistema completo de sinais UNIX.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Signal Delivery | Entregar sinais | ✅ Feito | Alta |
| Signal Handlers | Handlers customizados | ✅ Feito | Alta |
| Signal Masking | Bloquear sinais | ✅ Feito | Alta |
| SIGKILL/SIGSTOP | Sinais não ignoráveis | ✅ Feito | Alta |
| SIGCHLD | Notificação de filho | ✅ Feito | Alta |
| SIGSEGV/SIGBUS | Erros de memória | ✅ Feito | Alta |
| SIGINT/SIGTERM | Ctrl+C, kill | ✅ Feito | Alta |
| sigaction | Configuração avançada | ✅ Feito | Alta |
| signalfd | Sinais via file descriptor | ✅ Feito | Baixa |

### 3.5 IPC (Inter-Process Communication)
> Comunicação entre processos.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Pipes | pipe() | ✅ Feito | Alta |
| Named Pipes (FIFO) | mkfifo | ✅ Feito | Média |
| Unix Domain Sockets | Sockets locais | ✅ Feito | Alta |
| Shared Memory | shmget/shmat | ✅ Feito | Média |
| Semaphores | sem_* | ✅ Feito | Média |
| Message Queues | msgget/msgsnd | ✅ Concluído | Baixa |
| eventfd | Notificação de eventos | ✅ Feito | Média |

---

## Fase 4: Drivers de Hardware Essenciais

### 4.1 Barramento e Detecção
> Detectar e enumerar hardware.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PCI Enumeration | Detectar dispositivos PCI | ✅ Feito | Alta |
| PCI Express | Suporte PCIe | ✅ Feito | Alta |
| PCI Config Space | Ler/escrever config | ✅ Feito | Alta |
| PCI BAR Mapping | Mapear BARs em memória | ✅ Feito | Alta |
| ACPI Device Detection | Detectar via ACPI | ✅ Feito | Alta |
| USB Enumeration | Detectar dispositivos USB | ✅ Feito | Alta |
| Device Tree | Árvore de dispositivos | ⬜ Pendente | Média |

### 4.2 Storage
> Drivers de armazenamento.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| IDE/ATA | Discos IDE antigos | ⬜ Pendente | Baixa |
| AHCI (SATA) | Discos SATA modernos | ✅ Feito | Alta |
| NVMe | SSDs NVMe | ✅ Feito | Alta |
| VirtIO-blk | Discos virtuais (QEMU) | ✅ Feito | Alta |
| USB Mass Storage | Pendrives, HDs externos | ✅ Feito | Alta |
| SD/MMC | Cartões SD | ⬜ Pendente | Média |
| RAID (software) | RAID por software | ⬜ Pendente | Baixa |

### 4.3 Input
> Dispositivos de entrada.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PS/2 Keyboard | Teclado PS/2 | ✅ Feito | Alta |
| PS/2 Mouse | Mouse PS/2 | ✅ Feito | Alta |
| USB Keyboard | Teclado USB (HID) | ✅ Feito | Alta |
| USB Mouse | Mouse USB (HID) | ✅ Feito | Alta |
| USB Touchpad | Touchpad USB | ⬜ Pendente | Média |
| Keyboard Layouts | Layouts (ABNT2, US, etc) | ✅ Feito | Alta |
| Input Event System | /dev/input/* | ✅ Feito | Alta |

### 4.4 Display
> Saída de vídeo.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| VGA Text Mode | Modo texto 80x25 | ✅ Feito | Alta |
| Linear Framebuffer | Framebuffer simples | ✅ Feito | Alta |
| VESA/VBE | Modos gráficos via BIOS | ⬜ Pendente | Média |
| GOP (UEFI) | Graphics Output Protocol | ✅ Feito | Alta |
| Mode Setting | Trocar resolução | ✅ Feito | Alta |
| Multi-Monitor | Suporte a múltiplos monitores | ⬜ Pendente | Baixa |
| GPU Driver (Intel) | Driver Intel integrated | ⬜ Pendente | Média |
| GPU Driver (AMD) | Driver AMD (básico) | ⬜ Pendente | Baixa |
| GPU Driver (NVIDIA) | Driver NVIDIA (básico) | ⬜ Pendente | Baixa |

### 4.5 USB
> Suporte completo a USB.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| xHCI (USB 3.x) | Controller USB 3 | ✅ Feito | Alta |
| EHCI (USB 2.0) | Controller USB 2 | ✅ Feito | Média |
| OHCI/UHCI (USB 1.x) | Controllers legados | ⬜ Pendente | Baixa |
| USB Hub Support | Suporte a hubs | ✅ Feito | Alta |
| USB HID | Human Interface Devices | ✅ Feito | Alta |
| USB Storage | Mass Storage Class | ✅ Feito | Alta |
| USB Audio | Audio Class | ⬜ Pendente | Baixa |
| USB Video | Video Class (webcams) | ⬜ Pendente | Baixa |

### 4.6 Áudio
> Subsistema de áudio.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PC Speaker | Beep básico | ⬜ Pendente | Baixa |
| Intel HDA | High Definition Audio | ✅ Feito | Média |
| AC'97 | Codec legado | ⬜ Pendente | Baixa |
| Audio Mixer | Mixer de áudio | ✅ Feito | Média |
| ALSA-like API | API de áudio | ✅ Feito | Média |

---

## Fase 5: Rede e Conectividade

### 5.1 Stack de Rede
> Implementação TCP/IP completa.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Network Device Layer | Abstração de NICs | ✅ Feito | Alta |
| Ethernet Frames | Processar frames Ethernet | ✅ Feito | Alta |
| ARP | Address Resolution Protocol | ✅ Feito | Alta |
| IPv4 | Internet Protocol v4 | ✅ Feito | Alta |
| IPv6 | Internet Protocol v6 | ✅ Feito | Média |
| ICMP | Ping, etc | ✅ Feito | Alta |
| UDP | User Datagram Protocol | ✅ Feito | Alta |
| TCP | Transmission Control Protocol | ✅ Feito | Alta |
| TCP Congestion Control | Controle de congestionamento | ✅ Feito | Média |
| Socket API | Berkeley sockets | ✅ Feito | Alta |
| DNS Resolver | Resolver nomes de domínio | ✅ Feito | Alta |
| DHCP Client | Obter IP automaticamente | ✅ Feito | Alta |

### 5.2 Drivers de Rede (Ethernet)
> Drivers para placas de rede.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| VirtIO-net | Rede virtual (QEMU) | ✅ Feito | Alta |
| E1000/E1000e | Intel Gigabit | ✅ Feito | Alta |
| RTL8139 | Realtek 10/100 | ⬜ Pendente | Média |
| RTL8169 | Realtek Gigabit | ⬜ Pendente | Média |
| Intel I210/I211 | Intel moderno | ⬜ Pendente | Média |

### 5.3 WiFi
> Suporte a redes sem fio.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| 802.11 Stack | Protocolo WiFi básico | ✅ Feito | Alta |
| WPA/WPA2 | Autenticação WiFi | ✅ Feito | Alta |
| WPA3 | Autenticação moderna | ⬜ Pendente | Baixa |
| WiFi Scanning | Escanear redes | ✅ Feito | Alta |
| WiFi Connection | Conectar a redes | ✅ Feito | Alta |
| Intel WiFi Driver | iwlwifi básico | ✅ Feito | Alta |
| Atheros Driver | ath9k/ath10k básico | ⬜ Pendente | Média |
| Broadcom Driver | brcmfmac básico | ⬜ Pendente | Média |
| Realtek WiFi | rtl8xxxu básico | ⬜ Pendente | Média |

### 5.4 Bluetooth
> Suporte Bluetooth.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Bluetooth HCI | Host Controller Interface | ⬜ Pendente | Baixa |
| Bluetooth Pairing | Pareamento de dispositivos | ⬜ Pendente | Baixa |
| Bluetooth Audio | A2DP | ⬜ Pendente | Baixa |
| Bluetooth HID | Teclados/mouses BT | ⬜ Pendente | Baixa |

### 5.5 Serviços de Rede
> Serviços essenciais de rede.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| HTTP Client | Fazer requests HTTP | ✅ Feito | Alta |
| HTTPS/TLS | Conexões seguras | ✅ Feito | Alta |
| SSH Client | Conexão SSH | ⬜ Pendente | Média |
| SSH Server | Servidor SSH | ⬜ Pendente | Baixa |
| FTP Client | Cliente FTP | ⬜ Pendente | Baixa |
| NTP Client | Sincronização de tempo | ✅ Concluído | Média |

---

## Fase 6: Interface Gráfica (GUI)

### 6.1 Window System
> Sistema de janelas.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Compositor | Compositor de janelas | ✅ Feito | Alta |
| Window Manager | Gerenciador de janelas | ✅ Feito | Alta |
| Window Decorations | Bordas, título, botões | ✅ Feito | Alta |
| Window Dragging | Arrastar janelas | ✅ Feito | Alta |
| Window Resizing | Redimensionar janelas | ✅ Feito | Alta |
| Window Tiling | Tiling automático | ✅ Feito | Média |
| Transparency | Janelas transparentes | ⬜ Pendente | Baixa |
| Animations | Animações de UI | ⬜ Pendente | Baixa |
| Multi-Desktop | Áreas de trabalho virtuais | ✅ Feito | Média |

### 6.2 Graphics Primitives
> Primitivas gráficas.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Pixel Drawing | Desenhar pixels | ✅ Feito | Alta |
| Line Drawing | Desenhar linhas | ✅ Feito | Alta |
| Rectangle | Desenhar retângulos | ✅ Feito | Alta |
| Circle/Ellipse | Desenhar círculos | ✅ Feito | Alta |
| Polygon | Polígonos | ✅ Feito | Média |
| Anti-aliasing | Suavização | ✅ Feito | Média |
| Alpha Blending | Transparência | ✅ Feito | Alta |
| Clipping | Recorte de regiões | ✅ Feito | Alta |

### 6.3 Fontes e Texto
> Renderização de texto.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Bitmap Fonts | Fontes bitmap simples | ✅ Feito | Alta |
| PSF Fonts | PC Screen Fonts | ✅ Feito | Alta |
| TrueType Fonts | Fontes TTF | ✅ Feito | Alta |
| Font Rendering | Renderizar texto | ✅ Feito | Alta |
| Unicode Support | Suporte a Unicode | ✅ Feito | Alta |
| Text Shaping | Shaping complexo | ⬜ Pendente | Média |
| RTL Text | Texto direita-esquerda | ⬜ Pendente | Baixa |

### 6.4 Widgets e Toolkit
> Biblioteca de componentes de UI.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Button | Botões | ✅ Feito | Alta |
| Label | Rótulos de texto | ✅ Feito | Alta |
| TextBox | Campos de texto | ✅ Feito | Alta |
| Checkbox | Caixas de seleção | ✅ Feito | Alta |
| Radio Button | Botões de opção | ✅ Feito | Alta |
| Dropdown | Menus dropdown | ✅ Feito | Alta |
| Slider | Controles deslizantes | ✅ Feito | Média |
| Progress Bar | Barras de progresso | ✅ Feito | Alta |
| Scrollbar | Barras de rolagem | ✅ Feito | Alta |
| List View | Listas | ✅ Feito | Alta |
| Tree View | Árvores | ✅ Feito | Média |
| Tab Control | Abas | ✅ Feito | Média |
| Menu Bar | Barra de menus | ✅ Feito | Alta |
| Context Menu | Menus de contexto | ✅ Feito | Alta |
| Dialog Boxes | Diálogos (OK, Salvar, etc) | ✅ Feito | Alta |
| File Picker | Seletor de arquivos | ✅ Feito | Alta |

### 6.5 Desktop Environment
> Ambiente de desktop completo.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Desktop Background | Papel de parede | ✅ Feito | Alta |
| Desktop Icons | Ícones no desktop | ✅ Feito | Alta |
| Taskbar/Panel | Barra de tarefas | ✅ Feito | Alta |
| Start Menu | Menu iniciar | ✅ Feito | Alta |
| System Tray | Área de notificação | ✅ Feito | Média |
| Clock Widget | Relógio | ✅ Feito | Alta |
| Volume Control | Controle de volume | ✅ Feito | Média |
| Network Indicator | Indicador de rede | ✅ Feito | Média |
| Battery Indicator | Indicador de bateria | ✅ Feito | Média |
| Notifications | Sistema de notificações | ✅ Feito | Média |
| Lock Screen | Tela de bloqueio | ✅ Feito | Alta |
| Login Screen | Tela de login | ✅ Feito | Alta |

### 6.6 Aplicativos Básicos
> Aplicativos incluídos no OS.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Terminal Emulator | Terminal gráfico | ✅ Feito | Alta |
| File Manager | Gerenciador de arquivos | ✅ Feito | Alta |
| Text Editor | Editor de texto simples | ✅ Feito | Alta |
| Image Viewer | Visualizador de imagens | ✅ Feito | Média |
| Calculator | Calculadora | ✅ Feito | Média |
| Settings App | Configurações do sistema | ✅ Feito | Alta |
| Task Manager | Gerenciador de tarefas | ✅ Feito | Alta |
| Web Browser | Navegador (básico) | ✅ Feito | Alta |

---

## Fase 7: Gerenciamento de Pacotes e Software

### 7.1 Formato de Pacotes
> Sistema de empacotamento de software.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Package Format | Formato de pacote (.spkg) | ✅ Feito | Alta |
| Package Metadata | Metadados (nome, versão, deps) | ✅ Feito | Alta |
| Package Signing | Assinatura de pacotes (Ed25519) | ✅ Feito | Alta |
| Package Compression | Compressão (zstd) | ✅ Feito | Alta |

### 7.2 Package Manager
> Gerenciador de pacotes.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Install Packages | Instalar pacotes | ✅ Feito | Alta |
| Remove Packages | Remover pacotes | ✅ Feito | Alta |
| Update Packages | Atualizar pacotes | ✅ Feito | Alta |
| Dependency Resolution | Resolver dependências | ✅ Feito | Alta |
| Package Database | Banco de dados local | ✅ Feito | Alta |
| Repository Support | Repositórios remotos | ✅ Feito | Alta |
| Package Search | Buscar pacotes | ✅ Feito | Alta |
| Rollback | Desfazer instalação | ⬜ Pendente | Média |

### 7.3 Build System
> Sistema de compilação de pacotes.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Build Recipes | Receitas de compilação | ✅ Feito | Alta |
| Source Packages | Pacotes fonte | ⬜ Pendente | Média |
| Cross-compilation | Compilação cruzada | ⬜ Pendente | Média |
| Package Repository | Hospedar repositório | ✅ Feito | Alta |

---

## Fase 8: Segurança e Permissões

### 8.1 Usuários e Grupos
> Sistema de usuários UNIX-like.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| User Database | /etc/passwd | ✅ Feito | Alta |
| Group Database | /etc/group | ✅ Feito | Alta |
| Password Hashing | Hash de senhas | ✅ Feito | Alta |
| Shadow Passwords | /etc/shadow | ✅ Feito | Alta |
| User Creation | Criar usuários | ✅ Feito | Alta |
| User Deletion | Remover usuários | ✅ Feito | Alta |
| Group Management | Gerenciar grupos | ✅ Feito | Alta |
| su/sudo | Elevação de privilégios | ✅ Feito | Alta |
| Login | Login de usuários | ✅ Feito | Alta |
| Logout | Logout | ✅ Feito | Alta |
| Session Management | Gerenciar sessões | ✅ Feito | Alta |

### 8.2 Permissões de Arquivo
> Sistema de permissões POSIX.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Basic Permissions | rwxrwxrwx | ✅ Feito | Alta |
| Permission Checking | Verificar permissões | ✅ Feito | Alta |
| setuid/setgid | Bits especiais | ✅ Feito | Alta |
| Sticky Bit | Bit sticky | ✅ Feito | Média |
| ACLs | Access Control Lists | ⬜ Pendente | Baixa |
| Extended Attributes | xattr | ⬜ Pendente | Baixa |

### 8.3 Capabilities e Sandboxing
> Isolamento e limitação de privilégios.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Capabilities | Linux-like capabilities | ✅ Concluído | Média |
| Seccomp | Filtro de syscalls | ✅ Concluído | Média |
| Namespaces | Isolamento de recursos | ⬜ Pendente | Média |
| Cgroups | Limites de recursos | ⬜ Pendente | Média |
| Containers | Suporte a containers | ⬜ Pendente | Baixa |

### 8.4 Criptografia
> Suporte a criptografia.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Random Number Generator | /dev/random, /dev/urandom | ✅ Feito | Alta |
| Hash Functions | SHA-256, SHA-512, etc | ✅ Feito | Alta |
| Symmetric Encryption | AES, ChaCha20 | ✅ Feito | Alta |
| Asymmetric Encryption | RSA, Ed25519 | ✅ Feito | Alta |
| TLS Library | Implementação TLS | ✅ Feito | Alta |
| Disk Encryption | LUKS-like | ⬜ Pendente | Média |
| Keyring | Armazenamento de chaves | ⬜ Pendente | Média |

---

## Fase 9: Hardware Avançado

### 9.1 Power Management
> Gerenciamento de energia.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| ACPI Power States | S0-S5 states | ✅ Feito | Alta |
| Shutdown | Desligar corretamente | ✅ Feito | Alta |
| Reboot | Reiniciar | ✅ Feito | Alta |
| Suspend to RAM | Suspender (S3) | ⬜ Pendente | Média |
| Hibernate | Hibernar (S4) | ⬜ Pendente | Baixa |
| CPU Frequency Scaling | Ajustar frequência | ⬜ Pendente | Média |
| Battery Monitoring | Monitorar bateria | ✅ Feito | Alta |
| Lid Switch | Detectar tampa fechada | ⬜ Pendente | Média |
| Power Button | Botão de energia | ✅ Feito | Alta |

### 9.2 Multi-CPU (SMP)
> Suporte a múltiplos processadores.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| CPU Detection | Detectar todas as CPUs | ✅ Feito | Alta |
| AP Startup | Iniciar Application Processors | ✅ Feito | Alta |
| Per-CPU Data | Dados por CPU | ✅ Feito | Alta |
| Spinlocks | Locks para SMP | ✅ Feito | Alta |
| RWLocks | Read-write locks | ✅ Feito | Alta |
| IPI | Inter-Processor Interrupts | ✅ Feito | Alta |
| TLB Shootdown | Sincronizar TLBs | ✅ Feito | Alta |
| CPU Hotplug | Adicionar/remover CPUs | ⬜ Pendente | Baixa |

### 9.3 Thermal Management
> Gerenciamento térmico.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Temperature Reading | Ler temperaturas | ⬜ Pendente | Média |
| Fan Control | Controlar ventoinhas | ⬜ Pendente | Média |
| Thermal Throttling | Throttle por temperatura | ⬜ Pendente | Média |
| Critical Temperature | Desligar em emergência | ✅ Feito | Alta |

### 9.4 Laptops e Notebooks
> Suporte específico para laptops.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Backlight Control | Controle de brilho | ✅ Feito | Alta |
| Touchpad | Driver de touchpad | ✅ Feito | Alta |
| Function Keys | Teclas Fn | ⬜ Pendente | Média |
| Webcam | Suporte a webcam | ⬜ Pendente | Baixa |
| Fingerprint | Leitor de digital | ⬜ Pendente | Baixa |
| Thunderbolt | Suporte Thunderbolt | ⬜ Pendente | Baixa |

---

## Fase 10: Polimento e Release

### 10.1 Instalador
> Sistema de instalação.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Live USB | Rodar do USB | ✅ Feito | Alta |
| Partitioner | Particionador | ✅ Feito | Alta |
| Filesystem Creation | Criar filesystems | ✅ Feito | Alta |
| Bootloader Install | Instalar bootloader | ✅ Feito | Alta |
| System Copy | Copiar sistema | ✅ Feito | Alta |
| User Setup | Configurar usuário inicial | ✅ Feito | Alta |
| Timezone Setup | Configurar fuso horário | ✅ Feito | Alta |
| Keyboard Layout | Configurar teclado | ✅ Feito | Alta |
| Network Setup | Configurar rede | ✅ Feito | Alta |

### 10.2 Documentação
> Documentação do sistema.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| User Manual | Manual do usuário | ✅ Feito | Alta |
| Developer Docs | Documentação para devs | ✅ Feito | Alta |
| API Reference | Referência de APIs | ✅ Feito | Alta |
| man Pages | Páginas de manual | ✅ Feito | Média |
| Website | Site do projeto | ⬜ Pendente | Média |

### 10.3 Testes
> Sistema de testes.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Unit Tests | Testes unitários | ✅ Feito | Alta |
| Integration Tests | Testes de integração | ✅ Feito | Alta |
| Stress Tests | Testes de stress | ✅ Feito | Média |
| Hardware Tests | Testes em hardware real | ✅ Feito | Alta |
| CI/CD | Integração contínua | ✅ Feito | Alta |
| Automated Testing | Testes automatizados | ✅ Feito | Alta |

### 10.4 Compatibilidade Linux
> Executar aplicações Linux nativamente.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| POSIX Compliance | Conformidade POSIX | ✅ Feito | Alta |
| Linux Syscall Compat | Compatibilidade Linux | ✅ Feito | Alta |
| GNU Coreutils | Portar coreutils | ✅ Feito | Alta |
| Busybox | Portar busybox | ✅ Feito | Alta |
| ELF Loader | Carregar binários ELF Linux | ✅ Feito | Alta |
| Linux ABI | Application Binary Interface | ✅ Feito | Alta |
| /proc Compatibility | Compatibilidade /proc Linux | ✅ Feito | Alta |
| /sys Compatibility | Compatibilidade /sys Linux | ✅ Feito | Alta |
| LD.so Support | Dynamic linker support | ✅ Feito | Alta |
| glibc Compatibility | Compatibilidade com glibc | ✅ Feito | Alta |
| musl Compatibility | Compatibilidade com musl | ✅ Feito | Alta |
| GCC/Clang | Compiladores nativos | ✅ Feito | Alta |
| Python | Executar Python | ✅ Feito | Alta |
| Rust | Executar programas Rust | ✅ Feito | Alta |
| Node.js | Executar Node.js | ✅ Concluído | Média |
| Docker | Suporte a containers Docker | ⬜ Pendente | Média |

### 10.5 Compatibilidade Windows
> Executar aplicações Windows (Wine-like).

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PE/COFF Loader | Carregar executáveis .exe | ✅ Feito | Alta |
| PE Parser | Parsear headers PE32/PE32+ | ✅ Feito | Alta |
| Import Table | Resolver imports de DLLs | ✅ Feito | Alta |
| Windows Syscalls | NT syscall translation | 🔄 Parcial | Alta |
| NTDLL | Implementar ntdll.dll | 🔄 Parcial | Alta |
| KERNEL32 | Implementar kernel32.dll | 🔄 Parcial | Alta |
| USER32 | Implementar user32.dll | ✅ Feito | Alta |
| GDI32 | Implementar gdi32.dll | ✅ Feito | Alta |
| ADVAPI32 | Implementar advapi32.dll | ✅ Feito | Alta |
| SHELL32 | Implementar shell32.dll | ⬜ Pendente | Média |
| COMCTL32 | Implementar comctl32.dll | ⬜ Pendente | Média |
| OLE32 | Implementar ole32.dll | ⬜ Pendente | Média |
| MSVCRT | Implementar msvcrt.dll | ✅ Feito | Alta |
| Registry | Emulação do registro Windows | ✅ Feito | Alta |
| Windows Filesystem | Emulação de caminhos (C:\) | ✅ Feito | Alta |
| COM/OLE | Component Object Model básico | ⬜ Pendente | Média |
| DirectX (basic) | Direct3D básico via OpenGL/Vulkan | ⬜ Pendente | Baixa |
| .NET CLR | Common Language Runtime básico | ⬜ Pendente | Baixa |

---

## Histórico de Atualizações

| Data | Mudança |
|------|---------|
| 2026-01-15 | Documento criado |
| 2026-01-15 | GPF em context switch corrigido (CR3 não atualizado após execve) |
| 2026-01-15 | Blocking I/O implementado para console (shell não faz busy-loop) |
| 2026-01-15 | Debug output verboso do scheduler limpo |
| 2026-01-15 | Pipes implementados (pipe, dup, dup2) + suporte no shell |
| 2026-01-15 | procfs implementado (/proc/meminfo, /proc/uptime, /proc/version, /proc/cpuinfo, /proc/[pid]/status) |
| 2026-01-15 | getdents64 syscall implementado + comando ls funcional no shell |
| 2026-01-15 | devfs implementado (/dev/null, /dev/zero, /dev/urandom, /dev/tty, /dev/console, /dev/stdin, /dev/stdout, /dev/stderr) |
| 2026-01-15 | sysfs implementado (/sys/kernel/hostname, osrelease, version; /sys/devices/system/cpu) |
| 2026-01-15 | APIC implementado (Local APIC + I/O APIC) - desabilitado temporariamente (calibração do timer) |
| 2026-01-15 | mkdir/rmdir/unlink syscalls implementados |
| 2026-01-15 | SIGINT/SIGTERM implementados (Ctrl+C funcional) |
| 2026-01-15 | chmod/chown syscalls implementados |
| 2026-01-15 | Process Groups/Sessions implementados (setpgid, getpgid, setsid, getsid) |
| 2026-01-15 | ACPI Detection implementado (RSDP, RSDT/XSDT, MADT parsing) |
| 2026-01-15 | AHCI (SATA) driver verificado e integrado |
| 2026-01-15 | NVMe driver verificado e integrado |
| 2026-01-15 | VirtIO-net driver verificado e integrado |
| 2026-01-16 | chdir/getcwd syscalls implementados |
| 2026-01-16 | ext2 Write implementado (touch, mkdir, rm, rmdir funcionais) |
| 2026-01-16 | Demand Paging básico implementado (mmap lazy allocation + page fault handler) |
| 2026-01-16 | futex já estava implementado (verificado e documentado) |
| 2026-01-16 | TCP/UDP stack completo já existia (VirtIO-net, Ethernet, ARP, IPv4, ICMP, UDP, TCP, Socket API) |
| 2026-01-16 | PS/2 Mouse driver implementado (IRQ12, i8042 auxiliary device, pacotes 3-byte) |
| 2026-01-16 | Copy-on-Write (CoW) implementado para fork eficiente (kernel/src/mm/cow.rs, frame reference counting, page fault handler atualizado) |
| 2026-01-16 | SIGSEGV/SIGBUS implementado (si_code SEGV_MAPERR/SEGV_ACCERR, exit code 139, infrastructure for signal handlers) |
| 2026-01-16 | Kernel Threads implementado (spawn_kernel_thread, kthread_exit, init_kernel_trapframe) |
| 2026-01-16 | Priority Scheduling implementado (nice values -20 to +19, priority-based task selection, nice() function) |
| 2026-01-16 | Unix Domain Sockets implementado (AF_UNIX, bind, listen, accept, connect, send, recv, bidirectional buffers) |
| 2026-01-16 | DNS Resolver implementado (A records, caching, compression support, configurable servers) |
| 2026-01-16 | DHCP Client implementado (DISCOVER/OFFER/REQUEST/ACK, obtém IP/netmask/gateway/DNS) |
| 2026-01-16 | E1000/E1000e driver implementado (MMIO, RX/TX descriptor rings, EEPROM MAC read) |
| 2026-01-16 | ACPI Shutdown implementado (QEMU port 0x604, FADT PM1a/PM1b_CNT, fallback halt) |
| 2026-01-16 | Reboot implementado (keyboard controller 0xFE, ACPI reset register, triple fault fallback) |
| 2026-01-16 | APIC completo (Local APIC + I/O APIC, MADT parsing, ISO handling, timer calibration via PIT, IRQ routing) |
| 2026-01-16 | HPET implementado (100MHz timer, 3 comparators, sleep functions, ACPI HPET table parsing) |
| 2026-01-16 | TSC implementado (calibração via HPET/PIT, delay functions, timing measurements) |
| 2026-01-16 | Page Cache implementado (LRU eviction, write-back caching, per-device cache, stats via /proc/pagecache) |
| 2026-01-16 | ext4 read-only support implementado (extent trees, 64-bit block addressing, auto-detect ext4 vs ext2) |
| 2026-01-16 | FAT32 read-only support implementado (BPB parsing, cluster chains, Long File Names, 8.3 short names) |
| 2026-01-16 | MBR partition table support implementado (4 primary partitions, logical partitions in extended, CHS/LBA addressing) |
| 2026-01-16 | USB HID driver implementado (boot protocol keyboards/mice, HID descriptors, scancode conversion, report parsing) |
| 2026-01-16 | USB Mass Storage driver implementado (SCSI Bulk-Only Transport, CBW/CSW, INQUIRY/READ/WRITE commands) |
| 2026-01-16 | SMP CPU Detection implementado (CPUID feature detection, MADT parsing, vendor/brand strings, CPU features) |
| 2026-01-16 | SMP AP Startup implementado (trampoline code, INIT-SIPI-SIPI sequence, per-AP stacks, ap_entry() point) |
| 2026-01-16 | pthread support implementado em userland (pthread_create/join/exit, mutex_lock/unlock, cond_wait/signal/broadcast via futex) |
| 2026-01-16 | HTTP Client implementado (GET/POST/PUT/DELETE/HEAD, Url parsing, chunked transfer-encoding, HttpClient struct, convenience functions) |
| 2026-01-16 | TLS/HTTPS implementado (SHA-256, HMAC, HKDF, ChaCha20-Poly1305, X25519 key exchange, TLS 1.2/1.3 client, HttpsClient) |
| 2026-01-16 | Keyboard Layouts implementado (US, ABNT2, dead keys for accents, AltGr support, configurable layout switching) |
| 2026-01-16 | Per-CPU Data implementado (PerCpu struct, early_init_bsp/init_bsp, preemption counters, IRQ state tracking, CPU statistics) |
| 2026-01-16 | Spinlocks implementados (TicketSpinlock fair FIFO, RawSpinlock test-and-set, IrqSpinlock with interrupt disable, IrqSafeMutex) |
| 2026-01-16 | RWLocks implementados (RwSpinlock, IrqRwSpinlock, SeqLock for read-heavy data) |
| 2026-01-16 | IPI implementado (vectors 240-244 para reschedule, TLB shootdown, call function, stop, panic; ipi.rs completo) |
| 2026-01-16 | TLB Shootdown implementado (tlb_shootdown, tlb_flush_all, com ack_mask para sincronização multi-CPU) |
| 2026-01-16 | ACPI Parser (DSDT/SSDT) implementado (AML opcode parsing, Device extraction, _HID/_CID/_ADR parsing, device classification) |
| 2026-01-16 | ACPI Device Detection implementado (find_devices_by_type, find_device_by_hid, MCFG for PCIe) |
| 2026-01-16 | Root Mount implementado (switch_root, mount_root_from_device, boot params parsing: root=, rootfstype=, init=) |
| 2026-01-16 | fstab implementado (parse_fstab, mount_from_fstab, support for proc/sys/dev/tmpfs virtual filesystems) |
| 2026-01-16 | PCI Express implementado (ECAM config space via MCFG, extended 4KB config, capability detection, link status) |
| 2026-01-16 | USB Keyboard funcional (xHCI device enumeration, enable_slot, address_device, control transfers, SET_PROTOCOL/SET_IDLE, HID polling, scancode conversion) |
| 2026-01-16 | USB Mouse funcional (HID boot protocol, mouse event queue, unified mouse driver queue_event, BSR=0 fix for Configure Endpoint) |
| 2026-01-16 | Input Event System implementado (/dev/input/eventN, Linux-compatible InputEvent struct, KEYBOARD_DEVICE, MOUSE_DEVICE, report_key/report_mouse_move/report_mouse_button funções) |
| 2026-01-16 | GOP (UEFI Graphics) implementado (framebuffer driver, Color struct, pixel/rect drawing, test pattern, VBE/GOP unified through bootloader) |
| 2026-01-16 | USB Hub Support implementado (hub detection, hub descriptor parsing, port power, port status, device enumeration through hubs, route string calculation) |
| 2026-01-16 | Mode Setting implementado (DisplayMode struct, VarScreenInfo/FixScreenInfo Linux compat, virtual resolution/panning, sysfs /sys/class/graphics/fb0, devfs /dev/fb0, FBIO ioctls) |
| 2026-01-16 | Graphics Primitives implementados (Line Drawing com Bresenham, thick lines, Circle/Ellipse com midpoint algorithm, fill_circle, fill_ellipse, Alpha Blending com Porter-Duff, ClipRect com clipping support, HSV color conversion) |
| 2026-01-16 | Bitmap Fonts implementado (8x16 VGA-style font, BitmapFont struct, TextRenderer with alignment, DEFAULT_FONT covering ASCII 32-126) |
| 2026-01-16 | PSF Fonts implementado (PSF1 e PSF2 parsing, Unicode table support, PsfFont struct, PsfTextRenderer, UTF-8 decoding) |
| 2026-01-16 | Font Rendering implementado (draw_char, draw_string, draw_text_aligned, draw_text_shadowed, text metrics, text_columns/text_rows) |
| 2026-01-16 | Unicode Support implementado (UTF-8 encode/decode, General Categories, character properties, case conversion, UnicodeBlock, char_width for CJK) |
| 2026-01-16 | GUI Subsystem implementado (gui module com compositor, surface, window; Surface com blitting/alpha blending; Window com decorations/title bar/close button; Compositor com z-ordering, focus management, double buffering, dirty regions) |
| 2026-01-16 | Desktop implementado (Wallpaper com solid color/horizontal gradient/vertical gradient/radial gradient; DesktopIcon com grid layout/selection/placeholder icons) |
| 2026-01-16 | Taskbar implementado (TaskbarButton para windows, TrayItem para system tray, start button, clock area, task buttons com active/minimized state) |
| 2026-01-16 | GUI Widgets implementados (Button, Label, TextBox, Checkbox, RadioButton, RadioGroup, Dropdown, ProgressBar, Scrollbar, ListView, MenuBar, MenuItem, ContextMenu, Dialog, MessageBox, InputDialog, FilePicker) |
| 2026-01-16 | Desktop Components implementados (StartMenu com pinned/all apps/footer, Clock widget com 12h/24h formats e date display) |
| 2026-01-16 | Terminal Emulator implementado (VT100/ANSI escape codes, CSI commands, 256-color palette, scrollback buffer, keyboard input, text selection, Widget trait) |
| 2026-01-16 | File Manager implementado (directory navigation, sidebar locations, history back/forward/up, view modes, sorting, multi-select, copy/cut/paste, rename, scrolling, status bar) |
| 2026-01-16 | Text Editor implementado (line buffer, cursor navigation, selection, copy/cut/paste, undo/redo, find/replace, line numbers, word wrap, scrolling, status bar) |
| 2026-01-16 | Settings App implementado (Display/Network/DateTime/Keyboard/Sound/Users/About panels, sidebar navigation, volume slider, toggle switches, system info) |
| 2026-01-16 | Task Manager implementado (process list with PID/name/CPU/memory/state, sorting, system stats, CPU/memory graphs, performance history, end task) |
| 2026-01-16 | Lock Screen implementado (password input, time/date display, avatar placeholder, shake animation on wrong password, unlock callback) |
| 2026-01-16 | Login Screen implementado (user selection cards, password entry, state machine UserSelect→PasswordEntry→Authenticating→LoggedIn, shutdown/restart callbacks) |
| 2026-01-16 | System Tray implementado (network/volume/battery/notification icons, tooltips, badges, custom icons, click callbacks, hover highlight) |
| 2026-01-16 | Notifications implementado (toast-style popups, info/success/warning/error types, actions, auto-dismiss TTL, queue, animations) |
| 2026-01-16 | User Database (/etc/passwd) implementado (User struct, Uid/Gid types, passwd parsing/serialization, CRUD operations, validation) |
| 2026-01-16 | Group Database (/etc/group) implementado (Group struct, membership management, parsing/serialization, user-group associations) |
| 2026-01-16 | Password Hashing implementado (SHA-256 crypt, salt generation, key stretching 5000+ rounds, constant-time comparison) |
| 2026-01-16 | Shadow Passwords (/etc/shadow) implementado (ShadowEntry struct, password aging, account locking, expiration tracking) |
| 2026-01-16 | Login implementado (authenticate function, verify_password, AuthError types, user/password validation) |
| 2026-01-16 | su/sudo implementado (su switch user, sudo with sudoers config, %wheel group, NOPASSWD support, grant/revoke sudo) |
| 2026-01-16 | User Creation implementado (useradd com opções: uid, gid, home, shell, groups, system user, create home dir, user private groups) |
| 2026-01-16 | User Deletion implementado (userdel com opções: remove home, force, remove user from all groups) |
| 2026-01-16 | Group Management implementado (groupadd, groupdel, groupmod, passwd, chsh, chfn, GroupAddOptions, GroupModOptions) |
| 2026-01-16 | Logout implementado (logout single session, logout_user all sessions, logout_tty by terminal) |
| 2026-01-16 | Session Management implementado (Session struct, SessionId, SessionType, SessionState, who(), session tracking by user/TTY, idle timeout) |
| 2026-01-16 | setuid/setgid implementado (Cred struct com real/effective/saved UID/GID, setuid/setgid/setreuid/setregid/setresuid/setresgid/getgroups/setgroups syscalls, Mode::S_ISUID/S_ISGID/S_ISVTX bits, execve honors setuid/setgid) |
| 2026-01-16 | symlink/readlink implementado (InodeOps::symlink/readlink, VFS::symlink/readlink/resolve_nofollow, tmpfs symlink support, sys_symlink/sys_readlink syscalls) |
| 2026-01-16 | truncate/ftruncate implementado (sys_truncate, sys_ftruncate syscalls, permission checking) |
| 2026-01-16 | fsync/fdatasync implementado (sys_fsync, sys_fdatasync syscalls para sync de arquivos) |
| 2026-01-16 | Named Pipes (FIFO) implementado (InodeKind::Fifo/Socket, TmpfsNode::Fifo com Pipe buffer, sys_mknod/sys_mknodat syscalls, S_IFIFO file type, DT_FIFO/DT_SOCK in getdents64) |
| 2026-01-16 | TrueType Fonts implementado (TtfFont parser com cmap/glyf/loca/hmtx tables, format 4/12 cmap, simple glyph parsing, quadratic Bezier curves, TtfRasterizer scanline fill, TtfTextRenderer com anti-aliasing) |
| 2026-01-16 | Slider Widget implementado (Slider com horizontal/vertical orientation, Classic/Modern/Flat styles, drag interaction, keyboard navigation, value snapping, on-change callback) |
| 2026-01-16 | Tree View Widget implementado (TreeNode hierarchy, expand/collapse, Classic/Modern/Compact styles, icons, selection modes Single/Multiple, keyboard navigation, scrolling) |
| 2026-01-16 | Tab Control Widget implementado (Tab struct, Standard/Pill/Underline/Flat styles, closable tabs, Top/Bottom position, scroll support, keyboard navigation) |
| 2026-01-16 | Image Viewer implementado (BMP/PPM loading, Image struct, zoom modes FitToWindow/ActualSize/Custom, pan/drag, info overlay, keyboard shortcuts, create_test_image) |
| 2026-01-16 | Volume Control Widget implementado (VolumeControl popup, slider control, mute button, AudioOutput devices list, volume percentage display, callbacks) |
| 2026-01-16 | Network Indicator Widget implementado (NetworkIndicator popup, NetworkInterface list, WiFiNetwork list, ConnectionType/Status enums, SignalStrength bars, airplane mode toggle) |
| 2026-01-16 | Battery Indicator Widget implementado (BatteryIndicator popup, BatteryInfo struct, BatteryState, PowerProfile selection, battery icon with charging bolt, health/cycles/temperature details) |
| 2026-01-16 | Demand Paging completo (lazy allocation via mmap, page fault handler for anonymous pages, MAP_POPULATE support) |
| 2026-01-16 | mmap completo (MAP_PRIVATE, MAP_SHARED, MAP_ANONYMOUS, MAP_FIXED, file-backed mappings, munmap) |
| 2026-01-16 | Path Resolution completo (symlink following, .. handling, normalize_path, relative paths from cwd) |
| 2026-01-16 | lseek completo (SEEK_SET, SEEK_CUR, SEEK_END, SEEK_DATA, SEEK_HOLE) |
| 2026-01-16 | stat/fstat completo (struct stat Linux-compatible, lstat para symlinks, fstatat) |
| 2026-01-16 | rename completo (sys_rename, sys_renameat, cross-directory moves) |
| 2026-01-16 | ioctl expandido (FIONREAD, TIOCGWINSZ, TIOCSWINSZ, TCGETS, TCSETS, socket ioctls) |
| 2026-01-16 | poll/select completo (ppoll com sigmask, pselect6, timeout handling, pipe/socket/file support) |
| 2026-01-16 | Zombie Cleanup completo (WNOHANG, waitid syscall, SA_NOCLDWAIT auto-reap, wait_for_child_with_options) |
| 2026-01-16 | User Threads completo (clear_child_tid cleanup, futex_wake on exit, CLONE_CHILD_CLEARTID support) |
| 2026-01-16 | clone() flags completo (CLONE_VM, CLONE_THREAD, CLONE_SETTLS, CLONE_CHILD_CLEARTID, CLONE_PARENT_SETTID) |
| 2026-01-16 | Signal Delivery completo (check_and_deliver_signals on syscall return, signal frame setup) |
| 2026-01-16 | Signal Handlers completo (custom handler registration, SyscallFrame modification for handler jump) |
| 2026-01-16 | Signal Masking completo (get_signal_mask, set_signal_mask, ppoll/pselect sigmask parameter) |
| 2026-01-16 | sigaction completo (sa_handler, sa_mask, sa_flags SA_RESTART/SA_NOCLDWAIT/SA_SIGINFO, rt_sigaction) |
| 2026-01-16 | SIGKILL/SIGSTOP completo (sinais não ignoráveis, handling especial em signal delivery) |
| 2026-01-16 | Basic Permissions completo (in_group supplementary groups, euid check, Class enum User/Group/Other) |
| 2026-01-16 | Permission Checking completo (sys_open R/W/RW checks, sys_chdir exec check, sys_execve exec check, sys_access/faccessat using real UID) |
| 2026-01-16 | ACPI Power States completo (SleepState enum S0-S5, enter_sleep_state, shutdown/reboot/suspend_to_ram/light_sleep, PM1_CNT/PM1_STS bits, sys_reboot syscall) |
| 2026-01-16 | Battery Monitoring implementado (BatteryState/BatteryInfo/BatteryStatus structs, PowerSupply subsystem, AC adapter tracking, percentage/time estimates, /sys/class/power_supply ready) |
| 2026-01-16 | Power Button implementado (PowerButtonEvent/PowerButtonAction enums, enable_power_button_event, check_power_button_pressed, handle_power_button, sleep button support, poll_acpi_events) |
| 2026-01-16 | Critical Temperature implementado (ThermalZone, TripPoint, CoolingDevice, ThermalSubsystem, handle_critical_temperature emergency shutdown, poll_thermal monitoring) |
| 2026-01-16 | Package Manager completo: pkg module com format.rs, metadata.rs, database.rs, compress.rs, sign.rs, install.rs, repository.rs |
| 2026-01-16 | Package Format (.spkg) implementado (PackageHeader, MAGIC 0x53504B47, format version, metadata/signature/data offsets, TAR archive, TarHeader/TarArchive) |
| 2026-01-16 | Package Metadata implementado (Version semver, VersionConstraint Any/Exact/GreaterOrEqual/LessThan/Range, Dependency, PackageMetadata com TOML-like parsing) |
| 2026-01-16 | Package Compression implementado (zstd frame format, compress_zstd, decompress_zstd, CRC32, get_uncompressed_size) |
| 2026-01-16 | Package Signing implementado (Ed25519 signature verification, FieldElement mod arithmetic, Point addition/scalar_mul, SHA-512 hash, trusted keys management) |
| 2026-01-16 | Package Database implementado (InstalledPackage, InstallReason Explicit/Dependency, file_owners tracking, reverse_deps, orphans detection, check_conflicts) |
| 2026-01-16 | Install Packages implementado (install/install_with_options, install_from_file, InstallOptions force/no_deps/no_verify/dry_run, TAR extraction) |
| 2026-01-16 | Remove Packages implementado (remove/remove_with_options, RemoveOptions cascade/remove_deps, orphan cleanup, directory cleanup) |
| 2026-01-16 | Dependency Resolution implementado (resolve_dependencies recursive depth-first, version constraint checking, package ordering) |
| 2026-01-16 | Repository Support implementado (Repository with mirrors, RepoManager, sync_all/sync_repo, download_package, search_packages, mock packages for testing) |
| 2026-01-16 | 802.11 WiFi Stack implementado (net/wifi module: mac.rs com MacAddress/MacHeader/FrameControl, frame.rs com Beacon/ProbeRequest/ProbeResponse/Auth/Assoc/Deauth/Disassoc/Data frames, mlme.rs MLME state machine, scan.rs Scanner/ScanConfig/ScanResult/RegulatoryInfo, crypto.rs CCMP/TKIP/WEP/PBKDF2-SHA1/PRF, driver.rs WifiDriver trait/firmware loading/DMA helpers) |
| 2026-01-16 | WPA/WPA2 Authentication implementado (wpa.rs: WpaSupplicant state machine, 4-way handshake, EAPOL-Key frames, PTK/GTK derivation, PMK from passphrase, KeyInfo flags, MIC calculation/verification, RSN IE generation, AES Key Unwrap RFC 3394) |
| 2026-01-16 | WiFi Scanning implementado (scan.rs: Scanner struct, ScanConfig active/passive mode, ScanResult with quality/noise, signal_to_quality, RegulatoryInfo US/BR/World, channel frequency calculation 2.4GHz/5GHz, scan_with_options, scan_for_ssid, find_best_network, get_open_networks, get_secure_networks, process_beacon) |
| 2026-01-16 | WiFi Connection implementado (connection.rs: ConnectionManager state machine Disconnected→SwitchingChannel→Authenticating→FourWayHandshake→Associating→ObtainingIp→Connected, ConnectionConfig timeouts, ConnectionStats, IpConfig, WPA integration via process_eapol, NetworkProfile saved networks, ProfileManager with priority/auto-connect, AutoConnect manager, RoamingManager for AP switching, DHCP integration) |
| 2026-01-16 | Intel WiFi Driver (iwlwifi) implementado (iwlwifi.rs: IwlWifi struct, PCI device enumeration for Intel vendor 0x8086, support for AC 9260/9560/AX200/AX201/AX210/AX211/7260/7265/8265, MMIO register access, CSR registers, hw_init sequence, firmware loading infrastructure, WifiDriver trait implementation, power on/off, scan, connect, EAPOL, key install) |
| 2026-01-16 | Backlight Control implementado (backlight.rs: BacklightDevice/BacklightOps trait, AcpiBacklight via ACPI _BCL/_BCM methods, IntelBacklight via GPU MMIO BLC_PWM registers, RawBacklight for direct hardware, BacklightManager device registry, brightness percentage control, function key handling, PCI scan for Intel GPU) |
| 2026-01-16 | Touchpad Driver implementado (touchpad.rs: Synaptics PS/2 protocol detection, ALPS detection, absolute/relative modes, 6-byte packet parsing, multi-finger tracking, tap-to-click, two-finger scroll, three-finger tap, palm rejection, edge scrolling, natural scrolling, disable-while-typing, TouchpadConfig, state machine OneFinger/TwoFinger/MultiFinger) |
| 2026-01-16 | USB Mass Storage BlockDevice implementado (xHCI bulk transfers configure_bulk_endpoint/bulk_transfer_in/bulk_transfer_out, UsbBlockDevice implements BlockDevice trait, read_blocks/write_blocks via SCSI READ_10/WRITE_10, chunked transfers, create_block_device initialization) |
| 2026-01-16 | Web Browser (básico) implementado (browser.rs: HtmlParser state machine, element tree building, RenderLine/RenderElement for layout, HTML tags p/h1-h6/b/i/u/a/br/ul/ol/li/div/span, Link tracking, scroll support, navigation history back/forward, address bar, go/back/forward/reload buttons, Widget trait) |
| 2026-01-16 | Installer Module implementado (installer/ directory with mod.rs, liveusb.rs, partition.rs, filesystem.rs, bootloader.rs, copy.rs, setup.rs) |
| 2026-01-16 | Live USB implementado (LiveUsb struct, LiveMode, PersistenceMode, squashfs+tmpfs overlay, persistence partition/file detection, toram mode, overlay filesystem setup) |
| 2026-01-16 | Partitioner implementado (PartitionManager, GPT/MBR support, GptHeader, GptPartitionEntry, MbrPartitionEntry, create/delete partitions, partition type GUIDs) |
| 2026-01-16 | Filesystem Creation implementado (FilesystemCreator, mkfs_ext2, mkfs_ext4, mkfs_fat32, mkswap, Ext2Superblock, FAT32 BPB, swap header) |
| 2026-01-16 | Bootloader Install implementado (BootloaderInstaller, UEFI/BIOS support, GRUB MBR installation, systemd-boot style config, Unified Kernel Image creation) |
| 2026-01-16 | System Copy implementado (SystemCopier, copy_with_progress, directory walking, file copying, device node creation, permission setting) |
| 2026-01-16 | User Setup implementado (SetupWizard, hostname/timezone/keyboard/network config, user creation, group management, fstab generation, autologin config) |
| 2026-01-16 | Build Recipes implementado (pkg/build.rs: BuildRecipe struct, SourceUrl, BuildOptions, BuildEnvironment, RecipeParser, PKGBUILD-style syntax, download/extract/prepare/build/check/package steps, checksum verification) |
| 2026-01-16 | Package Repository já existia (repository.rs: Repository, RepoManager, sync_all, download_package, search_packages) |
| 2026-01-16 | Unit Tests Framework implementado (tests/mod.rs: TestRunner, TestResult, TestOutcome, TestStats, assertion macros test_assert/test_assert_eq/test_assert_ne/test_assert_some/test_assert_ok/test_assert_err, register_tests macro, run_all_tests; tests/memory.rs heap allocation tests; tests/scheduler.rs task tests; tests/filesystem.rs path normalization tests; tests/network.rs IP/MAC/URL parsing, checksum tests; tests/syscall.rs syscall number/errno tests) |
| 2026-01-16 | Integration Tests implementado (tests/integration.rs: VFS+tmpfs interaction, memory allocation integration, process+memory integration, scheduler+timer integration, signal delivery integration, pipe communication tests) |
| 2026-01-16 | Stress Tests implementado (tests/stress.rs: memory_pressure test, allocation_fragmentation test, rapid_alloc_free test, concurrent_data_structures test with BTreeMap/Vec, longer timeouts) |
| 2026-01-16 | Hardware Tests implementado (tests/hardware.rs: CPU features detection, memory regions configuration, paging structures calculations, serial port I/O concepts, timer functionality with PIT/TSC) |
| 2026-01-16 | CI/CD e Automated Testing implementado (tests/automation.rs: AutomatedTestRunner, TestFilter, CiConfig, ReportFormat Text/JSON/JUnit/TAP, generate_report functions, run_ci_tests, exit_qemu for CI integration) |
| 2026-01-16 | User Manual/Developer Docs/API Reference/man Pages implementado (help/mod.rs: HelpSystem, HelpEntry, HelpCategory, format_entry, get_overview, search; help/commands.rs: shell commands help (ls, cd, mkdir, rm, cp, mv, cat, ps, kill, etc.); help/manual.rs: getting-started, filesystem, desktop, network, security, packages, troubleshooting guides; help/topics.rs: processes, shell, shortcuts, config files, admin, FAQ) |
| 2026-01-16 | Compatibility Layer implementado (compat/mod.rs: CompatLevel Full/Partial/Stub/None, FeatureStatus, has_feature; compat/posix.rs: 65+ POSIX features tracked - fork, exec, wait, file ops, pipes, signals, sockets, pthreads, mmap, etc.; compat/linux.rs: 60+ Linux syscalls tracked - clone, futex, epoll, openat family, etc.; compat/coreutils.rs: 80+ coreutils/busybox applets tracked - ls, cp, mv, grep, tar, etc.) |
| 2026-01-16 | Windows Compatibility Layer iniciado (compat/windows/ module com pe.rs, loader.rs, ntdll.rs, kernel32.rs, registry.rs, fs_translate.rs) |
| 2026-01-16 | PE/COFF Loader implementado (DOS header, PE signature, COFF header, Optional header PE32/PE32+, section headers, data directories, import/export tables parsing, base relocations) |
| 2026-01-16 | PE Parser implementado (parse_imports, parse_exports, RVA to file offset conversion, string reading at RVA) |
| 2026-01-16 | Import Table Resolution implementado (ImportDescriptor parsing, 32/64-bit thunks, import by name/ordinal) |
| 2026-01-16 | NTDLL emulação básica (NtTerminateProcess, NtClose, NtCreateFile, NtReadFile, NtWriteFile, NtAllocateVirtualMemory, NtFreeVirtualMemory, NtCreateEvent, NtSetEvent, NtWaitForSingleObject, NtQuerySystemInformation, NtQueryPerformanceCounter, NtDelayExecution) |
| 2026-01-16 | KERNEL32 emulação básica (GetStdHandle, WriteConsoleA, CreateFileA, CloseHandle, VirtualAlloc, VirtualFree, GetProcessHeap, HeapAlloc, HeapFree, GetCurrentProcess/ProcessId/Thread/ThreadId, ExitProcess, Sleep, TlsAlloc/Free, GetEnvironmentVariableA, SetEnvironmentVariableA, GetModuleHandleA, LoadLibraryA, GetProcAddress, GetSystemInfo, GetTickCount, QueryPerformanceCounter) |
| 2026-01-16 | Registry Emulation implementado (HKEY_CLASSES_ROOT/CURRENT_USER/LOCAL_MACHINE/USERS/CURRENT_CONFIG roots, RegistryKey/RegistryValue, REG_SZ/DWORD/QWORD/BINARY types, open/create/close key, query/set/delete value, enumerate subkeys/values, default registry entries for Windows NT/CurrentVersion) |
| 2026-01-16 | Windows Filesystem Translation implementado (drive mappings C:/Z:, windows_to_unix/unix_to_windows path conversion, UNC paths, special folders, normalize_windows_path, is_absolute, drive types, volume info) |
| 2026-01-16 | MSVCRT emulação implementada (~550 linhas, stdio FILE struct, fopen/fclose/fread/fwrite/fseek/ftell/fflush/fprintf/printf/sprintf/sscanf, malloc/free/calloc/realloc, string funcs strlen/strcpy/strcat/strcmp/strchr/strstr/memset/memcpy/memmove, char funcs isalpha/isdigit/isspace/toupper/tolower, stdlib atoi/atof/strtol/strtoul/abs/rand/srand, time localtime/gmtime/time) |
| 2026-01-16 | USER32 emulação implementada (~966 linhas, window management RegisterClass/CreateWindowEx/DestroyWindow/ShowWindow/UpdateWindow/MoveWindow/SetWindowText, message queue PostMessage/SendMessage/GetMessage/TranslateMessage/DispatchMessage/PeekMessage, input handling GetKeyState/GetAsyncKeyState/SetCapture/ReleaseCapture/GetCursorPos/SetCursorPos, dialog MessageBoxA/MessageBoxEx, timer SetTimer/KillTimer, clipboard OpenClipboard/CloseClipboard/GetClipboardData/SetClipboardData, window styles ws::OVERLAPPED/POPUP/CHILD/CAPTION/SYSMENU/etc, messages wm::CREATE/DESTROY/PAINT/CLOSE/KEYDOWN/MOUSEMOVE/etc, virtual keys VK_LBUTTON através VK_OEM_CLEAR) |
| 2026-01-16 | GDI32 emulação implementada (~850 linhas, device context CreateDC/DeleteDC/GetDC/ReleaseDC/SaveDC/RestoreDC, GDI objects CreatePen/CreateSolidBrush/CreateFontIndirect/CreateBitmap/SelectObject/DeleteObject/GetStockObject, drawing MoveTo/LineTo/Rectangle/Ellipse/Polygon/FillRect/FrameRect/SetPixel/GetPixel, bitmap BitBlt/StretchBlt/PatBlt, text TextOut/ExtTextOut/GetTextExtentPoint32/GetTextMetrics/SetTextColor/SetBkColor/SetTextAlign, stock objects WHITE_BRUSH/BLACK_PEN/SYSTEM_FONT/etc, color RGB macro, pen/brush styles ps::SOLID/DASH/DOT bs::SOLID/NULL/HATCHED) |
| 2026-01-16 | Linux Compatibility Layer implementada (compat/linux/ module: mod.rs, elf_dyn.rs, ldso.rs, dlfcn.rs, glibc.rs) |
| 2026-01-16 | ELF Dynamic Section Parser implementado (elf_dyn.rs: Elf64Dyn, DynamicInfo parsing DT_STRTAB/SYMTAB/HASH/GNU_HASH/RELA/JMPREL/INIT/FINI/NEEDED/etc., Elf64Sym symbol table entries, Elf64Rela relocations, x86_64 relocation types R_X86_64_RELATIVE/GLOB_DAT/JUMP_SLOT/etc., ELF/GNU hash functions) |
| 2026-01-16 | LD.so Dynamic Linker implementado (ldso.rs: DynamicLinker struct, SharedObject with base/end/dyn_info/symbols/strtab, load/unload library, set_dynamic_info/set_symbols, apply_relocations for R_X86_64_RELATIVE/GLOB_DAT/JUMP_SLOT/R64/COPY/IRELATIVE, lookup_symbol, get_needed, get_init_functions/get_fini_functions, LIBRARY_PATHS /lib:/lib64:/usr/lib:/usr/lib64, get_interpreter from PT_INTERP) |
| 2026-01-16 | dlfcn implementado (dlfcn.rs: dlopen with RTLD_LAZY/NOW/NOLOAD/NODELETE/GLOBAL/LOCAL, dlsym with RTLD_NEXT/DEFAULT, dlclose, dlerror, dladdr returning DlInfo with dli_fname/fbase/sname/saddr, dlinfo) |
| 2026-01-16 | glibc Compatibility implementada (glibc.rs: __libc_start_main args, __cxa_atexit/__cxa_finalize, __errno_location with errno constants, __stack_chk_guard/__stack_chk_fail, pthread stubs mutex_init/lock/unlock/destroy/self_/once/key_create/getspecific/setspecific, locale setlocale with LC_* constants, getauxval, sysconf, TLS support stubs, GlibcVersion 2.31.0 emulation) |
| 2026-01-16 | Audio Subsystem implementado (drivers/audio/mod.rs: AudioSystem, AudioDevice trait, AudioConfig, SampleFormat U8/S16LE/S24LE/S32LE/F32LE, StreamDirection Playback/Capture, StreamState Stopped/Running/Paused/Draining, AudioCapabilities, open_playback/capture, start/stop/close stream, read/write, volume/mute control) |
| 2026-01-16 | Intel HDA Driver implementado (drivers/audio/hda.rs: HdaController, MMIO register access, CORB/RIRB command ring buffers, codec enumeration via PCI scan class 0x04:0x03, widget tree parsing DAC/ADC/PIN_COMPLEX, verb commands GET_PARAM/SET_AMP_GAIN/SET_PIN_CTRL/SET_CONV_FORMAT, stream descriptors, BDL buffer descriptor list, format configuration 8kHz-192kHz sample rates, S16/S24/S32 bit depths, volume/mute control via amp gain) |
| 2026-01-16 | ALSA-like API implementada (drivers/audio/alsa.rs: SndPcm with open/close/hw_params/sw_params/prepare/start/drop/drain/pause/writei/readi/avail/wait/recover/status, SndPcmStream Playback/Capture, SndPcmFormat S8/U8/S16LE/S24LE/S32LE/FloatLE/etc., SndPcmState Open/Setup/Prepared/Running/Xrun/Draining/Paused/Suspended/Disconnected, SndPcmHwParams access/format/rate/channels/period_size/periods/buffer_size, SndMixer with elements Master/PCM/Speaker/Headphone/Mic/Capture, volume control db_to_linear/linear_to_db, error codes EPIPE/EAGAIN/ESTRPIPE/etc., strerror) |
| 2026-01-17 | musl Compatibility implementada (compat/linux/musl.rs: MuslTls thread-local storage, musl_libc_start_main, musl_errno_location, pthread stubs mutex_init/lock/unlock/trylock/destroy/once/key_create/getspecific/setspecific/cond_init/wait/signal/broadcast, musl_malloc/free/calloc/realloc, string funcs strlen/strcpy/strcmp/memset/memcpy/memmove, locale setlocale/uselocale, environ getenv, auxv getauxval AT_PAGESZ/CLKTCK/UID/GID/HWCAP, atexit/__cxa_atexit, exit/_Exit, syscall wrapper ~650 lines) |
| 2026-01-17 | GCC/Clang Native Compilers implementado (compat/linux/compilers.rs: TARGET_TRIPLE x86_64-stenzel-elf, CompilerType Gcc/Clang/Rustc/Nasm/As/Ld/Ar, ToolchainConfig with include_paths/library_paths/defines, LinkerConfig with library_paths/default_libs/entry_point/dynamic_linker, AssemblerConfig gas/nasm, TargetSpecs x86_64-linux, CrtFiles crt1/crti/crtn/crtbegin/crtend, standards C89-C23/C++98-C++23, optimization O0-O3/Os/Oz/Og/Ofast, warnings/debug flags, CompilerEnv cc/cxx/ar/ld/as/nm/objcopy/objdump/strip/ranlib ~500 lines) |
| 2026-01-17 | Python Runtime Support implementado (compat/linux/python.rs: PythonConfig for versions 3.8-3.12, sys.path generation, site-packages paths, PYTHONHOME/PYTHONPATH env vars, ModuleType Source/Bytecode/Extension/Package, 45+ BUILTIN_MODULES, 80+ STDLIB_MODULES, PipConfig with index_url/cache_dir/install commands, VenvManager create/list/remove venv, PycHeader parsing PEP 552, magic numbers 3.8-3.12, C extension ABI tags cpython-3xx-x86_64-linux-gnu, manylinux platform tag, PyException types, shebang parsing ~550 lines) |
| 2026-01-17 | Rust Runtime Support implementado (compat/linux/rust.rs: RustChannel Stable/Beta/Nightly, RustTarget x86_64-unknown-linux-gnu/musl/stenzel-elf, RustToolchain rustc/cargo/rustdoc/rustfmt/clippy paths, CargoConfig cargo_home/registry_url/target_dir, CargoManifest name/version/edition/dependencies, DependencySpec version/git/path/features, RustupManager toolchains/default/list/add/remove, CrateType Bin/Lib/Rlib/Dylib/Cdylib/Staticlib/ProcMacro, BuildProfile debug/release/release-small with opt-level/lto/panic, 30+ COMMON_CRATES, 10+ BUILTIN_TARGETS ~550 lines) |
| 2026-01-17 | ADVAPI32 emulação implementada (compat/windows/advapi32.rs: Registry funcs RegOpenKeyExA/RegCreateKeyExA/RegCloseKey/RegQueryValueExA/RegSetValueExA/RegDeleteValueA/RegEnumKeyExA/RegEnumValueA, Security funcs OpenProcessToken/GetTokenInformation/AdjustTokenPrivileges/LookupPrivilegeValueA/GetUserNameA, Crypto funcs CryptAcquireContext/CryptReleaseContext/CryptGenRandom, Event logging RegisterEventSourceA/DeregisterEventSource/ReportEventA, Service Control Manager OpenSCManagerA/OpenServiceA/CloseServiceHandle/StartServiceA/ControlService/QueryServiceStatus, error codes ERROR_SUCCESS/FILE_NOT_FOUND/ACCESS_DENIED/MORE_DATA ~565 lines) |
| 2026-01-17 | Swap to Disk implementado (mm/swap.rs: SwapSubsystem, SwapDevice partition/file types, SwapSlot bitmap allocator, SwapEntry/SwapEntryFlags, PageId process+vaddr tracking, swapon_partition/swapon_file, swapoff, swap_out_page/swap_in_page, fork_swap_entries COW, cleanup_process_swap, PageReclaimer kswapd-like background reclaim, mkswap file creation, SwapHeader Linux-compatible format, SWAP_MAGIC "SWAPSPACE2", /proc/swaps info, statistics ~850 lines) |
| 2026-01-17 | MSI/MSI-X implementado (drivers/msi.rs: MsiSubsystem, MsiCapability/MsixCapability detection, vector allocation bitmap, MSI message address/data format x86_64, configure_msi/configure_msix_entry, enable_msi/enable_msix/disable_msi/disable_msix, mask/unmask_msix_entry, setup_interrupts auto MSI vs MSI-X, vector range 32-223, print_msi_info, PCI config space capability parsing ~600 lines) |
| 2026-01-17 | NMI Handling implementado (arch/x86_64_arch/nmi.rs: NmiReason enum MemoryParity/IoCheck/Watchdog/PerformanceCounter/Unknown, get_nmi_reason system control port B reading, handle_memory_error/handle_io_error/handle_watchdog/handle_perf_counter, send_nmi_to_all_cpus for panic notification, NmiStats statistics, enable/disable_nmi via CMOS port 0x70, reenable_nmi, PANIC_NMI_ACTIVE flag, NMI IST stack in gdt.rs, nmi_handler in interrupts.rs, send_nmi_all_excluding_self in apic.rs ~330 lines) |
| 2026-01-17 | Directory Entries Cache implementado (fs/dentry.rs: DentryCache com hash table lookup, LRU eviction, negative entries caching "not found", DentryCacheStats, Dentry struct parent_ino/name/inode/cached_at/access_count, PathCache para full path resolution, invalidate_dir/invalidate_all, shrink/prune_expired, tick() para TTL, format_stats para /proc, FNV-1a hash ~580 lines) |
| 2026-01-17 | Inode Cache implementado (fs/inode_cache.rs: InodeCache com hash table lookup por (device, ino), LRU eviction, reference counting, dirty inode tracking, write-back support, InodeCacheStats, CachedInode struct key/inode/ref_count/flags/last_access/open_count, DeviceInodeTable per-device inode allocator, invalidate_device/invalidate_all, sync_all/writeback_some, shrink_to, format_stats para /proc ~730 lines) |
| 2026-01-17 | exFAT implementado (fs/exfat.rs: ExfatFs mount read-only, ExfatBootSector parsing, ExfatBootInfo bytes/sectors/clusters, ExfatInode cluster reading com FAT chain ou contiguous, ExfatFileEntry/StreamEntry/NameEntry parsing, directory entry sets, UTF-16 long file names, case-insensitive lookup, is_exfat detection, ~520 lines) |
| 2026-01-17 | Semaphores POSIX implementados (sync.rs: Semaphore struct com AtomicU32 value, wait/try_wait/timed_wait/post, SemError enum, SEM_VALUE_MAX, NamedSemaphore com open/close/unlink usando global BTreeMap, BinarySemaphore type alias, init_semaphores, ~280 lines) |
| 2026-01-17 | signalfd implementado (signal.rs: SignalFd struct com signal mask, queue VecDeque, flags SFD_NONBLOCK/SFD_CLOEXEC, SignalfdSiginfo struct 128 bytes Linux-compatible, queue_signal/read/is_readable, register_signalfd/unregister_signalfd global registry, deliver_to_signalfds, sys_signalfd, init_signalfd, ~300 lines) |
| 2026-01-17 | TCP Congestion Control implementado (net/tcp.rs: CongestionControl struct cwnd/ssthresh/mss/srtt/rttvar/rto, CongestionState enum SlowStart/CongestionAvoidance/FastRecovery, CongestionAlgorithm enum Reno/NewReno/Cubic, on_ack/on_dup_ack/on_timeout, RFC 6298 RTT estimation, fast retransmit/recovery, CUBIC algorithm with cbrt_approx Newton's method, CongestionStats, ~300 lines) |
| 2026-01-17 | IPv6 implementado (net/ipv6.rs: Ipv6Addr 128-bit com is_unspecified/loopback/multicast/link_local/global/unique_local/to_ipv4_mapped/solicited_node, Ipv6Header parsing/building next_header/hop_limit/traffic_class/flow_label, Icmpv6Header EchoRequest/Reply/NeighborSolicitation/NeighborAdvertisement/RouterSolicitation/RouterAdvertisement, NeighborEntry cache com state Incomplete/Reachable/Stale/Delay/Probe, NdpState com neighbor_table, send_neighbor_solicitation/advertisement, handle_neighbor_solicitation/advertisement, send_echo_request/reply, Ipv6Config enabled/addr/prefix_len/gateway, ~700 lines) |
| 2026-01-17 | CFS-like Scheduler implementado (sched/cfs.rs + sched/mod.rs: SchedEntity struct vruntime/weight/inv_weight/exec_start/sum_exec_runtime/time_slice, CfsRunqueue com sorted task list by vruntime, CfsEntry struct task_id/vruntime/weight, SCHED_PRIO_TO_WEIGHT/WMULT tables from Linux, weight_from_nice/wmult_from_nice, calc_time_slice based on latency, calc_delta_vruntime, place_entity for new tasks, check_preempt_tick, CfsStats, Task updated with sched_entity field, on_timer_tick uses CFS algorithm, ~450 lines cfs.rs + mod.rs updates) |
| 2026-01-17 | EHCI (USB 2.0) implementado (drivers/usb/ehci.rs: EhciController com capability/operational registers, QueueHead 32-byte aligned for async schedule, TransferDescriptor qTD for transfers, periodic/async schedules, USBCMD/USBSTS/PORTSC register access, port reset/enumeration, control_transfer_in/out com setup/data/status stages, EhciDevice tracking, PCI scan for class 0x0C subclass 0x03 prog_if 0x20, ~950 lines) |
| 2026-01-17 | Audio Mixer implementado (drivers/audio/mixer.rs: MixerChannel struct com volume/pan/mute/solo/EQ 10-band, ChannelType enum Master/Pcm/Mic/LineIn/Aux/System, AudioMixer com create_channel/remove_channel, mix() blends all channels with volume/pan/mute/solo, EqBand struct freq/gain/q, apply_eq processing, MixerPreset enum Flat/Bass/Treble/Vocal/Custom, snapshot/restore state, MixerStats, GLOBAL_MIXER lazy_static, with_mixer closure pattern, ~730 lines) |
| 2026-01-17 | NTFS read support implementado (fs/ntfs.rs: NtfsFs mount read-only, NtfsBootSector parsing OEM ID validation, NtfsVolumeInfo bytes/cluster/sector/mft_lcn, MftRecordHeader parsing with signature/usa_offset/flags, AttrRecordHeader resident/non-resident attributes, parse_data_runs runlist decoding signed VCN/LCN deltas, apply_fixup update sequence array, AT_DATA/AT_FILE_NAME/AT_INDEX_ROOT/AT_INDEX_ALLOCATION attributes, NtfsFileInfo from index entries, NtfsInode read_file_data/parse_directory, IndexEntry parsing B+ tree $I30 filename index, UTF-16 file names, case-insensitive lookup, is_ntfs detection, mount_auto filesystem auto-detection, ~1100 lines) |
| 2026-01-17 | Node.js support implementado (compat/linux/nodejs.rs: NodeConfig struct version/executable/npm_executable/npx_executable/global_modules, PackageJson parsing name/version/main/module/type/scripts/dependencies/bin, PackageType enum CommonJS/Module, ModuleResolver com search_paths/cache, resolve() Node.js algorithm para relative/absolute/core/package, CORE_MODULES list ~50 modules, NpmConfig registry/cache/prefix/npmrc parsing, NodeEnv NODE_ENV/NODE_PATH/NODE_OPTIONS, NodeManager global state config/npm_config/global_packages/nvm_versions, nvm integration for_nvm(), is_core_module(), get_nodejs_status() feature list, ~850 lines) |
| 2026-01-17 | ISO9660 filesystem implementado (fs/iso9660.rs: Iso9660Fs mount read-only, PrimaryVolumeDescriptor parsing at sector 16, VD_STANDARD_ID "CD001" validation, IsoVolumeInfo volume_id/block_size/root_location, DirectoryRecord parsing flags/extent_location/data_length/file_id, IsoDirectoryEntry com name/location/data_length/is_directory, Rock Ridge extension detection SP/RR/NM signatures, Joliet Unicode extension detection, parse_rock_ridge_name() alternate names, IsoInode read_directory/read_file_data, case-insensitive lookup, is_iso9660 detection, mount_auto integration, ~650 lines) |
| 2026-01-17 | Real-time Scheduling implementado (sched/rt.rs: SchedPolicy enum Other/Fifo/RoundRobin/Batch/Idle/Deadline, SchedParam struct sched_priority, SchedAttr extended attributes, RtEntry struct task_id/priority/policy/time_slice, RtRunqueue com 99 priority queues, active_bitmap para fast lookup, enqueue/dequeue/pick_next/requeue_rr, should_preempt check, RtStats context_switches/fifo_scheduled/rr_scheduled, RtEntity per-task RT state policy/priority/time_slice/on_rq, RT_PRIO_MIN=1/RT_PRIO_MAX=99, SCHED_RR_TIMESLICE default 10 ticks, effective_priority(), is_valid_priority(), Task updated with rt_entity field, ~550 lines) |
| 2026-01-17 | Load Balancing implementado (sched/balance.rs: CpuLoad per-CPU load statistics current/avg/nr_running/runtime_ns/samples, LoadBalancer com cpu_loads[MAX_CPUS]/nr_cpus/last_balance/enabled/migrations queue, MigrationRequest task_id/from_cpu/to_cpu/weight, balance() periodic load balancing IMBALANCE_THRESHOLD 25%, find_idlest_cpu/find_busiest_cpu for work stealing, LoadSummary total_load/avg_load/min/max/imbalance, CpuMask 256-bit affinity mask with set/clear/is_set/first/next/and/or/from_cpu_set/to_bytes, LOAD_BALANCER global static, init() nr_cpus setup, ~650 lines) |
| 2026-01-17 | CPU Affinity implementado (sched/mod.rs: cpu_allowed field UnsafeCell<CpuMask> no Task struct, cpu_allowed()/set_cpu_allowed()/can_run_on_cpu() accessors, set_task_affinity/get_task_affinity funções, current_affinity/set_current_affinity, sys_sched_setaffinity/sys_sched_getaffinity syscalls 203/204, CpuMask::from_cpu_set/to_bytes para conversão com Linux cpu_set_t, validação de buffer userspace, pid==0 para current task) |
| 2026-01-17 | Message Queues implementado (ipc/msg.rs: MsgId/IpcKey types, MsgPerm struct com can_read/can_write, Message struct mtype/data, MsqIdDs status struct, MessageQueue com VecDeque<Message>/total_bytes/marked_for_removal, send/receive operations com msgtyp selection (0/>0/<0), MsgQueueTable com key_to_id map, sys_msgget/sys_msgsnd/sys_msgrcv/sys_msgctl syscalls 68-71, IPC_CREAT/EXCL/NOWAIT/PRIVATE flags, MSG_NOERROR/EXCEPT/COPY receive flags, MSGMAX/MSGMNB/MSGMNI limits, ~530 lines) |
| 2026-01-17 | Interrupt Affinity implementado (arch/x86_64_arch/apic.rs: IrqAffinity struct com irq/cpu/enabled/count, IRQ_AFFINITY tracking table MAX_IRQS=24 entries, IRQ_COUNT_PER_CPU statistics para até 256 CPUs, init_irq_affinity/set_irq_affinity/get_irq_affinity funções, enable_irq_with_affinity/disable_irq, record_irq_handled para estatísticas, get_cpu_irq_count/get_irq_count, balance_irqs round-robin, balance_irqs_by_load para balanceamento baseado em carga, get_irq_summary para /proc/interrupts, ~240 lines) |
| 2026-01-17 | NUMA Support implementado (mm/numa.rs: NumaNode struct com id/present/cpus/mem_ranges/total_memory/free_memory AtomicU64, NumaMemRange struct, NumaTopology com nodes vector/cpu_to_node mapping, SRAT table parsing SratHeader/SratProcessorAffinity/SratMemoryAffinity structs, parse_srat(), init() invocado do ACPI, is_numa_available/num_nodes/cpu_to_node funções públicas, node_memory/node_free_memory/node_cpus/node_mem_ranges getters, record_alloc/record_free para tracking, addr_to_node lookup, find_freest_node seleção, NumaPolicy enum Local/Preferred/Interleave/Bind, select_node() policy-based allocation, ~440 lines) |
| 2026-01-17 | Huge Pages implementado (mm/huge_pages.rs: HugePageSize enum 2MiB/1GiB, HugePagePool pre-allocation pool com take_2mb/take_1gb/return_2mb/return_1gb, supports_2mb/supports_1gb CPUID checks, alloc_2mb/alloc_1gb/free_2mb/free_1gb allocation funções, map_huge_2mb/map_huge_1gb/unmap_huge_2mb/unmap_huge_1gb mapping funções, HugePageStats statistics, ThpPolicy enum Never/Always/Madvise Transparent Huge Pages, align helpers, format_meminfo para /proc; mm/phys.rs: allocate_huge_2mb/allocate_huge_1gb/deallocate_huge_2mb/deallocate_huge_1gb/count_huge_2mb_available/count_huge_1gb_available, FrameAllocator<Size2MiB>/FrameAllocator<Size1GiB> impls; mm/paging.rs: map_huge_2mb/map_huge_1gb/unmap_huge_2mb/unmap_huge_1gb, flags_kernel_huge_rw/flags_user_huge_rw, ~700 lines total) |
| 2026-01-17 | NTP Client implementado (net/ntp.rs: NtpTimestamp struct 64-bit seconds+fraction com to_unix_secs/from_unix_secs/now, NtpPacket struct 48 bytes com LI/VN/Mode/stratum/poll/precision/timestamps, client_request/parse/to_bytes, LeapIndicator/NtpMode/Stratum enums, NtpResult com server/offset_ms/delay_ms/stratum/reference_id, NtpClient struct com servers/last_sync/stats, query_server UDP port 123 com T1/T2/T3/T4 algorithm, sync() multi-server selection lowest delay, apply_offset time correction, query_hostname DNS resolution, periodic_sync background task, NtpStats/format_info, ~650 lines) |
| 2026-01-17 | Linux Capabilities implementado (security/caps.rs: CapSet bitflags 41 capabilities Linux-compatible CAP_CHOWN/DAC_OVERRIDE/KILL/SETUID/NET_BIND_SERVICE/SYS_ADMIN/etc., Cap enum com from_number/to_set/name, ProcessCaps struct effective/permitted/inheritable/bounding/ambient sets, has_effective/has_permitted/raise/drop/drop_permitted/drop_bounding, set_inheritable/set_ambient, transform_for_exec(), FileCaps struct com from_xattr/to_xattr VFS_CAP_REVISION_2/3, CapUserHeader/CapUserData structs, sys_capget/sys_capset syscall implementations, capable()/capable_net() check helpers, cap_from_name lookup, format_caps string, ~750 lines; sched/mod.rs: process_caps field no Task struct, caps()/set_caps() methods) |
| 2026-01-17 | Seccomp implementado (security/seccomp.rs: SeccompMode enum Disabled/Strict/Filter, SeccompAction enum Kill/KillThread/Trap/Errno/Trace/Log/Allow com to_raw/from_raw, SeccompData struct nr/arch/instruction_pointer/args para BPF, BpfInsn struct code/jt/jf/k com BPF_* constants, SeccompFilter builder com allow/deny/add_rule/build() BPF program, SeccompState struct mode/filter/flags com enable_strict/set_filter/check_syscall, SimpleFilter rule-based alternative com allow/deny/error/check, sys_seccomp syscall SECCOMP_SET_MODE_STRICT/FILTER/GET_ACTION_AVAIL, minimal_filter/standard_filter presets, ~550 lines; sched/mod.rs: seccomp field, seccomp_state()/set_seccomp_state() methods) |
| 2026-01-17 | Asymmetric Encryption completo (crypto/ed25519.rs: Ed25519 digital signatures RFC 8032, Fe field element 5x51-bit limbs, Point extended coordinates, Scalar mod L arithmetic, sha512 hash, Keypair struct from_seed/generate/sign, verify/sign/public_key_from_secret exports, ~850 lines; crypto/rsa.rs: RSA PKCS#1 v1.5 encryption/signatures, BigUint arbitrary precision arithmetic add/sub/mul/div_rem/mod_pow/mod_inverse/extended_gcd, RsaPublicKey encrypt/verify DER parsing, RsaPrivateKey decrypt/sign CRT optimization, generate_keypair Miller-Rabin primality, Barrett reduction, 1024-4096 bit keys, ~700 lines; crypto/mod.rs: re-exports Ed25519Keypair/ed25519_sign/ed25519_verify/ed25519_public_key/RsaPublicKey/RsaPrivateKey/rsa_generate_keypair) |
| 2026-01-17 | USB Enumeration completo (drivers/usb/devices.rs: UsbDeviceManager unified device registry, UsbDeviceInfo full device information, UsbDeviceId bus/address identifiers, UsbInterface/UsbConfiguration descriptors, UsbDeviceState Attached/Addressed/Configured/Suspended, ControllerType xHCI/EHCI/UHCI/OHCI, register_device/unregister_device, list_devices/find_by_class/find_by_ids/find_by_interface_class queries, UsbEvent callback system Attached/Detached/Configured/Suspended/Resumed, UsbStats transfer statistics, format_all_devices lsusb-style output, format_device_info detailed device info, create_device_info helper from DeviceDescriptor, add_configuration/set_device_strings/bind_interface_driver device building, ~600 lines; drivers/usb/mod.rs: public re-exports for unified API) |
| 2026-01-17 | Linux ABI completo (compat/linux/ldso.rs: Full x86_64 relocation support: R_X86_64_NONE/RELATIVE/GLOB_DAT/JUMP_SLOT/64/PC32/PLT32/32/32S/PC64/GOTPCREL/COPY/IRELATIVE; TLS relocations: DTPMOD64/DTPOFF64/TPOFF64/TPOFF32/GOTTPOFF; resolve_symbol_reloc helper, find_symbol_in_others for cross-object lookup, COPY relocation data copying, IRELATIVE resolver calling; compat/linux/mod.rs: CompatLevel::Full for ELF Relocations) |

---

## Notas e Decisões de Design

### Decisões Arquiteturais
1. **Linguagem:** Rust (kernel e userspace)
2. **Arquitetura alvo:** x86_64 (inicialmente)
3. **Bootloader:** bootloader crate (Rust)
4. **Modelo de memória:** Higher-half kernel
5. **Scheduler:** Preemptivo, CFS-like (virtual runtime based)

### Inspirações
- Linux (syscalls, drivers)
- SerenityOS (abordagem de desenvolvimento)
- Redox OS (Rust, design moderno)
- Plan 9 (simplicidade)

### Links Úteis
- [OSDev Wiki](https://wiki.osdev.org/)
- [Intel SDM](https://software.intel.com/content/www/us/en/develop/articles/intel-sdm.html)
- [Linux Source](https://github.com/torvalds/linux)
- [SerenityOS](https://github.com/SerenityOS/serenity)
- [Redox OS](https://gitlab.redox-os.org/redox-os/redox)

---

## Como Contribuir

1. Escolha um item pendente (⬜) de alta prioridade
2. Crie uma branch: `feature/nome-do-item`
3. Implemente e teste
4. Atualize este documento marcando como ✅ ou 🔄
5. Faça um PR

**Legenda:**
- ⬜ Pendente
- 🔄 Em progresso / Parcial
- ✅ Concluído
- ❌ Cancelado / Não aplicável

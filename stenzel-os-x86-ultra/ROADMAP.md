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
| Boot Logo | Exibir logo durante boot | ✅ Concluído | Baixa |

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
| Device Tree | Árvore de dispositivos | ✅ Feito | Média |

### 4.2 Storage
> Drivers de armazenamento.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| IDE/ATA | Discos IDE antigos | ✅ Concluído | Baixa |
| AHCI (SATA) | Discos SATA modernos | ✅ Feito | Alta |
| NVMe | SSDs NVMe | ✅ Feito | Alta |
| VirtIO-blk | Discos virtuais (QEMU) | ✅ Feito | Alta |
| USB Mass Storage | Pendrives, HDs externos | ✅ Feito | Alta |
| SD/MMC | Cartões SD | ✅ Feito | Média |
| RAID (software) | RAID por software | ✅ Concluído | Baixa |

### 4.3 Input
> Dispositivos de entrada.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PS/2 Keyboard | Teclado PS/2 | ✅ Feito | Alta |
| PS/2 Mouse | Mouse PS/2 | ✅ Feito | Alta |
| USB Keyboard | Teclado USB (HID) | ✅ Feito | Alta |
| USB Mouse | Mouse USB (HID) | ✅ Feito | Alta |
| USB Touchpad | Touchpad USB | ✅ Feito | Média |
| Keyboard Layouts | Layouts (ABNT2, US, etc) | ✅ Feito | Alta |
| Input Event System | /dev/input/* | ✅ Feito | Alta |

### 4.4 Display
> Saída de vídeo.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| VGA Text Mode | Modo texto 80x25 | ✅ Feito | Alta |
| Linear Framebuffer | Framebuffer simples | ✅ Feito | Alta |
| VESA/VBE | Modos gráficos via BIOS | ✅ Feito | Média |
| GOP (UEFI) | Graphics Output Protocol | ✅ Feito | Alta |
| Mode Setting | Trocar resolução | ✅ Feito | Alta |
| Multi-Monitor | Suporte a múltiplos monitores | ✅ Concluído | Baixa |
| GPU Driver (Intel) | Driver Intel integrated | ✅ Feito | Média |
| GPU Driver (AMD) | Driver AMD (básico) | ✅ Concluído | Baixa |
| GPU Driver (NVIDIA) | Driver NVIDIA (básico) | ✅ Concluído | Baixa |

### 4.5 USB
> Suporte completo a USB.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| xHCI (USB 3.x) | Controller USB 3 | ✅ Feito | Alta |
| EHCI (USB 2.0) | Controller USB 2 | ✅ Feito | Média |
| OHCI/UHCI (USB 1.x) | Controllers legados | ✅ Concluído | Baixa |
| USB Hub Support | Suporte a hubs | ✅ Feito | Alta |
| USB HID | Human Interface Devices | ✅ Feito | Alta |
| USB Storage | Mass Storage Class | ✅ Feito | Alta |
| USB Audio | Audio Class | ✅ Concluído | Baixa |
| USB Video | Video Class (webcams) | ✅ Concluído | Baixa |

### 4.6 Áudio
> Subsistema de áudio.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PC Speaker | Beep básico | ✅ Concluído | Baixa |
| Intel HDA | High Definition Audio | ✅ Feito | Média |
| AC'97 | Codec legado | ✅ Concluído | Baixa |
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
| RTL8139 | Realtek 10/100 | ✅ Feito | Média |
| RTL8169 | Realtek Gigabit | ✅ Feito | Média |
| Intel I210/I211 | Intel moderno | ✅ Feito | Média |

### 5.3 WiFi
> Suporte a redes sem fio.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| 802.11 Stack | Protocolo WiFi básico | ✅ Feito | Alta |
| WPA/WPA2 | Autenticação WiFi | ✅ Feito | Alta |
| WPA3 | Autenticação moderna | ✅ Concluído | Baixa |
| WiFi Scanning | Escanear redes | ✅ Feito | Alta |
| WiFi Connection | Conectar a redes | ✅ Feito | Alta |
| Intel WiFi Driver | iwlwifi básico | ✅ Feito | Alta |
| Atheros Driver | ath9k/ath10k básico | ✅ Feito | Média |
| Broadcom Driver | brcmfmac básico | ✅ Feito | Média |
| Realtek WiFi | rtl8xxxu básico | ✅ Feito | Média |

### 5.4 Bluetooth
> Suporte Bluetooth.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Bluetooth HCI | Host Controller Interface | ✅ Concluído | Baixa |
| Bluetooth Pairing | Pareamento de dispositivos | ✅ Concluído | Baixa |
| Bluetooth Audio | A2DP | ✅ Concluído | Baixa |
| Bluetooth HID | Teclados/mouses BT | ✅ Concluído | Baixa |

### 5.5 Serviços de Rede
> Serviços essenciais de rede.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| HTTP Client | Fazer requests HTTP | ✅ Feito | Alta |
| HTTPS/TLS | Conexões seguras | ✅ Feito | Alta |
| SSH Client | Conexão SSH | ✅ Feito | Média |
| SSH Server | Servidor SSH | ✅ Concluído | Baixa |
| FTP Client | Cliente FTP | ✅ Concluído | Baixa |
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
| Transparency | Janelas transparentes | ✅ Concluído | Baixa |
| Animations | Animações de UI | ✅ Concluído | Baixa |
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
| Text Shaping | Shaping complexo | ✅ Concluído | Média |
| RTL Text | Texto direita-esquerda | ✅ Concluído | Baixa |

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
| Rollback | Desfazer instalação | ✅ Concluído | Média |

### 7.3 Build System
> Sistema de compilação de pacotes.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Build Recipes | Receitas de compilação | ✅ Feito | Alta |
| Source Packages | Pacotes fonte | ✅ Concluído | Média |
| Cross-compilation | Compilação cruzada | ✅ Concluído | Média |
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
| ACLs | Access Control Lists | ✅ Concluído | Baixa |
| Extended Attributes | xattr | ✅ Concluído | Baixa |

### 8.3 Capabilities e Sandboxing
> Isolamento e limitação de privilégios.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Capabilities | Linux-like capabilities | ✅ Concluído | Média |
| Seccomp | Filtro de syscalls | ✅ Concluído | Média |
| Namespaces | Isolamento de recursos | ✅ Feito | Média |
| Cgroups | Limites de recursos | ✅ Feito | Média |
| Containers | Suporte a containers | ✅ Concluído | Baixa |

### 8.4 Criptografia
> Suporte a criptografia.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Random Number Generator | /dev/random, /dev/urandom | ✅ Feito | Alta |
| Hash Functions | SHA-256, SHA-512, etc | ✅ Feito | Alta |
| Symmetric Encryption | AES, ChaCha20 | ✅ Feito | Alta |
| Asymmetric Encryption | RSA, Ed25519 | ✅ Feito | Alta |
| TLS Library | Implementação TLS | ✅ Feito | Alta |
| Disk Encryption | LUKS-like | ✅ Feito | Média |
| Keyring | Armazenamento de chaves | ✅ Feito | Média |

---

## Fase 9: Hardware Avançado

### 9.1 Power Management
> Gerenciamento de energia.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| ACPI Power States | S0-S5 states | ✅ Feito | Alta |
| Shutdown | Desligar corretamente | ✅ Feito | Alta |
| Reboot | Reiniciar | ✅ Feito | Alta |
| Suspend to RAM | Suspender (S3) | ✅ Feito | Média |
| Hibernate | Hibernar (S4) | ✅ Concluído | Baixa |
| CPU Frequency Scaling | Ajustar frequência | ✅ Feito | Média |
| Battery Monitoring | Monitorar bateria | ✅ Feito | Alta |
| Lid Switch | Detectar tampa fechada | ✅ Feito | Média |
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
| CPU Hotplug | Adicionar/remover CPUs | ✅ Concluído | Baixa |

### 9.3 Thermal Management
> Gerenciamento térmico.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Temperature Reading | Ler temperaturas | ✅ Feito | Média |
| Fan Control | Controlar ventoinhas | ✅ Feito | Média |
| Thermal Throttling | Throttle por temperatura | ✅ Feito | Média |
| Critical Temperature | Desligar em emergência | ✅ Feito | Alta |

### 9.4 Laptops e Notebooks
> Suporte específico para laptops.

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Backlight Control | Controle de brilho | ✅ Feito | Alta |
| Touchpad | Driver de touchpad | ✅ Feito | Alta |
| Function Keys | Teclas Fn | ✅ Feito | Média |
| Webcam | Suporte a webcam | ✅ Concluído | Baixa |
| Fingerprint | Leitor de digital | ✅ Concluído | Baixa |
| Thunderbolt | Suporte Thunderbolt | ✅ Concluído | Baixa |

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
| Docker | Suporte a containers Docker | ✅ Concluído | Média |

### 10.5 Compatibilidade Windows
> Executar aplicações Windows (Wine-like).

| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PE/COFF Loader | Carregar executáveis .exe | ✅ Feito | Alta |
| PE Parser | Parsear headers PE32/PE32+ | ✅ Feito | Alta |
| Import Table | Resolver imports de DLLs | ✅ Feito | Alta |
| Windows Syscalls | NT syscall translation | ✅ Feito | Alta |
| NTDLL | Implementar ntdll.dll | ✅ Feito | Alta |
| KERNEL32 | Implementar kernel32.dll | ✅ Feito | Alta |
| USER32 | Implementar user32.dll | ✅ Feito | Alta |
| GDI32 | Implementar gdi32.dll | ✅ Feito | Alta |
| ADVAPI32 | Implementar advapi32.dll | ✅ Feito | Alta |
| SHELL32 | Implementar shell32.dll | ✅ Feito | Média |
| COMCTL32 | Implementar comctl32.dll | ✅ Feito | Média |
| OLE32 | Implementar ole32.dll | ✅ Feito | Média |
| MSVCRT | Implementar msvcrt.dll | ✅ Feito | Alta |
| Registry | Emulação do registro Windows | ✅ Feito | Alta |
| Windows Filesystem | Emulação de caminhos (C:\) | ✅ Feito | Alta |
| COM/OLE | Component Object Model básico | ✅ Feito | Média |
| DirectX (basic) | Direct3D básico via OpenGL/Vulkan | ✅ Concluído | Baixa |
| .NET CLR | Common Language Runtime básico | ✅ Concluído | Baixa |

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
| 2026-01-17 | Broadcom WiFi driver implementado (BCM4313/4321/4331/4352/4360/43602/43684 support, D64 DMA descriptors, SPROM MAC reading, Silicon Backplane registers, brcm.rs ~700 lines) |
| 2026-01-17 | Realtek WiFi driver implementado (RTL8188CU/EU/RU, RTL8192CU/EU, RTL8723AU/BU, RTL8812AU, RTL8821AU, multi-vendor rebrand support, EFUSE MAC reading, Gen1/Gen2 power sequences, rtl8xxxu.rs ~900 lines) |
| 2026-01-17 | SSH Client implementado (RFC 4253 SSH-2, curve25519-sha256 key exchange, chacha20-poly1305@openssh.com encryption, password auth, channels, PTY, shell, exec, ssh.rs ~1000 lines) |
| 2026-01-17 | Suspend to RAM (S3) implementado (power.rs: PowerManaged trait, CPU state save/restore, driver suspend/resume callbacks, wake source detection, sysfs/procfs interfaces, power.rs ~600 lines) |
| 2026-01-17 | CPU Frequency Scaling implementado (cpufreq.rs: Intel EIST/SpeedStep, AMD Cool'n'Quiet, P-state management via MSRs, governors: performance/powersave/ondemand/userspace, Turbo Boost control, sysfs interface, ~650 lines) |
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
| 2026-01-17 | Windows Syscalls 100% completo (compat/windows/ntsyscall.rs: NtSyscallContext struct, dispatch_nt_syscall dispatcher 40+ syscalls, syscall_nr module com NT syscall numbers 0x00-0x2B, handle translation Windows→POSIX, file ops NtCreateFile/ReadFile/WriteFile/Close/QueryInformationFile/SetInformationFile/DeleteFile/CreateSection/MapViewOfSection, memory NtAllocateVirtualMemory/FreeVirtualMemory/ProtectVirtualMemory/QueryVirtualMemory, sync NtCreateEvent/SetEvent/ResetEvent/ClearEvent/WaitForSingleObject/WaitForMultipleObjects/CreateMutant/ReleaseMutant/CreateSemaphore/ReleaseSemaphore, process NtTerminateProcess/QueryInformationProcess/OpenProcess/CurrentProcess, thread NtTerminateThread/QueryInformationThread/GetContextThread/SetContextThread, registry NtOpenKey/CreateKey/QueryValueKey/SetValueKey/DeleteKey/DeleteValueKey, system NtQuerySystemInformation/QueryPerformanceCounter/DelayExecution ~1400 lines) |
| 2026-01-17 | NTDLL 100% completo (compat/windows/ntdll.rs: NtdllEmulator struct, NtSyscallContext integration, Nt* funcs ~50 exports conectados ao dispatcher, Rtl* funcs RtlInitUnicodeString/AnsiString/InitializeCriticalSection/EnterCriticalSection/LeaveCriticalSection/DeleteCriticalSection/CreateHeap/DestroyHeap/AllocateHeap/FreeHeap/CopyMemory/MoveMemory/ZeroMemory/FillMemory/CompareMemory/StringCchCopyW/StringCchCatW/IntegerToUnicodeString/UnicodeStringToInteger/GetNtVersionNumbers, Ldr* funcs LdrLoadDll/UnloadDll/GetProcedureAddress/GetDllHandle, Csr* funcs CsrClientCallServer/CaptureMessageBuffer, Dbg* funcs DbgPrint/DbgBreakPoint, ~1350 lines, 150+ function exports) |
| 2026-01-17 | KERNEL32 100% completo (compat/windows/kernel32.rs: Kernel32Emulator struct com file_handles/heap_id/tls_slots/critical_sections, Console GetStdHandle/WriteConsoleA/W/ReadConsoleA/SetConsoleCursorPosition/SetConsoleTextAttribute/GetConsoleScreenBufferInfo/AllocConsole/FreeConsole/SetConsoleTitle/GetConsoleCP/GetConsoleOutputCP, File CreateFileA/W/ReadFile/WriteFile/CloseHandle/SetFilePointer/SetFilePointerEx/GetFileSize/GetFileSizeEx/CreateDirectoryA/W/RemoveDirectoryA/W/DeleteFileA/W/GetFileAttributesA/W/SetFileAttributesA/W/GetFileType/FlushFileBuffers/GetFileInformationByHandle/FindFirstFileA/W/FindNextFileA/W/FindClose/GetFullPathNameA/W/GetTempPathA/W/GetTempFileNameA/W, Memory VirtualAlloc/VirtualFree/VirtualProtect/VirtualQuery/GetProcessHeap/HeapCreate/HeapDestroy/HeapAlloc/HeapReAlloc/HeapFree/HeapSize/GlobalAlloc/GlobalFree/GlobalLock/GlobalUnlock/LocalAlloc/LocalFree, Process GetCurrentProcess/ProcessId/CreateProcessA/W/ExitProcess/TerminateProcess/GetExitCodeProcess/OpenProcess, Thread GetCurrentThread/ThreadId/CreateThread/ExitThread/TerminateThread/ResumeThread/SuspendThread/GetExitCodeThread/Sleep/SleepEx/SwitchToThread, Sync WaitForSingleObject/WaitForMultipleObjects/CreateEventA/W/SetEvent/ResetEvent/CreateMutexA/W/ReleaseMutex/CreateSemaphoreA/W/ReleaseSemaphore/InitializeCriticalSection/EnterCriticalSection/TryEnterCriticalSection/LeaveCriticalSection/DeleteCriticalSection, TLS TlsAlloc/TlsFree/TlsGetValue/TlsSetValue, Module GetModuleHandleA/W/GetModuleFileNameA/W/LoadLibraryA/W/LoadLibraryExA/W/FreeLibrary/GetProcAddress, System GetSystemInfo/GetNativeSystemInfo/GetVersion/GetVersionExA/W/GetTickCount/GetTickCount64/QueryPerformanceCounter/QueryPerformanceFrequency/GetSystemTime/GetLocalTime/SetSystemTime/GetSystemTimeAsFileTime/GetTimeZoneInformation/GetComputerNameA/W/GetSystemDirectoryA/W/GetWindowsDirectoryA/W, Env GetEnvironmentVariableA/W/SetEnvironmentVariableA/W/GetEnvironmentStringsA/W/FreeEnvironmentStringsA/W/GetCommandLineA/W/GetCurrentDirectoryA/W/SetCurrentDirectoryA/W, Error GetLastError/SetLastError/FormatMessageA/W, ~2100 lines, 250+ function exports) |
| 2026-01-17 | SHELL32 100% completo (compat/windows/shell32.rs: Shell32Emulator struct, CSIDL special folders Desktop/Documents/AppData/ProgramFiles/Windows/System32/etc., SHGetSpecialFolderPath/SHGetFolderPath, SHFileOperation FO_COPY/MOVE/DELETE/RENAME com FOF_SILENT/NOCONFIRMATION/ALLOWUNDO flags, ShellExecute/ShellExecuteEx, ExtractIcon/ExtractIconEx, Shell_NotifyIcon NIM_ADD/MODIFY/DELETE sistema tray, ShellLinkData IShellLink shortcuts target/arguments/workingdir/description/icon/hotkey, DragAcceptFiles/DragQueryFile/DragFinish drag-drop, Path* functions 40+ PathAddBackslash/PathCombine/PathFindFileName/PathIsRelative/PathMatchSpec/etc., Str* functions StrFormatByteSize/StrToInt/StrTrim, SHBrowseForFolder/SHGetPathFromIDList, IL* ItemIDList functions ILClone/ILFree/ILCombine, SHEmptyRecycleBin/SHQueryRecycleBin, FindExecutable, CommandLineToArgvW, ShellAbout, ~1150 lines, 120+ function exports) |
| 2026-01-17 | ADVAPI32 get_exports adicionado (165+ exports: Registry funcs RegOpenKeyEx/CreateKeyEx/CloseKey/QueryValueEx/SetValueEx/DeleteValue/DeleteKey/EnumKeyEx/EnumValue/etc., Security funcs OpenProcessToken/GetTokenInformation/AdjustTokenPrivileges/LookupPrivilegeValue/GetUserName/AccessCheck/LogonUser/etc., Crypto funcs CryptAcquireContext/GenRandom/GenKey/Encrypt/Decrypt/CreateHash/SignHash/VerifySignature/etc., Event logging RegisterEventSource/ReportEvent/OpenEventLog/ReadEventLog/etc., SCM funcs OpenSCManager/OpenService/CreateService/StartService/ControlService/QueryServiceStatus/etc.) |
| 2026-01-17 | COMCTL32 100% completo (compat/windows/comctl32.rs: Comctl32Emulator struct, InitCommonControls/InitCommonControlsEx ICC flags, ListView LVS_*/LVM_*/LVIF_* styles/messages/flags, TreeView TVS_*/TVM_*/TVI_*/TVIF_* styles/messages/special handles/flags, TabControl TCS_*/TCM_* styles/messages, StatusBar SBARS_*/SB_* styles/messages, Toolbar TBSTYLE_*/TB_* styles/messages, ProgressBar PBS_*/PBM_* styles/messages, Trackbar TBS_*/TBM_* styles/messages, UpDown UDS_*/UDM_* styles/messages, ImageList ILC_*/ILD_* flags com Create/Destroy/Add/Remove/Replace/Duplicate, Tooltip TTS_*/TTM_*, PropertySheet/CreatePropertySheetPage/DestroyPropertySheetPage, DPA_*/DSA_* dynamic arrays, Subclassing SetWindowSubclass/GetWindowSubclass/RemoveWindowSubclass/DefSubclassProc, TaskDialog/TaskDialogIndirect, ~1100 lines, 130+ exports) |
| 2026-01-17 | OLE32 e COM/OLE 100% completo (compat/windows/ole32.rs: Ole32Emulator struct, HRESULT codes S_OK/E_FAIL/E_NOINTERFACE/REGDB_E_CLASSNOTREG/STG_E_*/etc., GUID/CLSID/IID structures com from_str parsing, well-known IIDs IID_IUNKNOWN/ICLASSFACTORY/IDISPATCH/ISTREAM/ISTORAGE/IDATAOBJECT/IDROPTARGET/IMONIKER/etc., CoInit/CoInitEx/CoUninitialize STA/MTA apartment model, CoCreateInstance/CoGetClassObject, CoRegisterClassObject/CoRevokeClassObject, CoTaskMemAlloc/Free, CLSIDFromString/StringFromCLSID/CoCreateGuid, STGM flags, Storage StgCreateDocfile/StgOpenStorage/StgIsStorageFile/CreateStream, Monikers CreateFileMoniker/CreateBindCtx/GetRunningObjectTable, OLE clipboard/drag-drop RegisterDragDrop/RevokeDragDrop/DoDragDrop/OleSetClipboard, RegisterClipboardFormat, FORMATETC/STGMEDIUM/STATSTG structs, IUnknown AddRef/Release/QueryInterface, CLSCTX/REGCLS/TYMED/CF/DVASPECT constants, ~1050 lines, 150+ exports) |
| 2026-01-17 | Device Tree implementado (drivers/devicetree.rs: DeviceTree struct com root node/phandle_map/aliases, DeviceNode struct com properties/children/phandle, Property/PropertyValue types Empty/U32/U64/String/StringList/Bytes/U32Array/U64Array/PHandle/Reg, FDT parsing from_dtb() com FDT_MAGIC validation/structure/strings parsing, from_acpi() para x86 com CPU/memory/PCI/serial/PS2/RTC nodes do hardware probing, get_node/get_node_by_phandle/find_compatible/find_by_type query APIs, DtDriver trait para driver binding compatible/probe/remove, register_driver/probe_drivers infrastructure, get_bootargs/get_model/get_compatible helpers, ~950 lines) |
| 2026-01-17 | SD/MMC driver implementado (drivers/storage/sdmmc.rs: SDHCI controller driver completo, SdError enum para error handling, SDHCI register definitions SDHCI_DMA_ADDRESS/BLOCK_SIZE/BLOCK_COUNT/ARGUMENT/TRANSFER_MODE/COMMAND/RESPONSE/PRESENT_STATE/HOST_CTRL/POWER_CTRL/CLK_CTRL/TIMEOUT_CTRL/SW_RESET/INT_STATUS/INT_ENABLE/CAPABILITIES, SD command definitions CMD0-CMD56/ACMD6/ACMD13/ACMD41/etc., CardType enum Unknown/Mmc/Sd/Sdhc/Sdxc/Emmc, CID/CSD structs para card identification/specific data, SdhciController struct com base_address/card_type/rca/cid/csd/block_count, MMIO read/write unsafe_read32/unsafe_write32, reset/wait_reset/set_power/set_clock/send_command/wait_command_complete/wait_data_complete, card_init() flow CMD0→CMD8→ACMD41→CMD2→CMD3→CMD9→CMD7, read_blocks/write_blocks com PIO transfer, detect_from_pci para PCI SD controller discovery, ~1050 lines) |
| 2026-01-17 | USB Touchpad driver implementado (drivers/touchpad.rs extensão: UsbTouchpad struct com slot_id/interface_number/endpoint_number/caps/fingers/buttons, UsbTouchpadCaps struct max_x/max_y/max_pressure/max_contacts/has_pressure/has_size/device_type/touch_report_id, UsbTouchpadType enum Unknown/Touchpad/Digitizer/Touchscreen/PrecisionTouchpad, UsbFingerState struct contact_id/tip_switch/in_range/confidence/x/y/pressure/width/height, hid_usage module com HID Usage Pages GENERIC_DESKTOP/DIGITIZER/BUTTON e usages X/Y/TIP_SWITCH/CONTACT_ID/CONTACT_COUNT/etc., parse_report_descriptor() para HID Report Descriptor parsing, process_report() para HID report processing, parse_precision_touchpad() para Windows PTP format, generate_events() integração com touchpad event system, register_usb_touchpad/poll_usb_touchpads/configure_usb_touchpad APIs, is_touchpad_interface() para HID interface detection, ~600 lines adicionadas) |
| 2026-01-17 | VESA/VBE implementado (drivers/vbe.rs: VbeControllerInfo struct com signature/version/capabilities/total_memory/oem_data, VbeModeInfo struct com mode_attributes/resolution/bits_per_pixel/memory_model/framebuffer/color_masks, VbeMemoryModel enum Text/Cga/Hercules/Planar/PackedPixel/NonChain4/DirectColor/Yuv, VbeDisplayMode simplified mode descriptor, standard_modes module com VBE mode numbers 0x100-0x11B 4/8/15/16/24bpp, MODE_PRESETS array VGA/SVGA/XGA/SXGA/720p/1080p/4K, VbeState struct modes/current_mode/controller_info/total_memory, init_from_framebuffer() integração com bootloader GOP/VBE, EdidBlock struct para monitor info com manufacturer/preferred_timing/checksum, vbe_function/vbe_status constants, ~750 lines) |
| 2026-01-17 | Intel GPU driver implementado (drivers/intel_gpu.rs: device_ids module com Gen4-Gen12/Xe device IDs G965/Ironlake/SandyBridge/IvyBridge/Haswell/Broadwell/Skylake/KabyLake/CoffeeLake/IceLake/TigerLake/AlderLake, GpuGeneration enum Gen4-Gen12/Unknown com from_device_id(), regs module MMIO register offsets GTT/DSPCNTR/DSPADDR/DSPSTRIDE/PIPEA_CONF/HTOTAL/VTOTAL/etc., dspcntr/pipe/gtt format bits, IntelDisplayMode struct com timings/refresh_rate presets mode_1080p/mode_720p/mode_xga, IntelGpu struct bus/device_id/generation/mmio_base/gtt_base/stolen_memory/current_mode, read32/write32/modify32/write_gtt_entry MMIO ops, init/init_gtt/init_display setup, set_mode() pipe disable/timings/enable flow, wait_vblank(), probe_pci() PCI enumeration, ~910 lines) |
| 2026-01-17 | RTL8139 network driver implementado (drivers/net/rtl8139.rs: RTL8139_VENDOR_ID/DEVICE_ID constants, regs module I/O port offsets MAC0-5/MAR0/TSD0-3/TSAD0-3/RBSTART/CR/CAPR/CBR/IMR/ISR/TCR/RCR/MSR/etc., cmd/tsd/rcr/tcr/intr bit definitions, RxPacketHeader struct com status/length, Rtl8139 struct io_base/mac/rx_buffer/tx_buffers/tx_index/rx_offset, read8/16/32 write8/16/32 I/O port ops, init() power-on/reset/MAC-read/buffer-setup/IRQ-config/RX-TX-enable, send() TX descriptor cycling/data-copy/size-write, recv() buffer-empty-check/header-parse/packet-copy/CAPR-update, handle_interrupt() status-acknowledge/overflow-reset, is_link_up/get_speed media status, PCI probe com BAR0 I/O base extraction, ~560 lines) |
| 2026-01-17 | RTL8169 Gigabit driver implementado (drivers/net/rtl8169.rs: suporte RTL8169/8168/8111/8101E/8167 device IDs, Descriptor struct 16-byte DMA ring entry opts1/opts2/addr_low/addr_high, flags OWN/EOR/FS/LS/CRC/buffer size, registers MMIO IDR0-5/TxDescAddr/RxDescAddr/ChipCmd/TxPoll/IntrMask/IntrStatus/TxConfig/RxConfig/MPC/ChipVersion, Rtl8169 struct mmio_base/mac/tx_ring/rx_ring/tx_buffers/rx_buffers/tx_index/tx_tail/rx_index/initialized/chip_version, read8/16/32 write8/16/32 MMIO ops, probe_pci() BAR1 MMIO extraction, init() reset/MAC-read/DMA-ring-setup/descriptor-init/RX-TX-enable/interrupt-config, send() descriptor-setup/TxPoll-trigger/ownership-wait, recv() OWN-check/length-extract/data-copy/descriptor-recycle, handle_interrupt()/is_link_up()/get_speed() status, integração em net/mod.rs ActiveDriver::Rtl8169, ~650 lines) |
| 2026-01-17 | Intel I210/I211/I219 IGB driver implementado (drivers/net/igb.rs: device_ids module com I210/I211/I217/I218/I219 series device IDs ~60 variants, regs module MMIO register offsets CTRL/STATUS/EERD/ICR/IMS/IMC/RCTL/TCTL/RDBAL/TDBAL/RAL/RAH/etc., ctrl/status/rctl/tctl/intr bit definitions, TxDesc/RxDesc 16-byte legacy descriptor structs com txcmd/txsts/rxsts/rxerr bits, Igb struct mmio_base/mac/device_id/tx_ring/rx_ring/tx_buffers/rx_buffers/tx_head/tx_tail/rx_index, MMIO read32/write32, eeprom_read() via EERD, reset() com RST bit, read_mac() RAL/RAH + EEPROM fallback, init_tx/init_rx ring setup, enable_interrupts() TXDW/LSC/RXT0/RXO, send() descriptor-setup/TDT-update/DD-wait, recv() DD-check/error-handling/EOP-check/data-copy, handle_interrupt() link status, is_link_up()/get_speed() status, integração em net/mod.rs ActiveDriver::Igb, ~750 lines) |
| 2026-01-17 | Atheros ath9k WiFi driver implementado (drivers/net/ath9k.rs: device_ids module com AR9280/AR9285/AR9287/AR9380/AR9382/AR9485/AR9462/AR9565/QCA9377/QCA6174 ~15 device IDs, supports_5ghz/supports_11ac capabilities, regs module MMIO offsets AR_CR/AR_IER/AR_ISR/AR_EEPROM/AR_STA_ID/AR_RXDP/AR_TXDP/AR_RTC_RC/AR_RTC_PLL/AR_RTC_STATUS/AR_RTC_FORCE_WAKE/AR_IMR/etc., cr/intr bit definitions, TxDesc/RxDesc 32-byte descriptor structs, txctl/rxsts bits, Ath9k struct mmio_base/mac/device_id/tx_ring/rx_ring/buffers/tx_head/tx_tail/rx_index/initialized/powered_on/connected/current_channel/scan_results, MMIO read32/write32, force_wake(), reset(), read_mac() from EEPROM, set_station_id(), init_tx/init_rx ring setup, set_channel() PLL config, start_scan() 2.4GHz+5GHz channels, process_rx_for_scan() beacon parsing, parse_beacon() SSID/BSSID/encryption extraction, ScanResult struct, recv_frame(), ~830 lines) |
| 2026-01-17 | Lid Switch implementado (drivers/acpi.rs: LidState enum Open/Closed/Unknown, LidAction enum None/Suspend/Lock/Hibernate/Shutdown, LidSwitch struct state/prev_state/present/close_action/open_action/gpe_number/wake_enabled, init_lid_switch() PNP0C0D device detection/EC detection/GPE detection, check_ec_lid_support()/check_gpe_lid_support() hardware probing, read_lid_state_internal()/read_ec_lid_state() state reading via EC ports 0x62/0x66, handle_lid_switch() event processing with configurable actions, get_lid_state()/has_lid_switch()/set_lid_close_action()/set_lid_open_action()/set_lid_wake_enabled() API, sysfs_lid_read()/sysfs_lid_action_read()/sysfs_lid_action_write() for /sys/class/power/lid, ~350 lines) |
| 2026-01-17 | Temperature Reading implementado (drivers/thermal.rs: cpuid() inline asm with RBX save/restore, CpuVendor enum Intel/Amd/Unknown, detect_cpu_vendor() via CPUID leaf 0 vendor string comparison, has_thermal_sensors() CPUID leaf 1 ACPI/TM2 + leaf 6 DTS check, get_tjmax() via MSR 0x1A2 with 100°C default, read_intel_core_temp() via MSR 0x19C digital readout offset from TjMax, read_intel_package_temp() via MSR 0x1B1, read_amd_legacy_temp() via PCI config space northbridge register 0xA4, read_amd_ryzen_temp() via SMN 0x59800 for Tctl, read_cpu_temperature() auto-detect vendor, read_core_temperature()/read_all_core_temperatures()/get_sensor_info() API, ThermalSensorInfo struct, ~200 lines) |
| 2026-01-17 | Fan Control implementado (drivers/thermal.rs: FanControlMode enum Auto/Manual/FullSpeed, FanSpeedSource enum EC/AcpiFst/SuperIo/Unknown, FanInfo struct id/name/speed_rpm/pwm_percent/mode/source/min_max_rpm, EC ports 0x66/0x62 for command/data, EC_OBF/EC_IBF status bits, EC_READ_CMD/EC_WRITE_CMD commands, ec_wait_ibe()/ec_wait_obf() polling, ec_read()/ec_write() EC byte access, EC_FAN1/2_SPEED_OFFSET/PWM_OFFSET register offsets, ec_read_fan_speed()/ec_read_fan_pwm()/ec_set_fan_pwm()/ec_set_fan_mode() low-level ops, FanController struct with init()/update()/set_fan_speed()/set_auto_mode()/set_full_speed(), global FAN_CONTROLLER, init_fans()/get_fan_count()/get_fan_speed()/set_fan_speed()/set_fans_auto()/set_fans_full()/update_fans() API, ~350 lines) |
| 2026-01-17 | Thermal Throttling implementado (drivers/thermal.rs: ThrottleLevel enum None/Light/Medium/Heavy/Maximum, performance_percent() 100/75/50/25/10, from_temp_margin() automatic level selection based on distance to critical, CURRENT_THROTTLE global state, apply_thermal_throttling() integrates with cpufreq to limit max frequency based on temperature margin, get_throttle_level()/is_throttled()/clear_throttling() API, poll_thermal() updated to read hardware temp via read_cpu_temperature(), apply throttling for Throttling/Hot states, set_fans_full() on Hot state, poll_thermal_status() returns (temp, state, throttle, fan_rpm) tuple, ~100 lines) |
| 2026-01-17 | Function Keys (Fn) implementado (drivers/fnkeys.rs: FnKeyAction enum com ~40 actions (brightness/volume/display/wireless/power/keyboard/touchpad/performance/media/misc), acpi_events module com event codes BRIGHTNESS_UP/DOWN/VIDEO_SWITCH/WIRELESS_TOGGLE/SUSPEND/etc., fn_scancodes module com extended scancodes SC_MUTE/VOLUME/PLAY_PAUSE/etc., FnKeyState struct fn_pressed/wireless_enabled/touchpad_enabled/airplane_mode/key_bindings/acpi_bindings/osd_enabled, process_extended_scancode()/process_acpi_event() handlers, execute_action() dispatcher, brightness_up/down via backlight module, volume_up/down via audio mixer, display_off/switch_display, toggle_wireless/wifi/bluetooth/airplane_mode, suspend_system via power module, keyboard_backlight_up/down/toggle, toggle_touchpad/fan_boost, cycle_performance_mode via cpufreq governors, media_play_pause/stop/previous/next, launch_calculator/browser/mail/search, sysfs_read_status/sysfs_write_control for /sys/class/fnkeys, ~700 lines) |
| 2026-01-17 | Linux Namespaces implementado (ipc/namespace.rs: NamespaceType enum Mount/Uts/Ipc/User/Pid/Net/Cgroup, flags module CLONE_NEWNS/NEWUTS/NEWIPC/NEWUSER/NEWPID/NEWNET/NEWCGROUP, PidNamespace struct id/parent/level/next_pid/init_pid com alloc_pid()/translate_pid(), UtsNamespace struct hostname/domainname/sysname/release/version/machine, IpcNamespace com shm_segments/sem_sets/msg_queues BTreeMaps, UserNamespace struct uid_map/gid_map/owner_uid/gid/level com add_uid/gid_mapping() e map_uid/gid_to/from_parent(), MountNamespace struct root/mounts com add_mount()/remove_mount()/find_mount(), NetNamespace struct interfaces/routes/iptables com loopback default, CgroupNamespace struct id/root, NamespaceSet struct com todos namespaces e new_root()/clone_share()/unshare()/get_ns_id(), sys_unshare()/sys_setns() syscalls, get/set_hostname()/domainname(), procfs_ns_info(), ~650 lines) |
| 2026-01-17 | Cgroups implementado (cgroups/mod.rs: CgroupId type, CpuController struct shares/cfs_quota_us/cfs_period_us/usage_ns/nr_throttled/throttled_time_ns com set_shares()/set_quota()/set_period()/can_run()/charge()/reset_period()/throttle()/get_bandwidth_percent(), MemoryController struct limit_bytes/soft_limit_bytes/usage_bytes/max_usage_bytes/failcnt/swap_limit_bytes/swap_usage_bytes/oom_control/under_oom com set_limit()/set_soft_limit()/try_charge()/uncharge()/under_pressure()/usage_percent()/trigger_oom()/clear_oom(), IoController struct read/write_bps_limit/iops_limit/bytes_read/written/read/write_ops/weight com set_read/write_bps()/iops()/weight()/account_read/write(), PidsController struct max/current/events_max com set_max()/try_charge()/uncharge()/count(), FreezerController struct state/self_freezing com state()/freeze()/frozen()/thaw()/should_stop(), Cgroup struct id/name/parent/children/members/controllers com add_process()/remove_process()/contains()/process_count()/create_child()/remove_child()/get_child()/check_limits()/path(), CgroupManager struct root/next_id/process_cgroups com create()/get()/delete()/attach_process()/get_process_cgroup()/detach_process()/can_run(), sys_cgroup_create/delete/attach/set_cpu_shares/set_cpu_quota/set_memory_limit/set_pids_max/freeze/thaw/stat syscalls, CgroupStat struct, sched_check_cgroup()/sched_charge_cpu()/mm_try_charge()/mm_uncharge()/on_process_exit() scheduler integration hooks, ~870 lines) |
| 2026-01-17 | Disk Encryption LUKS implementado (crypto/luks.rs: LuksKeySlot struct active/iterations/salt/key_material_offset/stripes, LuksHeader struct magic/version/cipher_name/cipher_mode/hash_spec/payload_offset/key_bytes/mk_digest/salt/uuid/key_slots parsing to_bytes()/from_bytes(), pbkdf2_sha256() key derivation com iterations configuráveis, AES-XTS encryption gf_mul2() GF(2^128)/aes_xts_encrypt_block()/aes_xts_decrypt_block()/aes_xts_encrypt_sector()/aes_xts_decrypt_sector(), LuksError enum InvalidHeader/Password/NoEmptySlot/SlotDisabled/IoError/KeyDerivation/Encryption/DecryptionFailed/AlreadyOpen/NotOpen, LuksVolume struct header/master_key/state/device_name com new()/from_header()/add_key_slot()/remove_key_slot()/unlock()/lock()/is_open()/encrypt_sector()/decrypt_sector(), LuksManager struct volumes com format()/open()/close()/list_open()/is_luks()/encrypt_sector()/decrypt_sector(), EncryptedBlockDevice struct BlockDevice wrapper, sys_luks_format()/open()/close()/is_luks() syscalls, ~580 lines; crypto/aes.rs: Full AES-128/256 S-box/inv S-box/RCON, GF ops gf_mul2/3/9/11/13/14, sub_bytes/inv_sub_bytes/shift_rows/inv_shift_rows/mix_columns/inv_mix_columns/add_round_key transforms, expand_key_128/256 key schedule, aes_encrypt_block/decrypt_block AES-256, aes128_encrypt_block/decrypt_block AES-128, aes_cbc_encrypt/decrypt CBC mode, aes_ctr CTR mode, ~430 lines; crypto/random.rs: get_random_u8/u16/u32/u64, fill_random, random_16/32/64, random_range, ~60 lines) |
| 2026-01-17 | Keyring implementado (security/keyring.rs: KeySerial type i32, KEY_SPEC_* special keyring constants THREAD/PROCESS/SESSION/USER/USER_SESSION/GROUP/REQKEY_AUTH/REQUESTOR, KEY_POS/USR/GRP/OTH_* permission constants VIEW/READ/WRITE/SEARCH/LINK/SETATTR/ALL, KeyType enum User/Login/Keyring/BigKey/Trusted/Encrypted/Logon/Pkcs7/X509/Asymmetric/DnsResolver/RequestKeyAuth/Custom com as_str()/from_str(), KeyState enum Valid/Construction/Negative/Revoked/Expired/Dead, KeyFlags struct quota_overrun/instantiated/revoked/dead/no_invalidate/retry, Key struct serial/key_type/description/payload/uid/gid/perm/state/flags/ctime/atime/expiry/ref_count/linked_keys com new()/instantiate()/revoke()/is_usable()/is_expired()/touch()/check_permission()/get_ref()/put_ref()/refs(), KeyringError enum NotFound/Exists/NoMemory/PermissionDenied/InvalidKey/InvalidKeyring/KeyRevoked/KeyExpired/QuotaExceeded/InvalidDescription/InvalidPayload, KeyringManager struct keys/next_serial/user_keyrings/user_session_keyrings/process_keyrings/thread_keyrings/session_keyrings com add_key()/add_keyring()/get_key()/request_key()/search_keyring()/link_key()/unlink_key()/read_key()/update_key()/revoke_key()/set_timeout()/set_perm()/get_user_keyring()/get_user_session_keyring()/get_process_keyring()/cleanup_process()/describe_key()/list_keyring()/resolve_special_keyring()/gc(), sys_add_key/request_key/keyctl_read/update/revoke/set_timeout/setperm/describe/link/unlink/search syscalls, ~850 lines) |
| 2026-01-17 | Docker Container Support implementado (compat/containers.rs: ContainerId type String, ContainerState enum Created/Running/Paused/Stopped/Removing, ResourceLimits struct memory_limit/cpu_shares/cpu_quota/pids_limit/io_weight, MountSpec struct source/target/fs_type/options/readonly, ContainerNetwork struct network_mode/ip_address/gateway/dns_servers/hostname/port_mappings, PortMapping struct host_port/container_port/protocol, ContainerConfig struct image/cmd/entrypoint/env/working_dir/user/resources/mounts/network/hostname/readonly_rootfs/tty/stdin_open, Container struct id/short_id/name/config/state/pid/namespaces/cgroup_path/created_at/started_at/finished_at/exit_code/restart_count/running AtomicBool com is_created/running/paused/stopped()/get_state()/get_uptime(), ContainerError enum NotFound/AlreadyExists/AlreadyRunning/NotRunning/NotPaused/FailedToCreate/FailedToStart/FailedToStop/NamespaceFailed/CgroupFailed, ContainerRuntime struct containers/next_id/running_count/paused_count/stopped_count/name_index com resolve_id()/get_container()/create() namespace setup + cgroup creation/list()/start() cgroup limits + hostname set/stop() SIGTERM signal/kill() SIGKILL/pause() cgroup freeze/unpause() cgroup thaw/remove()/exec()/logs()/inspect()/stats()/prune()/get_running_count/paused_count/stopped_count()/get_containers_by_state(), global CONTAINER_RUNTIME, init()/create/start/stop/kill/pause/unpause/remove/list/inspect/stats/exec/logs/prune() API functions, ~750 lines) |
| 2026-01-17 | Text Shaping implementado (gui/shaping.rs: Script enum Latin/Arabic/Hebrew/Cyrillic/Greek/Han/Hiragana/Katakana/Hangul/Thai/Devanagari/Tamil/Bengali/Common/Inherited/Unknown com is_rtl()/needs_shaping(), detect_script() por codepoint range, BidiClass enum L/R/AL/EN/ES/ET/AN/CS/NSM/BN/B/S/WS/ON/LRE/LRO/RLE/RLO/PDF/LRI/RLI/FSI/PDI, get_bidi_class() UAX #9, BidiRun struct start/end/level/visual_pos, BidiParagraph struct rtl/runs/levels/reorder_map com new() simplified bidi algorithm/get_visual_order()/is_pure_ltr()/is_pure_rtl(), ArabicJoiningType enum Right/Dual/Causing/NonJoining/Transparent, ArabicForm enum Isolated/Initial/Medial/Final, get_arabic_joining_type()/get_arabic_forms() presentation forms mapping, shape_arabic() joining context analysis, GraphemeBreak enum CR/LF/Control/Extend/ZWJ/RegionalIndicator/Prepend/SpacingMark/L/V/T/LV/LVT/Other, get_grapheme_break() UAX #29, GraphemeCluster struct start/end/chars, find_grapheme_clusters() boundary detection/should_break_grapheme() rules GB3-GB999, grapheme_count(), ShapedGlyph struct codepoint/cluster/x_offset/y_offset/x_advance/y_advance, ShapedRun struct start/end/script/rtl/glyphs, ShaperConfig struct ligatures/kerning/size/dpi_x/dpi_y, shape_text() full pipeline bidi+script+arabic+glyphs, itemize_by_script(), apply_latin_ligatures() ff/fi/fl/ffi/ffl/st, apply_arabic_ligatures() lam-alef, measure_shaped_width()/get_visual_string()/has_rtl()/needs_shaping() utils, ~1100 lines) |
| 2026-01-17 | Package Rollback implementado (pkg/rollback.rs: TransactionId type u64, OperationType enum Install/Remove/Upgrade/Downgrade/Reinstall, Operation struct op_type/package_name/new_version/old_version/files/dirs/timestamp, FileOperation struct path/action/backup_path/checksum, FileAction enum Create/Modify/Delete, DirOperation struct path/created, TransactionStatus enum InProgress/Completed/RolledBack/Failed, Transaction struct id/operations/status/start_time/end_time/description/user com new()/add_operation()/complete()/mark_rolled_back()/mark_failed()/can_rollback()/affected_packages(), SnapshotId type u64, Snapshot struct id/name/transaction_id/timestamp/packages/protected com create()/package_count(), SnapshotPackage struct name/version/reason, RollbackConfig struct max_transactions/max_snapshots/backup_dir/auto_snapshot/max_backup_size, RollbackState struct transactions/snapshots/current_transaction/next_txn_id/next_snapshot_id/config/total_backup_size, init()/begin_transaction()/record_file_operation()/record_dir_operation()/set_operation_info()/commit_transaction()/abort_transaction()/rollback_transaction()/create_snapshot()/restore_snapshot()/delete_snapshot()/protect_snapshot()/list_snapshots()/get_snapshot()/list_transactions()/get_transaction()/can_rollback()/get_config()/set_config()/backup_file()/restore_file()/checksum_file()/verify_checksum()/rollback_operations()/cleanup_transaction_backups()/undo()/get_undo_history()/auto_snapshot_if_enabled()/get_stats(), RollbackStats struct, ~700 lines) |
| 2026-01-17 | Source Packages implementado (pkg/source.rs: SourcePackageId type u64, SourcePackage struct id/name/version/source_type/recipe/patches/source_dir/timestamp/maintainer/category, SourceType enum Tarball{url,checksum}/Git{url,branch,tag,commit}/Svn{url,revision}/Hg{url,revision}/Local{path} com url()/is_vcs(), Patch struct name/content/strip_level/condition/description/is_security com new()/should_apply(), PatchCondition enum Arch/Feature/Version{min,max}/Custom, SourceRepository struct name/url/enabled/priority/repo_type/gpg_key, SourceRepoType enum Http/Git/Local, BuildEnvironment struct build_root/source_dir/pkg_dir/log_dir/arch/host/target/jobs/cflags/cxxflags/ldflags/env/use_sandbox/keep_build_on_fail/debug_info, BuildStatus enum Pending/Fetching/Extracting/Patching/Configuring/Building/Testing/Packaging/Success/Failed/Cancelled com as_str()/is_terminal(), BuildResult struct package/version/status/start_time/end_time/packages/log_path/error/warnings, SourceManagerState struct packages/repositories/active_builds/build_history/next_id/default_env, init()/add_repository()/remove_repository()/list_repositories()/sync_repositories()/register_package()/get_package()/search_packages()/add_patch()/remove_patch()/fetch_source()/apply_patches()/build_package()/do_build()/update_build_status()/run_build_command()/get_build_status()/get_build_history()/clean_build()/get_default_environment()/set_default_environment()/parse_source_url()/extract_version_from_filename()/compute_checksum()/verify_checksum()/get_stats(), SourceStats struct, ~700 lines) |
| 2026-01-17 | Cross-compilation implementado (pkg/crossbuild.rs: TargetArch struct arch/vendor/os/abi/endian/pointer_width/features com from_triple()/triple()/short_triple()/is_native()/arch_family(), Endianness enum Little/Big, common targets x86_64_linux_gnu/aarch64_linux_gnu/arm_linux_gnueabihf/riscv64_linux_gnu/x86_64_linux_musl, Toolchain struct name/target/prefix/root/cc/cxx/as_path/ld/ar/strip/objcopy/objdump/ranlib/nm/sysroot/gcc_version/is_llvm com gnu()/llvm()/env_vars()/compiler_flags(), Sysroot struct name/target/path/include_dirs/lib_dirs/pkg_config_path/is_complete com new()/include_flags()/lib_flags(), CrossBuildEnv struct host/target/build/toolchain/sysroot/cflags/cxxflags/ldflags/env/configure_flags/cmake_flags/meson_cross_file com new()/with_sysroot()/get_env()/cmake_system_name()/generate_meson_cross_file(), CrossBuildManager struct toolchains/sysroots/host/default_toolchains, init()/register_toolchain()/remove_toolchain()/get_toolchain()/list_toolchains()/set_default_toolchain()/get_default_toolchain()/register_sysroot()/remove_sysroot()/get_sysroot()/list_sysroots()/get_host()/create_cross_env()/find_sysroot_for_target()/is_target_supported()/list_supported_targets()/common_targets()/parse_target_from_env()/get_stats(), CrossStats struct, ~650 lines) |
| 2026-01-17 | Boot Logo implementado (drivers/boot_logo.rs: LogoStyle enum Default/Minimal/Modern/Retro, BootStage enum Starting/Hardware/Memory/Interrupts/Drivers/Filesystem/Network/Services/Desktop/Ready com name()/progress(), BootLogoState struct displayed/progress/show_progress/style/boot_stage, colors module PRIMARY/SECONDARY/ACCENT/BG_DARK/BG_GRADIENT_START/END/TEXT_PRIMARY/SECONDARY/PROGRESS_BG/PROGRESS_FILL branding colors, LOGO_ASCII/LOGO_LARGE/LOGO_SIMPLE ASCII art arrays, init()/show()/hide()/is_displayed(), show_default_logo() gradient background + ASCII logo + version/copyright, show_minimal_logo() centered text + underline, show_modern_logo() gradient + decorative circles + S emblem + tagline, show_retro_logo() BIOS-style blue screen, draw_gradient_background() vertical color interpolation, draw_decorative_circles() subtle decoration, set_progress()/set_stage() progress tracking, draw_progress_bar() with percentage text, draw_stage_text() boot status, draw_spinner() animated 8-segment spinner, show_splash() quick boot, show_error() error banner, show_complete() checkmark, ~500 lines) |
| 2026-01-17 | IDE/ATA Driver implementado (drivers/storage/ide.rs: IdeChannel enum Primary/Secondary com io_base()/ctrl_base(), IdeRole enum Master/Slave com drive_select(), IdeDeviceType enum Ata/Atapi/Unknown, IdeIdentity struct model/serial/firmware/device_type/supports_lba/supports_lba48/sectors_28/sectors_48/sector_size com from_identify_data()/parse_ata_string(), IdeChannelPorts struct com all I/O ports data/error/features/sector_count/lba_lo/mid/hi/drive/status/command + control alt_status/device_ctrl, read_status()/read_alt_status()/read_error()/wait_not_busy()/wait_drq_or_error()/wait_ready()/select_drive()/software_reset()/disable_interrupts() low-level ops, IdeDevice struct channel/role/identity/ports/device_id com read_sectors()/write_sectors() PIO mode LBA28/LBA48, BlockDevice trait impl id()/block_size()/num_blocks()/read_blocks()/write_blocks() with 255-sector chunking, IdeController struct devices/initialized com identify_device()/init()/devices()/first_ata_device(), probe() PCI class 0x01 subclass 0x01, init()/init_from_pci()/devices()/first_device()/get_device()/print_devices() API, integrated into storage/mod.rs init sequence, ~830 lines) |
| 2026-01-17 | Software RAID implementado (storage/raid.rs: RaidLevel enum Raid0/Raid1/Raid5/Raid10/Linear com min_devices()/name()/efficiency()/is_redundant(), DeviceState enum Active/Failed/Rebuilding/Spare/Removed, RaidMember struct index/device/state/error_count/size_blocks com Clone/Debug manual impls, RaidConfig struct level/stripe_size/device_count/uuid/name/created_at com new()/with_stripe_size(), RaidSuperblock struct magic/version/level/device_index/device_count/stripe_size/uuid/array_size/device_size/created_at/events/checksum com new()/calculate_checksum()/is_valid()/to_bytes()/from_bytes(), RaidStats struct reads/writes/bytes_read/written/read_errors/write_errors/parity_checks/rebuilds, RaidArray struct config/members/state/device_id/stats/usable_blocks/block_size/degraded/syncing com new()/info()/is_degraded()/is_syncing()/members()/stats()/map_block()/read_block()/write_block()/calculate_parity()/fail_device()/replace_device()/check_degraded()/write_superblock(), BlockDevice trait impl, RaidManager struct arrays/next_device_id com create_array()/get_array()/get_array_by_id()/list_arrays()/remove_array()/scan_arrays()/print_arrays(), init()/create_raid0/1/5/10()/create_linear()/get_array()/list_arrays()/print_status() API, generate_uuid()/format_uuid() helpers, ~750 lines) |
| 2026-01-17 | Multi-Monitor implementado (gui/multimon.rs: MonitorId type u32, ConnectorType enum Vga/Dvi/Hdmi/DisplayPort/Edp/Lvds/UsbC/Composite/Unknown com name(), ConnectionState enum Connected/Disconnected/Unknown, DisplayMode struct width/height/refresh_millihertz/bpp/interlaced com new()/refresh_hz()/format()/is_16_9()/is_16_10(), modes module com predefined modes VGA-UHD, EdidInfo struct manufacturer/product_code/serial_number/manufacture_week/year/name/supported_modes/preferred_mode/max_size_cm/supports_audio/is_digital com from_bytes()/manufacturer_string(), parse_detailed_timing()/parse_standard_timing()/parse_monitor_name() EDID parsing, Monitor struct id/connector/connector_index/state/edid/current_mode/available_modes/position/is_primary/rotation/scale/brightness/framebuffer_addr/stride com new()/name()/effective_resolution()/bounds()/contains_point()/set_mode()/Clone impl, ArrangementMode enum Extended/Clone/PrimaryOnly/ExternalOnly, DisplayArrangement struct mode/primary/virtual_width/height, MonitorEvent enum Connected/Disconnected/ModeChanged/ArrangementChanged, MultiMonitorManager struct monitors/arrangement/next_id/hotplug_enabled/change_callbacks com add_monitor()/remove_monitor()/get_monitor()/connected_monitors()/all_monitors()/primary_monitor()/set_primary()/set_arrangement_mode()/arrangement()/set_position()/set_mode()/set_edid()/set_connected()/recalculate_arrangement()/monitor_at_point()/auto_arrange_horizontal()/on_change()/notify()/print_info(), global MULTI_MONITOR, init()/add_from_framebuffer()/connected_monitors()/primary_monitor()/virtual_desktop_size()/monitor_at_point()/print_status() API, ~750 lines) |
| 2026-01-17 | PC Speaker implementado (drivers/audio/pcspkr.rs: PIT constants PIT_FREQUENCY/CHANNEL2_PORT/COMMAND_PORT/SPEAKER_PORT, common frequencies FREQ_BEEP/ERROR/SUCCESS/WARNING/CLICK, notes module com C4-C6 frequencies + MIDI lookup table FREQ_TABLE[128] para from_midi(), PcSpeakerState struct playing/current_freq/enabled/volume, SPEAKER_STATE/SPEAKER_PLAYING/CURRENT_FREQUENCY globals, set_pit_frequency() PIT channel 2 divisor config, enable_speaker()/disable_speaker() port 0x61 bit manipulation, is_speaker_enabled() status check, init()/play_tone()/stop()/beep()/system_beep()/error_beep()/success_beep()/warning_beep()/click() basic sounds, play_melody() note sequence, startup_sound()/shutdown_sound()/notification_sound() system jingles, is_playing()/current_frequency()/set_enabled()/is_enabled()/set_volume()/get_volume() state, delay_ms() busy-wait, play_music() simple notation parser NOTE[OCTAVE][DURATION], parse_note_name()/parse_octave()/parse_duration()/note_to_frequency() parsing, bios_codes module post_success/memory_error/video_error/keyboard_error/fatal_error beep patterns, format_status()/format_frequency_info() sysfs helpers, ~500 lines) |
| 2026-01-17 | AMD GPU Driver implementado (drivers/amd_gpu.rs: device_ids module com GCN 1.0 Southern Islands (HD 7000)/GCN 1.1 Sea Islands (R7/R9 200)/GCN 1.2 Volcanic Islands (R9 300)/GCN 3.0 Polaris (RX 400/500)/GCN 5.0 Vega/RDNA 1 Navi10 (RX 5000)/RDNA 2 Navi2x (RX 6000)/RDNA 3 Navi3x (RX 7000) ~80 device IDs + APUs, GpuFamily enum com from_device_id()/name()/is_apu()/min_vram()/uses_dcn(), regs module DCE/DCN MMIO offsets CRTC_H_TOTAL/V_TOTAL/GRPH_ENABLE/CONTROL/PITCH/PRIMARY_SURFACE_ADDRESS/OTG0_*+MC_VM+GRBM+IH+power registers, grph_control/crtc_control bit definitions, AmdDisplayMode struct timings presets mode_1080p/720p/1440p/4k, AmdGpu struct mmio_base/vram_base/doorbell_base/family/device_id/detected_vram, read32/write32/read_indirect/write_indirect MMIO ops, init()/detect_vram()/disable_interrupts()/init_display()/init_dce()/init_dcn() setup, set_mode()/set_mode_dce()/set_mode_dcn() timing+surface+pipe config, framebuffer_address()/framebuffer_size()/wait_vblank()/set_cursor_position()/set_cursor_visible()/disable() API, probe_pci() PCI enumeration AMD vendor 0x1002, get_info_string()/get_vram_string()/get_mode_string() sysfs, ~900 lines) |
| 2026-01-17 | NVIDIA GPU Driver implementado (drivers/nvidia_gpu.rs: device_ids module com Kepler (GTX 600/700)/Maxwell (GTX 900)/Pascal (GTX 10)/Turing (GTX 16/RTX 20)/Ampere (RTX 30)/Ada Lovelace (RTX 40) ~70 device IDs, GpuGeneration enum com from_device_id()/name()/supports_raytracing()/has_tensor_cores()/min_vram(), regs module PMC/PBUS/PTIMER/PFB/PDISP/PCRTC/PRAMDAC MMIO offsets HEAD_SYNC/BLANK/TOTAL/SURFACE registers, surface_format constants A8R8G8B8/X8R8G8B8/R5G6B5, NvidiaDisplayMode struct timings presets mode_1080p/720p/1440p/4k, NvidiaGpu struct mmio_base/vram_base/bar2_base/generation/device_id/detected_vram/boot0, read32/write32/modify32 MMIO ops, init()/detect_vram()/init_display() setup, set_mode() horizontal/vertical timing + surface config per-head, framebuffer_address()/framebuffer_size()/wait_vblank()/set_cursor_position()/disable() API, probe_pci() PCI enumeration NVIDIA vendor 0x10DE VGA/3D controller classes, get_info_string()/get_vram_string()/get_mode_string() sysfs, ~850 lines) |
| 2026-01-17 | OHCI/UHCI USB 1.x implementado (drivers/usb/ohci.rs: OHCI 1.0 spec para chipsets AMD/VIA/SiS, MMIO registers HC_REVISION/CONTROL/CMDSTATUS/INTRSTATUS/INTRENABLE/HCCA/PERIODIC_CURRENT/CONTROL_HEAD/CURRENT/BULK/FM/RH, Hcca 256-byte com interrupt_table[32]/frame_number/done_head, EndpointDescriptor 16-byte aligned FA/EN/D/S/K/F/MPS/TailP/HeadP/NextED, TransferDescriptor 16-byte R/DP/DI/T/EC/CC/CBP/NextTD/BE, OhciController struct mmio_base/hcca/hcca_phys/control_ed/ed_pool/td_pool/next_address/devices, hc_control bits CLE/BLE/HCFS, functional_state enum USBReset/USBResume/USBOperational/USBSuspend, port reset/enumeration, ~800 lines; drivers/usb/uhci.rs: UHCI 1.1 spec para Intel, I/O port registers USBCMD/USBSTS/USBINTR/FRNUM/FRBASEADD/SOFMOD/PORTSC, TransferDescriptor 16-byte link/td_ctrl/td_token/buffer, QueueHead 16-byte queue_link/element_link, UhciController struct io_base/frame_list/frame_list_phys/qhs/td_pool/next_address/devices, frame_list[1024] para periodic scheduling, port reset/enumeration, ~650 lines; drivers/usb/mod.rs atualizado com pub mod ohci/uhci) |
| 2026-01-17 | USB Audio implementado (drivers/usb/audio.rs: UAC 1.0/2.0 USB Audio Class, class_codes module AUDIOCONTROL/AUDIOSTREAMING/MIDISTREAMING subclasses + CS_INTERFACE/CS_ENDPOINT descriptor types + AC_HEADER/INPUT_TERMINAL/OUTPUT_TERMINAL/FEATURE_UNIT/CLOCK_SOURCE subtypes, terminal_types module USB_STREAMING/MICROPHONE/SPEAKER/HEADPHONES/HEADSET/etc., AudioFormat enum PCM/IeeeFloat/Alaw/Mulaw, SampleRate struct 8kHz-192kHz presets, ChannelConfig MONO/STEREO/SURROUND_51/71, Terminal/FeatureUnit/ClockSource/StreamingFormat/StreamingEndpoint/StreamingInterface descriptors, FeatureControls bitmap mute/volume/bass/treble/EQ, UsbAudioDevice struct terminals/feature_units/streaming_interfaces/volume/mute, UacVersion enum UAC1/2/3, UsbAudioDriver device registry, parse_ac_interface/parse_as_format descriptor parsing, requests module SET_CUR/GET_CUR/MUTE_CONTROL/VOLUME_CONTROL, VolumeDb dB-to-percent conversion, ~900 lines) |
| 2026-01-17 | AC'97 Audio implementado (drivers/audio/ac97.rs: AC'97 2.3 spec, mixer_regs module RESET/MASTER_VOL/PCM_OUT_VOL/MIC_VOL/LINE_IN_VOL/RECORD_SELECT/EXT_AUDIO_ID/VENDOR_ID registers, busmaster_regs module PI/PO/MC buffer descriptor registers + GLOB_CNT/GLOB_STA global control, BufferDescriptor 8-byte DMA entry address/length/IOC/BUP flags, Ac97Capabilities struct vendor_id/variable_rate/double_rate/spdif/surround, Ac97Controller struct nambar/nabmbar I/O ports + BD arrays + buffers, I/O port access read_mixer/write_mixer + read_bm8/16/32/write_bm8/16/32, init() cold reset/codec ready/VRA enable, set_sample_rate() VRA 8-48kHz, set_volume()/set_mute()/set_pcm_volume()/set_record_source()/set_record_gain(), start_playback()/stop_playback()/start_capture()/stop_capture() DMA control, handle_interrupt() BCIS/LVBCI handling, is_ac97_controller() Intel/VIA/SiS/nVidia/AMD device IDs, ~750 lines) |
| 2026-01-17 | USB Video (UVC) implementado (drivers/usb/video.rs: UVC 1.0/1.1/1.5 spec, class_codes module SC_VIDEOCONTROL/SC_VIDEOSTREAMING subclasses + VC_INPUT_TERMINAL/OUTPUT_TERMINAL/PROCESSING_UNIT/VS_FORMAT_*/VS_FRAME_* subtypes, terminal_types module ITT_CAMERA/OTT_DISPLAY, VideoFormat enum Uncompressed/Mjpeg/H264/Vp8, PixelFormat enum Yuy2/Nv12/Mjpeg/H264/Rgb24 GUID parsing, FrameSize struct QVGA/VGA/HD720/HD1080/UHD4K presets, FrameInterval struct FPS_5-60 presets, FrameDescriptor/FormatDescriptor structs, CameraControls bitmap scanning/exposure/focus/zoom/pan-tilt/privacy, ProcessingControls bitmap brightness/contrast/hue/saturation/sharpness/gamma/white-balance/gain, InputTerminal/OutputTerminal/ProcessingUnit/StreamingInterface descriptors, UsbVideoDevice struct terminals/formats/brightness/contrast/streaming state, VideoProbeCommit struct for format negotiation, UsbVideoDriver device registry, parse_vc_interface() descriptor parsing, requests module VS_PROBE/COMMIT + CT_*/PU_* control selectors, ~850 lines) |
| 2026-01-17 | Webcam Support implementado (drivers/webcam.rs: CameraPixelFormat enum Yuyv/Uyvy/Nv12/Rgb24/Bgr24/Rgba32/Mjpeg/H264/Grey fourcc codes, CameraResolution presets QVGA-UHD4K, CameraFormat config, CameraControl enum brightness/contrast/saturation/hue/gamma/gain/exposure/focus/zoom/etc. V4L2-compatible IDs, CameraControlInfo min/max/default/current, CameraCapabilities struct, CameraState Closed/Open/Streaming/Error, CameraFrame struct data/format/sequence/timestamp, BufferQueue com queue/dequeue/ready/mark_ready for streaming, Camera struct handle/state/format/buffer_queue/usb_device_id, CameraManager device registry, V4L2 Compatibility Layer: V4l2BufType/V4l2Memory/V4l2Format/V4l2RequestBuffers/V4l2Buffer/V4l2Control structs, v4l2_ioctl module QUERYCAP/S_FMT/G_FMT/REQBUFS/QBUF/DQBUF/STREAMON/STREAMOFF/G_CTRL/S_CTRL commands, V4l2Device ioctl handler, Image Processing: yuyv_to_rgb24()/nv12_to_rgb24() color space conversion, extract_mjpeg_frame() JPEG marker detection, Public API: init()/open()/close()/list()/count()/capture_frame()/format_camera_info(), USB Video integration via list_cameras()/start_streaming()/stop_streaming()/set_control(), ~950 lines) |
| 2026-01-17 | Bluetooth HCI implementado (drivers/bluetooth/mod.rs: BdAddr struct [u8;6] para endereços Bluetooth com from_bytes()/to_string()/Display impl, DeviceClass struct [u8;3] com Major/Minor service classes, LinkType enum Sco/Acl/Esco, LinkKeyType enum Combination/DebugCombination/UnauthenticatedP192/etc., ControllerState enum Off/Initializing/Ready/Scanning/Advertising/Connecting/Connected/Error, ControllerCapabilities struct features BR/EDR/LE/SSP, RemoteDevice struct address/name/device_class/rssi/connected/paired/link_key/link_type, BluetoothController struct id/state/address/name/capabilities/devices/transport/scanning com start_scan()/stop_scan()/get_device()/process_event(), BluetoothManager struct controllers/default_controller/next_id com register_controller()/unregister_controller()/get_default()/get_by_id()/list_controllers(), global BLUETOOTH_MANAGER, init()/controller_count()/start_scan()/stop_scan()/discovered_devices()/format_status() API; drivers/bluetooth/hci.rs: packet_types module COMMAND/ACL_DATA/SCO_DATA/EVENT, LAP_GIAC/LAP_LIAC inquiry access codes, events module ~60 HCI event codes INQUIRY_COMPLETE/CONNECTION_COMPLETE/DISCONNECTION/etc., le_events module ~20 LE event codes, errors module ~30 HCI error codes SUCCESS/UNKNOWN_COMMAND/NO_CONNECTION/etc., commands module com opcodes RESET/INQUIRY/CREATE_CONNECTION/DISCONNECT/etc. + builder functions reset()/inquiry()/create_connection()/read_bd_addr()/etc., AclHeader struct handle/flags/length com parse()/build(); drivers/bluetooth/l2cap.rs: cid module SIGNALING/CONNECTIONLESS/ATT/SMP channel IDs, psm module SDP/RFCOMM/HID_CONTROL/HID_INTERRUPT/AVDTP/AVCTP PSMs, signal module CONNECTION_REQUEST/RESPONSE/CONFIGURE/DISCONNECT codes, L2capChannel struct local_cid/remote_cid/psm/state/config/handle/credits, L2capManager struct channels/next_local_cid/next_identifier com allocate_channel()/get_channel()/build_connection_request()/build_data_packet()/process_signaling()/process_packet(), L2capEvent enum; drivers/bluetooth/usb_transport.rs: device_ids module Intel/Broadcom/Atheros/Realtek/MediaTek/CSR VID/PIDs ~50 device IDs, endpoints module CONTROL/INTERRUPT_IN/BULK_OUT/BULK_IN/ISOCH endpoints, h4 packet indicators, UsbBluetoothTransport struct slot_id/interface_number/state/endpoints/rx_queue/vendor_id/product_id com init()/send_command()/send_acl()/send_sco()/receive_event()/receive_acl()/poll()/close(), is_bluetooth_device()/is_known_bluetooth_adapter()/get_adapter_name()/scan_usb_adapters() detection, ~2200 lines total) |
| 2026-01-17 | Bluetooth Pairing implementado (drivers/bluetooth/pairing.rs: IoCapability enum DisplayOnly/DisplayYesNo/KeyboardOnly/NoInputNoOutput/KeyboardDisplay para SSP, OobDataPresent enum NotPresent/P192Present/P256Present/P192AndP256Present, AuthRequirements struct mitm_required/bonding/secure_connections/keypress/ct2 com to_byte()/from_byte(), PairingMethod enum JustWorks/NumericComparison/PasskeyEntry/OutOfBand com determine() IO capability matrix + provides_mitm_protection(), PairingState enum Idle/IoCapabilityExchange/PublicKeyExchange/AuthenticationStage1/2/LinkKeyCalculation/Bonding/Complete/Failed state machine, PairingContext struct remote_address/handle/state/io_cap/auth_req/pairing_method/numeric_value/passkey/pin_code/link_key/secure_connections para active pairing tracking, StoredLinkKey struct address/key/key_type/authenticated/created/last_used, LinkKeyStorage struct BTreeMap-based storage com store()/get()/has_key()/remove()/paired_devices()/export()/import() persistência, global LINK_KEY_STORAGE, commands module HCI pairing opcodes AUTHENTICATION_REQUESTED/LINK_KEY_REQUEST_REPLY/PIN_CODE_REQUEST_REPLY/IO_CAPABILITY_REQUEST_REPLY/USER_CONFIRMATION_REQUEST_REPLY/USER_PASSKEY_REQUEST_REPLY + builder functions, events module LINK_KEY_REQUEST/LINK_KEY_NOTIFICATION/PIN_CODE_REQUEST/IO_CAPABILITY_REQUEST/RESPONSE/USER_CONFIRMATION_REQUEST/PASSKEY_REQUEST/SIMPLE_PAIRING_COMPLETE/AUTHENTICATION_COMPLETE event codes, smp module para BLE: code module PAIRING_REQUEST/RESPONSE/CONFIRM/RANDOM/FAILED/ENCRYPTION_INFORMATION/etc., error module PASSKEY_ENTRY_FAILED/OOB_NOT_AVAILABLE/AUTHENTICATION_REQUIREMENTS/etc. ~15 error codes, SmIoCapability enum, KeyDistribution struct enc_key/id_key/sign_key/link_key, SmPairingParams struct com build_pairing_request()/build_pairing_response()/from_pdu(), build_pairing_confirm()/random()/failed()/encryption_information()/central_identification()/identity_information()/signing_information()/security_request()/pairing_public_key()/pairing_dhkey_check() PDU builders, PairingEvent enum Started/IoCapabilityExchanged/NumericComparison/PasskeyDisplay/PasskeyRequest/PinCodeRequest/Success/Failed para callbacks, PairingManager struct contexts/default_io_cap/default_auth_req/event_callbacks/auto_accept_just_works/default_pin com start_pairing()/handle_io_capability_request()/handle_io_capability_response()/handle_user_confirmation_request()/handle_user_passkey_request()/handle_link_key_request()/handle_link_key_notification()/handle_pin_code_request()/handle_simple_pairing_complete()/handle_authentication_complete()/confirm_numeric_comparison()/enter_passkey()/enter_pin()/cancel_pairing()/unpair()/is_paired()/paired_devices() full pairing flow, global PAIRING_MANAGER, Public API: init()/pair()/is_paired()/unpair()/paired_devices()/set_io_capability()/set_auth_requirements()/set_auto_accept_just_works()/set_default_pin()/confirm_numeric()/enter_passkey()/enter_pin()/cancel()/format_status(), Integrated into BluetoothController.process_event() handling all pairing HCI events, ~1550 lines) |
| 2026-01-17 | Bluetooth HID implementado (drivers/bluetooth/hid.rs: psm module HID_CONTROL(0x0011)/HID_INTERRUPT(0x0013) L2CAP PSMs, transaction module HANDSHAKE/HID_CONTROL/GET_REPORT/SET_REPORT/GET_PROTOCOL/SET_PROTOCOL/GET_IDLE/SET_IDLE/DATA/DATC types, handshake result codes SUCCESSFUL/NOT_READY/ERR_INVALID_REPORT_ID/ERR_UNSUPPORTED_REQUEST/etc., control params SUSPEND/EXIT_SUSPEND/VIRTUAL_CABLE_UNPLUG, ReportType enum Input/Output/Feature, ProtocolMode enum Boot/Report, HidSubclass None/BootInterface, HidDeviceType enum Unknown/Keyboard/Mouse/ComboKeyboardMouse/Gamepad/Joystick/Digitizer/CardReader/RemoteControl com from_device_class()/name(), usage_page module GENERIC_DESKTOP/KEYBOARD/BUTTON/CONSUMER/etc., generic_desktop module POINTER/MOUSE/JOYSTICK/GAMEPAD/KEYBOARD/X/Y/Z/RX/RY/RZ/WHEEL/HAT_SWITCH, keyboard_codes module ~80 USB HID scan codes A-Z/0-9/F1-F12/arrows/modifiers/etc., keyboard_modifiers LEFT_CTRL/SHIFT/ALT/GUI + RIGHT variants, mouse_buttons LEFT/RIGHT/MIDDLE/BUTTON_4/5, BootKeyboardReport struct modifiers/reserved/keys[6] com from_bytes()/is_modifier_pressed()/is_key_pressed()/pressed_keys(), BootMouseReport struct buttons/x/y/wheel com from_bytes()/is_button_pressed(), HidConnectionState enum Disconnected/Connecting/Connected/Ready/Suspended/Error, BluetoothHidDevice struct address/name/device_type/state/acl_handle/control_cid/interrupt_cid/protocol_mode/hid_descriptor/last_keyboard_report/last_mouse_report/report_ids, HidInputEvent enum KeyEvent/MouseButton/MouseMove/MouseWheel/GamepadButton/GamepadAxis para input callbacks, BluetoothHidManager struct devices/event_callbacks/auto_reconnect/known_devices com add_device()/get_device()/remove_device()/connected_devices()/handle_l2cap_connect()/handle_l2cap_disconnect()/process_hid_data()/process_input_report()/process_keyboard_report()/process_mouse_report()/process_gamepad_report()/build_get_report()/build_set_report()/build_set_protocol()/build_handshake()/build_hid_control()/set_auto_reconnect()/add_known_device(), global HID_MANAGER, Public API: init()/add_device()/remove_device()/device_count()/connected_count()/on_input()/handle_connect()/handle_disconnect()/process_data()/format_status()/scancode_to_ascii()/scancode_name(), ~950 lines) |
| 2026-01-17 | Bluetooth Audio (A2DP) implementado (drivers/bluetooth/a2dp.rs: AVDTP 1.3 Audio/Video Distribution Transport Protocol, signal_ids module DISCOVER/GET_CAPABILITIES/SET_CONFIGURATION/OPEN/START/CLOSE/SUSPEND/ABORT/GET_CONFIGURATION/RECONFIGURE/SECURITY_CONTROL/GET_ALL_CAPABILITIES/DELAY_REPORT, error_codes module BAD_HEADER_FORMAT/BAD_LENGTH/BAD_ACP_SEID/SEP_IN_USE/SEP_NOT_IN_USE/BAD_SERVICE_CATEGORY/etc. ~30 error codes, ServiceCategory enum MediaTransport/Reporting/Recovery/ContentProtection/HeaderCompression/Multiplexing/MediaCodec/DelayReporting com from_u8()/to_u8(), MediaType enum Audio/Video/Multimedia, SepType enum Source/Sink, CodecType enum Sbc/Mpeg12Audio/Mpeg24Aac/Atrac/VendorSpecific, sbc_config module constants FREQ_16000/32000/44100/48000 + CHANNEL_MONO/DUAL/STEREO/JOINT + BLOCK_4/8/12/16 + SUBBANDS_4/8 + ALLOC_SNR/LOUDNESS, SbcConfiguration struct sample_rate/channels/block_length/subbands/allocation/min_bitpool/max_bitpool com default()/from_bytes()/to_bytes()/frame_length()/bitrate(), StreamEndpointState enum Idle/Configured/Open/Streaming/Closing/Aborting, StreamEndpoint struct seid/sep_type/in_use/media_type/codec_type/state/remote_seid/sbc_config/transport_cid, A2dpConnectionState enum Disconnected/Connecting/Connected/Configured/Streaming/Error, A2dpRole enum Source/Sink, A2dpConnection struct address/state/role/acl_handle/signaling_cid/local_seps/sequence_number/timestamp/mtu/audio_callback, avdtp_build module discover_request/response()/get_capabilities_request/response()/set_configuration_request/response()/open_request/response()/start_request/response()/close_request/response()/suspend_request/response()/abort_request/response()/general_reject() packet builders, AvdtpHeader struct message_type/packet_type/transaction/signal_id com from_bytes()/to_bytes(), rtp module RtpHeader struct flags1/flags2/sequence/timestamp/ssrc com new()/build()/parse()/payload_type()/marker()/increment_sequence()/set_timestamp(), sbc_media_header struct, A2dpEvent enum Connected/Disconnected/Configured/StreamStarted/StreamSuspended/AudioData para callbacks, A2dpManager struct connections/default_role/ssrc/next_transaction/audio_callback com connect()/disconnect()/discover_endpoints()/get_capabilities()/configure_stream()/start_stream()/suspend_stream()/close_stream()/send_audio()/handle_signaling()/handle_media()/process_discover_response()/process_capabilities_response()/process_configuration_response()/process_open_response()/process_start_response()/process_suspend_response()/encode_sbc()/fire_event(), global A2DP_MANAGER, Public API: init()/connect()/disconnect()/configure()/start_streaming()/suspend_streaming()/send_audio_data()/on_audio()/format_status(), ~900 lines) |
| 2026-01-17 | WPA3 implementado (net/wifi/wpa3.rs: Wpa3Mode enum Personal/PersonalH2E/Enterprise192/Enterprise/Transition com akm_suite()/requires_pmf()/cipher_suite(), SAE (Simultaneous Authentication of Equals) Dragonfly protocol: SaeState enum Nothing/Committed/Confirmed/Accepted, sae_status module SUCCESS/UNSPECIFIED_FAILURE/UNSUPPORTED_FINITE_CYCLIC_GROUP/AUTHENTICATION_REJECTED/ANTI_CLOGGING_TOKEN_REQUIRED/UNKNOWN_PASSWORD_IDENTIFIER, sae_groups module GROUP_19-30 (NIST P-256/384/521 + Brainpool curves) com prime_order(), sae_frame_type COMMIT/CONFIRM, EcPoint struct x/y com from_uncompressed()/to_uncompressed()/coord_size(), SaeCommit struct group_id/token/scalar/element/password_id com parse()/to_bytes(), SaeConfirm struct send_confirm/confirm com parse()/to_bytes(), SaeAuthFrame struct algorithm/seq_num/status/body com parse()/to_bytes(), SaeFrameBody enum Commit/Confirm/TokenRequest/Empty, SaeInstance struct state/group/own_mac/peer_mac/password/password_id/rand/peer_scalar/element/own_scalar/element/pwe/k/kck/pmk/send_confirm/sync/token/pending_commit com initiate()/process()/process_peer_commit()/derive_pwe() hunting-and-pecking/generate_commit()/compute_shared_secret()/derive_keys()/generate_confirm()/verify_confirm()/scalar_mult()/get_pmk()/get_pmkid()/is_complete(), OWE (Opportunistic Wireless Encryption): OweState enum Idle/WaitingAssocResponse/Complete/Failed, owe_groups GROUP_19/20, OweInstance struct state/group/private_key/public_key/peer_public_key/pmk/pmkid com generate_dh_ie()/process_assoc_response()/derive_pmk()/get_pmk()/get_pmkid()/is_complete(), Wpa3Supplicant: Wpa3State enum Idle/SaeInProgress/SaeComplete/Handshaking/Complete/Failed, Wpa3Event enum None/SaeCommitReady/SaeConfirmReady/SaeComplete/Message2Ready/Message4Ready/Complete/Failed, struct sta_mac/ap_mac/mode/sae/owe/pmk/ptk/gtk/snonce/anonce/replay_counter/outgoing com start_sae()/process_sae()/get_owe_dh_ie()/process_owe_assoc()/process_eapol()/process_msg1()/process_msg3()/build_msg2()/build_msg4()/build_rsn_ie()/calculate_mic()/verify_mic()/extract_gtk()/get_outgoing()/get_ptk()/get_gtk()/get_pmk()/is_complete(), crypto helpers sha256/hmac_sha256/hmac_sha1/kdf_sha256/hkdf_expand/constant_time_compare/aes_unwrap/aes_decrypt_block, Public API: create_wpa3_personal()/create_wpa3_transition()/create_owe()/supports_wpa3()/supports_owe()/format_status(), ~1700 lines) |
| 2026-01-17 | SSH Server implementado (net/ssh.rs server extension: SshServerConfig struct port/banner/max_auth_tries/allow_password_auth/allow_pubkey_auth/idle_timeout/max_connections com default port 22, SshHostKeys struct ed25519_private/public/rsa_private/public com generate() auto-keypair generation + ed25519_blob()/ed25519_sign() para host key authentication, SshUser struct username/password_hash/authorized_keys/shell/home_dir/uid/gid com with_password()/add_authorized_key()/verify_password()/verify_pubkey() user management, SshServerState enum WaitingVersion/SentVersion/KeyExchange/Authenticating/Authenticated/Active/Closing/Closed state machine, SshServerSession struct id/tcp_key/state/client_version/server_kex_init/client_kex_init/session_id/exchange_hash/shared_secret/ephemeral keys/encryption keys c2s+s2c/iv c2s+s2c/seq c2s+s2c/encrypted/username/auth_attempts/channels/recv_buffer/pty_allocated/pty_cols/rows com send_raw()/recv_raw()/send_version()/recv_version()/send_kex_init()/recv_kex_init()/handle_kex_ecdh() curve25519 key exchange + signature/derive_keys() A-D key derivation/handle_service_request()/handle_auth_request() password+publickey methods/send_auth_success()/send_auth_failure()/handle_channel_open() session channels/handle_channel_request() pty-req+shell+exec+subsystem+data/send_channel_data()/send_exit_status()/send_packet()/recv_packet() encrypted+unencrypted, ChannelRequest enum Pty/Shell/Exec/Subsystem/Data/Eof/Close para request routing, SshServer struct config/host_keys/users/sessions/next_session_id/running/listen_port com add_user()/remove_user()/start()/stop()/accept()/authenticate()/accept_channel()/poll_session()/send_data()/close_channel()/get_username()/close_session()/session_count()/is_running() full server lifecycle, global SSH_SERVER, Public API: init_server()/server_add_user()/server_start()/server_stop()/format_server_status(), constant_time_eq() secure password comparison, ~1300 lines server extension) |
| 2026-01-17 | FTP Client implementado (net/ftp.rs: RFC 959 File Transfer Protocol, reply_codes module RESTART_MARKER/SERVICE_READY_IN_MINUTES/DATA_CONN_ALREADY_OPEN/FILE_STATUS_OK/COMMAND_OK/SYSTEM_STATUS/DIRECTORY_STATUS/FILE_STATUS/HELP_MESSAGE/SYSTEM_TYPE/SERVICE_READY/SERVICE_CLOSING/DATA_CONN_OPEN/CLOSING_DATA_CONN/ENTERING_PASSIVE_MODE/ENTERING_EXTENDED_PASSIVE/USER_LOGGED_IN/FILE_ACTION_OK/PATHNAME_CREATED/USER_NAME_OK/NEED_ACCOUNT/FILE_ACTION_PENDING/SERVICE_NOT_AVAILABLE/CANT_OPEN_DATA_CONN/CONN_CLOSED_TRANSFER_ABORTED/FILE_ACTION_NOT_TAKEN/ACTION_ABORTED/ACTION_NOT_TAKEN_NO_SPACE/SYNTAX_ERROR/SYNTAX_ERROR_PARAM/COMMAND_NOT_IMPLEMENTED/BAD_COMMAND_SEQUENCE/COMMAND_NOT_IMPLEMENTED_FOR_PARAM/NOT_LOGGED_IN/NEED_ACCOUNT_FOR_STORING/FILE_UNAVAILABLE/PAGE_TYPE_UNKNOWN/EXCEEDED_STORAGE/FILE_NAME_NOT_ALLOWED is_success()/is_intermediate()/is_error() helpers, TransferMode enum Ascii/Binary com command(), ConnectionMode enum Active/Passive para data connections, FtpState enum Disconnected/Connected/LoggedIn/Transferring state machine, FtpResponse struct code/message com is_success()/is_intermediate()/is_error()/is_positive_preliminary(), FtpDirEntry struct name/size/is_dir/modified/permissions com parse_list() LIST output parser, FtpClient struct control/state/server_addr/transfer_mode/connection_mode/current_dir/recv_buffer/timeout_ms com connect()/disconnect()/login()/send_command()/read_response()/read_multiline_response() control channel + setup_data_connection() PORT/PASV handling + open_passive_data_connection()/open_active_data_connection() data channels + send_data()/recv_data() transfer + set_transfer_mode()/set_connection_mode()/pwd()/cwd()/cdup()/list()/nlst()/retrieve()/store()/append()/delete()/mkdir()/rmdir()/rename()/size()/mdtm()/quit()/noop()/syst()/feat()/type_ascii()/type_binary()/pasv()/port() all FTP commands, global FTP_CLIENT, Public API: connect_and_login()/disconnect()/download_file()/upload_file()/list_dir()/delete_file()/make_dir()/remove_dir()/rename_file()/get_file_size()/get_pwd()/change_dir()/set_passive_mode()/set_active_mode()/set_ascii_mode()/set_binary_mode()/format_status(), ~750 lines) |
| 2026-01-17 | Extended Attributes (xattr) implementado (fs/xattr.rs: POSIX extended attributes, XattrNamespace enum User/System/Trusted/Security com from_name()/prefix()/full_name() namespace parsing e prefixing, XattrFlags struct XATTR_CREATE/XATTR_REPLACE com is_create()/is_replace() para setxattr flags, XattrEntry struct value com len()/is_empty(), XattrStorage struct BTreeMap-based storage com get()/set()/remove()/list()/list_namespace()/list_size()/is_empty()/count()/clear() in-memory xattr storage, SyncXattrStorage thread-safe wrapper com RwLock, check_xattr_permission() permission checking per-namespace (User needs file r/w, System read follows file, Trusted/Security root-only), system_attrs module POSIX_ACL_ACCESS/POSIX_ACL_DEFAULT well-known names, security_attrs module SELINUX/APPARMOR/CAPABILITY/IMA/EVM well-known names, trusted_attrs module OVERLAY_OPAQUE/OVERLAY_REDIRECT, Syscall implementations: getxattr()/lgetxattr()/fgetxattr() get attribute value, setxattr()/lsetxattr()/fsetxattr() set attribute, removexattr()/lremovexattr()/fremovexattr() remove attribute, listxattr()/llistxattr()/flistxattr() list all attribute names, Convenience functions: set_user_xattr()/get_user_xattr()/remove_user_xattr() user namespace helpers, set_security_xattr()/get_security_xattr() security namespace, copy_xattrs() copy all xattrs between inodes, VFS integration: added getxattr()/setxattr()/removexattr()/listxattr() to InodeOps trait with default NotSupported returns, tmpfs integration: added SyncXattrStorage field to TmpfsInode struct, implemented all xattr methods for tmpfs filesystem, updated all TmpfsInode creation points to initialize xattrs, XATTR_NAME_MAX 255 bytes / XATTR_SIZE_MAX 64KB / XATTR_LIST_MAX 64KB limits, ~600 lines) |
| 2026-01-17 | POSIX ACLs implementado (fs/acl.rs: POSIX.1e Access Control Lists, AclTag enum UserObj/User/GroupObj/Group/Mask/Other com from_u16()/to_u16()/requires_qualifier()/name() tag types, AclPerm struct READ/WRITE/EXECUTE com from_mode()/bits()/can_read()/can_write()/can_execute()/to_string()/masked() permission bits, AclEntry struct tag/perm/qualifier com new()/user_obj()/user()/group_obj()/group()/mask()/other()/format() individual entries, Acl struct entries Vec com new()/from_mode()/from_mode_with_ids()/add_entry()/remove_entry()/get_entry()/get_entry_mut()/entries()/mask()/is_minimal()/to_mode()/validate()/recalculate_mask()/to_xattr()/from_xattr()/format() full ACL management, check_acl_access() permission checking following POSIX algorithm (owner->named user->group class->other with mask application), DefaultAcl struct para directory inheritance com acl()/acl_mut()/inherit_for_file()/inherit_for_dir()/to_xattr()/from_xattr() default ACL handling, Inode ACL operations: get_acl()/set_acl()/remove_acl()/get_default_acl()/set_default_acl()/remove_default_acl()/has_acl()/has_default_acl() xattr-based storage, Convenience functions: grant_user_read()/grant_user_rw()/grant_user_full()/grant_group_read()/grant_group_rw()/revoke_user()/revoke_group() easy ACL manipulation, format_acl_info() getfacl-style output, ACL_VERSION 0x0002 / ACL_MAX_ENTRIES 32, stored in system.posix_acl_access/system.posix_acl_default xattrs, ~650 lines) |
| 2026-01-17 | Hibernate (S4) implementado (power/hibernate.rs: suspend-to-disk implementation, HibernateState enum Running/Freezing/SavingDevices/Snapshotting/Writing/PoweringOff/Resuming/RestoringDevices/Thawing/Error state machine, HibernateHeader struct signature/version/flags/image_size/page_count/checksum/timestamp/offsets/compression com to_bytes()/from_bytes()/is_valid() image header, CpuHibernateState struct all x86_64 registers (rax-r15/rip/rflags/segments/cr0-4/gdtr/idtr/efer/fs_base/gs_base/kernel_gs_base) com save_current()/restore()/to_bytes()/from_bytes() CPU state save/restore using inline assembly, PageDescriptor struct pfn/flags/compressed_size for memory pages, HibernateConfig struct image_path/compression_enabled/level/max_image_size/resume_delay configuration, HibernateDevice trait name()/suspend()/resume()/save_state()/restore_state() for device drivers, HibernateManager struct state/config/devices/in_progress/last_error/estimated_size com init()/register_device()/unregister_device()/hibernate()/resume()/has_image()/freeze_tasks()/thaw_tasks()/suspend_devices()/resume_devices()/restore_devices()/create_snapshot()/collect_saveable_pages()/collect_device_state()/write_image()/read_image()/restore_snapshot()/clear_image()/power_off()/calculate_checksum()/estimate_image_size() full hibernate lifecycle, HibernateSnapshot struct cpu_state/pages/device_state, power/mod.rs PowerState enum/init()/shutdown()/reboot()/suspend() power management API, ACPI shutdown via ports 0x604/0xB004, keyboard reset 0x64 0xFE, triple fault fallback, ~850 lines) |
| 2026-01-17 | CPU Hotplug implementado (arch/x86_64_arch/cpu_hotplug.rs: dynamic CPU online/offline management, CpuState enum NotPresent/Offline/BringingUp/Online/GoingDown/Dying/Frozen state machine, HotplugAction enum Online/Active/OfflinePrepare/Offline/Dying/Frozen/Thawed for notifiers, NotifierPriority enum Scheduler/High/Normal/Low priority levels, HotplugCallback type for driver notifications, HotplugNotifier struct name/priority/callback, CpuHotplugState struct state/can_offline/ref_count/is_boot_cpu per-CPU tracking, CpuHotplugManager struct cpu_states/notifiers/hotplug_lock/online_count/enabled com init()/register_notifier()/unregister_notifier()/call_notifiers()/cpu_up()/cpu_down()/start_cpu() via SMP start_ap/stop_cpu() via IPI/migrate_tasks_away()/get_state()/is_online()/online_count()/cpu_get()/cpu_put() reference counting/freeze_cpus()/thaw_cpus() for suspend/set_can_offline(), global HOTPLUG_MANAGER, Public API: init()/cpu_up()/cpu_down()/is_online()/get_state()/online_count()/register_notifier()/unregister_notifier()/cpu_get()/cpu_put()/freeze_cpus()/thaw_cpus()/format_status(), smp.rs start_ap() public wrapper for hotplug com stack allocation/INIT-SIPI-SIPI sequence/KResult return, ~600 lines) |
| 2026-01-17 | RTL Text implementado (gui/shaping.rs: complete Right-to-Left text support, Script enum Latin/Arabic/Hebrew/Cyrillic/Greek/Han/Hiragana/Katakana/Hangul/Thai/Devanagari/Tamil/Bengali/Common/Inherited/Unknown com is_rtl()/needs_shaping(), detect_script() Unicode range detection, BidiClass enum 23 UAX #9 classes L/R/AL/EN/ES/ET/AN/CS/NSM/BN/B/S/WS/ON/LRE/LRO/RLE/RLO/PDF/LRI/RLI/FSI/PDI, get_bidi_class() character classification, BidiRun struct start/end/level/visual_pos, BidiParagraph struct rtl/runs/levels/reorder_map com new() simplified UAX #9 algorithm P2-P3/W1-W7/N0-N2/L1-L4/level-based reordering, get_visual_order()/is_pure_ltr()/is_pure_rtl(), Arabic shaping: ArabicJoiningType enum Right/Dual/Causing/NonJoining/Transparent, ArabicForm enum Isolated/Initial/Medial/Final, get_arabic_joining_type() letter classification, get_arabic_forms() presentation forms FE70-FEFF mapping for 28 Arabic letters, shape_arabic() contextual form selection, GraphemeBreak enum CR/LF/Control/Extend/ZWJ/RegionalIndicator/Prepend/SpacingMark/L/V/T/LV/LVT/Other, get_grapheme_break() UAX #29, GraphemeCluster struct, find_grapheme_clusters()/should_break_grapheme() GB3-GB999 rules, ShapedGlyph struct codepoint/cluster/offsets/advances, ShapedRun struct start/end/script/rtl/glyphs, ShaperConfig struct ligatures/kerning/size/dpi, shape_text() full pipeline bidi+script+arabic+glyphs com RTL reordering, itemize_by_script() script itemization, apply_latin_ligatures() ff/fi/fl/ffi/ffl/st, apply_arabic_ligatures() lam-alef, measure_shaped_width()/get_visual_string()/has_rtl()/needs_shaping() utilities, ~1100 lines) |
| 2026-01-17 | Window Transparency implementado (gui/transparency.rs: complete window transparency system, Opacity struct u8 0-255 com from_percent()/to_percent()/as_f32()/is_opaque()/is_transparent() opacity handling, BlendMode enum Normal/Multiply/Screen/Overlay/Add/Subtract/Replace com blend_pixel() Porter-Duff compositing, no_std math helpers round_f32()/exp_f32() Taylor series/sqrt_f32() Newton's method for blur calculations, BlurType enum Box/Gaussian, BlurConfig struct blur_type/radius/passes, box_blur() 2-pass separable horizontal+vertical, gaussian_blur() via box blur approximation, gaussian_kernel() weight generation, ShadowConfig struct color/offset_x/offset_y/blur_radius/spread, generate_shadow() drop shadow surface generation, GlassConfig struct tint_color/tint_opacity/blur_radius/saturation/brightness, apply_glass_effect() Aero-style glass effect, apply_saturation()/apply_brightness() color adjustments, WindowTransparency struct opacity/blend_mode/shadow_enabled/shadow_config/glass_enabled/glass_config/click_through, blit_with_transparency() alpha-blended surface compositing com blend mode support, apply_transparency_effects() combined shadow+glass+opacity pipeline, gui/window.rs: Window struct extended com transparency field, transparency()/transparency_mut()/set_opacity()/set_opacity_percent()/opacity()/opacity_percent()/enable_shadow()/has_shadow()/enable_glass()/has_glass()/set_blend_mode()/blend_mode()/is_opaque()/should_click_through() window methods, gui/mod.rs: pub mod transparency + re-exports Opacity/BlendMode/WindowTransparency/BlurConfig/ShadowConfig/GlassConfig + transparency::init() call, ~700 lines) |
| 2026-01-17 | UI Animations implementado (gui/animations.rs: complete animation framework ~1250 lines, EasingFunction enum 30+ easing functions Linear/EaseInQuad/EaseOutQuad/EaseInOutQuad/EaseInCubic/EaseOutCubic/EaseInOutCubic/EaseInQuart/EaseOutQuart/EaseInOutQuart/EaseInQuint/EaseOutQuint/EaseInOutQuint/EaseInSine/EaseOutSine/EaseInOutSine/EaseInExpo/EaseOutExpo/EaseInOutExpo/EaseInCirc/EaseOutCirc/EaseInOutCirc/EaseInBack/EaseOutBack/EaseInOutBack/EaseInElastic/EaseOutElastic/EaseInOutElastic/EaseInBounce/EaseOutBounce/EaseInOutBounce com apply() Taylor series sin/cos/exp/ln/sqrt approximations for no_std, AnimationState enum Pending/Running/Paused/Completed/Cancelled, AnimationDirection enum Normal/Reverse/Alternate/AlternateReverse, AnimationFillMode enum None/Backwards/Forwards/Both, AnimatedProperty enum X/Y/Width/Height/Opacity/ScaleX/ScaleY/Rotation/Color/BackgroundColor/BorderRadius/Custom com interpolate()/name(), PropertyValue enum Float/Color, lerp()/lerp_color() interpolation, AnimationConfig struct duration_ms/delay_ms/easing/direction/fill_mode/iterations com builder pattern with_easing()/with_delay()/with_direction()/with_iterations()/infinite(), Animation struct id/name/target/properties/config/state/start_time/current_iteration/on_update/on_complete com start()/pause()/resume()/cancel()/update()/is_active()/is_complete(), AnimationSequence struct for sequential playback com start()/update() iteration handling, AnimationGroup struct for parallel playback com start()/update(), presets module fade_in/fade_out/slide_in_left/right/top/bottom/scale_in/scale_out/bounce/shake/pulse/rotate/spin/color_transition/flash/wobble/elastic_in/minimize/maximize common animations, AnimationManager struct animations/sequences/groups BTreeMap tracking com play()/play_sequence()/play_group()/cancel()/pause()/resume()/update()/get()/get_state()/is_active()/active_count()/cancel_all()/cancel_for_window(), global ANIMATION_MANAGER, init()/manager()/play()/play_sequence()/play_group()/update()/cancel()/active_count()/format_status() API, gui/mod.rs: pub mod animations + re-exports Animation/AnimationConfig/AnimationDirection/AnimationFillMode/AnimationGroup/AnimationId/AnimationManager/AnimationSequence/AnimationState/AnimatedProperty/EasingFunction/PropertyValue/animation_presets + animations::init() call) |
| 2026-01-17 | Thunderbolt/USB4 implementado (drivers/thunderbolt.rs: complete Thunderbolt 3/4/5 and USB4 support ~780 lines, device_ids module Intel Alpine Ridge/Titan Ridge/Maple Ridge/Ice Lake/Tiger Lake/Alder Lake/Raptor Lake/Meteor Lake + AMD USB4 device IDs, ThunderboltGeneration enum Thunderbolt1/2/3/4/5/Usb4/Unknown com from_device_id()/name()/max_speed_gbps() 10-120 Gbps, SecurityLevel enum None/User/Secure/DpOnly/UsbOnly com name()/allows_pcie()/allows_displayport()/requires_approval(), TunnelType enum Pcie/DisplayPort/Usb/Dma, TunnelState enum Inactive/Activating/Active/Deactivating/Error, Tunnel struct id/tunnel_type/state/source_port/dest_port/bandwidth_gbps, DeviceType enum Dock/Egpu/Storage/Display/Hub/PcieAdapter/Network/Generic/Unknown, ConnectionState enum Disconnected/PendingAuthorization/Authorizing/Connected/Suspended/Error, ThunderboltDevice struct id/uuid/vendor_id/device_id/names/device_type/state/generation/port/route_string/authorized/security_key/tunnels/upstream_id/downstream_ids/link_speed_gbps/num_lanes com is_connected()/is_pending()/has_pcie_tunnel()/has_dp_tunnel(), nhi_regs module TX/RX ring base/size/head_tail + INTR_STATUS/MASK + CONTROL + SECURITY + FW_VERSION + PORT_STATUS NHI registers, control_bits/port_status_bits register bit definitions, ThunderboltController struct bus/device/function/vendor_id/device_id/generation/mmio_base/security_level/num_ports/devices/hotplug_enabled/firmware_version/nvm_version/initialized com read32()/write32() MMIO + init() BAR0/enable/reset/scan + scan_devices() + authorize_device()/deauthorize_device() + create_tunnel() + handle_hotplug() + info_string(), ThunderboltEvent enum DeviceConnected/Disconnected/Pending/Authorized/TunnelCreated/Removed/Error, ThunderboltManager struct controllers/default_security/event_callbacks com probe() PCI scan + controller_count()/device_count()/pending_count() + find_device()/authorize_device()/set_security_level()/all_devices()/on_event()/format_status(), global THUNDERBOLT_MANAGER, init()/manager()/controller_count()/device_count()/pending_count()/authorize()/set_security()/format_status()/is_thunderbolt_controller() API, drivers/mod.rs: pub mod thunderbolt) |
| 2026-01-17 | Fingerprint Reader implementado (drivers/fingerprint.rs: complete fingerprint sensor support ~900 lines, vendor_ids module Validity/Synaptics/Elan/Goodix/AuthenTec/Upek/Stm/Focal/Fpc USB VIDs, device_ids module VFS495/VFS5011/VFS5111/VFS7552/Prometheus/Metallica/Elan/Goodix/AuthenTec/FPC device IDs, SensorType enum Validity/Synaptics/Elan/Goodix/AuthenTec/Upek/Stm/Focal/Fpc/Unknown com from_vendor_id()/name(), SensorTechnology enum Capacitive/Optical/Ultrasonic/Thermal/Unknown, ScanQuality enum Excellent/Good/Acceptable/Poor/Failed com from_score()/is_acceptable() quality scoring, FingerPosition enum 10 finger positions LeftThumb-RightPinky, EnrollmentState enum Idle/WaitingForFinger/Capturing/Processing/NeedMoreCaptures/Complete/Failed state machine, VerifyResult enum Match/NoMatch/NoFinger/PoorQuality/Error/InProgress, DeviceState enum Disconnected/Connected/Initializing/Ready/Busy/Error/Suspended, FingerprintTemplate struct id/user_id/finger/data/format_version/created_at/last_used/use_count/quality_score/label com new()/touch(), ScanResult struct image/width/height/bpp/quality/timestamp com quality_level(), EnrollmentProgress struct state/captures_done/captures_needed/last_quality/partial_data/user_id/finger com new()/progress_percent(), FingerprintDevice struct vendor_id/device_id/sensor_type/technology/state/resolution_dpi/image_width/height/name/firmware_version/serial_number/cmd_endpoint/data_endpoint/usb_slot_id/enrollment/templates/on_scan/on_match com init()/init_validity()/init_elan()/init_goodix()/init_generic() + capture()/start_enrollment()/enrollment_capture()/finish_enrollment()/cancel_enrollment()/verify()/verify_user()/get_template()/get_user_templates()/delete_template()/delete_user_templates()/info_string(), FingerprintManager struct devices/default_device com probe()/register_device()/device_count()/get_device()/get_default_device()/set_default_device()/enroll()/verify()/verify_user()/all_templates()/template_count()/format_status(), global FINGERPRINT_MANAGER, init()/manager()/device_count()/verify()/verify_user()/enroll()/template_count()/format_status()/is_fingerprint_device()/register_usb_device() API, drivers/mod.rs: pub mod fingerprint) |
| 2026-01-17 | DirectX 9 implementado (compat/windows/d3d9.rs: complete Direct3D 9 compatibility layer ~1400 lines, d3d_ok/d3derr result codes SUCCESS/INVALIDCALL/NOTAVAILABLE/OUTOFVIDEOMEMORY/DEVICELOST/etc., D3DFormat enum 50+ formats R8G8B8/A8R8G8B8/X8R8G8B8/R5G6B5/D24S8/DXT1-5/etc. com bits_per_pixel()/is_depth_format(), D3DDevType enum Hal/Ref/Sw/NullRef, D3DResourceType enum Surface/Volume/Texture/VertexBuffer/IndexBuffer, D3DPool enum Default/Managed/SystemMem/Scratch, D3DMultiSample enum None/2-16 samples, D3DSwapEffect enum Discard/Flip/Copy/Overlay, D3DPrimitiveType enum PointList/LineList/LineStrip/TriangleList/TriangleStrip/TriangleFan com vertex_count(), D3DTransformState enum View/Projection/Texture0-7/World0-3, D3DRenderState enum 100+ states ZEnable/FillMode/CullMode/AlphaBlendEnable/Lighting/etc., D3DTextureStageState/D3DSamplerState enums, d3dfvf module FVF flags XYZ/XYZRHW/NORMAL/DIFFUSE/SPECULAR/TEX0-8 com vertex_size(), d3dclear/d3dusage/d3dlock flags, D3DPresentParameters struct backbuffer/format/multisampling/swap/windowed/depth configuration, D3DMatrix struct 4x4 float com identity()/zero(), D3DViewport9 struct x/y/width/height/minz/maxz, D3DMaterial9/D3DColorValue/D3DLight9/D3DVector structs, D3DLockedRect/D3DSurfaceDesc structs, D3D9Texture struct handle/width/height/levels/format/pool/usage/data mip levels, D3D9VertexBuffer/D3D9IndexBuffer structs, D3D9DeviceState struct render_states/texture_stage_states/sampler_states/transforms/viewport/material/lights/fvf/shaders/stream_sources/textures, D3D9Device struct handle/present_params/state/textures/vertex_buffers/index_buffers/in_scene/device_lost/frame_count com begin_scene()/end_scene()/present()/clear()/set_render_state()/get_render_state()/set_transform()/get_transform()/set_viewport()/get_viewport()/set_fvf()/create_texture()/set_texture()/create_vertex_buffer()/create_index_buffer()/set_stream_source()/set_indices()/draw_primitive()/draw_indexed_primitive()/draw_primitive_up()/set_material()/set_light()/light_enable()/reset()/test_cooperative_level(), D3DAdapterIdentifier struct, Direct3D9 struct adapter_count/devices com get_adapter_count()/get_adapter_identifier()/check_device_type()/create_device()/get_device(), global D3D9_INSTANCE, direct3d_create9()/init()/format_status() API, compat/windows/mod.rs: pub mod d3d9 + init() + FeatureStatus D3D9) |
| 2026-01-17 | .NET CLR implementado (compat/windows/clr.rs: basic Common Language Runtime ~1100 lines based on ECMA-335 CLI spec, ClrError enum InvalidAssembly/InvalidMetadata/TypeNotFound/MethodNotFound/FieldNotFound/InvalidIL/StackOverflow/NullReference/InvalidCast/IndexOutOfRange/NotImplemented/SecurityException/OutOfMemory, ElementType enum 30+ CLI types Void/Boolean/Char/I1/U1/I2/U2/I4/U4/I8/U8/R4/R8/String/Ptr/ByRef/ValueType/Class/Var/Array/GenericInst/TypedByRef/I/U/FnPtr/Object/SzArray/MVar/CmodReqd/CmodOpt/Internal/Modifier/Sentinel/Pinned, ILOpcode enum 70+ IL opcodes Nop/Break/Ldarg_0-3/Ldloc_0-3/Stloc_0-3/Ldnull/Ldc_I4_M1-8/Ldc_I4/Ldc_I4_S/Ldc_I8/Ldc_R4/Ldc_R8/Dup/Pop/Jmp/Call/Calli/Ret/Br/Brfalse/Brtrue/Beq/Bge/Bgt/Ble/Blt/Bne_Un/Bge_Un/Bgt_Un/Ble_Un/Blt_Un/Switch/Ldind/Stind/Add/Sub/Mul/Div/Div_Un/Rem/And/Or/Xor/Shl/Shr/Neg/Not/Conv/Callvirt/Ldobj/Ldstr/Newobj/Castclass/Isinst/Box/Unbox/Throw/Ldfld/Ldflda/Stfld/Ldsfld/Ldsflda/Stsfld/Stobj/Newarr/Ldlen/Ldelema/Ldelem/Stelem/Unbox_Any/Ceq/Cgt/Clt/Initobj/Sizeof/Rethrow/Endfinally/Leave, TypeAttributes/FieldAttributes/MethodAttributes bitflags, TypeDef struct token/name/namespace/base_type/attributes/fields/methods/size/pack/nested_types com full_name(), FieldDef struct token/name/type_sig/attributes/offset/parent_type, MethodDef struct token/name/signature/attributes/rva/il_code/locals/max_stack/entry_point, ClrValue enum Null/Int32/Int64/Float32/Float64/String/Object/Array/Boolean/Char/IntPtr/UIntPtr com as_i32()/as_i64()/as_f32()/as_f64()/as_bool()/is_null()/type_name(), ManagedObject struct handle/type_def/fields_data/sync_block/gc_generation/marked, ManagedArray struct handle/element_type/length/data/gc_generation/marked, GcGeneration enum Gen0/Gen1/Gen2/Large, GarbageCollector struct gen0/gen1/gen2/large_objects/next_handle/gc_count/total_allocated com alloc_object()/alloc_array()/mark()/sweep()/collect()/collect_gen0/1/2()/heap_size()/object_count(), StackFrame struct method/pc/locals/eval_stack/arg_count, ClrAssembly struct name/version_major/minor/build/revision/public_key_token/types/fields/methods/strings/user_strings/blobs/guids/entry_point com find_type()/find_method()/get_string()/get_user_string(), ClrRuntime struct assemblies/gc/call_stack/static_fields/string_pool/current_domain/trusted com load_assembly()/execute_entry_point()/execute_il() main IL interpreter loop com 50+ opcode implementations arithmetic/comparison/branching/method-calls/field-access/object-creation/array-ops, create_clr_runtime()/clr_runtime()/load_assembly()/execute_entry_point() API, init()/format_status(), compat/windows/mod.rs: pub mod clr + clr::init() + FeatureStatus .NET CLR Partial) |
| 2026-01-17 | Container Runtime implementado (compat/containers.rs: Docker/OCI-compatible container runtime ~870 lines, ContainerState enum Created/Running/Paused/Stopped/Removing com as_str(), ResourceLimits struct cpu_shares/cpu_quota/cpu_count/memory_limit/memory_swap_limit/pids_limit/blkio_weight/oom_kill_disable, NetworkMode enum None/Host/Bridge/Container, Mount struct source/destination/mount_type/options/read_only, ContainerConfig struct image/command/working_dir/env/user/hostname/domainname/mounts/network_mode/resources/privileged/read_only_rootfs/labels/restart_policy/port_bindings, RestartPolicy enum No/OnFailure/Always/UnlessStopped, Container struct id/short_id/name/config/state/pid/exit_code/created_at/started_at/finished_at/namespaces/cgroup_path/cgroup_id/rootfs/restart_count/running com new()/is_running()/info(), ContainerError enum NotFound/AlreadyExists/AlreadyRunning/NotRunning/InvalidConfig/ResourceLimit/NamespaceError/CgroupError/MountError/NetworkError/ImageNotFound/StartFailed/StopFailed/ExecFailed, ContainerRuntime struct containers/name_to_id/next_num/bridge_name/bridge_subnet/next_ip com generate_id()/generate_name()/create()/start()/stop()/kill()/pause()/unpause()/remove()/list()/get_container()/resolve_id()/inspect()/logs()/exec()/stats() full container lifecycle, ContainerInfo/ContainerInspect/ContainerStats structs, global CONTAINER_RUNTIME, init()/runtime() API, sys_container_create/start/stop/kill/remove/list/pause/unpause/inspect/stats() syscall interface, integrates namespaces/cgroups/seccomp for isolation, compat/mod.rs: init_containers() + init_all() updated) |

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

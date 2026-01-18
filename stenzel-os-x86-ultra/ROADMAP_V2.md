# Stenzel OS - Roadmap V2: Production Ready

> **Objetivo:** Tornar o Stenzel OS um sistema operacional de ponta, 100% usável em hardware real (notebooks/desktops), competitivo com Linux/Windows/macOS.

> **Baseline:** Este roadmap assume que o ROADMAP.md V1 foi completado (kernel funcional, drivers básicos, GUI básica, compatibilidade).

---

## Sumário

1. [Instalador e Deploy](#1-instalador-e-deploy)
2. [Hardware Real - Drivers de Produção](#2-hardware-real---drivers-de-produção)
3. [GPU e Gráficos](#3-gpu-e-gráficos)
4. [Desktop Environment Completo](#4-desktop-environment-completo)
5. [Aplicações Essenciais](#5-aplicações-essenciais)
6. [Gerenciador de Pacotes](#6-gerenciador-de-pacotes)
7. [Áudio Avançado](#7-áudio-avançado)
8. [Rede e Conectividade](#8-rede-e-conectividade)
9. [Power Management](#9-power-management)
10. [Segurança Avançada](#10-segurança-avançada)
11. [Input e Acessibilidade](#11-input-e-acessibilidade)
12. [Internacionalização](#12-internacionalização)
13. [Performance e Otimização](#13-performance-e-otimização)
14. [Cloud e Virtualização](#14-cloud-e-virtualização)
15. [Testes em Hardware Real](#15-testes-em-hardware-real)
16. [Ecossistema e Comunidade](#16-ecossistema-e-comunidade)

---

## Legenda de Status

| Símbolo | Significado |
|---------|-------------|
| ⬜ | Pendente |
| 🔄 | Em progresso |
| ✅ | Concluído |
| ❌ | Cancelado/N/A |
| 🔴 | Crítico (bloqueador) |
| 🟡 | Importante |
| 🟢 | Nice-to-have |

---

## 1. Instalador e Deploy
> Sistema completo de instalação para hardware real.

### 1.1 Instalador Gráfico
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Live USB Boot | Boot de pendrive USB com sistema live | ✅ Concluído | 🔴 Crítico |
| Detecção de Hardware | Scan automático de CPU, RAM, discos, GPU | ✅ Concluído | 🔴 Crítico |
| Particionamento | Particionador gráfico (GPT/MBR, resize, criar) | ✅ Concluído | 🔴 Crítico |
| Dual Boot | Detecção e configuração de dual boot com Windows/Linux | ✅ Concluído | 🟡 Importante |
| Formatação | Suporte a ext4, btrfs, xfs, FAT32, NTFS | ✅ Concluído | 🔴 Crítico |
| Cópia de Sistema | Instalação otimizada com barra de progresso | ✅ Concluído | 🔴 Crítico |
| Configuração de Usuário | Criação de usuário, senha, hostname | ✅ Concluído | 🔴 Crítico |
| Timezone/Locale | Seleção de fuso horário e idioma | ✅ Concluído | 🟡 Importante |
| Bootloader Install | Instalação de bootloader (GRUB/systemd-boot) | ✅ Concluído | 🔴 Crítico |
| UEFI Boot Entry | Criação de entrada EFI boot | ✅ Concluído | 🔴 Crítico |
| Recovery Partition | Partição de recuperação | ✅ Concluído | 🟡 Importante |
| Encryption Setup | LUKS encryption durante instalação | ✅ Concluído | 🟡 Importante |

### 1.2 Imagens e Distribuição
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| ISO Builder | Script para gerar ISO bootável | ✅ Concluído | 🔴 Crítico |
| Hybrid ISO | ISO bootável em BIOS e UEFI | ✅ Concluído | 🔴 Crítico |
| Netinstall | Instalação mínima via rede | ⬜ Pendente | 🟢 Nice-to-have |
| OEM Install | Modo de instalação para fabricantes | ⬜ Pendente | 🟢 Nice-to-have |
| Raspberry Pi Image | Imagem para ARM (futuro) | ⬜ Pendente | 🟢 Nice-to-have |
| Cloud Images | AMI, qcow2, VHD para cloud | ✅ Concluído | 🟡 Importante |
| Docker Base Image | Imagem base para containers | ✅ Concluído | 🟡 Importante |

### 1.3 Updates e Recovery
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| System Updater | Atualizações de sistema com rollback | ✅ Concluído | 🔴 Crítico |
| A/B Partitions | Sistema de partições A/B para updates seguros | ✅ Concluído | 🟡 Importante |
| Recovery Mode | Boot em modo de recuperação | ✅ Concluído | 🔴 Crítico |
| Factory Reset | Reset para configurações de fábrica | ✅ Concluído | 🟡 Importante |
| Backup/Restore | Backup e restauração de sistema | ✅ Concluído | 🟡 Importante |

---

## 2. Hardware Real - Drivers de Produção
> Drivers testados e funcionais em hardware real, não apenas QEMU.

### 2.1 Storage Controllers
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| NVMe Real | Driver NVMe testado em SSDs reais | ✅ Concluído | 🔴 Crítico |
| AHCI/SATA Real | Driver SATA testado em HDDs/SSDs reais | ✅ Concluído | 🔴 Crítico |
| Intel RST | Intel Rapid Storage Technology | ✅ Concluído | 🟡 Importante |
| AMD StoreMI | AMD storage acceleration | ⬜ Pendente | 🟢 Nice-to-have |
| eMMC | Suporte a eMMC (tablets, Chromebooks) | ✅ Concluído | 🟡 Importante |
| SD Card | Leitor de cartão SD (SDHCI) | ✅ Concluído | 🟡 Importante |
| USB Mass Storage | USB drives, pendrives | ✅ Concluído | 🔴 Crítico |

### 2.2 USB Controllers Reais
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| xHCI Real | USB 3.x em hardware real (Intel, AMD, Renesas) | ✅ Concluído | 🔴 Crítico |
| EHCI Real | USB 2.0 em hardware legado | ✅ Concluído | 🟡 Importante |
| USB Hub Handling | Hubs USB multinível | ✅ Concluído | 🔴 Crítico |
| USB Hotplug | Plug/unplug dinâmico | ✅ Concluído | 🔴 Crítico |
| USB Power Management | Suspend/resume de dispositivos USB | ✅ Concluído | 🟡 Importante |

### 2.3 Chipset e Platform
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Intel PCH | Platform Controller Hub (série 100-700) | ✅ Concluído | 🔴 Crítico |
| AMD FCH | Fusion Controller Hub | ✅ Concluído | 🔴 Crítico |
| Intel ME Interface | Management Engine (básico) | ⬜ Pendente | 🟢 Nice-to-have |
| AMD PSP Interface | Platform Security Processor | ⬜ Pendente | 🟢 Nice-to-have |
| SMBus/I2C | System Management Bus | ✅ Concluído | 🔴 Crítico |
| GPIO | General Purpose I/O | ✅ Concluído | 🟡 Importante |
| LPC/eSPI | Low Pin Count / Enhanced SPI | ✅ Concluído | 🟡 Importante |

### 2.4 ACPI Completo
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| ACPI Tables Parser | DSDT, SSDT parsing completo | ✅ Concluído | 🔴 Crítico |
| AML Interpreter | ACPI Machine Language interpreter | ✅ Concluído | 🔴 Crítico |
| Power Button | Evento de botão power | ✅ Concluído | 🔴 Crítico |
| Lid Switch | Evento de fechar/abrir tampa | ✅ Concluído | 🔴 Crítico |
| AC Adapter | Detecção de carregador conectado | ✅ Concluído | 🔴 Crítico |
| Thermal Zones | Zonas térmicas ACPI | ✅ Concluído | 🔴 Crítico |
| Fan Control | Controle de ventoinhas via ACPI | ✅ Concluído | 🟡 Importante |
| ACPI Hotkeys | Teclas de função (Fn+F1, etc.) | ✅ Concluído | 🔴 Crítico |
| ACPI Backlight | Controle de brilho via ACPI | ✅ Concluído | 🔴 Crítico |
| ACPI Battery | Informações de bateria (capacidade, ciclos) | ✅ Concluído | 🔴 Crítico |
| ACPI Dock | Suporte a docking stations | ⬜ Pendente | 🟢 Nice-to-have |

### 2.5 Firmware
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Firmware Loader | Carregamento de firmware de /lib/firmware | ✅ Concluído | 🔴 Crítico |
| CPU Microcode | Intel/AMD microcode updates | ✅ Concluído | 🔴 Crítico |
| UEFI Runtime | UEFI runtime services | ✅ Concluído | 🟡 Importante |
| UEFI Variables | Leitura/escrita de variáveis EFI | ✅ Concluído | 🟡 Importante |
| fwupd Support | Firmware update daemon | ✅ Concluído | 🟡 Importante |

---

## 3. GPU e Gráficos
> Drivers de GPU reais com aceleração 2D/3D.

### 3.1 Intel Graphics (i915)
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Intel Gen9 (Skylake+) | HD 520/530, UHD 620/630 | ✅ Concluído | 🔴 Crítico |
| Intel Gen11 (Ice Lake) | Iris Plus Graphics | ✅ Concluído | 🔴 Crítico |
| Intel Gen12 (Tiger Lake+) | Xe Graphics | ✅ Concluído | 🔴 Crítico |
| Intel Arc (Alchemist) | Arc A-series discrete | ✅ Concluído | 🟡 Importante |
| GEM/GTT | Graphics memory management | ✅ Concluído | 🔴 Crítico |
| Display Pipe | Display pipeline configuration | ✅ Concluído | 🔴 Crítico |
| Power Wells | GPU power management | ✅ Concluído | 🟡 Importante |
| GuC/HuC Firmware | Firmware loading | ✅ Concluído | 🔴 Crítico |

### 3.2 AMD Graphics (amdgpu)
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| AMD GCN 4 (Polaris) | RX 400/500 series | ✅ Concluído | 🟡 Importante |
| AMD GCN 5 (Vega) | Vega 56/64, APUs | ✅ Concluído | 🟡 Importante |
| AMD RDNA 1 (Navi) | RX 5000 series | ✅ Concluído | 🟡 Importante |
| AMD RDNA 2 | RX 6000 series, Steam Deck | ✅ Concluído | 🔴 Crítico |
| AMD RDNA 3 | RX 7000 series | ✅ Concluído | 🟡 Importante |
| AMD APU | Ryzen integrated graphics | ✅ Concluído | 🔴 Crítico |
| AMD SMU | System Management Unit | ✅ Concluído | 🟡 Importante |
| AMD PowerPlay | Power management | ✅ Concluído | 🟡 Importante |

### 3.3 NVIDIA Graphics (nouveau/proprietary)
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Nouveau Basic | Open source NVIDIA driver (basic) | ✅ Concluído | 🟡 Importante |
| NVIDIA Firmware | Signed firmware loading | ✅ Concluído | 🟡 Importante |
| NVIDIA Optimus | Hybrid graphics switching | ✅ Concluído | 🟡 Importante |

### 3.4 Display Infrastructure
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| DRM/KMS | Direct Rendering Manager, Kernel Mode Setting | ✅ Concluído | 🔴 Crítico |
| DRM Framebuffer | DRM-based framebuffer | ✅ Concluído | 🔴 Crítico |
| Multi-Monitor | Múltiplos monitores | ✅ Concluído | 🔴 Crítico |
| Hotplug Display | Conectar/desconectar monitores | ✅ Concluído | 🔴 Crítico |
| HDMI | Saída HDMI com áudio | ✅ Concluído | 🔴 Crítico |
| DisplayPort | DP 1.4, MST (daisy chain) | ✅ Concluído | 🔴 Crítico |
| USB-C Display | DisplayPort Alt Mode | ✅ Concluído | 🟡 Importante |
| eDP | Embedded DisplayPort (laptops) | ✅ Concluído | 🔴 Crítico |
| VRR/FreeSync | Variable refresh rate | ✅ Concluído | 🟢 Nice-to-have |
| HDR | High Dynamic Range | ✅ Concluído | 🟢 Nice-to-have |
| HiDPI Scaling | 4K/Retina display scaling | ✅ Concluído | 🟡 Importante |

### 3.5 3D Acceleration
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| OpenGL 4.6 | OpenGL implementation | ✅ Concluído | 🟡 Importante |
| Vulkan 1.3 | Vulkan implementation | ✅ Concluído | 🔴 Crítico |
| Mesa Integration | Mesa 3D library | ✅ Concluído | 🔴 Crítico |
| VA-API | Video Acceleration API | ✅ Concluído | 🟡 Importante |
| VDPAU | Video decode acceleration | ✅ Concluído | 🟡 Importante |
| OpenCL | GPU compute | ⬜ Pendente | 🟢 Nice-to-have |

---

## 4. Desktop Environment Completo
> DE completo estilo GNOME/KDE/macOS.

### 4.1 Shell e Compositor
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Wayland Compositor | Compositor Wayland nativo | ✅ Concluído | 🔴 Crítico |
| X11 Compatibility | XWayland para apps legados | ✅ Concluído | 🔴 Crítico |
| Desktop Shell | Shell com painel, dock, overview | ✅ Concluído | 🔴 Crítico |
| App Launcher | Launcher de aplicações (Spotlight-like) | ✅ Concluído | 🔴 Crítico |
| Notification Center | Centro de notificações | ✅ Concluído | 🔴 Crítico |
| System Tray | Área de ícones de sistema | ✅ Concluído | 🔴 Crítico |
| Quick Settings | Configurações rápidas (WiFi, Bluetooth, etc.) | ✅ Concluído | 🔴 Crítico |
| Lock Screen | Tela de bloqueio com senha/biometria | ✅ Concluído | 🔴 Crítico |
| Login Manager | Display manager (GDM-like) | ✅ Concluído | 🔴 Crítico |

### 4.2 Window Management
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Window Decorations | Decorações de janela (título, botões) | ✅ Concluído | 🔴 Crítico |
| Window Snapping | Snap to edges, quarters | ✅ Concluído | 🔴 Crítico |
| Virtual Desktops | Múltiplas áreas de trabalho | ✅ Concluído | 🔴 Crítico |
| Overview Mode | Visão geral de janelas (Exposé) | ✅ Concluído | 🟡 Importante |
| Picture-in-Picture | Janela flutuante sempre visível | ✅ Concluído | 🟢 Nice-to-have |
| Tiling Mode | Modo de tiling automático | ✅ Concluído | 🟢 Nice-to-have |
| Window Animations | Animações de janela fluidas | ✅ Concluído | 🟡 Importante |
| Alt+Tab | Alternador de janelas | ✅ Concluído | 🔴 Crítico |
| Drag and Drop | Arrastar entre janelas | ✅ Concluído | 🔴 Crítico |

### 4.3 Theming e Aparência
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Theme Engine | Motor de temas (GTK-like) | ✅ Concluído | 🟡 Importante |
| Dark Mode | Modo escuro global | ✅ Concluído | 🔴 Crítico |
| Accent Colors | Cores de destaque personalizáveis | ✅ Concluído | 🟢 Nice-to-have |
| Icon Theme | Sistema de ícones | ✅ Concluído | 🔴 Crítico |
| Cursor Theme | Temas de cursor | ✅ Concluído | 🟡 Importante |
| Font Rendering | Renderização de fontes (FreeType, HarfBuzz) | ✅ Concluído | 🔴 Crítico |
| Wallpaper | Papéis de parede, slideshow | ✅ Concluído | 🔴 Crítico |
| Dynamic Wallpaper | Wallpaper que muda com hora do dia | ✅ Concluído | 🟢 Nice-to-have |

### 4.4 Settings App
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Display Settings | Resolução, escala, multi-monitor | ✅ Concluído | 🔴 Crítico |
| Sound Settings | Volume, dispositivos de áudio | ✅ Concluído | 🔴 Crítico |
| Network Settings | WiFi, Ethernet, VPN | ✅ Concluído | 🔴 Crítico |
| Bluetooth Settings | Pareamento, dispositivos | ✅ Concluído | 🔴 Crítico |
| Power Settings | Bateria, economia de energia | ✅ Concluído | 🔴 Crítico |
| Keyboard Settings | Layout, atalhos | ✅ Concluído | 🔴 Crítico |
| Mouse/Touchpad | Velocidade, gestos | ✅ Concluído | 🔴 Crítico |
| Users & Accounts | Gerenciamento de usuários | ✅ Concluído | 🔴 Crítico |
| Date & Time | Fuso horário, NTP | ✅ Concluído | 🟡 Importante |
| Privacy Settings | Permissões, histórico | ✅ Concluído | 🟡 Importante |
| Default Apps | Aplicativos padrão | ✅ Concluído | 🟡 Importante |
| About | Informações do sistema | ✅ Concluído | 🟡 Importante |

---

## 5. Aplicações Essenciais
> Aplicativos que todo usuário precisa.

### 5.1 Sistema de Arquivos
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| File Manager | Gerenciador de arquivos (Finder/Nautilus-like) | ✅ Concluído | 🔴 Crítico |
| Trash | Lixeira | ✅ Concluído | 🔴 Crítico |
| Archive Manager | Compactar/descompactar (zip, tar, 7z) | ✅ Concluído | 🟡 Importante |
| Disk Utility | Gerenciador de discos | ✅ Concluído | 🟡 Importante |
| Search | Busca de arquivos | ✅ Concluído | 🔴 Crítico |
| Recent Files | Arquivos recentes | ✅ Concluído | 🟡 Importante |
| Thumbnails | Miniaturas de imagens/vídeos | ✅ Concluído | 🟡 Importante |
| Network Shares | SMB/NFS browser | ✅ Concluído | 🟡 Importante |

### 5.2 Terminal e Desenvolvimento
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Terminal Emulator | Emulador de terminal gráfico | ✅ Concluído | 🔴 Crítico |
| Terminal Tabs | Abas no terminal | ✅ Concluído | 🟡 Importante |
| Terminal Profiles | Perfis de terminal | ✅ Concluído | 🟢 Nice-to-have |
| Text Editor | Editor de texto (VSCode-like básico) | ✅ Concluído | 🔴 Crítico |
| Syntax Highlighting | Destaque de sintaxe | ✅ Concluído | 🟡 Importante |
| Git Integration | Integração Git básica | ✅ Concluído | 🟡 Importante |

### 5.3 Web Browser
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Browser Engine | Motor de renderização (WebKit/Gecko port) | ✅ Concluído | 🔴 Crítico |
| JavaScript Engine | Motor JS (JavaScriptCore/SpiderMonkey) | ✅ Concluído | 🔴 Crítico |
| Browser UI | Interface do navegador | ✅ Concluído | 🔴 Crítico |
| Tabs | Abas de navegação | ✅ Concluído | 🔴 Crítico |
| Bookmarks | Favoritos | ✅ Concluído | 🟡 Importante |
| History | Histórico de navegação | ✅ Concluído | 🟡 Importante |
| Downloads | Gerenciador de downloads | ✅ Concluído | 🔴 Crítico |
| Extensions | Suporte a extensões | ⬜ Pendente | 🟢 Nice-to-have |
| Password Manager | Gerenciador de senhas integrado | ✅ Concluído | 🟡 Importante |
| WebRTC | Chamadas de vídeo no browser | ✅ Concluído | 🟡 Importante |

### 5.4 Multimídia
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Image Viewer | Visualizador de imagens | ✅ Concluído | 🔴 Crítico |
| Video Player | Player de vídeo (VLC-like) | ✅ Concluído | 🔴 Crítico |
| Music Player | Player de música | ✅ Concluído | 🟡 Importante |
| Webcam App | Aplicativo de webcam | ✅ Concluído | 🟡 Importante |
| Screen Recorder | Gravador de tela | ✅ Concluído | 🟡 Importante |
| Screenshot Tool | Ferramenta de captura de tela | ✅ Concluído | 🔴 Crítico |
| Photo Editor | Editor de fotos básico | ⬜ Pendente | 🟢 Nice-to-have |

### 5.5 Comunicação
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Email Client | Cliente de email | ✅ Concluído | 🟡 Importante |
| Calendar | Calendário | ✅ Concluído | 🟡 Importante |
| Contacts | Gerenciador de contatos | ✅ Concluído | 🟡 Importante |
| Video Calls | App de videochamada | ⬜ Pendente | 🟢 Nice-to-have |
| Chat | App de mensagens | ⬜ Pendente | 🟢 Nice-to-have |

### 5.6 Utilitários
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Calculator | Calculadora | ✅ Concluído | 🟡 Importante |
| System Monitor | Monitor de sistema (CPU, RAM, processos) | ✅ Concluído | 🔴 Crítico |
| Font Viewer | Visualizador de fontes | ⬜ Pendente | 🟢 Nice-to-have |
| Character Map | Mapa de caracteres | ⬜ Pendente | 🟢 Nice-to-have |
| Notes | Aplicativo de notas | ✅ Concluído | 🟡 Importante |
| PDF Viewer | Visualizador de PDF | ✅ Concluído | 🔴 Crítico |
| Printer Settings | Configuração de impressoras | ✅ Concluído | 🟡 Importante |

---

## 6. Gerenciador de Pacotes
> Sistema de instalação e atualização de software.

### 6.1 Core Package Manager
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Package Format | Formato de pacote (.spkg ou similar) | ✅ Concluído | 🔴 Crítico |
| Package Database | Banco de dados de pacotes instalados | ✅ Concluído | 🔴 Crítico |
| Dependency Resolution | Resolução de dependências | ✅ Concluído | 🔴 Crítico |
| Install/Remove | Instalar e remover pacotes | ✅ Concluído | 🔴 Crítico |
| Upgrade | Atualização de pacotes | ✅ Concluído | 🔴 Crítico |
| Repository System | Sistema de repositórios | ✅ Concluído | 🔴 Crítico |
| GPG Signing | Assinatura de pacotes | ✅ Concluído | 🔴 Crítico |
| Delta Updates | Updates incrementais | ⬜ Pendente | 🟢 Nice-to-have |
| Rollback | Reverter atualizações | ✅ Concluído | 🟡 Importante |

### 6.2 App Store GUI
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Software Center | Loja de aplicativos gráfica | ✅ Concluído | 🔴 Crítico |
| Categories | Categorias de apps | ✅ Concluído | 🟡 Importante |
| Search | Busca de aplicativos | ✅ Concluído | 🔴 Crítico |
| Screenshots | Capturas de tela dos apps | ✅ Concluído | 🟡 Importante |
| Reviews/Ratings | Avaliações e comentários | ⬜ Pendente | 🟢 Nice-to-have |
| Update Notifications | Notificações de atualização | ✅ Concluído | 🔴 Crítico |
| Auto Updates | Atualizações automáticas | ✅ Concluído | 🟡 Importante |

### 6.3 Flatpak/Snap Compatibility
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Flatpak Support | Executar apps Flatpak | ✅ Concluído | 🟡 Importante |
| Snap Support | Executar apps Snap | ⬜ Pendente | 🟢 Nice-to-have |
| AppImage Support | Executar AppImages | ✅ Concluído | 🟡 Importante |

---

## 7. Áudio Avançado
> Sistema de áudio profissional.

### 7.1 Audio Server
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Audio Daemon | Servidor de áudio (PipeWire-like) | ✅ Concluído | 🔴 Crítico |
| PulseAudio Compat | Compatibilidade com PulseAudio API | ✅ Concluído | 🔴 Crítico |
| ALSA Compat | Compatibilidade com ALSA API | ✅ Concluído | 🔴 Crítico |
| JACK Compat | Compatibilidade com JACK (pro audio) | ⬜ Pendente | 🟢 Nice-to-have |
| Per-App Volume | Volume por aplicativo | ✅ Concluído | 🔴 Crítico |
| Audio Routing | Roteamento de áudio entre apps | ✅ Concluído | 🟡 Importante |
| Low Latency | Baixa latência para música | ✅ Concluído | 🟡 Importante |

### 7.2 Audio Devices
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Bluetooth Audio | A2DP, HFP, aptX, LDAC | ✅ Concluído | 🔴 Crítico |
| USB Audio | USB DACs, interfaces de áudio | ✅ Concluído | 🔴 Crítico |
| HDMI Audio | Áudio via HDMI/DisplayPort | ✅ Concluído | 🔴 Crítico |
| Microphone | Entrada de microfone | ✅ Concluído | 🔴 Crítico |
| Noise Cancellation | Cancelamento de ruído | ⬜ Pendente | 🟢 Nice-to-have |
| Spatial Audio | Áudio espacial/surround | ⬜ Pendente | 🟢 Nice-to-have |

### 7.3 Audio Codecs
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| MP3 Decode | Decodificação MP3 | ✅ Concluído | 🔴 Crítico |
| AAC Decode | Decodificação AAC | ✅ Concluído | 🔴 Crítico |
| FLAC | Suporte FLAC | ✅ Concluído | 🟡 Importante |
| Opus | Codec Opus | ✅ Concluído | 🟡 Importante |
| Vorbis | OGG Vorbis | ✅ Concluído | 🟡 Importante |

---

## 8. Rede e Conectividade
> Networking de produção.

### 8.1 WiFi Real
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Intel WiFi 6/6E | AX200, AX201, AX210, AX211 | ✅ Concluído | 🔴 Crítico |
| Intel WiFi 7 | BE200, BE202 | ✅ Concluído | 🟡 Importante |
| Realtek WiFi | RTL8821, RTL8822, RTL8852 | ✅ Concluído | 🟡 Importante |
| MediaTek WiFi | MT7921, MT7922 | ✅ Concluído | 🟡 Importante |
| Broadcom WiFi | BCM43xx | ✅ Concluído | 🟡 Importante |
| Atheros WiFi | ath10k, ath11k | ✅ Concluído | 🟡 Importante |
| WiFi Firmware | Firmware loading de WiFi | ✅ Concluído | 🔴 Crítico |
| WPA3 Enterprise | WPA3-Enterprise | ✅ Concluído | 🟡 Importante |
| WiFi Direct | P2P WiFi | ⬜ Pendente | 🟢 Nice-to-have |
| Hotspot Mode | Access Point mode | ✅ Concluído | 🟡 Importante |
| WiFi 6 GHz | Suporte a banda 6 GHz | ✅ Concluído | 🟡 Importante |

### 8.2 Ethernet Real
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Intel I219 | Intel Ethernet comum em laptops | ✅ Concluído | 🔴 Crítico |
| Intel I225/I226 | Intel 2.5GbE | ⬜ Pendente | 🟡 Importante |
| Realtek RTL8111/8168 | Realtek GbE comum | ✅ Concluído | 🔴 Crítico |
| Realtek RTL8125 | Realtek 2.5GbE | ⬜ Pendente | 🟡 Importante |
| USB Ethernet | Adaptadores USB-Ethernet | ⬜ Pendente | 🟡 Importante |

### 8.3 Bluetooth Real
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Intel Bluetooth | Bluetooth integrado Intel | ✅ Concluído | 🔴 Crítico |
| Realtek Bluetooth | RTL8761B, etc. | ⬜ Pendente | 🟡 Importante |
| BLE | Bluetooth Low Energy | ⬜ Pendente | 🟡 Importante |
| Bluetooth Mesh | Bluetooth Mesh networking | ⬜ Pendente | 🟢 Nice-to-have |
| LE Audio | Bluetooth LE Audio | ⬜ Pendente | 🟢 Nice-to-have |

### 8.4 VPN e Segurança
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| WireGuard | VPN WireGuard | ✅ Concluído | 🔴 Crítico |
| OpenVPN | VPN OpenVPN | ✅ Concluído | 🟡 Importante |
| IPsec/IKEv2 | VPN IPsec | ✅ Concluído | 🟡 Importante |
| Firewall GUI | Interface gráfica para firewall | ✅ Concluído | 🟡 Importante |
| Network Profiles | Perfis de rede (casa, trabalho, público) | ✅ Concluído | 🟡 Importante |

### 8.5 Mobile Data
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| 4G/LTE Modem | Suporte a modems 4G | ⬜ Pendente | 🟡 Importante |
| 5G Modem | Suporte a modems 5G | ⬜ Pendente | 🟢 Nice-to-have |
| SIM Manager | Gerenciamento de SIM | ⬜ Pendente | 🟡 Importante |
| SMS/MMS | Mensagens via modem | ⬜ Pendente | 🟢 Nice-to-have |

---

## 9. Power Management
> Gerenciamento de energia para notebooks.

### 9.1 Suspend/Resume
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| S3 Suspend (RAM) | Suspend to RAM real | ✅ Concluído | 🔴 Crítico |
| S0ix (Modern Standby) | Modern Standby (Intel/AMD) | ✅ Concluído | 🔴 Crítico |
| S4 Hibernate | Hibernação real | ✅ Concluído | 🟡 Importante |
| Hybrid Sleep | Suspend + Hibernate | ⬜ Pendente | 🟢 Nice-to-have |
| Resume Speed | Tempo de resume otimizado | ✅ Concluído | 🟡 Importante |
| Wake Timers | Agendamento de wake | ⬜ Pendente | 🟢 Nice-to-have |
| Wake on LAN | Wake pela rede | ⬜ Pendente | 🟢 Nice-to-have |
| Wake on USB | Wake por dispositivo USB | ⬜ Pendente | 🟢 Nice-to-have |

### 9.2 CPU Power
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| CPU Frequency Scaling | Governors (powersave, performance) | ✅ Concluído | 🔴 Crítico |
| Intel P-State | Driver P-State Intel | ✅ Concluído | 🔴 Crítico |
| AMD P-State | Driver P-State AMD | ✅ Concluído | 🔴 Crítico |
| Intel EPP | Energy Performance Preference | ✅ Concluído | 🟡 Importante |
| Turbo Boost Control | Controle de turbo | ✅ Concluído | 🟡 Importante |
| Core Parking | Desativar cores ociosos | ✅ Concluído | 🟡 Importante |

### 9.3 Battery
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Battery Status | Status da bateria | ✅ Concluído | 🔴 Crítico |
| Time Remaining | Estimativa de tempo restante | ✅ Concluído | 🔴 Crítico |
| Charge Limit | Limitar carga a 80% | ✅ Concluído | 🟡 Importante |
| Battery Health | Saúde da bateria | ✅ Concluído | 🟡 Importante |
| Low Battery Warning | Avisos de bateria baixa | ✅ Concluído | 🔴 Crítico |
| Critical Battery Action | Ação em bateria crítica | ✅ Concluído | 🔴 Crítico |
| Power Profiles | Perfis de energia | ✅ Concluído | 🔴 Crítico |

### 9.4 Thermal
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Thermal Monitoring | Monitoramento de temperatura | ✅ Concluído | 🔴 Crítico |
| Thermal Throttling | Throttling por temperatura | ✅ Concluído | 🔴 Crítico |
| Fan Profiles | Perfis de ventoinha | ✅ Concluído | 🟡 Importante |
| Custom Fan Curves | Curvas de ventoinha customizadas | ⬜ Pendente | 🟢 Nice-to-have |

---

## 10. Segurança Avançada
> Segurança de nível empresarial.

### 10.1 Boot Security
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Secure Boot | UEFI Secure Boot | ✅ Concluído | 🔴 Crítico |
| MOK (Machine Owner Key) | Gerenciamento de chaves | ✅ Concluído | 🟡 Importante |
| Measured Boot | Boot medido com TPM | ✅ Concluído | 🟡 Importante |
| Verified Boot | Verificação de integridade do boot | ⬜ Pendente | 🟡 Importante |

### 10.2 TPM
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| TPM 2.0 Driver | Driver TPM 2.0 | ✅ Concluído | 🔴 Crítico |
| TPM Key Storage | Armazenamento de chaves no TPM | ⬜ Pendente | 🟡 Importante |
| TPM Attestation | Remote attestation | ⬜ Pendente | 🟢 Nice-to-have |
| TPM Disk Unlock | Desbloqueio de disco via TPM | ⬜ Pendente | 🟡 Importante |

### 10.3 Disk Encryption
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| LUKS2 | Full disk encryption | ✅ Concluído | 🔴 Crítico |
| Auto Unlock | Desbloqueio automático com TPM/biometria | ⬜ Pendente | 🟡 Importante |
| Encrypted Home | Home directory criptografado | ⬜ Pendente | 🟡 Importante |
| Recovery Keys | Chaves de recuperação | ✅ Concluído | 🔴 Crítico |

### 10.4 Authentication
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| PAM | Pluggable Authentication Modules | ✅ Concluído | 🔴 Crítico |
| Fingerprint Login | Login com impressão digital | ✅ Concluído | 🟡 Importante |
| Face Recognition | Login com reconhecimento facial | ⬜ Pendente | 🟢 Nice-to-have |
| Smart Card | Login com smart card | ⬜ Pendente | 🟢 Nice-to-have |
| FIDO2/WebAuthn | Suporte a chaves de segurança | ⬜ Pendente | 🟡 Importante |
| Password Policies | Políticas de senha | ⬜ Pendente | 🟡 Importante |

### 10.5 Application Sandboxing
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| App Sandbox | Isolamento de aplicações | ✅ Concluído | 🔴 Crítico |
| Permission System | Sistema de permissões (câmera, microfone, etc.) | ✅ Concluído | 🔴 Crítico |
| Portal System | Portals para acesso controlado | ⬜ Pendente | 🟡 Importante |
| SELinux/AppArmor | MAC policies | ⬜ Pendente | 🟡 Importante |

---

## 11. Input e Acessibilidade
> Suporte a diversos dispositivos de entrada e acessibilidade.

### 11.1 Input Devices
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Touchpad Gestures | Gestos multi-touch (2, 3, 4 dedos) | ✅ Concluído | 🔴 Crítico |
| Touchpad Palm Rejection | Rejeição de palma | ✅ Concluído | 🔴 Crítico |
| Touchscreen | Suporte a tela touch | ✅ Concluído | 🟡 Importante |
| Stylus/Pen | Caneta stylus com pressão | ✅ Concluído | 🟡 Importante |
| Graphics Tablet | Tablets Wacom, etc. | ⬜ Pendente | 🟢 Nice-to-have |
| Game Controllers | Controles de jogo (Xbox, PS) | ✅ Concluído | 🟡 Importante |
| MIDI Devices | Teclados MIDI | ⬜ Pendente | 🟢 Nice-to-have |

### 11.2 Accessibility
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Screen Reader | Leitor de tela | ✅ Concluído | 🟡 Importante |
| High Contrast | Modo alto contraste | ✅ Concluído | 🟡 Importante |
| Large Text | Texto grande | ✅ Concluído | 🟡 Importante |
| Screen Magnifier | Lupa de tela | ✅ Concluído | 🟡 Importante |
| Sticky Keys | Teclas de aderência | ✅ Concluído | 🟡 Importante |
| Slow Keys | Teclas lentas | ✅ Concluído | 🟢 Nice-to-have |
| Bounce Keys | Teclas de repique | ✅ Concluído | 🟢 Nice-to-have |
| Mouse Keys | Controle do mouse pelo teclado | ✅ Concluído | 🟡 Importante |
| On-Screen Keyboard | Teclado virtual | ✅ Concluído | 🟡 Importante |
| Voice Control | Controle por voz | ⬜ Pendente | 🟢 Nice-to-have |
| Color Filters | Filtros para daltonismo | ⬜ Pendente | 🟢 Nice-to-have |
| Reduce Motion | Reduzir animações | ✅ Concluído | 🟡 Importante |

---

## 12. Internacionalização
> Suporte global a idiomas e regiões.

### 12.1 Language Support
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| UTF-8 Complete | UTF-8 em todo o sistema | ✅ Concluído | 🔴 Crítico |
| Locale System | Sistema de locales | ✅ Concluído | 🔴 Crítico |
| Translations | Framework de traduções | ✅ Concluído | 🔴 Crítico |
| Portuguese (BR) | Tradução para português brasileiro | ✅ Concluído | 🔴 Crítico |
| Spanish | Tradução para espanhol | ⬜ Pendente | 🟡 Importante |
| French | Tradução para francês | ⬜ Pendente | 🟡 Importante |
| German | Tradução para alemão | ⬜ Pendente | 🟡 Importante |
| Chinese | Tradução para chinês | ⬜ Pendente | 🟡 Importante |
| Japanese | Tradução para japonês | ⬜ Pendente | 🟡 Importante |
| Korean | Tradução para coreano | ⬜ Pendente | 🟡 Importante |

### 12.2 Input Methods
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| IBus Framework | Framework de input methods | ⬜ Pendente | 🟡 Importante |
| Chinese Input | Pinyin, Wubi | ⬜ Pendente | 🟡 Importante |
| Japanese Input | Hiragana, Katakana, Kanji | ⬜ Pendente | 🟡 Importante |
| Korean Input | Hangul | ⬜ Pendente | 🟡 Importante |
| Arabic Input | Teclado árabe com RTL | ⬜ Pendente | 🟡 Importante |
| Emoji Picker | Seletor de emoji | ⬜ Pendente | 🟡 Importante |

### 12.3 Regional Formats
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Date Formats | Formatos de data regionais | ✅ Concluído | 🔴 Crítico |
| Time Formats | Formatos de hora (12h/24h) | ✅ Concluído | 🔴 Crítico |
| Number Formats | Formatos numéricos (,/.) | ✅ Concluído | 🔴 Crítico |
| Currency Formats | Formatos de moeda | ⬜ Pendente | 🟡 Importante |
| Calendar Systems | Calendários (Gregoriano, Lunar, etc.) | ⬜ Pendente | 🟢 Nice-to-have |

---

## 13. Performance e Otimização
> Tornar o sistema rápido e eficiente.

### 13.1 Boot Performance
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Fast Boot | Boot em <10 segundos | ✅ Concluído | 🔴 Crítico |
| Parallel Init | Inicialização paralela | ✅ Concluído | 🔴 Crítico |
| Boot Cache | Cache de boot | ⬜ Pendente | 🟡 Importante |
| Readahead | Pré-carregamento de arquivos | ⬜ Pendente | 🟡 Importante |

### 13.2 Memory
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Memory Compression | zswap/zram | ✅ Concluído | 🟡 Importante |
| Memory Deduplication | KSM-like deduplication | ⬜ Pendente | 🟢 Nice-to-have |
| OOM Killer | OOM killer inteligente | ✅ Concluído | 🔴 Crítico |
| Memory Cgroups | Limites de memória por app | ⬜ Pendente | 🟡 Importante |

### 13.3 I/O
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| I/O Scheduler | Scheduler de I/O otimizado (mq-deadline, BFQ) | ✅ Concluído | 🟡 Importante |
| TRIM Support | TRIM para SSDs | ✅ Concluído | 🔴 Crítico |
| Write Caching | Cache de escrita otimizado | ✅ Concluído | 🟡 Importante |
| Filesystem Tuning | Otimizações de filesystem | ✅ Concluído | 🟡 Importante |

### 13.4 Profiling
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| perf Support | Suporte a perf | ✅ Concluído | 🟡 Importante |
| eBPF | Extended BPF | ✅ Concluído | 🟡 Importante |
| Tracing | ftrace, systemtap | ✅ Concluído | 🟡 Importante |
| CPU Profiler | Profiler de CPU | ✅ Concluído | 🟡 Importante |
| Memory Profiler | Profiler de memória | ✅ Concluído | 🟡 Importante |

---

## 14. Cloud e Virtualização
> Suporte a ambientes cloud e VMs.

### 14.1 Guest Support
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| VirtIO Drivers | Drivers VirtIO otimizados | ⬜ Pendente | 🟡 Importante |
| VMware Tools | VMware guest support | ⬜ Pendente | 🟡 Importante |
| Hyper-V Integration | Hyper-V guest support | ⬜ Pendente | 🟡 Importante |
| VirtualBox Additions | VirtualBox guest support | ⬜ Pendente | 🟡 Importante |
| QEMU Guest Agent | QEMU agent | ⬜ Pendente | 🟡 Importante |
| Cloud-init | Cloud instance initialization | ⬜ Pendente | 🟡 Importante |

### 14.2 Host Support
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| KVM Support | Host KVM | ⬜ Pendente | 🟢 Nice-to-have |
| QEMU Integration | QEMU como hypervisor | ⬜ Pendente | 🟢 Nice-to-have |
| libvirt | Gerenciamento de VMs | ⬜ Pendente | 🟢 Nice-to-have |
| VFIO | GPU passthrough | ⬜ Pendente | 🟢 Nice-to-have |

---

## 15. Testes em Hardware Real
> Validação em hardware real.

### 15.1 Reference Hardware
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| ThinkPad X1 Carbon | Laptop Intel business | ⬜ Pendente | 🔴 Crítico |
| Dell XPS 13/15 | Laptop Intel consumer | ⬜ Pendente | 🔴 Crítico |
| Framework Laptop | Laptop modular | ⬜ Pendente | 🟡 Importante |
| Lenovo ThinkPad (AMD) | Laptop AMD | ⬜ Pendente | 🔴 Crítico |
| HP Spectre/Envy | Laptop consumer | ⬜ Pendente | 🟡 Importante |
| MacBook (Intel) | MacBook Pro/Air Intel | ⬜ Pendente | 🟡 Importante |
| Steam Deck | AMD handheld | ⬜ Pendente | 🟡 Importante |
| Mini PC Intel | Intel NUC ou similar | ⬜ Pendente | 🟡 Importante |
| Desktop AMD | Desktop com Ryzen | ⬜ Pendente | 🟡 Importante |
| Desktop Intel | Desktop com Core | ⬜ Pendente | 🟡 Importante |

### 15.2 Hardware Certification
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Hardware Database | DB de hardware compatível | ⬜ Pendente | 🟡 Importante |
| Test Suite | Suite de testes de hardware | ✅ Concluído | 🔴 Crítico |
| Bug Tracker | Sistema de bugs por hardware | ⬜ Pendente | 🟡 Importante |
| Compatibility Reports | Relatórios de compatibilidade | ⬜ Pendente | 🟡 Importante |

---

## 16. Ecossistema e Comunidade
> Construir comunidade e ecossistema.

### 16.1 Documentation
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Installation Guide | Guia de instalação | ⬜ Pendente | 🔴 Crítico |
| User Guide | Guia do usuário | ⬜ Pendente | 🔴 Crítico |
| Developer Guide | Guia para desenvolvedores | ⬜ Pendente | 🟡 Importante |
| API Documentation | Documentação de APIs | ⬜ Pendente | 🟡 Importante |
| Wiki | Wiki comunitária | ⬜ Pendente | 🟡 Importante |
| Video Tutorials | Tutoriais em vídeo | ⬜ Pendente | 🟢 Nice-to-have |

### 16.2 Community
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Website | Site oficial | ⬜ Pendente | 🔴 Crítico |
| Forum | Fórum de discussão | ⬜ Pendente | 🟡 Importante |
| Discord/Matrix | Chat da comunidade | ⬜ Pendente | 🟡 Importante |
| Bug Tracker | Sistema de bugs público | ⬜ Pendente | 🔴 Crítico |
| Mailing List | Lista de email | ⬜ Pendente | 🟢 Nice-to-have |
| Blog | Blog oficial | ⬜ Pendente | 🟡 Importante |
| Newsletter | Newsletter | ⬜ Pendente | 🟢 Nice-to-have |

### 16.3 Developer Experience
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| SDK | SDK para desenvolvimento | ⬜ Pendente | 🟡 Importante |
| IDE Integration | Plugins para VSCode, etc. | ⬜ Pendente | 🟢 Nice-to-have |
| App Templates | Templates para apps | ⬜ Pendente | 🟢 Nice-to-have |
| CI/CD Templates | Templates de CI/CD | ⬜ Pendente | 🟡 Importante |
| Contributor Guide | Guia para contribuidores | ⬜ Pendente | 🟡 Importante |

---

## Histórico de Atualizações V2

> **IMPORTANTE:** Toda implementação DEVE adicionar uma entrada aqui com o formato:
> `| YYYY-MM-DD | [Seção] Item implementado (arquivo.rs: descrição técnica detalhada) |`

| Data | Mudança |
|------|---------|
| 2026-01-17 | [Setup] Documento ROADMAP_V2.md criado com 374 itens em 16 seções, incluindo Instalador, Hardware Real, GPU, Desktop Environment, Aplicações, Package Manager, Áudio, Rede, Power Management, Segurança, Input/Accessibility, i18n, Performance, Cloud/Virt, Hardware Test, Ecossistema |
| 2026-01-17 | [Instalador] Live USB Boot (installer/live.rs: LiveSession struct, LiveUsbBuilder para criar USB bootáveis, InitramfsBuilder para initramfs, OverlayFs para persistência, squashfs/overlay support, boot method detection, ~400 linhas) |
| 2026-01-17 | [Instalador] Detecção de Hardware (installer/hwdetect.rs: HardwareInfo struct com CpuInfo, MemoryInfo, DiskInfo, GpuInfo, NetworkInfo, detect_hardware() para scan completo, FirmwareType enum, ~230 linhas) |
| 2026-01-17 | [Instalador] Particionamento (installer/partition.rs: PartitionLayout struct, GptHeader/GptEntry para GPT, PartitionScheme enum GPT/MBR/Hybrid, partition_disk() com layouts auto/custom, ~250 linhas) |
| 2026-01-17 | [Instalador] Formatação (installer/format.rs: format_partitions() com suporte ext4/btrfs/xfs/FAT32/swap/LUKS, format_with_label(), ~100 linhas) |
| 2026-01-17 | [Instalador] Cópia de Sistema (installer/copy.rs: copy_system() com rsync-style copy, generate_fstab(), configure_locale(), configure_timezone(), ~120 linhas) |
| 2026-01-17 | [Instalador] Configuração de Usuário (installer/user.rs: configure_users() com criação usuário/grupos, setup_sudo(), hash_password(), ~100 linhas) |
| 2026-01-17 | [Instalador] Bootloader Install (installer/bootloader.rs: install_bootloader() com suporte systemd-boot/GRUB2/EFI stub, create_loader_conf(), create_boot_entry(), generate_grub_config(), ~120 linhas) |
| 2026-01-17 | [Instalador] UEFI Boot Entry (installer/uefi_entry.rs: UefiBootEntry struct, create_boot_entry(), list_boot_entries(), remove_boot_entry(), set_default_entry(), UEFI variable management, ~100 linhas) |
| 2026-01-17 | [Instalador] ISO Builder + Hybrid ISO (installer/iso.rs: IsoConfig struct, build_iso() com El Torito/UEFI boot, squashfs compression, make_hybrid() para USB boot, verify_iso(), ~120 linhas) |
| 2026-01-17 | [Instalador] System Updater (installer/updater.rs: UpdateStatus/UpdateState structs, check_for_updates(), install_update() com rollback snapshot, version_compare(), ~160 linhas) |
| 2026-01-17 | [Instalador] Recovery Mode (installer/recovery.rs: RecoveryOption enum, RecoveryStatus struct, enter_recovery_mode(), execute_recovery() com repair bootloader/fs, factory reset, backup restore, ~160 linhas) |
| 2026-01-17 | [Instalador] Módulo principal (installer/mod.rs: InstallConfig struct, InstallError enum, Installer struct com run() orquestrando todo o flow de instalação, InstallStage enum, ~200 linhas) |
| 2026-01-17 | [Hardware Real] NVMe Real (drivers/storage/nvme.rs: NvmeController completo com admin/IO queues, PRP handling, Identify Controller/Namespace, BlockDevice impl, ~650 linhas) + API list_drives() |
| 2026-01-17 | [Hardware Real] AHCI/SATA Real (drivers/storage/ahci.rs: AhciController com HBA management, Command Lists, FIS handling, multi-port support, DMA read/write, ~550 linhas) + storage/mod.rs DiskDriveInfo API |
| 2026-01-17 | [Hardware Real] USB Mass Storage (drivers/usb/storage.rs: MassStorageDevice, Cbw/Csw structs, SCSI commands, UsbBlockDevice impl BlockDevice, ~607 linhas) |
| 2026-01-17 | [Hardware Real] xHCI Real (drivers/usb/xhci.rs: XhciController completo com TRB rings, command/transfer rings, port management, device enumeration, bulk/control transfers, ~1923 linhas) |
| 2026-01-17 | [Hardware Real] USB Hub Handling (drivers/usb/hub.rs: UsbHub struct, port status management, device attachment, hub enumeration, ~281 linhas) |
| 2026-01-17 | [Hardware Real] USB Hotplug (drivers/usb/xhci.rs: port status change handling, device attach/detach events, enumeration triggers) |
| 2026-01-17 | [ACPI] Tables Parser (drivers/acpi.rs: RSDP/RSDT/XSDT/DSDT/SSDT parsing, ~2495 linhas total) |
| 2026-01-17 | [ACPI] AML Interpreter (drivers/acpi.rs: aml_opcode module, AML parsing básico para dispositivos) |
| 2026-01-17 | [ACPI] Power Button (drivers/acpi.rs: enable_power_button_event(), check_power_button_pressed(), handle_power_button()) |
| 2026-01-17 | [ACPI] Lid Switch (drivers/acpi.rs: LidSwitch struct, init_lid_switch(), handle_lid_switch(), get_lid_state()) |
| 2026-01-17 | [ACPI] AC Adapter (drivers/battery.rs: ac_connected(), power source detection) |
| 2026-01-17 | [ACPI] Thermal Zones (drivers/thermal.rs: ThermalZone struct, temperature monitoring, cooling policies, fan control, ~1392 linhas) |
| 2026-01-17 | [ACPI] Fan Control (drivers/thermal.rs: integrado com Thermal Zones, cooling device management) |
| 2026-01-17 | [ACPI] Hotkeys (drivers/fnkeys.rs: FnKeyHandler, hotkey detection, brightness/volume/WiFi toggles, ~837 linhas) |
| 2026-01-17 | [ACPI] Backlight (drivers/backlight.rs: BacklightDevice struct, brightness control ACPI/Intel, ~608 linhas) |
| 2026-01-17 | [ACPI] Battery (drivers/battery.rs: BatteryInfo struct, capacity, health, charging status, ~485 linhas) |
| 2026-01-17 | [GPU] Intel Gen9/11/12 (drivers/intel_gpu.rs: IntelGpuDriver com PCI detection, MMIO setup, mode setting básico, ~907 linhas) |
| 2026-01-17 | [GPU] AMD RDNA 2 (drivers/amd_gpu.rs: AmdGpuDriver com PCI detection, MMIO setup, display output, ~1102 linhas) |
| 2026-01-17 | [Chipset] Intel PCH (drivers/pch.rs: PchGeneration enum Sunrise/Union/Cannon/Comet/Tiger/Alder/RaptorPoint, PchInfo struct, SMBus init, GPIO init, thermal init, smbus_read_byte/write_byte, smbus_scan, ~392 linhas) |
| 2026-01-17 | [Chipset] AMD FCH (drivers/fch.rs: FchGeneration enum Bolton/Promontory/Promontory500/600/IntegratedApu, FchInfo struct, SMBus init, GPIO init, smbus_read_byte/write_byte, smbus_scan, ~339 linhas) |
| 2026-01-17 | [Chipset] SMBus/I2C (drivers/pch.rs + drivers/fch.rs: SMBus protocol implementation para Intel e AMD, read/write byte, device scanning) |
| 2026-01-17 | [Firmware] Firmware Loader (drivers/firmware.rs: FirmwareError enum, FirmwareInfo struct, request_firmware() com cache, decompress_xz/zstd stubs, firmware path constants para Intel WiFi/GPU, AMD GPU, Realtek, ~235 linhas) |
| 2026-01-17 | [Firmware] CPU Microcode (drivers/microcode.rs: MicrocodeInfo/CpuVendor structs, IntelMicrocodeHeader/AmdMicrocodeHeader packed structs, detect_cpu_vendor(), get_cpu_signature(), get_current_microcode_revision(), load_intel_microcode(), load_amd_microcode(), ~385 linhas) |
| 2026-01-17 | [GPU] GEM/GTT (drivers/gem.rs: GemState/GemObject/GttEntry/Ppgtt structs, GemTiling enum, create_gem_object(), gem_mmap(), gem_close(), gem_pread/pwrite(), gtt_insert/remove(), ppgtt_create/destroy(), execbuffer(), ~570 linhas) |
| 2026-01-17 | [GPU] Display Pipe (drivers/display_pipe.rs: DisplayState, Crtc/Plane/Encoder/Connector structs, Pipe enum, DisplayMode, crtc_enable/disable(), plane_update(), encoder_setup(), connector_detect(), modeset_commit(), ~550 linhas) |
| 2026-01-17 | [GPU] GuC/HuC Firmware (drivers/guc_huc.rs: GuCState/HuCState structs, GucError enum, upload_guc_firmware(), upload_huc_firmware(), guc_submit(), guc_get_timestamp(), huc_auth(), DMA buffer allocation, ~450 linhas) |
| 2026-01-17 | [GPU] AMD APU (drivers/amd_apu.rs: AmdApu/ApuInfo structs, ApuGeneration enum Raven/Renoir/Cezanne/Rembrandt/Phoenix, smu_init(), smu_send_message(), set_power_limit(), get_gpu_clock(), ~400 linhas) |
| 2026-01-17 | [GPU] DRM/KMS (drivers/drm.rs: DrmDevice/DrmFramebuffer/DrmCrtcState/DrmMode structs, drm_framebuffer_create/destroy(), drm_mode_set(), drm_get_crtc/connector/encoder(), atomic_check/commit(), IOCTL interface, ~480 linhas) |
| 2026-01-17 | [GPU] DRM Framebuffer (drivers/drm_fb.rs: FramebufferState, Framebuffer, ScanoutBuffer, PageFlipEvent structs, create_framebuffer(), destroy_framebuffer(), setup_scanout(), page_flip(), complete_page_flip(), dirty_fb(), wait_vblank(), double/triple buffering, ~510 linhas) |
| 2026-01-17 | [GPU] Multi-Monitor (drivers/multimon.rs: MultiMonitorState, MonitorInfo, ConnectionType, Rotation, DisplayArrangement enums, detect_monitors(), EDID parsing, set_position/mode/rotation/scale(), clone/extend modes, virtual desktop calculation, ~550 linhas) |
| 2026-01-17 | [GPU] Hotplug Display (drivers/hotplug.rs: HotplugState, ConnectorState, HotplugEvent structs, HotplugEventType enum, handle_hpd_interrupt(), poll_connectors(), register/unregister_callback(), process_event(), event queue, ~320 linhas) |
| 2026-01-17 | [GPU] HDMI (drivers/hdmi.rs: HdmiPort, HdmiSinkCaps, HdmiColorFormat, HdcpState, AviInfoFrame, AudioInfoFrame, CecMessage structs, enable_output(), enable_audio(), set_color_format(), start_hdcp(), cec_send/power_on/standby(), EDID parsing, ~580 linhas) |
| 2026-01-17 | [GPU] DisplayPort (drivers/displayport.rs: DpPort, DpLinkConfig, DpcdInfo, MstHub structs, aux_read/write(), read_dpcd_caps(), link_train(), enable_mst(), enable_psr(), calculate_bandwidth(), DP 1.4 DPCD registers, ~520 linhas) |
| 2026-01-17 | [GPU] eDP (drivers/edp.rs: EdpPanel, PanelInfo, PanelPowerState, PowerSequenceTiming, PsrState, BacklightInfo structs, power_on/off(), set_brightness(), enable_psr(), parse_vbt(), panel power sequencing T1-T12, ~600 linhas) |
| 2026-01-17 | [3D] Vulkan 1.3 (drivers/vulkan.rs: VulkanState, PhysicalDevice/LogicalDevice, PhysicalDeviceProperties/Features/Limits, Queue/QueueFlags, CommandPool, Buffer/Image, DeviceMemory, Surface, Swapchain, Format/PresentMode enums, create_instance(), enumerate_physical_devices(), create_device(), create_surface(), create_swapchain(), acquire_next_image(), queue_present(), ~1034 linhas) |
| 2026-01-17 | [3D] Mesa Integration (drivers/mesa.rs: MesaState, DriDriver, DriDriverType enum Iris/RadeonSi/LlvmPipe/etc, RenderContext, GlProfile, GpuMapping, ShaderCache, ShaderCacheEntry, ShaderStage, MesaFeatures, DriConfig, VBlankMode, create_context(), map_gpu_memory(), cache_shader(), get_cached_shader(), IOCTL interface, ~560 linhas) |
| 2026-01-17 | [Desktop] Wayland Compositor (drivers/wayland.rs: WaylandState, Client, Surface/Subsurface, Region, Buffer/ShmPool, Callback, Output, Seat/Pointer/Keyboard/Touch, DataDevice, Dmabuf/DmabufPlane, create_compositor(), connect_client(), create_surface(), attach_buffer(), commit_surface(), damage_surface(), input handling, dmabuf import, wl_shm protocol, ~650 linhas) |
| 2026-01-17 | [Desktop] XWayland (drivers/xwayland.rs: XWaylandState, XClient, XWindow, XPixmap, XGraphicsContext, WindowClass/Attributes, GcFunction/LineStyle/FillStyle enums, XProperty, XSelection, XScreen/XDepth/XVisual, XEvent variants, atoms module, create_window(), destroy_window(), map/unmap_window(), configure_window(), change_property(), intern_atom(), create_pixmap(), create_gc(), set_selection_owner(), convert_input_event(), map_to_wayland_surface(), ~850 linhas) |
| 2026-01-17 | [Desktop] Desktop Shell (gui/shell.rs: ShellState, ShellConfig, PanelPosition/DockPosition enums, HotCorners/HotCornerAction, ShellTheme, PanelState/PanelItem/WindowButton/TrayIcon, DockState/DockItem, WorkspaceState/Workspace, show/hide_overview(), show/hide_launcher(), switch_workspace(), add/remove_window(), pin/unpin_from_dock(), add/remove_tray_icon(), handle_hot_corner(), get_work_area(), ~650 linhas) |
| 2026-01-17 | [Desktop] App Launcher (gui/launcher.rs: LauncherState, LauncherConfig, LauncherMode/Theme, Application, AppCategory, SearchResult/AppResult/FileResult/QuickActionResult, QuickActionType, register/unregister_app(), show/hide/toggle(), search() com fuzzy matching, calculate_match_score(), try_quick_action(), evaluate_expression(), select_next/previous(), activate_selected(), ~700 linhas) |
| 2026-01-17 | [Desktop] Notification Center (gui/notification_center.rs: NotificationCenterState, NotificationCenterConfig, Position/Theme, Notification, NotificationIcon/Urgency/Action/Hints, QuickSetting/QuickSettingType, notify(), close_notification(), mark_read(), enable/disable_dnd(), toggle_quick_setting(), get_history(), scroll(), ~650 linhas) |
| 2026-01-17 | [Desktop] System Tray (gui/systray.rs: SystemTray widget, TrayItemId, TrayIconType enum Network/Volume/Battery/DateTime/Notification/Custom, NetworkStatus/VolumeLevel/BatteryStatus, TrayItem struct, add/remove item, built-in icons, click callbacks, ~350 linhas - existente) |
| 2026-01-17 | [Desktop] Quick Settings (gui/notification_center.rs: QuickSetting struct, QuickSettingType enum Toggle/Slider/Menu, WiFi/Bluetooth/DND/NightLight/AirplaneMode/Brightness/Volume settings, toggle_quick_setting(), set_quick_setting_value(), integrado com Notification Center) |
| 2026-01-17 | [Desktop] Lock Screen (gui/lockscreen.rs: LockScreen widget, LockState enum, password input, time/date display, user info, shake animation para erro, unlock callback, password visibility toggle, ~250 linhas - existente) |
| 2026-01-17 | [Desktop] Login Manager (gui/loginscreen.rs: LoginScreen widget, LoginState enum, UserEntry struct, user selection, password entry, authentication, shutdown/restart callbacks, multi-user support, ~300 linhas - existente) |
| 2026-01-17 | [Desktop] Window Manager (gui/window_manager.rs: WindowManagerState, ManagedWindow, VirtualDesktop, SwitcherState, SnapZone enum, PipState, snap_window(), switch_desktop(), start_switcher(), enable_pip(), tiling mode, drag/resize handling, ~750 linhas) |
| 2026-01-17 | [Theming] Theme Engine (gui/theme.rs: ThemeState, Theme, ThemeColors, BackgroundColors/ForegroundColors/BorderColors/StateColors/SemanticColors, WidgetStyles, ButtonStyle/InputStyle/SwitchStyle, DecorationStyle, ColorScheme enum Light/Dark/Auto, set_color_scheme(), set_accent_color(), get_current_colors(), color_utils module, ~800 linhas) |
| 2026-01-17 | [Theming] Dark Mode (gui/theme.rs: ColorScheme::Dark, is_dark_mode(), light_colors/dark_colors em Theme, auto-detection por hora do dia, notificação de mudança de tema via callbacks) |
| 2026-01-17 | [Theming] Accent Colors (gui/theme.rs: AccentColor enum Blue/Purple/Pink/Red/Orange/Yellow/Green/Teal/Graphite/Custom, to_rgb(), to_hex(), set_accent_color(), 9 cores pré-definidas + custom) |
| 2026-01-17 | [Theming] Icon Theme (gui/icons.rs: IconThemeState, IconTheme, IconDirectory, IconContext enum, BuiltinIcon enum com 70+ ícones, render() para ícones vetoriais, lookup_icon(), get_file_icon(), rendering geométrico para ícones de sistema, ~850 linhas) |
| 2026-01-17 | [Theming] Cursor Theme (gui/cursors.rs: CursorThemeState, CursorTheme, CursorType enum com 30+ cursors, CursorDefinition, CursorFrame, animated cursors para wait/progress, set_cursor(), update_animation(), rendering de cursors Arrow/IBeam/Hand/Resize/etc, ~900 linhas) |
| 2026-01-17 | [Theming] Font Rendering (gui/fonts.rs: FontState, Font, FontData TrueType/Bitmap/Builtin, FontMetrics, GlyphBitmap, RenderMode Gray/Mono/Lcd, HintingMode, render_glyph(), layout_text(), render_text(), built-in 8x16 bitmap font com ASCII completo, glyph cache, ~750 linhas) |
| 2026-01-17 | [Theming] Wallpaper (gui/wallpaper.rs: WallpaperState, WallpaperInfo, WallpaperType Static/Dynamic/Live/SolidColor/Gradient, ScaleMode Fill/Fit/Stretch/Center/Tile/Span, SlideshowConfig, render_gradient() Linear/Radial/Conic, scale_wallpaper(), dominant colors extraction, multi-monitor support, ~700 linhas) |
| 2026-01-17 | [Theming] Dynamic Wallpaper (gui/wallpaper.rs: DynamicConfig, TimeVariant structs, wallpapers que mudam com hora do dia, transition_duration, LiveConfig para wallpapers animados, LoopMode Forever/Once/PingPong) |
| 2026-01-17 | [Settings] Settings App Module (gui/settings/mod.rs: SettingsState, SettingsPanel enum com 12 panels, SettingsItem for search, navigation history, search(), navigate_to(), ~290 linhas) |
| 2026-01-17 | [Settings] Display Settings (gui/settings/display.rs: DisplaySettings, MonitorInfo, Resolution, Rotation/ScaleFactor enums, NightLightSettings/Schedule, ConnectionType, set_resolution/rotation/scale/position(), set_night_light_*(), ~280 linhas) |
| 2026-01-17 | [Settings] Sound Settings (gui/settings/sound.rs: SoundSettings, AudioDevice, AudioDeviceType enum 11 types, AppVolume, set_master_volume(), set_output/input_device(), set_device_volume/balance(), ~250 linhas) |
| 2026-01-17 | [Settings] Network Settings (gui/settings/network.rs: NetworkSettings, WifiNetwork, WifiSecurity enum, EthernetConnection, VpnConnection/VpnType, ProxySettings/ProxyMode, HotspotSettings/WifiBand, connect_wifi(), add_vpn(), ~380 linhas) |
| 2026-01-17 | [Settings] Bluetooth Settings (gui/settings/bluetooth.rs: BluetoothSettings, BluetoothDevice, BluetoothDeviceType enum 12 types, start_scan(), pair/unpair_device(), connect/disconnect_device(), ~280 linhas) |
| 2026-01-17 | [Settings] Power Settings (gui/settings/power.rs: PowerSettings, BatteryInfo/BatteryState, PowerSource/PowerProfile/LidAction/PowerButtonAction/CriticalBatteryAction enums, set_power_profile(), set_*_timeout(), ~350 linhas) |
| 2026-01-17 | [Settings] Keyboard Settings (gui/settings/keyboard.rs: KeyboardSettings, KeyboardLayout, Shortcut/ShortcutAction 40+ actions, KeyBinding/Modifiers, CapsLockBehavior/ComposeKey, add/remove_layout(), set_shortcut(), accessibility sticky/slow/bounce keys, ~500 linhas) |
| 2026-01-17 | [Settings] Mouse Settings (gui/settings/mouse.rs: MouseSettings, MouseConfig/TouchpadConfig, PointingDevice/PointingDeviceType, AccelerationProfile, ScrollMethod/ClickMethod, GestureSettings/GestureAction 10 types, tap_to_click, natural_scroll, ~450 linhas) |
| 2026-01-17 | [Settings] Users Settings (gui/settings/users.rs: UsersSettings, UserAccount, AccountType enum, LoginOptions, OnlineAccount/OnlineAccountProvider/OnlineService, create/delete_user(), set_auto_login(), add_online_account(), ~380 linhas) |
| 2026-01-17 | [Settings] DateTime Settings (gui/settings/datetime.rs: DateTimeSettings, Timezone 7 presets, TimeFormat/DateFormat/Weekday enums, NTP servers, set_timezone(), set_time/date_format(), show_seconds/date/week_numbers, ~300 linhas) |
| 2026-01-17 | [Settings] Privacy Settings (gui/settings/privacy.rs: PrivacySettings, LocationSettings/LocationPermission/LocationAccessLevel, DeviceAccessSettings camera/mic/screen, AppFileAccess, BackgroundRefreshSettings, DiagnosticsSettings, ~400 linhas) |
| 2026-01-17 | [Settings] Default Apps (gui/settings/defaults.rs: DefaultsSettings, CategoryDefault, AppCategory enum 12 types, MimeAssociation, UrlHandler, ApplicationInfo, get/set_default_for_category/mime/scheme(), register_application(), ~420 linhas) |
| 2026-01-17 | [Settings] About (gui/settings/about.rs: AboutSettings, SystemInfo/BootMode, HardwareInfo with CpuInfo/MemoryInfo/GpuInfo/NetworkAdapterInfo/AudioDeviceInfo, StorageDevice/StorageType, SoftwareInfo, format_uptime/bytes(), ~450 linhas) |
| 2026-01-17 | [Apps] File Manager (gui/apps/filemanager.rs: FileManager widget, directory navigation, file listing, cut/copy/paste, delete, create folder/file, breadcrumb path, icon/list view, ~1270 linhas - existente) |
| 2026-01-17 | [Apps] Terminal Emulator (gui/apps/terminal.rs: Terminal widget, PTY handling, input/output, ANSI escape codes, scrollback buffer, VT100 emulation, ~1209 linhas - existente) |
| 2026-01-17 | [Apps] Text Editor (gui/apps/texteditor.rs: TextEditor widget, multi-line editing, cursor movement, selection, word wrap, undo/redo, find/replace, line numbers, ~1368 linhas - existente) |
| 2026-01-17 | [Apps] Browser UI (gui/apps/browser.rs: Browser widget, URL bar, back/forward/refresh, tab bar placeholder, page content area, navigation history, ~1007 linhas - existente) |
| 2026-01-17 | [Apps] Image Viewer (gui/apps/imageviewer.rs: ImageViewer widget, zoom in/out, fit to window, rotation, panning, image decoding BMP/PNG basic, ~658 linhas - existente) |
| 2026-01-17 | [Apps] Task Manager (gui/apps/taskmanager.rs: TaskManager widget, process list, CPU/memory usage, kill process, sort by column, system stats, ~1003 linhas - existente) |
| 2026-01-17 | [Apps] Calculator (gui/apps/calculator.rs: Calculator widget, basic arithmetic, scientific functions, expression evaluation, display, button grid, ~700 linhas - existente) |
| 2026-01-17 | [Apps] Trash (gui/apps/trash.rs: TrashViewer widget, TrashedItem/TrashFileType/TrashError, trash_file/directory(), restore(), delete_permanently(), empty_trash(), metadata persistence, ~460 linhas) |
| 2026-01-17 | [Apps] Search (gui/apps/search.rs: SearchWidget, IndexedFile/SearchFileType/SearchFilter/SearchResult, index_file(), rebuild_index(), search() com name/content matching, quick_search(), ~540 linhas) |
| 2026-01-17 | [Apps] Screenshot Tool (gui/apps/screenshot.rs: ScreenshotWidget, CaptureMode Full/Window/Region, ImageFormat PNG/BMP, SelectionState, capture_framebuffer(), encode_png/bmp(), delay capture, ~850 linhas) |
| 2026-01-17 | [Apps] Video Player (gui/apps/videoplayer.rs: VideoPlayer widget, MediaInfo/ContainerFormat/AudioTrack/SubtitleTrack/SubtitleFormat, PlaybackState/LoopMode, open(), play/pause/stop/seek(), volume control, playlist management, ~750 linhas) |
| 2026-01-17 | [Apps] PDF Viewer (gui/apps/pdfviewer.rs: PdfViewer widget, PdfDocument/PdfPage/PdfLink/PdfAnnotation/OutlineEntry, ZoomLevel/PageMode, open(), navigation, zoom, scroll, outline sidebar, ~680 linhas) |
| 2026-01-18 | [Browser] Browser Engine (gui/apps/browser/engine.rs: BrowserEngine struct coordenando HTML/CSS parsing, layout, rendering, user agent stylesheet, resource cache, hit testing, load_html(), render(), scroll_by(), ~370 linhas) |
| 2026-01-18 | [Browser] DOM Module (gui/apps/browser/dom.rs: Dom/DomNode/DomNodeType/DomElement/DomText structs, getElementById/getElementsByTagName/querySelector/querySelectorAll, NodeIterator para depth-first traversal, serialize_node(), escape_html(), ~510 linhas) |
| 2026-01-18 | [Browser] HTML Parser (gui/apps/browser/html_parser.rs: HtmlParser struct, HtmlDocument/HtmlLink/HtmlScript/HtmlStylesheet, HTML5 parsing com DOM tree, entity decoding, inline style parsing, ~450 linhas) |
| 2026-01-18 | [Browser] CSS Parser (gui/apps/browser/css_parser.rs: CssParser struct, StyleSheet/CssRule/CssSelector/CssDeclaration/CssColor/CssLength, selector specificity, at-rules, keyframes, color parsing hex/rgb/hsl/named, length units, parse_f32() no_std compatible, ~1100 linhas) |
| 2026-01-18 | [Browser] Layout Engine (gui/apps/browser/layout.rs: LayoutEngine struct, LayoutBox/LayoutMode Block/Inline/Flex, BoxDimensions, ComputedStyle, CSS Box Model, margin/padding/border/positioning, ~750 linhas) |
| 2026-01-18 | [Browser] Renderer (gui/apps/browser/render.rs: Renderer struct, PaintCommand enum, RenderTree/RenderNode, DisplayList para efficient painting, background/border/text rendering, ~400 linhas) |
| 2026-01-18 | [Browser] Network Module (gui/apps/browser/network.rs: HttpClient struct, HttpRequest/HttpResponse/Url/Cookie/CookieJar, URL parsing, HTTP/1.1 client, cookies, redirects, ~550 linhas) |
| 2026-01-18 | [Browser] JavaScript Engine (gui/apps/browser/javascript.rs: JsEngine struct, JsValue/JsContext/JsFunction/JsObject, lexer/parser/evaluator, built-in console/Math/JSON objects, floor_f64/ceil_f64/round_f64 no_std helpers, ~1500 linhas) |
| 2026-01-18 | [Browser] Tab Manager (gui/apps/browser/tabs.rs: TabManager struct, Tab/TabId/TabState/SslInfo/SessionState, navigation history, tab pinning, tab groups, new_tab/close_tab/navigate/go_back/go_forward/reload, ~500 linhas) |
| 2026-01-18 | [Browser] Downloads Manager (gui/apps/browser/downloads.rs: DownloadManager struct, Download/DownloadState/DownloadOptions, progress tracking, pause/resume, start_download/cancel_download/delete_download, ~450 linhas) |
| 2026-01-18 | [Package Manager] Package Format (pkg/format.rs: PackageHeader struct, SPKG magic, flags, TarHeader/TarArchive/TarEntry for tar handling, Package struct with from_bytes/decompress_data/verify_signature, ~341 linhas - pré-existente) |
| 2026-01-18 | [Package Manager] Package Database (pkg/database.rs: PackageDatabase struct, InstalledPackage/InstallReason, register_package/unregister_package/get_package, file_owner, reverse_deps, orphans detection, ~232 linhas - pré-existente) |
| 2026-01-18 | [Package Manager] Dependency Resolution (pkg/install.rs: resolve_dependencies() function, Transaction struct para atomic installs, dependency graph resolution - pré-existente) |
| 2026-01-18 | [Package Manager] Install/Remove (pkg/install.rs: InstallOptions/RemoveOptions, install()/remove()/upgrade_all(), Transaction.execute(), ~470 linhas - pré-existente) |
| 2026-01-18 | [Package Manager] Upgrade (pkg/install.rs: upgrade_all() com dependency resolution, version comparison, rollback support - pré-existente) |
| 2026-01-18 | [Package Manager] Repository System (pkg/repository.rs: RepoManager, Repository/Mirror/RepoUrl structs, add_repo/remove_repo, update_index, search_packages, ~400 linhas - pré-existente) |
| 2026-01-18 | [Package Manager] GPG Signing (pkg/sign.rs: Ed25519 signature implementation, verify_signature/add_trusted_key, FieldElement math, ~500 linhas - pré-existente) |
| 2026-01-18 | [Package Manager] Rollback (pkg/rollback.rs: RollbackSystem, snapshot creation, restore_snapshot, automatic rollback on install failure - pré-existente) |
| 2026-01-18 | [Software Center] GUI Application (gui/apps/softwarecenter.rs: SoftwareCenter widget, SoftwareCenterView enum Browse/Search/Details/Updates/Installed/Settings, AppCategory enum 11 categories, AppEntry struct, DownloadProgress/DownloadState, UpdateEntry, search/install/remove/apply_update, Widget trait impl, ~960 linhas) |
| 2026-01-18 | [Software Center] Update Notifications (gui/apps/softwarecenter.rs: UpdateNotificationService, UpdateNotification struct, check_interval, should_check/check_updates, Notification struct with NotificationType - integrado) |
| 2026-01-18 | [Áudio] Audio Daemon (drivers/audio/daemon.rs: AudioDaemon struct PipeWire-like, Node/Port/Link/Client/Session structs, NodeId/PortId/LinkId/ClientId types, NodeType Sink/Source/Filter, PortDirection/PortType/PortFlags, LinkState, SessionType Music/Video/Voice/Game, create_node/destroy_node, create_link/destroy_link, connect_client/disconnect_client, set_default_sink/source, create_session/destroy_session, stream routing, client management, ~1100 linhas) |
| 2026-01-18 | [Áudio] PulseAudio Compat (drivers/audio/pulse.rs: PaContext/PaContextState/PaContextFlags structs, PaStream/PaStreamState/PaStreamFlags, PaSampleSpec/PaSampleFormat, PaCvolume/PaChannelMap/PaChannelPosition, PaOperation/PaSinkInfo/PaSourceInfo/PaSinkInputInfo, pa_volume_from_linear/pa_volume_to_linear, cbrt_approx() Newton's method para no_std, connect/disconnect/create_stream/write/drain/flush, ~900 linhas) |
| 2026-01-18 | [Áudio] ALSA Compat (drivers/audio/alsa.rs: pré-existente - AlsaDevice/AlsaPcmState structs, SND_PCM constants, snd_pcm_open/close/prepare/start/drop, snd_pcm_writei/readi, hw_params handling) |
| 2026-01-18 | [Áudio] Per-App Volume (drivers/audio/mixer.rs: pré-existente - ChannelType::Application, MixerChannel struct com volume/pan/mute per-app, register_app_channel, set_app_volume) |
| 2026-01-18 | [Áudio] Audio Routing (drivers/audio/daemon.rs: integrado - Link struct para conexões Node-to-Node, create_link/destroy_link, route_stream, automatic routing baseado em SessionType) |
| 2026-01-18 | [Áudio] Bluetooth Audio (drivers/audio/bluetooth.rs: BluetoothAudioState/BluetoothAudioDevice structs, AudioProfile A2DP/HFP/HSP/AVRCP/LEAudio, BluetoothCodec SBC/AAC/aptX/aptXHD/LDAC/LC3/CVSD/mSBC, AvrcpController para media controls, connect/disconnect_device, start/stop_stream, set_codec, play/pause/next/previous, volume_up/down, battery level, ~750 linhas) |
| 2026-01-18 | [Áudio] USB Audio (drivers/audio/usb_audio.rs: UsbAudioState/UsbAudioDevice structs, UsbAudioClass UAC1/UAC2/UAC3, UsbAudioTerminalType, UsbAudioFeatureUnit volume/mute/bass/treble controls, UsbAudioFormat S16LE/S24LE/S32LE/F32LE, open/close_device, start/stop_stream, set_volume_percent linear conversion no_std compatible, ~600 linhas) |
| 2026-01-18 | [Áudio] HDMI Audio (drivers/audio/hdmi.rs: HdmiAudioState/HdmiAudioDevice structs, HdmiAudioCapabilities, ShortAudioDescriptor/AudioFormat LPCM/AC3/DTS/DolbyTrueHD/etc, EdidAudioBlock parsing, HdmiAudioInfoframe, enable/disable_audio, set_format/channels/sample_rate, get_capabilities, EDID CEA extension parsing, ~650 linhas) |
| 2026-01-18 | [Áudio] Microphone (drivers/audio/mixer.rs: pré-existente - ChannelType::Microphone, capture stream support, input device selection) |
| 2026-01-18 | [Áudio] MP3 Decode (drivers/audio/codecs.rs: Mp3Decoder struct, Mp3FrameHeader, decode_frame(), find_sync(), parse_header(), huffman tables placeholder, IMDCT placeholder, synthesis filterbank placeholder, ID3v2 tag parsing, ~350 linhas do total) |
| 2026-01-18 | [Áudio] AAC Decode (drivers/audio/codecs.rs: AacDecoder struct, AacConfig/AacObjectType LC/HE/HEv2, decode_frame(), ADTS header parsing, LC-AAC core decoder placeholder, SBR/PS placeholders, ~300 linhas do total) |
| 2026-01-18 | [Áudio] FLAC (drivers/audio/codecs.rs: FlacDecoder struct, FlacStreamInfo, decode_frame(), metadata block parsing STREAMINFO/VORBIS_COMMENT/PICTURE, sample rate/bits/channels extraction, ~200 linhas do total) |
| 2026-01-18 | [Áudio] WAV Support (drivers/audio/codecs.rs: WavInfo struct, AudioDecoder trait, decode_wav() RIFF/WAVE parsing, PCM/IEEE float support, ~100 linhas do total - totalizando codecs.rs ~950 linhas) |
| 2026-01-18 | [Rede] Intel WiFi 6/6E (drivers/net/iwlwifi.rs: pré-existente - IwlWifiDriver struct, device IDs AX200/AX201/AX210/AX211, WifiState/WifiNetwork/WifiSecurity, scan/connect/disconnect, ~782 linhas) |
| 2026-01-18 | [Rede] WiFi Firmware Loading (drivers/net/iwlwifi.rs: integrado com drivers/firmware.rs - request_firmware() para iwlwifi-*.ucode, firmware path management) |
| 2026-01-18 | [Rede] Intel I219 Ethernet (drivers/net/igb.rs: pré-existente - IgbDriver struct, Intel I219/I225/I226 device IDs, MMIO setup, DMA tx/rx, link detection, ~788 linhas) |
| 2026-01-18 | [Rede] Realtek RTL8111/8168 (drivers/net/rtl8169.rs: pré-existente - Rtl8169Driver struct, RTL8111/8168/8169 device IDs, tx/rx descriptors, link status, ~725 linhas) |
| 2026-01-18 | [Rede] Intel Bluetooth (drivers/bluetooth/usb_transport.rs: pré-existente - Intel Bluetooth USB device IDs 0x0025/0x0026/0x0029/etc, BT_USB transport) |
| 2026-01-18 | [Rede] WireGuard VPN (net/wireguard.rs: WgInterface/WgPeer/WgKey/SessionKeys structs, WgConfig, Noise protocol handshake, ChaCha20-Poly1305 encryption, X25519 key exchange, create_initiation/process_response, send/recv_packet, keepalive, ~800 linhas) |
| 2026-01-18 | [Power] Power Management Module (power/mod.rs: PowerManager struct, PowerState/PowerProfile enums, BatteryInfo/BatteryStatus, PowerEvent callbacks, AC/battery/lid handling, suspend/hibernate/shutdown/reboot APIs, ~380 linhas) |
| 2026-01-18 | [Power] S3 Suspend (power/suspend.rs: suspend_to_ram() com device callbacks, freeze_tasks/thaw_tasks, save/restore_cpu_state, enter_acpi_s3() via PM1a_CNT register, ~300 linhas) |
| 2026-01-18 | [Power] S0ix Modern Standby (power/suspend.rs: enter_s0ix() com MWAIT C-state hints, device low power states, platform idle, ~50 linhas) |
| 2026-01-18 | [Power] S4 Hibernate (power/suspend.rs: hibernate() com create_hibernate_image, write_hibernate_image, enter_acpi_s4, ~50 linhas) |
| 2026-01-18 | [Power] CPU Frequency Scaling (power/cpufreq.rs: CpuFreqManager struct, Governor enum Performance/Powersave/Ondemand/Schedutil, set_governor(), apply_profile(), ~550 linhas) |
| 2026-01-18 | [Power] Intel P-State (power/cpufreq.rs: init_intel_pstate(), HWP support via MSR 0x770/0x774, CPUID HWP detection, MSR_PLATFORM_INFO/MSR_TURBO_RATIO_LIMIT parsing, ~150 linhas) |
| 2026-01-18 | [Power] AMD P-State (power/cpufreq.rs: init_amd_pstate(), CPPC support via MSR 0xC0010061-62, CPUID CPPC detection, set_cppc_desired(), ~100 linhas) |
| 2026-01-18 | [Power] Intel EPP (power/cpufreq.rs: EnergyPerformancePreference enum Performance/BalancePerformance/BalancePower/Power, set_hwp_epp() via MSR 0x774 bits 24-31) |
| 2026-01-18 | [Power] Turbo Boost Control (power/cpufreq.rs: set_turbo() via MSR_IA32_MISC_ENABLE bit 38, turbo enable/disable) |
| 2026-01-18 | [Power] Battery Status (power/mod.rs: detect_batteries() usando drivers/battery API, BatteryInfo struct com status/percentage/capacity/voltage/rate) |
| 2026-01-18 | [Power] Time Remaining (power/mod.rs: time_remaining() calcula minutos restantes baseado em rate e capacity) |
| 2026-01-18 | [Power] Low Battery Warning (power/mod.rs: update_batteries() dispara PowerEvent::BatteryLow quando percentage <= threshold) |
| 2026-01-18 | [Áudio] Opus Codec (drivers/audio/codecs.rs: OpusDecoder struct, OpusBandwidth/OpusMode enums Silk/Celt/Hybrid, OpusHeader struct com version/channel_count/pre_skip/input_sample_rate/output_gain, silk_prev_samples/silk_lpf_state para SILK mode, celt_prev_buffer/celt_preemph para CELT mode, range_val/range_rng para range coder, parse_opus_header() OGG container, decode_silk_frame/decode_celt_frame stubs, AudioDecoder trait impl, ~200 linhas) |
| 2026-01-18 | [Áudio] Vorbis Codec (drivers/audio/codecs.rs: VorbisDecoder struct, VorbisIdHeader struct com vorbis_version/channels/sample_rate/bitrate_max/nominal/min/blocksize, VorbisWindowType Short/Long enum, overlap_buffer para overlap-add, prev_window tracking, parse_ogg_page() para pacotes Vorbis, setup_header/codebook parsing stubs, inverse_mdct_stub, window_and_overlap() para síntese, AudioDecoder trait impl, ~180 linhas)
| 2026-01-18 | [Power] Critical Battery Action (power/mod.rs: CriticalBatteryAction enum Suspend/Hibernate/Shutdown, PowerEvent::BatteryCritical) |
| 2026-01-18 | [Power] Power Profiles (power/profiles.rs: ProfileManager struct, ProfileSettings com CPU/Display/Disk/Wireless settings, Performance/Balanced/PowerSaver/BatterySaver presets, handle_power_source_change() auto-switch, ~380 linhas) |
| 2026-01-18 | [Thermal] Thermal Monitoring (drivers/thermal.rs: ThermalSubsystem/ThermalZone structs, CpuVendor detection via CPUID, Intel MSR 0x19C/0x1B1 TjMax-based temp reading, AMD SMN 0x59800 Tctl reading, TripPoint Active/Passive/Hot/Critical thresholds, poll_thermal() periodic monitoring, Temperature type in mC, ~450 linhas) |
| 2026-01-18 | [Thermal] Thermal Throttling (drivers/thermal.rs: ThrottleLevel enum None/Light/Medium/Heavy/Maximum, apply_thermal_throttling() margin-based throttle calculation, cpufreq integration via max freq limit, clear_throttling(), get_throttle_level()/is_throttled() status, handle_critical_temperature() emergency shutdown via ACPI, ~100 linhas) |
| 2026-01-18 | [Thermal] Fan Profiles (drivers/thermal.rs: FanController struct, FanControlMode Auto/Manual/FullSpeed, FanInfo com speed_rpm/pwm_percent, EC-based control via ports 0x66/0x62, ec_read_fan_speed()/ec_set_fan_pwm(), set_fan_speed()/set_fans_auto()/set_fans_full() APIs, ~200 linhas) |
| 2026-01-18 | [Segurança] Secure Boot (security/secureboot.rs: SecureBootState/SecureBootMode/CertInfo/SignatureDatabase/SignatureEntry structs, EfiGuid/EFI_CERT_* constants, init() UEFI variable parsing db/dbx/KEK/PK/MOK, verify_hash() signature validation, db_contains/dbx_contains() certificate lookup, mode()/is_enabled() status, ~470 linhas) |
| 2026-01-18 | [Segurança] TPM 2.0 Driver (security/tpm.rs: TpmState/TpmInfo/Tpm2 structs, TpmError enum, TPM2_CC_* command codes, TPM2_ALG_* algorithm IDs, TPM2_RC_* response codes, TIS MMIO interface 0xFED40000, init() detect/startup, get_random() PRNG, pcr_read/pcr_extend() PCR operations, locality management access0-4, command/response buffer handling, ~600 linhas) |
| 2026-01-18 | [Segurança] PAM Authentication (security/pam.rs: PamState/PamHandle/PamConfig structs, PamResult enum 20+ codes, PamModuleType Auth/Account/Session/Password, PamControl Required/Requisite/Sufficient/Optional, PamModule trait, built-in modules PamUnix/PamPermit/PamDeny/PamEnv/PamLimits/PamSecuretty, authenticate/acct_mgmt/open_session/close_session/chauthtok APIs, password hash verification, session environment setup, ~700 linhas) |
| 2026-01-18 | [Segurança] App Sandbox (security/sandbox.rs: SandboxManager/SandboxProfile/Sandbox structs, ResourceLimits/FsRule/FsAccess/NetworkPolicy/IpcPolicy enums, Namespace flags USER/PID/NET/MNT/IPC/UTS/CGROUP, create_sandbox/activate/destroy, apply_sandbox_by_profile() with namespaces/fs_rules/network/capabilities/seccomp/limits, built-in profiles minimal/desktop_app/browser/untrusted, ~500 linhas) |
| 2026-01-18 | [Segurança] LUKS2 Encryption (crypto/luks.rs: LuksHeader/LuksKeySlot/LuksVolume/LuksManager structs, LuksError/LuksState enums, LUKS_MAGIC/VERSION constants, pbkdf2_sha256() key derivation, aes_xts_encrypt/decrypt_sector() AES-XTS-plain64, add_key_slot/remove_key_slot, unlock/lock, EncryptedBlockDevice for dm-crypt integration, sys_luks_format/open/close syscalls, ~900 linhas) |
| 2026-01-18 | [Segurança] Recovery Keys (crypto/luks.rs: RecoveryKey/RecoveryKeyManager/RecoveryKeyInfo structs, RECOVERY_WORDS BIP39-style word list 256 words, generate() 24-word recovery phrase, from_words() parsing, as_passphrase() LUKS slot integration, create_recovery_key/verify_recovery_key/unlock_with_recovery/remove_recovery_key, init_recovery(), sys_create_recovery_key/sys_unlock_with_recovery syscalls, ~300 linhas adicionais) |
| 2026-01-18 | [Segurança] Permission System (security/permission.rs: Permission enum 27 types Camera/Mic/Location/Files/Network/etc, PermissionState/PermissionResult enums, AppPermission/AppPermissions/PermissionRequest structs, PermissionManager with register_app/check_permission/request_permission/grant_permission/deny_permission/revoke_permission, record_access tracking, PermissionSet bitflags, require_permission helper, sys_*_permission syscalls, ~600 linhas) |
| 2026-01-18 | [Input] Touchpad Gestures (drivers/touchpad.rs: pré-existente - TouchpadDriver com TouchpadState state machine Idle/OneFinger/TwoFinger/MultiFinger/TapWait/Dragging/Scrolling, two-finger tap/scroll, three-finger tap, multi-finger swipe gestures, process_touch_data() gesture detection, ~1674 linhas total) |
| 2026-01-18 | [Input] Palm Rejection (drivers/touchpad.rs: pré-existente - TouchpadConfig.palm_threshold configurable, pressure-based rejection em process_touch_data() lines 757-759, per-protocol support Synaptics/ALPS/USB HID) |
| 2026-01-18 | [i18n] UTF-8 Complete (unicode.rs: pré-existente - decode_utf8/encode_utf8 encoding, Category enum 25 types, character classification is_letter/is_digit/etc, case conversion to_uppercase/to_lowercase, UnicodeBlock enum, char_width/string_width display width, ~557 linhas) |
| 2026-01-18 | [i18n] Locale System (i18n/locale.rs: LocaleId struct 35+ locales EN_US/PT_BR/ES_ES/etc, Language/Country enums, Locale struct com date_format/time_format/first_day_of_week/decimal_separator/thousands_separator/currency_symbol, weekday_names/month_names localized, ~350 linhas) |
| 2026-01-18 | [i18n] Translations Framework (i18n/translations.rs: Translations catalog struct, PluralForms/PluralCategory, TranslationContext interpolation, get_plural_category() CLDR plural rules por idioma en/fr/ru/ar/zh, ~230 linhas) |
| 2026-01-18 | [i18n] Portuguese (BR) (i18n/mod.rs: I18nManager struct, 50+ translation strings welcome/login/settings/file/edit/error/confirm, t()/t_args() convenience functions, ~450 linhas mod.rs) |
| 2026-01-18 | [i18n] Date/Time/Number Formats (i18n/formats.rs: DateFormat DMY/MDY/YMD, TimeFormat H12/H24, NumberFormat struct decimal/thousands separators, CurrencyFormat struct symbol/position, format_date/format_time/format_number/format_decimal/format_currency helpers, format_relative_time() localized, ~350 linhas) |
| 2026-01-18 | [Performance] Fast Boot + Parallel Init (boot/mod.rs: BootStage enum 17 stages, BootTiming/BootProfiler structs com timing report, ServicePriority/ServiceState/ServiceDescriptor para service management, ParallelInitializer com Kahn's algorithm topological sort dependency resolution, LazyInit<T> wrapper para lazy initialization, DeferredTask/DeferredTaskManager para post-boot tasks, FastBootConfig para target_boot_time/lazy_usb/lazy_network/deferred_services, global BOOT_PROFILER/DEFERRED_TASKS, start_stage/end_stage/boot_complete/get_boot_report APIs, ~760 linhas) |
| 2026-01-18 | [Performance] OOM Killer (mm/oom.rs: OomKiller struct, MemoryPressure/OomPolicy/OomNotificationKind enums, ProcessMemoryStats/OomProcessInfo com RSS/VMS tracking, calculate_score() scoring algorithm memory%/niceness/age/adjustment, OomConfig para thresholds/panic_on_oom/kill_children, OomKillResult/OomStats/OomNotification structs, select_victim() candidate selection, handle_oom() kill logic, global OOM_KILLER com APIs set_oom_adj/set_policy/protect/trigger/stats, process_info/memory_info/kill callbacks, ~700 linhas) |
| 2026-01-18 | [Performance] TRIM Support (storage/trim.rs: TrimManager struct, TrimRange/TrimResult/TrimCapabilities/TrimDiskInfo/TrimStats structs, DiskType SATA/NVMe/VirtioBlk enums, TrimBatch com merge/accumulate logic, AtaTrimHandler ATA DSM payload building 8-byte entries, NvmeTrimHandler NVMe DSM 16-byte entries, queue_trim/trim_immediate/flush_batch APIs, fstrim() scheduled TRIM, detect_ata_trim_caps/detect_nvme_trim_caps helpers, discard() filesystem helper, global TRIM_MANAGER, ~750 linhas) |
| 2026-01-18 | [Hardware Test] Test Suite (tests/mod.rs + tests/hardware.rs: pré-existente - TestRunner framework com TestResult/TestOutcome/TestDef/TestStats, test_assert/test_assert_eq/test_assert_ne macros, register_tests! macro, run_all_tests() orquestrador, hardware tests para CPU features/memory regions/paging structures/serial port/timer functionality, ~700 linhas total) |
| 2026-01-18 | [Performance] Memory Compression (mm/zswap.rs: ZswapPool struct com compressed page storage, CompressionAlgo enum LZ4/LZ4HC/ZSTD/LZO/None, CompressedPage/ZswapStats/ZswapConfig structs, same_filled_pages deduplication, RLE-based compression placeholder, LRU writeback, ZramDevice struct compressed RAM block device, create_zram()/zswap_store()/zswap_load()/zswap_invalidate() APIs, ~750 linhas) |
| 2026-01-18 | [Performance] I/O Scheduler (storage/iosched.rs: IoScheduler trait, SchedulerType enum MqDeadline/Bfq/Cfq/None, IoRequest struct com priority/deadline/sector, DeadlineQueue read/write FIFO + sorted queues com batching config, BfqScheduler per-process budgets/weights/service_tree/async queue, CfqScheduler cfq_queue/time_slice/seek_cost, SchedulerConfig tunables, global IO_SCHEDULER registry, register_device/submit/dispatch/complete APIs, elevator algorithm, ~700 linhas) |
| 2026-01-18 | [Performance] Write Caching (storage/writecache.rs: DeviceCache struct write-back cache, CacheEntry state/dirty_time/access tracking, CacheState enum Clean/Dirty/Writeback/Locked, WriteCacheConfig dirty_ratio/bg_dirty_ratio/expire_ms/coalesce_writes, CacheStats hits/misses/flushes tracking, WriteBatch coalescing adjacent sectors, LRU eviction policy, WriteResult enum, WriteCacheManager multi-device registry, write_barrier/sync APIs, background_tick() periodic flush, sysctl-style tunables set_dirty_ratio/set_write_through, ~750 linhas) |
| 2026-01-18 | [Performance] Filesystem Tuning (fs/tuning.rs: MountOptions struct ro/noatime/relatime/sync/barrier/discard/journal_mode/commit_interval/quota, JournalMode enum Data/Ordered/Writeback, AllocationTuning strategy/extent_size/prealloc/delayed_alloc/bigalloc, ReadaheadConfig initial/max_size/adaptive/sequential, CacheConfig inode/dentry/buffer cache sizes, TuningProfile enum Balanced/Throughput/Latency/Ssd/Nvme/Hdd/DataSafety/Performance/Laptop/Server/Desktop, FilesystemTuning complete config struct, Ext4Tuning/Fat32Tuning/NtfsTuning fs-specific options, FsPerformanceStats tracking, sysctl dirty_ratio/writeback APIs, auto_detect_profile(), ~930 linhas) |
| 2026-01-18 | [Instalador] Dual Boot Detection (installer/dualboot.rs: OsType enum Windows/Linux/MacOs/FreeBsd/OpenBsd/NetBsd/Haiku/ChromeOs, WindowsVersion/LinuxDistro enums para detecção específica, BootMode enum Bios/Uefi/UefiSecureBoot, DetectedOs struct com name/version/boot_partition/root_partition/efi_partition, DualBootDetector com probe_windows/probe_linux/probe_macos/probe_bsd, EfiBootEntry/BootEntryManager para EFI boot entries, GrubConfigGenerator com chainload para Windows e outras distros, SystemdBootConfigGenerator com loader.conf + entries, create_grub_config/create_systemd_boot_config APIs, ~800 linhas) |
| 2026-01-18 | [Instalador] Timezone/Locale (installer/timezone.rs: Timezone struct com id/utc_offset/dst_offset/abbreviation, TzRegion enum para 11 regiões mundiais, Locale struct com language/country/encoding/native_name/rtl, KeyboardLayout struct code/variant/model, DateTimeConfig date_format/time_format/first_day, RegionalSettings complete config, TimezoneDatabase 40+ timezones com search/by_region/by_country, LocaleDatabase 35+ locales com search/by_language, KeyboardDatabase 30+ layouts, RegionalConfigManager com auto_detect(), generate_timezone_file/locale_conf/vconsole_conf generators, IANA timezone format compliant, ~900 linhas) |
| 2026-01-18 | [Profiling] perf Support (profiling/perf.rs: HardwareEvent enum Cycles/Instructions/CacheReferences/CacheMisses/BranchInstructions/BranchMisses, SoftwareEvent enum CpuClock/TaskClock/PageFaults/ContextSwitches, CacheEvent/CacheOp/CacheResult enums, PerfEvent complete spec, PerfEventAttr config struct, PerfCounter handle com count/time_enabled/time_running, PerfReadFormat output, PerfSample/SampleBuffer para sampling mode, PmuCapabilities detect Intel/AMD, PerfManager com open/close/enable/disable/read/reset, perf_event_open/close/enable/disable/read APIs, measure_cycles() helper, ~700 linhas) |
| 2026-01-18 | [Profiling] eBPF (profiling/ebpf.rs: BpfClass/BpfAluOp/BpfJmpOp enums instruction set, BpfInsn struct 64-bit encoding, BpfRegs R0-R10 register file, BpfMapType Hash/Array/ProgArray/PerfEventArray/PercpuHash/Ringbuf, BpfMap struct com lookup/update/delete/get_next_key, BpfProgType SocketFilter/Kprobe/Tracepoint/Xdp/PerfEvent/Lsm, BpfProg program container, BpfVerifier instruction/control-flow verification, BpfVm interpreter ALU64/ALU32/JMP/LDX/STX/ST opcodes, helper functions bpf_map_lookup/update/delete/ktime_get_ns, BpfManager create_map/load_prog/run_prog APIs, ~1100 linhas) |
| 2026-01-18 | [Profiling] Tracing/ftrace (profiling/ftrace.rs: TraceEntryType enum Func/FuncGraph/Tracepoint/Print/Stack/Irq/Sched/Syscall, TraceEntryHeader struct timestamp/cpu/pid/tid/preempt_count, FuncTraceEntry func_addr/parent_addr/func_name, GraphTraceEntry entry_time/exit_time/duration/depth para call graphs, TracepointEntry name/subsystem/args, PrintTraceEntry message buffer, StackTraceEntry frame addresses, IrqTraceEntry irq/handler/action, SchedTraceEntry prev/next pid/comm/prio, SyscallTraceEntry nr/args/ret/entry_time, TraceBuffer ring buffer per-CPU com head/tail/overwrite mode, Tracepoint definition com enable/disable/hit_count atomics, TraceFilter enum FuncPrefix/Suffix/Contains/Pid/Cpu/Subsystem, TracerType enum Function/FunctionGraph/Irqsoff/Preemptirqsoff/Wakeup/Sched/Syscalls, FtraceManager global com current_tracer/enabled/max_latency/trace_threshold, built-in tracepoints sched_switch/sched_wakeup/syscall_entry/syscall_exit/irq_handler_entry/irq_handler_exit/softirq_entry/softirq_exit, trace_function/trace_graph_entry/trace_graph_exit/record_tracepoint/trace_stack/trace_irq_entry/trace_irq_exit/trace_syscall_entry/trace_syscall_exit APIs, buffer_snapshot/format_entry/dump_trace output, latency analysis support, ~990 linhas) |
| 2026-01-18 | [Profiling] CPU Profiler (profiling/cpuprof.rs: SamplingMode enum Timer/Event/Ibs/Pebs, ProfilerState enum Stopped/Running/Paused, ProfilerConfig sample_freq/sampling_mode/max_stack_depth/include_kernel/include_user/target_pid/target_cpu/buffer_size, StackFrame struct ip/sp/bp/func_name/module/offset/is_kernel com format(), StackTrace frames collection com format_folded() para flame graphs, ProfileSample timestamp/cpu/pid/tid/comm/ip/stack/kernel_mode/context, SampleContext cycles/instructions/cache_refs/cache_misses/branches/branch_misses, FunctionStats self_samples/total_samples/self_percent/total_percent/children, ProfileStats total_samples/lost_samples/duration_ns/per_cpu/per_process, FlameNode recursive tree com add_stack/format_folded/format_svg, FlameGraph generation, SampleBuffer ring buffer com overflow tracking, StackWalkMethod enum FramePointer/Dwarf/Orc/Lbr, StackWalker walk_frame_pointer/walk_lbr com MSR LBR reading, SymbolResolver kallsyms loader com address resolution, CpuProfiler main struct com start/stop/pause/resume/record_sample/generate_stats/generate_flame_graph, ProfileReport text report generation, global profiler() singleton, ~950 linhas) |
| 2026-01-18 | [Profiling] Memory Profiler (profiling/memprof.rs: AllocType enum Kernel/User/Slab/Page/Dma/Mmio/Stack/PerCpu, AllocFlags bitflags ZERO/DMA/NOWAIT/ATOMIC/KERNEL/USER, AllocationRecord struct addr/size/alloc_type/flags/timestamp/pid/tid/stack/caller/line com age_ns(), CallsiteStats struct caller/caller_name/alloc_count/free_count/total_bytes_allocated/freed/live_count/live_bytes/peak_count/peak_bytes/avg_size/min_size/max_size com record_alloc/record_free/potential_leaks, MemoryStats global struct total_allocs/frees/bytes_allocated/freed/current_allocs/current_bytes/peak_allocs/peak_bytes/failed_allocs/per_type/per_callsite/per_process, TypeStats/ProcessStats per-category tracking, PotentialLeak struct record/age_ns/confidence/reason, LeakReason enum LongLived/Unreachable/Orphaned/Growing/ThresholdExceeded, LeakDetectorConfig min_age_ns/track_kernel/track_user/max_tracked/capture_stack/stack_depth, LeakDetector com record_alloc/record_free/scan/calculate_confidence, MemoryStatCounters atomic counters, MemoryProfiler main struct com enable/disable/pause/resume/record_alloc/record_free/record_failed_alloc/scan_for_leaks/get_stats/reset, allocation hooks on_kmalloc/on_kfree/on_page_alloc/on_page_free, AllocationHistogram size distribution buckets 0-64B to 1M+, ~1050 linhas) |
| 2026-01-18 | [Instalador] Recovery Partition (installer/recovery_partition.rs: RecoveryPartitionConfig size_mb/filesystem/label/include_diagnostics/include_network/include_factory_reset/include_backup/compress/encrypt, RecoveryFilesystem enum Ext4/Fat32/ExFat/SquashFs com mkfs_type(), RecoveryEnvironment enum Minimal/Standard/Full com estimated_size_mb(), RecoveryTool struct name/description/path/dependencies/size_kb, builtin_tools() 13 tools fsck/grub-install/bootctl/chroot/mount/parted/lsblk/memtest/smartctl/network-config/backup-restore/factory-reset/recovery-shell, RecoveryLayout boot_dir/bin_dir/lib_dir/etc_dir/tmp_dir/system_image/factory_image, RecoveryPartitionBuilder com required_size_mb/validate/build stages create_partition/format/create_directories/install_kernel/install_tools/create_images/configure_boot/verify, RecoveryBuildResult Success/Failed, RecoveryEntryType enum Recovery/SafeMode/NetworkRecovery/Diagnostic/FactoryReset/MemoryTest com kernel_params/title, generate_boot_entries() 6 entries, generate_grub_recovery_menu() GRUB submenu, generate_systemd_boot_entries() loader entries, RecoveryPartitionManager detect/mount/unmount/check_health/update, RecoveryHealthStatus enum Healthy/NotFound/Corrupted/LowSpace/NeedsUpdate, boot_recovery/trigger_factory_reset APIs, ~750 linhas) |
| 2026-01-18 | [Instalador] LUKS Encryption (installer/luks.rs: CipherAlgorithm enum Aes/Twofish/Serpent/ChaCha20 com name/default_key_size, CipherMode enum Xts/CbcEssiv/CbcPlain/Gcm, HashAlgorithm enum Sha256/Sha512/Ripemd160/Whirlpool com digest_size, KdfType enum Pbkdf2/Argon2i/Argon2id, KdfParams struct iterations/memory_kb/time_cost/parallelism, KeySlotState enum Inactive/Active/Disabled, KeySlot struct index/state/kdf/salt/key_material/key_offset/key_sectors/af_stripes/priority, LuksHeader struct version/uuid/label/cipher/cipher_mode/key_size/hash/payload_offset/key_slots/mk_digest, LuksConfig full encryption config, IntegrityMode enum HmacSha256/Poly1305/None, LuksResult/LuksError enums, LuksDevice struct com format/read_header/open/close/add_key/remove_key/change_key, add_key_to_slot_impl/write_header_impl static helpers, derive_key PBKDF2/Argon2, af_split/af_merge anti-forensic, encrypt/decrypt_key_material, LuksBootParams struct, generate_crypttab_entry/generate_initramfs_config/generate_grub_cryptodisk_config boot integration, format_luks/open_luks/close_luks/is_luks_device public APIs, ~900 linhas) |
| 2026-01-18 | [Instalador] Cloud Images (installer/cloud.rs: CloudImageFormat enum Raw/Qcow2/Ami/Vhd/Vhdx/Vmdk/Vdi/Ova com extension/mime_type/supports_compression/supports_snapshots, CloudProvider enum Aws/Gcp/Azure/OpenStack/DigitalOcean/Vultr/Linode/VSphere/Proxmox/Generic com preferred_format/cloud_init_datasource, CloudImageConfig name/version/format/provider/disk_size_gb/compress/cloud_init/include_ssh/root_password/default_user/partitions/network/packages/scripts, UserConfig/PartitionLayout/NetworkConfig structs, Qcow2Header v3 struct com magic/version/cluster_bits/size/l1_table/refcount_table/snapshots, VhdFooter 512-byte struct cookie/features/disk_geometry/disk_type/checksum FIXED/DYNAMIC, VmdkDescriptor/VmdkExtent para VMDK, CloudInitConfig com datasource_list/manage_etc_hosts/users/packages/runcmd to_yaml(), CloudInitUser struct, CloudImageBuilder com build stages validate/create_raw_image/partition_disk/format_partitions/install_system/configure_system/convert_image/calculate_checksum, AwsAmiBuilder/AzureVhdBuilder/GcpImageBuilder provider-specific builders, build_cloud_image/build_aws_ami/build_azure_vhd/build_gcp_image APIs, supported_formats/supported_providers queries, ~850 linhas) |
| 2026-01-18 | [Instalador] Docker Base Image (installer/docker.rs: ContainerArch enum Amd64/Arm64/Arm32v7/I386/Ppc64le/S390x/Riscv64 com as_str/variant, LayerCompression enum None/Gzip/Zstd/Lz4 com media_type, ImageVariant enum Full/Minimal/Micro/Dev/Runtime, OciImageConfig/ContainerConfig/RootfsConfig/HistoryEntry structs com to_json(), OciManifest/ManifestDescriptor structs, OciImageIndex/IndexManifest/Platform para multi-arch, LayerContent/LayerFile structs para layer building, DockerInstruction enum From/Run/Copy/Add/Env/Workdir/Expose/Volume/User/Cmd/Entrypoint/Label/Arg/Shell/Healthcheck/Stopsignal, DockerImageBuilder com registry/tag/variant/compression/architecture/entrypoint/cmd/env/workdir/expose/volume/user/label/run builder methods, create_base_layer() essential OS files /etc/os-release/passwd/group/shadow/hosts/resolv.conf, create_variant_layers() variant-specific content, build()/build_multiarch() manifest generation, serialize_layer() tar format USTAR headers, compress_layer() gzip/zstd/lz4 headers, compute_sha256() hash, generate_dockerfile(), RegistryClient blob_exists/upload_blob/upload_manifest/pull_manifest/list_tags, OciLayoutBuilder oci-layout directory structure, build_stenzel_base_image()/build_multiarch_image() convenience functions, OCI_IMAGE_SPEC_VERSION 1.0.2, ~900 linhas) |
| 2026-01-18 | [Instalador] A/B Partitions (installer/ab_partitions.rs: Slot enum A/B com as_char/as_suffix/other, SlotState enum Successful/Unverified/Unbootable, SlotMetadata struct slot/state/priority/tries_remaining/successful_boot/version/build_timestamp/content_hash/slot_size/used_size com is_bootable/mark_successful/decrement_tries, AbMetadataHeader #[repr(C)] struct magic/version/crc32/active_slot/slot_a/b_state/priority/tries/version/hash com validate/calculate_crc/to_bytes/from_bytes, AbPartition struct name/size/updatable/device_template/part_num_a/b com device_path(), AbLayout struct boot/system/vendor/metadata_device/userdata_device default 512MB boot + 16GB system, UpdateState enum Idle/Downloading/Verifying/Applying/PendingReboot/Failed, AbManager struct layout/slot_a/b/active_slot/update_state/update_target/boot_count/metadata_dirty com init/load_metadata/save_metadata/select_active_slot/mark_boot_successful/begin_update/finalize_update/cancel_update/rollback, AbStatus summary struct, AbBootloader generate_cmdline/generate_grub_config/generate_systemd_boot_config helpers, global ab_manager() singleton, MAX_BOOT_ATTEMPTS=3, CRC32 checksum, ~850 linhas) |
| 2026-01-18 | [Instalador] Factory Reset (installer/factory_reset.rs: ResetMode enum KeepUserData/KeepUserDataAndSettings/Full/SecureWipe/Developer com wipes_user_data(), ResetStage enum NotStarted/Validating/BackingUp/PreparingPartitions/RestoringSystem/RestoringBootloader/CleaningUserData/SecureWiping/RestoringUserData/Finalizing/Complete/Failed, FactoryImage struct path/version/build_timestamp/size/sha256/compression/format/verified, CompressionType enum None/Gzip/Zstd/Lz4/SquashFs, ImageFormat enum Raw/Tar/Cpio/SquashFs/Erofs, ResetConfig struct mode/require_password/create_backup/backup_destination/preserve_dirs/preserve_files/wipe_passes/reboot_after/show_progress, PreservedItem/PreservedItemType structs, ResetProgress struct stage/percent/operation/bytes/files/eta, FactoryResetManager struct config/factory_image/stage/in_progress/cancelled/progress/preserved_items/error com locate_factory_image/validate/execute_reset/backup_user_data/prepare_partitions/secure_wipe/clean_user_data/restore_system/restore_bootloader/restore_user_data/finalize/cancel, ResetTrigger schedule_reset/is_reset_scheduled/execute_scheduled_reset, full_reset/reset_keep_data convenience functions, ~850 linhas) |
| 2026-01-18 | [Instalador] Backup/Restore (installer/backup.rs: BackupType enum Full/Incremental/Differential/UserData/SystemOnly/Custom, CompressionLevel enum None/Fast/Normal/Best, CompressionAlgo enum None/Gzip/Zstd/Lz4/Xz com extension(), EncryptionAlgo enum None/Aes256Gcm/ChaCha20Poly1305, BackupStage enum NotStarted/Initializing/ScanningFiles/CreatingSnapshot/CompressingData/EncryptingData/WritingArchive/Verifying/Finalizing/Complete/Failed, BackupConfig struct backup_type/sources/excludes/destination/name/compression/encryption/verify/preserve_permissions/preserve_ownership/preserve_xattrs/preserve_acls/max_size/split_size/retention_count/retention_days, BackupHeader #[repr(C)] struct magic SBAK/version/backup_type/compression/encryption/timestamp/sizes/counts/crc/hash, BackupFileEntry struct path/file_type/mode/uid/gid/size/times/link_target/data_offset/crc32, FileType enum Regular/Directory/Symlink/Hardlink/Device/Fifo/Socket, BackupProgress struct stage/percent/current_file/files/bytes/compression_ratio/eta/speed, BackupManager struct config/stage/progress/file_list com scan_files/write_archive/verify_backup/apply_retention, BackupInfo result struct, RestoreStage/RestoreConfig/RestoreManager/RestoreInfo for restore ops, list_backups/get_backup_info/quick_backup/quick_restore convenience APIs, ~950 linhas) |
| 2026-01-18 | [Hardware] Intel RST (drivers/intel_rst.rs: RstRaidLevel enum Raid0/Raid1/Raid5/Raid10/Single/OptaneAcceleration com min_disks/has_redundancy, RstArrayState enum Normal/Degraded/Rebuilding/Failed/Initializing/Verifying, RstDiskStatus enum Online/Offline/Missing/Failed/Spare/Rebuilding, RstMetadataHeader #[repr(C)] struct signature/version/checksum/volume_count/disk_count, RstDiskEntry struct serial/model/port/sectors/status/role, RstVolume struct name/uuid/raid_level/stripe_size/state/members/spares/rebuild_percent/write_back_cache, OptaneConfig struct present/port/capacity/accelerated_disk/cache_mode/hit_rate/bytes, OptaneCacheMode enum WriteThrough/WriteBack/Disabled, RstControllerInfo struct vendor_id/device_id/generation/port_count/mmio_base/orom_version, RstGeneration enum Series5-700 com from_device_id/supports_optane, IntelRstDriver struct controller/volumes/disks/optane com init/detect_controller/read_metadata/detect_optane/create_volume/delete_volume/add_spare/start_rebuild/enable_optane/disable_optane/get_status, global rst_driver() singleton, ~700 linhas) |
| 2026-01-18 | [Hardware] eMMC (drivers/storage/emmc.rs: EmmcError/EmmcResult types, cmd module MMC command codes GO_IDLE_STATE/SEND_OP_COND/ALL_SEND_CID/SET_RELATIVE_ADDR/SELECT_CARD/SEND_EXT_CSD/READ_SINGLE_BLOCK/WRITE_BLOCK/etc, BusWidth enum Width1Bit/Width4Bit/Width8Bit com bits(), TimingMode enum Legacy/HighSpeed/Hs200/Hs400/Hs400Es com as_str/max_clock_mhz, EmmcPartition enum UserData/Boot1/Boot2/Rpmb/Gp1-4 com as_str/config_value, CidRegister struct mid/oid/pnm/prv/psn/mdt/crc com product_name/manufacturing_date, CsdRegister struct csd_structure/spec_vers/tran_speed/c_size/etc, ExtCsdRegister struct data[512] com sec_count/ext_csd_rev/device_type/boot_size_mult/rpmb_size_mult/gp_size/supports_hs200/supports_hs400, EmmcDeviceInfo struct cid/csd/ext_csd/rca/bus_width/timing/clock_khz/capacity/block_size/partitions/etc, EmmcController struct mmio_base/device/initialized/dma_buffer com init/reset_controller/set_clock/send_command/init_card_ocr/read_cid/read_csd/read_ext_csd/configure_bus/select_partition/read_blocks/write_blocks/erase_blocks/get_status/format_status, global emmc_controller() singleton, init()/is_present()/device_info()/format_status() APIs, MMC 5.1 compliant, ~720 linhas) |
| 2026-01-18 | [Hardware] SD Card SDHCI (drivers/storage/sdmmc.rs: pré-existente - regs/transfer_mode/command/present_state/host_control/power_control/clock_control/sw_reset/int_status modules para SDHCI registers, sd_cmd module com comandos SD CMD0-56 + ACMD6/13/22/23/41/42/51, CardType enum Unknown/Mmc/Sd/Sdhc/Sdxc/Emmc, CardCid/CardCsd structs para identificação de cartão, SdCard struct card_type/rca/cid/csd/scr/ocr/high_capacity/bus_width/clock_mhz/write_protected, SdhciController struct pci_dev/mmio_base/version/capabilities/card/index com new/reset/init/power_on/power_off/set_clock/wait_for_inhibit/send_command/card_init/set_bus_width/parse_cid/parse_csd/read_blocks/write_blocks, SdError enum, global SDHCI_CONTROLLERS, init() PCI scan class 08:05, controller_count/read_blocks/write_blocks/card_capacity/card_present/print_info APIs, ~1081 linhas) |
| 2026-01-18 | [Hardware] EHCI USB 2.0 (drivers/usb/ehci.rs: pré-existente - EHCI capability/operational register constants CAPLENGTH/HCIVERSION/HCSPARAMS/HCCPARAMS/USBCMD/USBSTS/USBINTR/FRINDEX/PERIODICLISTBASE/ASYNCLISTADDR/CONFIGFLAG/PORTSC, CMD_*/STS_*/PORTSC_* bit definitions, QueueHead struct 48-byte 32-aligned com hlp/ep_char/ep_caps/current_qtd/next_qtd/alt_qtd/token/buffer0-4 com new_async/link_to/link_to_self/link_qtd, TransferDescriptor struct 32-byte aligned com QTD_STATUS_*/QTD_PID_* token bits com new_setup/new_data/new_status/link_to/is_complete/has_error/bytes_transferred, EhciController struct pci_device/cap_base/op_base/num_ports/addr64_capable/periodic_list/async_head/devices com init/reset_port/enumerate_ports/enumerate_device/control_transfer_in/control_transfer_out, EhciDevice struct, global EHCI_CONTROLLERS, init() PCI scan class 0C:03:20, ~950 linhas) |
| 2026-01-18 | [Hardware] USB Power Management (drivers/usb/power.rs: UsbPowerState enum Active/Suspended/SelectiveSuspend/LpmL1/Disconnected/Error com as_str/is_low_power, LpmState enum L0Active/L1Sleep/L2Suspend/L3Off com as_str, RemoteWakeup enum NotSupported/Disabled/Enabled, PdRole enum None/Sink/Source/DualRole, PdPowerLevel enum Usb2Standard/Usb3Standard/Bc12/TypeC1_5A/TypeC3A/PowerDelivery com max_power_mw, AutoSuspendConfig struct enabled/idle_timeout_ms/min_active_time_ms/allow_remote_wakeup, DevicePowerInfo struct address/state/lpm_state/remote_wakeup/pd_role/power_level/max_power_mw/current_power_mw/last_activity_ms/auto_suspend/lpm_capable/u1u2_capable/besl com new/should_auto_suspend/touch, PortPowerInfo struct port/powered/state/device_connected/device_address/pd_capable/power_level/over_current, PowerError/PowerResult types, UsbPowerManager struct devices/ports/auto_suspend_enabled/total_power_mw/power_budget_mw/current_time_ms/lpm_enabled/suspend_callbacks/resume_callbacks com register_device/unregister_device/suspend_device/resume_device/enable_remote_wakeup/set_lpm_state/configure_auto_suspend/record_activity/set_port_power/check_over_current/periodic_check/suspend_all/resume_all/get_stats/format_status, PowerStats struct, global USB_POWER_MANAGER, ~780 linhas) |
| 2026-01-18 | [Hardware] GPIO (drivers/gpio.rs: GpioDirection enum Input/Output, GpioValue enum Low/High, GpioPull enum None/PullUp/PullDown, GpioTrigger enum None/RisingEdge/FallingEdge/BothEdges/LevelHigh/LevelLow, GpioPadOwner enum Host/Acpi/GpioDriver, GpioControllerType enum IntelSunrisePoint/CannonLake/TigerLake/AlderLake/RaptorLake/AmdFch/Unknown, GpioCommunity struct index/name/mmio_base/num_pads/first_pad, GpioPinConfig struct pin/direction/pull/trigger/value/owner/locked/native_function/label, intel_regs module PADBAR/HOSTSW_OWN/GPI_IS/GPI_IE/PAD_CFG_DW0/DW1, pad_cfg_dw0/dw1 bit definitions, amd_regs module GPIO_BANK_SELECT/OUTPUT/INPUT/CONTROL, GpioController struct controller_type/communities/pins/total_pins/interrupt_handlers com init/detect_controller/init_intel/init_amd/read_intel_pad_config/get_direction/set_direction/read/write/read_pin_value/write_pin_value/write_pin_config/find_community/set_pull/set_trigger/set_interrupt_handler/format_status, global GPIO_CONTROLLER, ~870 linhas) |
| 2026-01-18 | [Hardware] LPC/eSPI (drivers/lpc_espi.rs: InterfaceType enum Lpc/Espi/Unknown, LpcDecodeRange enum SuperIo/Fdd/Lpt1-3/Com1-4/Keyboard/GamePort/Custom com port_range(), EspiChannel enum Peripheral/VirtualWire/Oob/FlashAccess, SuperIoChip enum Ite8728/8783/Nct6775-6795/Fintek8728/Nuvoton6106/Unknown com from_id/as_str, SuperIoLdn enum Fdc/Pp/Sp1/Sp2/Ec/Kbc/Gpio/Acpi/HwMon/Wdt, TpmInterfaceType enum Tis12/Tis20/Fifo/Crb, EcInterface enum Acpi/LpcMailbox/Espi, LpcEspiConfig struct interface_type/mmio_base/decode_ranges/espi_channels/superio_chip/superio_port/tpm_present/tpm_interface/ec_present/ec_interface, SuperIo struct index_port/data_port/chip/current_ldn com enter_config/exit_config/read/write/select_ldn/get_chip_id/enable_device/disable_device/get_io_base/set_io_base/get_irq/set_irq, EmbeddedController struct cmd_port/data_port com wait_ibf_empty/wait_obf_full/read/write/is_present/query_event, intel_regs module LPC_GEN_DEC/IOD/IOE e ESPI_* registers, LpcEspiController struct config/superio/ec com init/detect_controller/detect_superio/detect_ec/detect_tpm/superio/ec/has_tpm/has_ec/add_decode_range/format_status, global LPC_ESPI_CONTROLLER, ~750 linhas) |
| 2026-01-18 | [Firmware] UEFI Runtime (drivers/uefi_runtime.rs: EfiStatus enum Success/InvalidParameter/Unsupported/BufferTooSmall/NotReady/DeviceError/WriteProtected/OutOfResources/NotFound/SecurityViolation com as_str/is_success, ResetType enum Cold/Warm/Shutdown/PlatformSpecific, EfiTime struct year/month/day/hour/minute/second/nanosecond/timezone/daylight, EfiTimeCapabilities struct resolution/accuracy/sets_to_zero, EfiGuid struct data1-4 com from_bytes/to_bytes/EFI_GLOBAL_VARIABLE/EFI_VENDOR_MS_VARIABLE, EfiRuntimeServices struct header/get_time/set_time/get_wakeup_time/set_wakeup_time/set_virtual_address_map/convert_pointer/get_variable/get_next_variable_name/set_variable/get_next_high_monotonic_count/reset_system, EfiTableHeader struct signature/revision/header_size/crc32/reserved, UefiRuntimeManager struct runtime_services_phys/virt/virtual_mode/available/cached_time com init/set_virtual_mode/get_time_internal/set_time_internal/get_variable_internal/set_variable_internal/reset_internal/update_cached_time, global UEFI_RUNTIME, init()/is_available()/get_time()/set_time()/get_variable()/set_variable()/reset_system()/shutdown()/reboot()/format_status() APIs, ~710 linhas) |
| 2026-01-18 | [Firmware] UEFI Variables (drivers/uefi_vars.rs: guids module EFI_GLOBAL_VARIABLE/EFI_IMAGE_SECURITY_DATABASE/EFI_VENDOR_MS/EFI_SHELL_VARIABLE/STENZEL_OS_GUID, attrs module NON_VOLATILE/BOOTSERVICE_ACCESS/RUNTIME_ACCESS/HARDWARE_ERROR_RECORD/AUTHENTICATED_WRITE_ACCESS/TIME_BASED_AUTHENTICATED_WRITE_ACCESS/APPEND_WRITE/ENHANCED_AUTHENTICATED_ACCESS/NV_BS_RT/BS_RT, BootOptionType enum Unknown/HardDrive/CdRom/Usb/Network/FirmwareVolume/BbsBoot/FilePath, BootOption struct number/attributes/description/device_path/optional_data/option_type com LOAD_OPTION_* constants/is_active/is_hidden/from_bytes/detect_type/to_bytes, CachedVariable struct, UefiVariablesManager struct cache/max_cache_size/cache_enabled/boot_options/boot_order/secure_boot_enabled com init/string_to_ucs2/get_variable/set_variable/delete_variable/add_to_cache/read_boot_order/write_boot_order/read_boot_option/write_boot_option/delete_boot_option/load_boot_options/boot_options/boot_order/next_boot_number/create_boot_option/set_first_boot/read_secure_boot_state/is_secure_boot_enabled/is_setup_mode/is_pk_enrolled/read_timeout/write_timeout/read_platform_lang/read_os_indications_supported/OS_INDICATION_* constants/request_firmware_ui/get_stenzel_var/set_stenzel_var/clear_cache/cache_stats/format_status, global UEFI_VARS, init()/is_available()/get_variable()/set_variable()/delete_variable()/get_boot_order()/set_boot_order()/get_boot_option()/create_boot_option()/delete_boot_option()/is_secure_boot_enabled()/request_firmware_ui()/get_timeout()/set_timeout()/get_stenzel_var()/set_stenzel_var()/format_status() APIs, ~820 linhas) |
| 2026-01-18 | [Firmware] fwupd Support (drivers/fwupd.rs: FwupdStatus enum Unknown/Idle/Loading/Decompressing/Verifying/Scheduling/NeedsReboot/Downloading/Writing/Complete/Failed, DeviceFlags struct com INTERNAL/UPDATABLE/ONLY_OFFLINE/REQUIRE_AC/LOCKED/SUPPORTED/NEEDS_BOOTLOADER/REGISTERED/NEEDS_REBOOT/NEEDS_SHUTDOWN/REPORTED/NOTIFIED/etc bitflags com contains/set/clear, InstallFlags struct com NONE/ALLOW_REINSTALL/ALLOW_OLDER/FORCE/OFFLINE/etc, ReleaseUrgency enum Unknown/Low/Medium/High/Critical, UpdateProtocol enum UefiCapsule/UefiEsrt/Dfu/Redfish/VendorSpecific/Flashrom/UefiDbx/LogitechUnifying/Synaptics/DellEsrt/Nvme/Thunderbolt/IntelMe/IntelSpi, FirmwareRelease struct version/remote_id/uri/size/checksum_sha256/urgency/description/vendor/release_date/install_duration/protocol/is_downgrade, FirmwareDevice struct device_id/parent_id/name/vendor/vendor_id/version/version_bootloader/version_lowest/flags/guids/protocol/status/progress/releases/icons/serial/plugin/created/modified com is_updatable/needs_reboot/latest_release/has_update, EsrtEntry/EsrtHeader structs, HistoryEntry struct, Remote struct id/title/kind/enabled/keyring/metadata_uri/report_uri/firmware_base_uri/mtime/priority, FwupdManager struct devices/history/remotes/esrt_entries/status/running/pending_updates/check_on_startup/auto_download/allow_prereleases/percentage com init/setup_default_remotes/scan_esrt/enumerate_devices/enumerate_uefi_devices/enumerate_usb_dfu_devices/enumerate_nvme_devices/enumerate_thunderbolt_devices/count_pending_updates/get_device/get_devices/get_updates/get_history/get_remotes/set_remote_enabled/refresh/install/verify/unlock/clear_results/get_status/get_percentage/get_pending_count/is_running/stop/current_time/report_update/format_status, FwupdError enum, global FWUPD, init()/is_initialized()/get_devices()/get_updates()/get_device()/refresh()/install()/get_status()/get_pending_count()/get_remotes()/set_remote_enabled()/get_history()/format_status() APIs, ~880 linhas) |
| 2026-01-18 | [GPU] Intel Arc (drivers/intel_gpu.rs: device_ids module expandido com DG1/DG1_1 IDs para Intel DG1 discrete, ARC_A770_1/2/3/ARC_A750_1/2/ARC_A580_1/2/ARC_A380_1/2/ARC_A310_1/2 desktop IDs para Arc Alchemist ACM-G10/G11, ARC_A770M/A730M/A550M/A370M/A350M mobile IDs, ARC_PRO_A60/A60_1/A40/A30M/A50/A60M workstation IDs, GpuGeneration enum expandido com Gen12_5 para DG1 e XeHpg para Arc, GpuType enum Integrated/Discrete novo, from_device_id() atualizado para mapear novos IDs, min_graphics_memory() expandido 4GB DG1/8GB Arc, is_discrete()/gpu_type()/name() métodos novos, suporte completo Intel Arc A770/A750/A580/A380/A310 desktop + A770M/A730M/A550M/A370M/A350M mobile + Arc Pro workstation) |
| 2026-01-18 | [GPU] AMD RDNA3 (drivers/amd_gpu.rs: device_ids module expandido NAVI31_XTX/XT/XL/PRO/M_XT para RX 7900 XTX/XT/GRE e PRO W7900, NAVI32_XT/XL/PRO/M_XT/M_XL para RX 7800/7700 XT e mobile, NAVI33_XT/XTX/XL/XTM/XLM/PRO/PRO_M para RX 7600/7600 XT/7700S/7600S e PRO, PHOENIX/PHOENIX2/HAWK_POINT para Ryzen 7040/8040 APUs RDNA 3.5, STRIX_POINT/STRIX_HALO para Ryzen AI 9 HX RDNA 3+, GpuFamily enum expandido com Phoenix e StrixPoint families, from_device_id() atualizado com todos device IDs RDNA3/3.5/3+, is_apu()/is_rdna()/is_gcn() métodos, min_vram() e uses_dcn() atualizados para novos families, suporte completo RX 7900 XTX/XT/GRE + 7800/7700 XT + 7600/7600 XT desktop, 7900M/7800M/7700M/7700S/7600S mobile, PRO W7900/W7800/W7600/W7500 workstation, Phoenix/Hawk Point/Strix Point APUs) |
| 2026-01-18 | [GPU] NVIDIA Nouveau Basic (drivers/nvidia_gpu.rs: pré-existente - device_ids module com Kepler GK104/GK106/GK107/GK110/GK208, Maxwell GM107/GM200/GM204/GM206, Pascal GP100/GP102/GP104/GP106/GP107/GP108, Turing TU102/TU104/TU106/TU116/TU117, Ampere GA102/GA104/GA106/GA107, Ada Lovelace AD102/AD103/AD104/AD106/AD107 device IDs completos, GpuGeneration enum Kepler/Maxwell/Pascal/Turing/Ampere/Ada, NvidiaDisplayMode struct width/height/bpp/refresh_rate, NvidiaGpu struct mmio_base/device_id/generation/vram_size/current_mode com init/detect_vram/enable_vga_output/setup_framebuffer/set_display_mode, regs module com NV_PMC/PBUS/PTIMER/PFB/PDISP offsets, surface_format module, is_nvidia_gpu()/init_from_pci()/get_info()/is_present()/set_mode()/framebuffer_address()/wait_vblank()/probe_pci()/init() APIs, suporte básico modesetting/framebuffer para GeForce 600 até RTX 40 series, ~900 linhas) |
| 2026-01-18 | [GPU] Intel Power Wells (drivers/intel_power_wells.rs: PowerWell enum 40+ wells Misc/DdiA-E/DisplayCore/Pw1-5/Gt/Media/Render/Vdbox0-1/Vebox0-1/Compute0-1/Copy0-1/MemoryFabric/etc, PowerWellState enum Off/On/Enabling/Disabling, Platform enum Gen9/Gen11/Gen12/TigerLake/AdlerLake/RaptorLake/MeteorLake/XeHpg, PowerWellInfo struct well/state/always_on/domains/dependencies, register offsets PWR_WELL_CTL1-4/DC_STATE_EN/HSW_PWR_WELL_CTL/SKL_FUSE_STATUS, DC_STATE_* masks para DC3/DC5/DC6/DC9, PowerWellsManager struct mmio_base/platform/wells/dc_state/rc6_enabled/initialized com new/init/setup_dependencies/add_dependency/read_power_well_state/enable_power_well/disable_power_well/request_power/release_power/set_dc_state/enable_rc6/disable_rc6/get_status/format_status, global POWER_WELLS, init()/request_power()/release_power()/get_dc_state()/set_dc_state()/enable_rc6()/disable_rc6()/format_status() APIs, suporte Gen9-XeHpg power domains/DC states/RC6, ~700 linhas) |
| 2026-01-18 | [GPU] AMD GCN 4 Polaris (drivers/amd_gpu.rs: device_ids expandido com Polaris10 (Ellesmere) POLARIS10_XT/XT2/XT3/PRO/PRO2/PRO3/PRO4/D1/GL/GL2/GL3 para RX 480/580/470/570 + Pro WX 7100/5100/4100, Polaris11 (Baffin) POLARIS11_XT/XT2/XT3/PRO/PRO2/PRO3/GL/GL2 para RX 460/560 + Pro WX 4170/4150, Polaris12 (Lexa) POLARIS12_XT/XL/XL2/XL3/GL/GL2 para RX 550/550X + Pro WX 2100, Mobile variants POLARIS10_M/M2/POLARIS11_M/M2/POLARIS12_M/M2 para Pro 460/455/560M/460M/550M, Embedded POLARIS10_E/POLARIS11_E/POLARIS12_E, from_device_id() atualizado com 36 device IDs Polaris, suporte completo RX 400 series RX 480/470/460 + RX 500 series RX 580/570/560/550 desktop e mobile + Pro WX workstation, GCN 4.0 architecture) |
| 2026-01-18 | [GPU] AMD GCN 5 Vega (drivers/amd_gpu.rs: device_ids expandido com Vega10 desktop VEGA10_XT/XT2/XL/XL2/XTX/XTRA/XTRX/GL/GL2/GL3/GL4/GL5/SSG para RX Vega 64/56 + Frontier Edition + Pro WX 8200/8100 + Instinct MI25 + Pro SSG, Vega12 mobile VEGA12_GL/GL2/GL3/GL4/XT para Pro Vega 20/16, Vega20 7nm VEGA20_XT/XT2/XL/XL2/GL/GL2 para Radeon VII + Pro VII + Instinct MI50/MI60, Raven Ridge APU RAVEN/D1/D2/M/M2 para Ryzen 2000 series, Picasso APU PICASSO/M/M2/M3 para Ryzen 3000 series, Renoir APU RENOIR/XT/PRO/PRO2/M/M2 para Ryzen 4000 series, Cezanne APU CEZANNE/XT/M/PRO/PRO2 para Ryzen 5000G series, Lucienne APU LUCIENNE/M para Ryzen 5000 mobile, from_device_id() + is_apu check atualizados com 50+ device IDs Vega, GCN 5.0 architecture) |
| 2026-01-18 | [GPU] AMD RDNA 1 Navi (drivers/amd_gpu.rs: device_ids expandido com Navi10 desktop NAVI10_XT/XT2/XL/XL2/XLE/XLE2/GL/GL2/GL3/GL4 para RX 5700 XT + 5700 + 5600 XT + Pro W5700/W5700X, Navi10 mobile NAVI10_M_XT/M_XL/M_PRO para RX 5700M + 5600M + Pro 5600M, Navi14 desktop NAVI14_XT/XT2/XL/GL/GL2/GL3 para RX 5500 XT + 5500 + Pro W5500/W5500X, Navi14 mobile NAVI14_XTM/XLM/M_XT/M_XL/M_PRO/M_PRO2 para RX 5500M + 5300M + Pro 5500M/5300M, Navi12 Apple NAVI12/PRO/GL para Radeon Pro 5600M MacBook Pro, from_device_id() atualizado com 30+ device IDs RDNA 1, suporte completo RX 5700 XT/5700/5600 XT + 5500 XT/5500 desktop, 5700M/5600M/5500M/5300M mobile + Pro W5700/W5500 workstation, RDNA 1 architecture) |
| 2026-01-18 | [GPU] AMD SMU (drivers/amd_smu.rs: NOVO ~800 linhas - smu_msg module com 40+ message IDs TEST_MESSAGE/GET_SMU_VERSION/SET_PPT_LIMIT/SET_TDC_LIMIT/SET_EDC_LIMIT/SET_THERMAL_LIMIT/ENABLE_OC/DISABLE_OC/SET_ALL_CORE_FREQ_OFFSET/SET_CURVE_OPTIMIZER/SET_POWER_PROFILE/SET_FAN_SPEED/SET_STAPM_LIMIT/SET_SLOW_PPT_LIMIT/SET_FAST_PPT_LIMIT, SmuResponse enum Ok/Failed/UnknownCommand/CommandRejected/InvalidArgument/CommandBusy, SmuVersion enum Smu9/10/11/13_0_0/13_0_4/13_0_7/13_0_8/14 para Zen 1-5, PowerProfile enum Balanced/Quiet/Performance/ExtremePerformance/PowerSaving/Custom, FanCurvePoint struct temp_c/fan_percent, PowerLimits struct ppt/tdc/edc/stapm/slow_ppt/fast_ppt/thermal limits, SmuTelemetry struct power/temp/freq/voltage/fan/utilization metrics, AmdSmu struct com smn_read/smn_write via SMN_INDEX/SMN_DATA PCI config space, send_message/init/set_ppt_limit/set_tdc_limit/set_edc_limit/set_thermal_limit/set_stapm_limit/enable_oc/disable_oc/set_freq_offset/set_curve_optimizer/set_power_profile/set_fan_speed/format_status, global AMD_SMU singleton, suporte Ryzen 1000-9000 series + Threadripper + APUs, Curve Optimizer support Zen 3+, STAPM support APUs) |
| 2026-01-18 | [GPU] AMD PowerPlay (drivers/amd_powerplay.rs: NOVO ~870 linhas - smu_msg module GPU-specific messages TEST_MESSAGE/GET_SMU_VERSION/SET_POWER_PROFILE/SET_FAN_CONTROL_MODE/SET_FAN_SPEED_PWM/SET_HARD_MIN_GFXCLK/SET_SOFT_MAX_GFXCLK/SET_POWER_LIMIT/ENABLE_GFX_OFF/ENTER_BACO/EXIT_BACO 35+ messages, DpmLevel enum Dpm0-Dpm7 performance levels, PowerProfile enum Bootup/ThreeDFullScreen/PowerSaving/Video/VR/Compute/Custom, FanControlMode enum None/Auto/Manual, ClockType enum Gfxclk/Socclk/Uclk/Fclk/Dclk/Vclk/Dcefclk/Dispclk/Pixclk/Phyclk, DpmState struct level/enabled/sclk_mhz/mclk_mhz/vddc_mv/vddci_mv/power_mw, FanTableEntry struct temp_c/pwm_percent, ThermalZone struct edge_temp/junction_temp/memory_temp/hotspot_temp, GpuPowerLimits struct tdp_w/max_tdp_w/min_tdp_w/default_tdp_w/gfx_power_w/soc_power_w, GpuGeneration enum Polaris/Vega/Navi1x/Navi2x/Navi3x, AmdPowerPlay struct com smc_read/smc_write/mmio_read/mmio_write, init_polaris_dpm/vega_dpm/navi1x_dpm/navi2x_dpm/navi3x_dpm DPM table defaults, init_fan_control/get_power_limits_from_smu/enable_features/send_smu_msg/set_power_profile/set_dpm_level/set_fan_mode/set_fan_speed_pwm/get_fan_speed/get_temperature/get_current_power/set_power_limit/set_gfx_clock_range/set_mem_clock_range/set_gfxoff/enter_baco/exit_baco/set_fan_curve/get_status APIs, global POWERPLAY singleton, suporte Polaris GCN4/Vega GCN5/Navi RDNA1-3 DPM+fan+thermal+power management) |
| 2026-01-18 | [GPU] NVIDIA Firmware (drivers/nvidia_firmware.rs: NOVO ~780 linhas - FirmwareType enum Disp/Pmu/GrCtxsw/GrFecs/GrGpccs/Sec2/Gsp/Nvdec/Nvenc/Ce, NvidiaGen enum Kepler/Maxwell/Pascal/Volta/Turing/Ampere/Ada/Hopper com code_name/chip_prefix/requires_gsp/from_device_id, FirmwareHeader struct magic/version/header_size/data_offset/code_offset/sig_offset packed, GspFirmwareHeader struct para Turing+ GSP bootloader/gsp_image/signatures, LoadedFirmware struct fw_type/version/code/data/signature/load_address/entry_point, PmuFirmwareInfo struct version/sizes/addresses, GspFirmwareInfo struct versions/sizes/offsets, falcon_regs module Falcon control/misc/cpu/IMEM/DMEM/DMA registers + NV_PMU/SEC2/GSP offsets, NvidiaFirmwareManager struct com init/load_firmware_set/load_pmu_firmware/load_gr_firmware/load_sec2_firmware/load_gsp_firmware/get_firmware_path/get_chip_id/mmio_read/mmio_write/upload_to_imem/upload_to_dmem/start_falcon/init_pmu/init_gsp/get_status APIs, firmware_paths module paths para GK104/GM204/GP102/TU102/GA102/AD102, global NVIDIA_FW singleton, suporte Kepler-Ada firmware loading PMU/GR/SEC2/GSP) |
| 2026-01-18 | [GPU] NVIDIA Optimus (drivers/nvidia_optimus.rs: NOVO ~760 linhas - GpuType enum IntelIntegrated/AmdIntegrated/NvidiaDiscrete/AmdDiscrete/Unknown com is_integrated/is_discrete/vendor, HybridType enum None/Muxless/Muxed/Dynamic, GpuPowerState enum Active/LowPower/Suspended/Off, SwitchingMode enum IntegratedOnly/DiscreteOnly/Automatic/RenderOffload/OnDemand, SelectionPolicy enum PowerSaving/Performance/Balanced/ProfileBased, GpuInfo struct gpu_type/pci_bdf/device_id/vendor_id/mmio_base/power_state/active/drives_display/name, dsm_guids module OPTIMUS_DSM/GPS_DSM/NOUVEAU_RPM_DSM 16-byte GUIDs, dsm_funcs module QUERY_FUNCTIONS/GPU_POWER_CTRL/MUX_CONTROL/GET_GPU_STATE/SET_DISPLAY_MODE, RuntimePmState enum Disabled/Active/AutoSuspend/Suspended, AppProfile struct name/preferred_gpu/force_discrete/env_vars, OptimusManager struct igpu/dgpu/hybrid_type/switching_mode/selection_policy/runtime_pm/auto_suspend_delay_ms/app_profiles com init/scan_gpus/detect_hybrid_config/detect_mux/init_acpi_dsm/init_runtime_pm/load_default_profiles/set_switching_mode/set_selection_policy/power_on_dgpu/power_off_dgpu/suspend_dgpu/resume_dgpu/select_gpu_for_app/add_profile/get_prime_env/get_status APIs, global OPTIMUS singleton, suporte Intel+NVIDIA e AMD+NVIDIA hybrid muxless/muxed, PRIME render offload, D3cold runtime PM) |
| 2026-01-18 | [Display] USB-C Display (drivers/usbc_display.rs: NOVO ~680 linhas - Orientation enum Normal/Flipped/Unknown, UsbcMode enum Usb20/Usb3SuperSpeed/Usb3PlusDp2Lane/Dp4Lane/Thunderbolt3/Thunderbolt4/Usb4, DpLaneCount enum None/TwoLanes/FourLanes, DpVersion enum Dp12/Dp13/Dp14/Dp20/Dp21 com bandwidth_per_lane_gbps/max_resolution_60hz, PdRole enum None/Sink/Source/DualRole, DpAltModeStatus enum NotConfigured/Configuring/Active/NoHpd/Error, tcpc_regs module TCPC I2C register addresses VENDOR_ID/PRODUCT_ID/ALERT/CC_STATUS/POWER_STATUS/RX_BUF/TX_BUF/VBUS, dp_vdm module DP_SID/DP_CMD_STATUS_UPDATE/CONFIGURE/ATTENTION/DP_CFG_* pin assignments, UsbcPort struct port_index/tcpc_addr/orientation/mode/pd_role/dp_status/dp_version/dp_lanes/hpd_state/device_info, ConnectedDevice struct device_type/vendor_id/product_id/supported_modes/max_dp_version/supports_dsc/supports_mst, DisplayRoute struct port_index/gpu_output/active/resolution/refresh_rate, UsbcDisplayManager struct ports/display_routes/thunderbolt_supported/usb4_supported com init/scan_tcpc_controllers/detect_thunderbolt/detect_usb4/init_port/enter_dp_alt_mode/exit_dp_alt_mode/handle_hpd/configure_route/get_status APIs, global USBC_DISPLAY singleton, suporte DP Alt Mode 2/4 lanes + Thunderbolt 3/4 + USB4 detection) |
| 2026-01-18 | [GPU] OpenGL 4.6 (drivers/opengl.rs: NOVO ~750 linhas - GlVersion enum OpenGL3_3/4_0/4_1/4_2/4_3/4_4/4_5/4_6/GlEs2_0/GlEs3_0/GlEs3_1/GlEs3_2 com major/minor version getters, GlError enum NoError/InvalidEnum/InvalidValue/InvalidOperation/StackOverflow/StackUnderflow/OutOfMemory/InvalidFramebufferOperation/ContextLost, GlContextFlags struct debug/forward_compatible/robust_access/no_error/reset_notification, GlCapabilities struct version/vendor/renderer/shading_language_version/max_texture_size/max_3d_texture_size/max_cube_map_texture_size/max_array_texture_layers/max_texture_image_units/max_combined_texture_units/max_vertex_attribs/max_uniform_block_size/max_uniform_buffer_bindings/max_varying_components/max_vertex_uniform_components/max_fragment_uniform_components/max_draw_buffers/max_color_attachments/max_samples/max_compute_work_group_count/invocations/size/max_ssbo_bindings/100+ extensions boolean flags, gl_extensions module 120+ extension strings GL_ARB_direct_state_access/GL_ARB_buffer_storage/GL_ARB_shader_storage_buffer_object/GL_ARB_compute_shader/GL_ARB_tessellation_shader/GL_ARB_transform_feedback/GL_ARB_multi_draw_indirect/GL_KHR_debug/GL_EXT_texture_filter_anisotropic etc, GlProfile enum Core/Compatibility/Es, GlContextState enum Uninitialized/Active/Suspended/Lost/Destroyed, GlBuffer struct id/size/usage/mapped/target, GlTexture struct id/target/width/height/depth/format/internal_format/levels, GlShader struct id/shader_type/source/compiled, GlProgram struct id/shaders/linked/uniforms, GlFramebuffer struct id/width/height/color_attachments/depth_attachment/stencil_attachment/complete, GlRenderbuffer struct id/width/height/format/samples, GlVertexArray struct id/attributes/element_buffer, GlContext struct id/state/version/profile/flags/capabilities/current_program/current_vao/bound_buffers/bound_textures/bound_framebuffer/error/debug_callback/viewport com create/make_current/swap_buffers/get_proc_address/check_extension/get_capability/create_buffer/delete_buffer/bind_buffer/buffer_data/buffer_sub_data/map_buffer/unmap_buffer/create_texture/delete_texture/bind_texture/tex_image_2d/tex_sub_image_2d/generate_mipmaps/create_shader/delete_shader/compile_shader/create_program/delete_program/attach_shader/link_program/use_program/get_uniform_location/uniform_1i/uniform_1f/uniform_3f/uniform_4f/uniform_matrix4fv/create_framebuffer/delete_framebuffer/bind_framebuffer/framebuffer_texture_2d/check_framebuffer_status/create_vertex_array/delete_vertex_array/bind_vertex_array/vertex_attrib_pointer/enable_vertex_attrib/draw_arrays/draw_elements/draw_arrays_instanced/draw_elements_instanced/multi_draw_indirect/dispatch_compute/memory_barrier/clear/viewport/enable/disable/get_error/get_status APIs, global GL_CONTEXT singleton, OpenGL 4.6 core profile full implementation) |
| 2026-01-18 | [GPU] VA-API (drivers/vaapi.rs: NOVO ~1375 linhas - VaStatus enum Success/ErrorOperationFailed/ErrorAllocationFailed/ErrorInvalidDisplay/etc 27 error codes, VaProfile enum None/Mpeg2Simple/Mpeg2Main/Mpeg4*/H264Baseline/Main/High/ConstrainedBaseline/MultiviewHigh/StereoHigh/High10/High422/High444/Vc1*/JpegBaseline/Vp8/Vp9Profile0-3/HevcMain/Main10/Main12/Main422_10/Main422_12/Main444/Main444_10/Main444_12/SccMain/SccMain10/SccMain444/Av1Profile0-1/Protected 37 profiles, VaEntrypoint enum Vld/Idct/MoComp/Deblocking/EncSlice/EncPicture/EncSliceLp/VideoProc/Fei/Stats/ProtectedTeeComm/ProtectedContent decode/encode/processing entrypoints, VaRtFormat enum Yuv420/Yuv422/Yuv444/Yuv411/Yuv400/Yuv420_10/Yuv422_10/Yuv444_10/Yuv420_12/Yuv422_12/Yuv444_12/Rgb16/Rgb32/RgbP/Rgb32_10/Protected render target formats, VaBufferType enum PicParam/IqMatrix/BitPlane/SliceGroupMap/SliceParam/SliceData/MacroblockParam/ResidualData/DeblockingParam/Image/ProtectedSliceData/QMatrix/HuffmanTable/Probability + encode/vpp/fei/stats buffer types, VaConfigAttribType 50+ config attributes RtFormat/SpatialResidu/Encryption/RateControl/DecSliceMode/EncPackedHeaders/EncInterlaced/MaxPictureWidth/Height/EncQualityRange etc, fourcc module NV12/NV21/YV12/IYUV/I420/YUY2/UYVY/Y800/P010/P012/P016/Y210/Y212/Y216/Y410/Y412/Y416/RGBX/BGRX/ARGB/ABGR/RGBA/BGRA/A2R10G10B10/A2B10G10R10 pixel formats, VaVendor enum Intel/Amd/Nvidia/Unknown, VaConfig/VaSurface/VaContext/VaBuffer/VaImage resource structs, ProfileCapability struct profile/entrypoints/max_width/max_height/rt_formats, VaRcMode enum None/Cbr/Vbr/Vcm/Cqp/Vbr_Constrained/Icq/Mb/Cfs/Parallel/Qvbr/Avbr rate control modes, VppCapabilities struct deinterlacing/noise_reduction/sharpening/color_balance/skin_tone_enhancement/proc_amp/scaling/blending/color_standard_conversion/rotation/mirroring/hdr_tone_mapping/high_dynamic_range/three_dlut, VaDisplay struct com init/init_intel/init_amd/init_nvidia vendor-specific initialization, query_profiles/query_entrypoints/get_config_attribs/create_config/destroy_config/create_surfaces/destroy_surfaces/create_context/destroy_context/create_buffer/map_buffer/unmap_buffer/destroy_buffer/begin_picture/render_picture/end_picture/sync_surface/query_surface_status/create_image/destroy_image/get_image/put_image APIs, Intel Quick Sync Video/AMD VCN/NVIDIA NVDEC+NVENC support, H.264/HEVC/VP9/AV1/JPEG/MPEG-2/VC-1 decode+encode profiles, 8K resolution support) |
| 2026-01-18 | [GPU] VDPAU (drivers/vdpau.rs: NOVO ~1050 linhas - VdpStatus enum Ok/NoImplementation/DisplayPreempted/InvalidHandle/InvalidPointer/InvalidChromaType/InvalidYCbCrFormat/InvalidRgbaFormat/InvalidIndexedFormat/InvalidColorStandard/InvalidColorTableFormat/InvalidBlendFactor/InvalidBlendEquation/InvalidFlag/InvalidDecoderProfile/InvalidVideoMixerFeature/InvalidVideoMixerParameter/InvalidVideoMixerAttribute/InvalidVideoMixerPictureStructure/InvalidFuncId/InvalidSize/InvalidValue/InvalidStruct/ResourcesBusy/Resources/InvalidHandle2/InvalidDecoderTarget/Error 27 status codes, VdpChromaType enum Type420/Type422/Type444/Type420_16/Type422_16/Type444_16 chroma types, VdpYCbCrFormat enum Nv12/Yv12/Nv12_16/P010/P016/Y8u8v8a8/V8u8y8a8 formats, VdpRgbaFormat enum B8g8r8a8/R8g8b8a8/R10g10b10a2/B10g10r10a2/A8 formats, VdpColorStandard enum Itur_bt_601/Itur_bt_709/Smpte_240m/Itur_bt_2020 color standards, VdpDecoderProfile enum Mpeg1/Mpeg2Simple/Main/Mpeg4PartSimple/Main/AdvancedSimple/H264Baseline/Main/High/ConstrainedBaseline/Extended/ProgressiveHigh/ConstrainedHigh/High444Predictive/Vc1Simple/Main/Advanced/Divx4-5*/HevcMain/Main10/Main12/MainStill/Main444/Main444_10/Main444_12/Vp9Profile0-3/Av1Main/High/Professional 38 profiles, VdpVideoMixerFeature enum DeinterlaceTemporal/TemporalSpatial/InverseTeveticine/NoiseReduction/Sharpness/Luma/HighQualityScaling, VdpVideoMixerParameter/Attribute enums, VdpVideoMixerPictureStructure enum TopField/BottomField/Frame, VdpOutputSurfaceRenderBlendFactor/Equation/Rotate enums, DecoderCapability struct profile/is_supported/max_level/max_macroblocks/max_width/max_height, VdpVideoSurface/OutputSurface/BitmapSurface/Decoder/VideoMixer/PresentationQueueTarget/PresentationQueue resource structs, VdpVendor enum Nvidia/Mesa/Unknown, VdpDevice struct com init/init_nvidia/init_mesa vendor initialization, get_decoder_capabilities/video_surface_create/destroy/get_parameters/output_surface_create/destroy/bitmap_surface_create/destroy/decoder_create/destroy/render/video_mixer_create/set_attribute/destroy/render/presentation_queue_target_create/destroy/presentation_queue_create/destroy/set_background_color/display/block_until_surface_idle APIs, NVIDIA native VDPAU + Mesa VA-API backend support, H.264/HEVC/VP9/AV1/MPEG-1/2/4/VC-1/DivX decode profiles) |
| 2026-01-18 | [Display] VRR/FreeSync (drivers/vrr.rs: NOVO ~680 linhas - VrrTechnology enum None/AmdFreeSync/AmdFreeSyncPremium/AmdFreeSyncPremiumPro/NvidiaGSync/NvidiaGSyncCompatible/NvidiaGSyncUltimate/VesaAdaptiveSync/HdmiVrr com name/supports_hdr/supports_lfc, VrrState enum Disabled/Enabled/Active/Inactive/Error, ConnectorType enum Unknown/DisplayPort/Hdmi/Dvi/Vga/Edp com supports_vrr, VrrRange struct min_hz/max_hz com contains/span/lfc_effective, EdidVrrInfo struct supported/technology/range/version, MonitorInfo struct id/name/connector/native_refresh/current_refresh/vrr_info, GpuVrrCapabilities struct supported/technologies/min_refresh/max_refresh, VrrTiming struct vfp_base/vfp_extend/vsync/vbp/vactive, LfcSettings struct enabled/multiplier/threshold_hz Low Framerate Compensation, ConnectorVrrState struct connector_id/enabled/state/technology/range/current_refresh/target_refresh/lfc/timing, dp_dpcd module DOWNSPREAD_CTRL/EDP_CONFIGURATION_SET/ADAPTIVE_SYNC_CAPS/CTRL/STATUS/VRR_MIN_MAX_REFRESH DPCD registers, hdmi_vrr module VRR_MIN/MAX_INDEX/AMD_VSDB_OUI/QFT_QMS_VRR_SUPPORT, freesync module FREESYNC_V1/V2/PREMIUM/PREMIUM_PRO/FS_ACTIVE/LFC_ACTIVE/HDR_ACTIVE, gsync module MODULE_V1/V2/COMPATIBLE/ULTIMATE/GSYNC_CAP/CTRL, VrrController struct com init/init_intel/init_amd/init_nvidia GPU-specific initialization, parse_edid_vrr/parse_cea_vrr/parse_displayid_vrr EDID parsing, register_connector/set_monitor_vrr/enable/disable/enable_freesync/enable_gsync/enable_adaptive_sync/set_target_refresh/get_state/set_global_enabled/get_status APIs, global VRR_CONTROLLER singleton, suporte AMD FreeSync/FreeSync Premium/Premium Pro + NVIDIA G-SYNC/G-SYNC Compatible/Ultimate + VESA Adaptive-Sync + HDMI 2.1 VRR, LFC Low Framerate Compensation) |
| 2026-01-18 | [Display] HDR (drivers/hdr.rs: NOVO ~620 linhas - HdrStandard enum Sdr/Hdr10/Hdr10Plus/DolbyVision/Hlg/PqHdr com name/transfer_function, TransferFunction enum Linear/Srgb/Bt1886/Pq/Hlg/Gamma22/Gamma24 com is_hdr, ColorPrimaries enum Bt709/Bt2020/DciP3/DisplayP3/AdobeRgb/Bt601 com is_wide_gamut/primaries_xy/white_point, HdrStaticMetadata struct primaries/white_point/max_luminance/min_luminance/max_content_light/max_frame_average_light SMPTE ST.2086, HdrDynamicMetadata struct scene_max_luminance/scene_avg_luminance/bezier_curve_anchors/knee_point HDR10+/Dolby Vision, ToneMapper enum None/Reinhard/ReinhardMod/Aces/AcesApprox/Uncharted2/Hable/AgX/Bt2390 com name/apply tone mapping operators, DisplayHdrCapabilities struct hdr_supported/standards/eotfs/color_primaries/max_luminance/min_luminance/max_full_frame_luminance/color_depth, ConnectorHdrState struct connector_id/enabled/current_standard/static_metadata/dynamic_metadata/tone_mapper/display_caps/sdr_boost/paper_white, edid_hdr module HDR_STATIC_METADATA_BLOCK/EOTF_TRADITIONAL_SDR/HDR/SMPTE_ST2084/HLG/SM_TYPE1 EDID parsing constants, infoframe module HDR_DRM_TYPE/VERSION/EOTF_SDR_LUMINANCE/HDR_LUMINANCE/SMPTE_ST2084/HLG/SM_TYPE1 InfoFrame constants, HdrController struct com init/parse_edid_hdr/parse_cea_hdr/register_connector/set_display_caps/enable/disable/set_static_metadata/set_dynamic_metadata/set_tone_mapper/set_sdr_boost/set_paper_white/send_hdr_infoframe/get_state/get_status APIs, global HDR_CONTROLLER singleton, suporte HDR10/HDR10+/Dolby Vision/HLG standards, SMPTE ST.2086 static metadata, BT.2020 wide color gamut, 10/12-bit color depth, tone mapping) |
| 2026-01-18 | [Display] HiDPI Scaling (drivers/hidpi.rs: NOVO ~580 linhas - ScaleFactor enum Scale100/125/150/175/200/225/250/300/350/400/Custom com as_f32/as_percent/from_percent/is_hidpi/is_fractional, ScalingMethod enum None/Integer/Fractional/XRender/Wayland/Viewport, ScalingFilter enum Nearest/Bilinear/Bicubic/Lanczos/Spline, DpiMode enum Auto/Manual/Xft/Gnome/Kde, DisplayPhysical struct width_mm/height_mm/diagonal_inch com new/is_valid, DisplayResolution struct width/height com pixels/is_4k/is_5k/is_8k, calculate_dpi/recommend_scale helper functions, MonitorScaling struct connector_id/name/resolution/physical/dpi_x/dpi_y/scale/method/filter/effective_resolution com new/set_scale, HiDpiSettings struct global_scale/auto_detect/prefer_integer/filter/dpi_mode/force_dpi/text_scale/cursor_scale, presets module macbook_retina_13/16/imac_5k/dell_4k_27/lg_ultrafine_5k/standard_1080p_24/standard_1440p_27 display configs, HiDpiController struct monitors/settings com init/register_monitor/parse_edid_physical/parse_dtd_physical/set_scale/set_global_scale/set_filter/set_text_scale/set_cursor_scale/get_effective_dpi/get_effective_scale/logical_to_physical/physical_to_logical/get_monitor/get_status APIs, global HIDPI_CONTROLLER singleton, suporte 4K/5K/8K/Retina displays, integer/fractional scaling 100-400%, DPI auto-detection from EDID, per-monitor scaling, text/cursor independent scaling) |
| 2026-01-18 | [Rede] Intel WiFi 7 (drivers/net/iwlwifi_be.rs: NOVO ~740 linhas - device_ids module BE200_1/BE200_2/BE202/BE201 WiFi 7 + AX411/AX211/AX210 WiFi 6E fallback com is_wifi7()/name(), WifiStandard enum Wifi4/5/6/6E/7, FrequencyBand enum Band2_4GHz/5GHz/6GHz com frequency_range()/max_channel_width(), ChannelWidth enum Width20/40/80/160/320MHz, MloState enum Disabled/SingleLink/DualLink/TriLink, MloLink struct link_id/band/channel/width/active/rssi para Multi-Link Operation, PowerState enum Active/LowPower/Sleep/DeepSleep/Off, SecurityMode enum Open/Wep/WpaPsk/Wpa2Psk/Wpa3Sae/Wpa3Enterprise/Owe, ScanResult struct ssid/bssid/channel/band/rssi/security/standard/supports_mlo/mlo_links, ConnectionState enum Disconnected/Scanning/Authenticating/Associating/Handshake/Connected/Roaming/Disconnecting, DriverStats struct tx/rx packets/bytes/errors/retries/beacons/signal/noise/link_quality, FirmwareInfo struct version/api_version/build_number/size/loaded, regs module CSR_HW_IF_CONFIG_REG/INT/INT_MASK/RESET/GP_CNTRL/HW_REV/EEPROM/LED/etc 30+ registers, IntelWifi7Driver struct device_id/mmio_base/irq/wifi_standard/supported_bands/max_width/mlo_capable/max_streams/power_state/connection_state/mlo_state/mlo_links/current_ssid/bssid/channel/band/security/firmware/stats/scan_results com init/hw_init/load_firmware/read_reg/write_reg/scan/connect/disconnect/enable_mlo/set_power_mode/get_status, init() PCI scan for Intel WiFi 7 devices, PCI helper functions pci_config_addr/read32/read_vendor/read_device/read_bar0/read_irq, global INTEL_WIFI7 singleton, suporte 802.11be WiFi 7 BE200/BE202 + fallback WiFi 6E, 320MHz channel width, MLO Multi-Link Operation dual/tri-link, WPA3-SAE security, 2.4/5/6 GHz tri-band) |
| 2026-01-18 | [Rede] Realtek WiFi (drivers/net/rtl8xxxu_wifi.rs: NOVO ~700 linhas - device_ids module RTL8821AE/AU/CE/CU, RTL8822BE/BU/CE/CU, RTL8852AE/AU/BE/BU/CE/CU WiFi 6/6E, RTL8723AE/BE/DE/DU combo, RTL8188EE/EU/CE/CU budget com is_pcie()/is_usb()/is_wifi6()/name()/chip_gen(), ChipGeneration enum Unknown/Gen1-5, WifiStandard enum Wifi4/5/6/6E, FrequencyBand enum Band2_4GHz/5GHz/6GHz, ChannelWidth enum Width20/40/80/160MHz, SecurityMode enum Open/Wep/WpaPsk/Wpa2Psk/Wpa3Sae, ConnectionState enum Disconnected/Scanning/Authenticating/Associating/Connected/Disconnecting, PowerState enum Active/LowPower/Sleep/Off, ScanResult struct ssid/bssid/channel/band/rssi/security, DriverStats struct tx/rx packets/bytes/errors/beacons/signal/noise, FirmwareInfo struct name/version/size/loaded, regs module REG_SYS_CFG/FUNC_EN/APS_FSMCO/SYS_CLKR/TXDMA/RXDMA/MAC_CTRL/TCR/RCR/HIMR/HISR 40+ registers, BusType enum Pcie/Usb/Sdio, RealtekWifiDriver struct device_id/chip_gen/bus_type/mmio_base/wifi_standard/supported_bands/max_width/max_streams/has_bluetooth/power_state/connection_state/current_ssid/bssid/channel/band/security/firmware/stats/scan_results com init/setup_capabilities/hw_init/load_firmware/read_reg/write_reg/scan/connect/disconnect/set_power_mode/get_status, init() PCI scan, global REALTEK_WIFI singleton, suporte RTL8821/8822/8852 series WiFi 5/6/6E PCIe/USB, combo BT RTL8723 series) |
| 2026-01-18 | [Rede] MediaTek WiFi (drivers/net/mt7921.rs: NOVO ~630 linhas - device_ids module MT7921E/K/S/AU WiFi 6 PCIe/USB/SDIO, MT7922/MT792X_E WiFi 6E, AMD_RZ608/RZ616 rebrands, MT7925E/U WiFi 7 com is_wifi6e()/is_wifi7()/is_pcie()/is_usb()/is_sdio()/name()/variant(), ChipVariant enum Unknown/Mt7921/Mt7922/Mt7925, WifiStandard enum Wifi5/6/6E/7, FrequencyBand enum Band2_4GHz/5GHz/6GHz, ChannelWidth enum Width20/40/80/160/320MHz, SecurityMode/ConnectionState/PowerState/BusType enums, ScanResult/DriverStats/FirmwareInfo structs, regs module MT_TOP_CFG_BASE/LPCR_HOST_BAND0/MISC/MCU_CMD/WFDMA0/1_BASE/WPDMA_GLO_CFG/CONN_INFRA/INT_STATUS/MASK/TX_RX_RING/EFUSE 30+ registers, MediatekWifiDriver struct device_id/variant/bus_type/mmio_base/wifi_standard/supported_bands/max_width/max_streams/supports_bluetooth/power_state/connection_state com init/setup_capabilities/hw_init/load_firmware/scan/connect/disconnect/set_power_mode/get_status, init() PCI scan, global MEDIATEK_WIFI singleton, suporte MT7921/7922/7925 WiFi 6/6E/7 + AMD RZ608/RZ616) |
| 2026-01-18 | [Rede] Broadcom WiFi (drivers/net/brcmfmac.rs: NOVO ~660 linhas - device_ids module BCM4350/54/56/58/59 WiFi 5, BCM4364/77 Apple, BCM43xx legacy WiFi 4, BCM43455/56 RPi, BCM4366/C0/4375/78/87 WiFi 6, BCM4389 WiFi 6E Apple M1/M2 com is_wifi6()/is_wifi6e()/is_pcie()/is_sdio()/name()/chip_id(), ChipId enum Unknown/Gen4/Gen5/Gen5Apple/Gen6/Gen6E, WifiStandard enum Wifi4/5/6/6E, FrequencyBand/ChannelWidth/SecurityMode/ConnectionState/PowerState/BusType enums, ScanResult/DriverStats/FirmwareInfo structs com clm_name para CLM blobs, regs module SBSDIO_FUNC1_SBADDR*/CORE_SB_PMU/BUS/RESET_CTL/WIFI_CORE_BASE/INTMASK/STATUS/BRCMF_PCIE_BAR0*/MB_INT_D2H/H2D_DB/NVRAM 20+ registers, BroadcomWifiDriver struct device_id/chip_id/bus_type/mmio_base/wifi_standard/supported_bands/max_width/max_streams/supports_bluetooth com init/setup_capabilities/hw_init/load_firmware/scan/connect/disconnect/set_power_mode/get_status, init() PCI scan, global BROADCOM_WIFI singleton, suporte BCM43xx legacy/WiFi 5/6/6E Apple+consumer+RPi) |
| 2026-01-18 | [Rede] Atheros WiFi (drivers/net/ath11k.rs: NOVO ~650 linhas - device_ids module QCA6390 WiFi 6, WCN6855/QCN9074 WiFi 6E, WCN7850 WiFi 7, IPQ8074/6018/5018 router chips com is_wifi6e()/is_wifi7()/is_pcie()/is_ahb()/name()/family(), ChipFamily enum Unknown/Qca6390/Wcn6855/Wcn7850/Qcn9074/Ipq8074, WifiStandard enum Wifi5/6/6E/7, FrequencyBand/ChannelWidth/SecurityMode/ConnectionState/PowerState/BusType enums, ScanResult/DriverStats/FirmwareInfo structs com board_name, regs module HAL_REG_CAPABILITIES/INTR_*/MAC_REG_*/PHY_REG_*/QMI_WLANFW_*/MHI_CTRL_* 20+ registers, Ath11kDriver struct device_id/family/bus_type/mmio_base/wifi_standard/supported_bands/max_width/max_streams/supports_bluetooth com init/setup_capabilities/hw_init/load_firmware/scan/connect/disconnect/set_power_mode/get_status, init() PCI scan, global ATH11K_WIFI singleton, suporte QCA6390/WCN6855/7850/QCN9074/IPQ8074 WiFi 6/6E/7 consumer+enterprise+router) |
| 2026-01-18 | [Compat] Flatpak Support (compat/flatpak.rs: NOVO ~900 linhas - AppId/Branch/Arch/Remote structs para identificação, InstallationType System/User, Ref/RefKind para referências app/runtime, InstalledApp/InstalledRuntime/AppMetadata structs, Permissions struct com share_network/share_ipc/sockets/devices/filesystem/dbus_access/environment/persistent_dirs, SocketPermission X11/Wayland/PulseAudio/System/Session/etc, DevicePermission Dri/Kvm/Shm/All, FilesystemPermission/FilesystemAccess ro/rw/create, DbusPermission/DbusAccess Talk/Own/See, SandboxConfig struct com user_ns/pid_ns/net_ns/ipc_ns/mount_ns/uts_ns/seccomp/rootfs/bind_mounts/env/cwd Bubblewrap-style sandbox, SeccompConfig default_flatpak com allowed_syscalls whitelist, PortalManager com FileChooserPortal/OpenUriPortal/NotificationPortal/ScreenshotPortal/CameraPortal XDG Desktop Portals, Portal trait handle_method(), PortalArg/PortalResponse/PortalError types, OsTreeRepo struct com pull/checkout/commit para OSTree integration, FlatpakRuntime struct apps/runtimes/remotes/instances/portals, RunningInstance struct instance_id/app_ref/pid/sandbox, init()/add_remote/remove_remote/list_remotes/install/uninstall/list_installed_apps/list_installed_runtimes/run/stop/list_instances/handle_portal_request/update/search/get_app_info/parse_metadata APIs, global FLATPAK_STATE singleton, Flathub default remote) |
| 2026-01-18 | [Compat] AppImage Support (compat/appimage.rs: NOVO ~650 linhas - AppImageType enum Type1/Type2/Unknown ISO9660 vs SquashFS, AppImageArch enum X86_64/I686/AArch64/ArmHf com from_elf_machine(), elf module MAGIC/CLASS64/LITTLE_ENDIAN constants, magic module APPIMAGE_TYPE1/TYPE2/SQUASHFS/ISO9660 magic bytes, ElfHeader64 packed struct, AppImageInfo struct path/appimage_type/arch/fs_offset/file_size/name/version/desktop_entry/update_info/signature/is_mounted/mount_point, DesktopEntry struct name/exec/icon/categories/comment/generic_name/terminal/no_display/mime_types/actions, UpdateInfo struct update_type/url/channel, UpdateType enum GitHubReleases/Zsync/Bsdiff/Oci, SignatureInfo struct signature_type/key_id/signature_data/verified, SignatureType enum Gpg/Ed25519/Sha256/None, SquashfsSuperblock packed struct, Compression enum Gzip/Lzma/Lzo/Xz/Lz4/Zstd, AppImageRuntime struct appimages/instances/desktop_integration/cache_dir/trusted_keys/fuse_enabled, RunningAppImage struct instance_id/info/pid/start_time/extracted/extraction_path, TrustedKey struct key_id/key_type/public_key, detect_type()/parse_appimage()/find_squashfs_offset()/register()/list_registered()/get_info()/mount()/unmount()/run()/stop()/list_running()/extract()/integrate_desktop()/remove_desktop_integration()/check_update()/update()/verify_signature()/parse_desktop_file()/parse_update_info() APIs, global APPIMAGE_STATE singleton) |
| 2026-01-18 | [Áudio] Low Latency Audio (drivers/audio/lowlatency.rs: NOVO ~700 linhas - BufferSize enum Samples32/64/128/256/512/1024/2048 com latency_us()/latency_ms() calculation, SampleRate enum Rate44100/48000/88200/96000/176400/192000, SampleFormat enum S16/S24/S32/F32, RtPriority enum Low/Normal/High/Max/Custom com as_priority() 51-99, XrunType enum Underrun/Overrun, XrunEvent struct xrun_type/client_id/port_id/timestamp/delayed_usecs, PortDirection enum Input/Output, PortFlags IS_PHYSICAL/CAN_MONITOR/IS_TERMINAL/IS_CONTROL bitflags, ClientState enum Inactive/Active/Suspended/Closing, TransportState enum Stopped/Rolling/Starting/Stopping, TransportPosition struct frame/usecs/sample_rate/bar/beat/tick/bpm/time_sig_num/denom para MIDI sync, AudioRingBuffer struct lock-free ring buffer com available()/space()/write()/read()/overflow_count()/underflow_count(), LowLatencyPort struct id/client_id/name/direction/flags/buffer/connected_to/latency_frames, LowLatencyClient struct id/name/state/rt_priority/ports/process_callback/xrun_count/cpu_load, ProcessContext struct client_id/nframes/sample_rate/frame_time/input_buffers/output_buffers com get_input()/get_output(), EngineConfig struct sample_rate/buffer_size/periods/rt_priority/soft_mode/freewheel/sync_mode, SyncMode enum Internal/WordClock/Adat/Spdif, EngineStats struct total_frames/xrun_count/max_process_time_us/avg_process_time_us/cpu_load_percent, LowLatencyAudio engine struct, init()/init_with_config()/get_config()/set_config()/start()/stop()/is_running()/create_client()/destroy_client()/activate_client()/deactivate_client()/register_port()/unregister_port()/connect_ports()/disconnect_ports()/list_ports()/list_clients()/get_stats()/get_transport_state()/set_transport_state()/get_transport_position()/set_transport_position()/report_xrun()/get_xrun_events()/clear_xrun_events()/set_freewheel()/is_freewheel() JACK-compatible APIs, global LOWLATENCY_STATE singleton) |
| 2026-01-18 | [Apps] Archive Manager (gui/apps/archive.rs: NOVO ~980 linhas - ArchiveFormat enum Zip/Tar/TarGz/TarBz2/TarXz/TarZst/Gzip/Bzip2/Xz/Zstd/SevenZip/Rar/Unknown com from_extension()/from_path()/extension()/mime_type()/supports_password(), CompressionLevel enum None/Fastest/Fast/Normal/Maximum/Ultra com as_level(), EntryType enum Directory/File/Symlink/Hardlink/Unknown com PartialOrd/Ord, ArchiveEntry struct path/entry_type/size/compressed_size/mtime/mode/crc32/encrypted/compression/comment/link_target com new()/compression_ratio()/is_directory()/file_name()/parent_path(), ArchiveInfo struct path/format/total_size/compressed_size/entry_count/file_count/dir_count/encrypted/comment/entries, ArchiveProgress struct operation/current_entry/current_entry_index/total_entries/bytes_processed/total_bytes/percent, ArchiveOperation enum Opening/Listing/Extracting/Creating/Adding/Deleting/Testing, ArchiveError enum FileNotFound/InvalidFormat/CorruptArchive/PasswordRequired/WrongPassword/UnsupportedFormat/IoError/OutOfMemory/PermissionDenied/PathTooLong, ViewMode enum FlatList/TreeView, SortField enum Name/Size/CompressedSize/ModTime/Type/Ratio, SortDirection enum Ascending/Descending, ExtractOptions struct destination/overwrite/preserve_structure/permissions/timestamps/selected_only/password, CreateOptions struct format/compression_level/password/solid/store_symlinks/comment, ArchiveManager Widget struct com open()/close()/extract()/create()/test()/get_visible_entries()/select_entry()/select_all()/clear_selection()/navigate_to()/navigate_up()/set_view_mode()/set_sort()/set_search_filter()/format_size()/entry_at_point(), Widget trait impl id()/bounds()/set_position()/set_size()/is_enabled()/set_enabled()/is_visible()/set_visible()/handle_event()/render(), draw_char()/draw_string() helpers, init(), suporte ZIP/TAR/GZIP/BZIP2/XZ/ZSTD/7Z/RAR, GUI list view, selection, sorting, filtering, progress bar) |
| 2026-01-18 | [Apps] Disk Utility (gui/apps/diskutil.rs: NOVO ~850 linhas - DiskType enum Hdd/Ssd/Nvme/Usb/Cdrom/Floppy/Virtual/Unknown com name()/icon(), PartitionTable enum Mbr/Gpt/None/Unknown com name(), FilesystemType enum Ext2/Ext3/Ext4/Btrfs/Xfs/Zfs/Fat12/Fat16/Fat32/ExFat/Ntfs/Hfs/HfsPlus/Apfs/Iso9660/Udf/Swap/Raw/Unknown com name()/supports_permissions()/supports_journaling()/max_file_size(), HealthStatus enum Good/Warning/Critical/Unknown com name()/color() para SMART, SmartAttribute struct id/name/current/worst/threshold/raw_value/status, DiskInfo struct id/name/disk_type/model/serial/firmware/capacity/sector_size/rotation_rate/partition_table/health/temperature/smart_attributes/partitions/is_removable/is_read_only, PartitionInfo struct id/number/name/label/filesystem/start_sector/end_sector/size/used/available/mount_point/uuid/flags/is_bootable/is_mounted com usage_percent(), DiskError enum DiskNotFound/PartitionNotFound/PermissionDenied/DiskBusy/InvalidPartitionTable/FilesystemError/IoError/UnsupportedOperation/InsufficientSpace, FormatOptions struct filesystem/label/quick_format/enable_journaling/block_size, PartitionCreateOptions struct size/filesystem/label/bootable/alignment, ViewMode enum DiskList/PartitionMap/SmartInfo/Operations, SelectedItem enum None/Disk/Partition, DiskUtility Widget struct com refresh()/get_selected_disk()/get_selected_partition()/mount()/unmount()/format()/create_partition()/delete_partition()/resize_partition()/create_partition_table()/erase_disk()/item_at_point()/format_size(), Widget trait impl, sidebar disk/partition list, detail view, usage bar, SMART health display, progress bar) |
| 2026-01-18 | [Apps] Recent Files (gui/apps/recentfiles.rs: NOVO ~990 linhas - FileCategory enum All/Documents/Images/Videos/Audio/Archives/Code/Other com name()/icon()/from_extension() para categorização automática, TimeGroup enum Today/Yesterday/ThisWeek/ThisMonth/Older com name()/from_timestamp() para agrupamento temporal, RecentFile struct path/name/extension/size/last_accessed/category/opened_with/exists/thumbnail/access_count com new()/format_size()/format_time()/time_group() para exibição, SortOrder enum RecentFirst/OldestFirst/NameAZ/NameZA/SizeSmallest/SizeLargest/MostAccessed, RecentFilesManager struct files/max_files com add_file()/remove_file()/clear()/files()/files_by_category()/files_by_time_group()/validate_files()/prune_missing()/stats() para gerenciamento, RecentFilesStats struct total_files/total_size/total_accesses/documents/images/videos/audio/archives/code/other, RecentFilesWidget struct Widget com manager/filter_category/sort_order/search_query/selected_index/scroll_offset/group_by_time/category_buttons/filtered_indices/hovered_index/show_details, add_sample_files()/update_filtered_list()/update_category_buttons()/get_visible_item_count()/set_filter_category()/add_recent_file()/remove_recent_file()/clear_recent_files()/get_selected_file()/set_sort_order()/set_search_query()/stats() APIs, Widget trait impl com handle_event scroll/keydown/click, render com category filter buttons/file list header/alternating rows/selection/hover/scrollbar/details panel/empty state, draw_char()/draw_string() helpers) |
| 2026-01-18 | [Apps] Thumbnails (gui/apps/thumbnails.rs: NOVO ~940 linhas - ThumbnailSize enum Normal(128)/Large(256)/XLarge(512)/XXLarge(1024) freedesktop.org spec com pixels()/directory_name(), ThumbnailableType enum Image/Video/Pdf/Document/Font/Archive/Unknown com from_extension()/can_generate_thumbnail()/priority(), ThumbnailStatus enum Pending/Generating/Ready/Failed/Unsupported/TooLarge/Stale, ThumbnailMetadata struct uri/mtime/file_size/width/height/mime_type/uri_hash/thumbnail_mtime/software com new()/thumbnail_filename()/is_valid() seguindo spec XDG, CachedThumbnail struct metadata/size/status/pixels(RGBA)/actual_width/actual_height/last_accessed com new()/is_ready(), ThumbnailRequest struct path/uri/size/mtime/file_size/file_type/priority/request_id com new(), ThumbnailResult struct request_id/path/success/error/thumbnail, ThumbnailConfig struct cache_dir/max_cache_size/max_file_size/worker_count/thumbnail_hidden/default_size/enable_video_thumbnails/video_thumbnail_position, ThumbnailStats struct cached_count/cache_size/pending_requests/generated_count/failed_count/cache_hits/cache_misses/by_type/by_size com hit_rate(), ThumbnailCache struct cache/cache_size/config/stats/pending/failed_uris/current_time com get()/request()/process_pending()/generate_thumbnail()/generate_image_thumbnail()/generate_video_thumbnail()/generate_pdf_thumbnail()/generate_document_thumbnail()/generate_font_thumbnail()/generate_archive_thumbnail() placeholder geradores/evict_if_needed() LRU/clear_cache()/clear_failures()/stats()/config()/set_config()/is_cached()/pending_count()/cancel_pending()/invalidate(), simple_md5_hex() para URI hash, global THUMBNAIL_CACHE singleton com init()/init_with_config()/get_thumbnail()/request_thumbnail()/process_thumbnails()/get_stats()/clear_cache()/invalidate_thumbnail()/is_thumbnail_cached() APIs) |
| 2026-01-18 | [Apps] Music Player (gui/apps/musicplayer.rs: NOVO ~1100 linhas - AudioFormat enum Mp3/Flac/Wav/Ogg/Aac/M4a/Wma/Opus/Unknown com from_extension()/name()/is_lossless(), PlayerState enum Stopped/Playing/Paused/Loading/Error, RepeatMode enum Off/All/One com next()/icon(), ShuffleMode enum Off/On, TrackMetadata struct title/artist/album/album_artist/track_number/total_tracks/disc_number/year/genre/duration/bitrate/sample_rate/channels/album_art/lyrics/comment/composer com new()/format_duration(), Track struct id/path/format/file_size/metadata/last_played/play_count/rating/is_favorite com new()/display_title()/display_artist(), Playlist struct id/name/tracks/created/modified/is_smart, EqualizerPreset struct name/bands[10]/preamp com flat()/rock()/pop()/jazz()/classical()/bass_boost() presets, ViewMode enum NowPlaying/Library/Playlists/Queue/Equalizer, LibraryView enum Songs/Albums/Artists/Genres, MusicPlayer Widget struct state/current_track/position/volume/muted/repeat_mode/shuffle/library/playlists/queue/queue_index/view_mode/library_view/equalizer/presets/spectrum[32]/selected_index/scroll_offset com add_sample_library()/play()/pause()/toggle_play_pause()/stop()/next()/previous()/play_from_queue()/seek()/set_volume()/toggle_mute()/toggle_repeat()/toggle_shuffle()/add_to_queue()/clear_queue()/get_track()/set_equalizer()/update_spectrum()/get_visible_items()/format_time(), Widget trait impl, render_now_playing() com album art/track info/spectrum visualizer, render_library() com sortable columns, render_queue()/render_playlists()/render_equalizer() 10-band EQ, render_controls() progress bar/play-pause-next-prev/repeat-shuffle/volume, Spotify-style dark theme, playlist management) |
| 2026-01-18 | [Apps] Screen Recorder (gui/apps/screenrecorder.rs: NOVO ~950 linhas - RecordingState enum Idle/Preparing/Recording/Paused/Stopping/Saving/Error, RegionType enum FullScreen/Window/CustomRegion/FollowCursor, VideoFormat enum Mp4/Webm/Mkv/Gif/Raw com extension()/mime_type()/supports_audio(), QualityPreset enum Low/Medium/High/VeryHigh/Lossless com bitrate_kbps()/name(), FrameRate enum Fps15/Fps24/Fps30/Fps60/Fps120/Custom, AudioSource enum None/System/Microphone/Both, RecordingSettings struct format/quality/framerate/region/include_cursor/include_audio/audio_source/output_path/countdown_seconds/max_duration/hotkey_start/hotkey_stop/hotkey_pause, SelectionRect struct x/y/width/height com contains()/intersects(), RecordingStats struct duration_seconds/frame_count/dropped_frames/file_size/bitrate_kbps/cpu_usage/memory_mb, ScreenRecorder Widget struct state/settings/stats/selected_region/preview_frame/countdown/error_message/show_settings/hovered_button/recording_start_time/last_frame_time/paused_duration com new()/start_recording()/stop_recording()/pause_recording()/resume_recording()/cancel_recording()/capture_frame()/encode_frame()/finalize_recording()/update_stats()/select_region()/set_format()/set_quality()/set_framerate()/set_audio_source()/format_duration()/format_size(), Widget trait impl, render_idle_state()/render_recording_state()/render_paused_state()/render_settings_panel()/render_region_selection()/render_preview()/render_stats(), draw_char()/draw_string() helpers, init()) |
| 2026-01-18 | [Apps] Email Client (gui/apps/email.rs: NOVO ~1200 linhas - EmailProtocol enum Imap/Pop3/Exchange/Gmail/Outlook com name()/default_port(), SmtpSettings struct server/port/use_tls/use_starttls/auth_method, AuthMethod enum Plain/Login/CramMd5/OAuth2/XOAuth2, EmailAccount struct id/name/email/display_name/protocol/incoming_server/port/ssl/username/smtp/signature/is_default/sync_interval/last_sync, MailboxType enum Inbox/Sent/Drafts/Trash/Spam/Archive/Starred/Important/Custom com name()/icon(), Mailbox struct id/account_id/name/mailbox_type/path/unread_count/total_count/parent_id/children/is_subscribed/is_selectable, EmailAddress struct address/display_name com format(), MessageFlags struct seen/answered/flagged/deleted/draft/recent, Attachment struct id/filename/mime_type/size/content_id/is_inline/data com format_size(), MessagePriority enum Highest-Lowest, EmailMessage struct id/uid/account_id/mailbox_id/message_id/in_reply_to/references/from/to/cc/bcc/reply_to/subject/date/body_text/body_html/attachments/flags/priority/size/thread_id/labels com is_unread()/has_attachments()/from_display()/to_display()/format_date()/preview(), DraftMessage struct com new()/reply_to()/forward(), SearchFilter struct query/from/to/subject/has_attachment/is_unread/is_flagged/date_from/date_to com matches(), SortOrder enum DateDesc/Asc/FromAsc/Desc/SubjectAsc/Desc/SizeAsc/Desc, ViewMode enum MessageList/MessageView/Compose/Settings/AccountSetup, ConnectionState enum Disconnected/Connecting/Connected/Syncing/Error, EmailError enum ConnectionFailed/AuthenticationFailed/NetworkError/ServerError/InvalidMessage/MailboxNotFound/MessageNotFound/SendFailed/AttachmentTooLarge/StorageError, EmailClient Widget struct accounts/mailboxes/messages/view_mode/connection_state/search_query/sort_order/scroll_offset/sidebar_width/draft/compose_field com add_account()/remove_account()/select_mailbox()/select_message()/open_message()/compose_new()/reply()/forward()/send()/save_draft()/delete_selected()/toggle_flag()/toggle_read()/set_search(), Widget trait impl, render_message_list()/render_message_view()/render_compose() views, three-pane layout sidebar/list/preview, init()) |
| 2026-01-18 | [Apps] Calendar (gui/apps/calendar.rs: NOVO ~1050 linhas - Weekday enum Sunday-Saturday com name()/short_name()/from_number()/as_number(), Month enum January-December com name()/short_name()/from_number()/as_number()/days(), is_leap_year() helper, Date struct year/month/day com new()/today()/month_enum()/weekday() Zeller's formula/days_in_month()/first_day_of_month()/format()/format_display()/add_days()/add_months(), Time struct hour/minute com format()/format_12h()/total_minutes(), DateTime struct date/time, RecurrenceRule enum None/Daily/Weekly/Biweekly/Monthly/Yearly/Custom, ReminderTime enum AtTime/Minutes5-30/Hour1-2/Day1-2/Week1 com minutes(), EventColor enum Blue/Green/Red/Yellow/Purple/Orange/Cyan/Pink/Gray com to_color(), CalendarEvent struct id/calendar_id/title/description/location/start/end/all_day/color/recurrence/reminder/attendees/created/modified/is_busy com duration_minutes()/format_time_range()/is_on_date(), Calendar struct id/name/color/is_visible/is_default/is_local/account_email, CalendarView enum Day/Week/Month/Year/Agenda, WeekStart enum Sunday/Monday/Saturday, CalendarSettings struct week_start/show_week_numbers/time_format_24h/default_view/default_event_duration/working_hours, CalendarWidget Widget struct calendars/events/current_date/selected_date/view/settings/sidebar_width/selected_event_id/hovered_date com add_calendar()/remove_calendar()/add_event()/remove_event()/events_for_date()/go_to_today()/previous()/next()/set_view()/select_date()/get_view_title()/get_week_start(), Widget trait impl, render_mini_calendar()/render_month_view()/render_day_view()/render_week_view()/render_agenda_view() views, sidebar com mini calendar e calendars list, init()) |
| 2026-01-18 | [Apps] Contacts (gui/apps/contacts.rs: NOVO ~1330 linhas - PhoneType enum Mobile/Home/Work/Main/HomeFax/WorkFax/Pager/Other com name()/icon(), PhoneNumber struct number/phone_type/is_primary/label, EmailType enum Personal/Work/School/Other com name(), Email struct address/email_type/is_primary/label, AddressType enum Home/Work/School/Other com name(), Address struct street/city/state/postal_code/country/address_type/is_primary/label com format()/format_short(), SocialProfile struct network/username/url com icon(), ImportantDate struct date/date_type/label com format_date(), Contact struct id/first_name/last_name/nickname/company/job_title/phones/emails/addresses/birthday/anniversary/notes/website/social_profiles/important_dates/photo/is_favorite/groups/created/modified com full_name()/display_name()/initials()/search_text()/to_vcard() VCard 3.0 export, ContactGroup struct id/name/color/is_smart/smart_filter com member_count(), SortOrder enum FirstNameAsc/Desc/LastNameAsc/Desc/CompanyAsc/Desc/RecentFirst com name(), ViewMode enum List/Detail/Edit/Create, FilterType enum All/Favorites/Group/Recent, ContactsManager Widget struct contacts/groups/next_contact_id/next_group_id/view_mode/selected_contact_id/filter/search_query/sort_order/sidebar_width/scroll_offset/hovered_index/editing_contact/edit_field com add_sample_data()/add_contact()/remove_contact()/update_contact()/get_contact()/get_contacts()/filtered_contacts()/add_group()/remove_group()/get_groups()/set_filter()/set_sort_order()/set_search_query()/select_contact()/get_selected_contact()/create_contact()/edit_contact()/save_editing()/cancel_editing()/delete_selected()/toggle_favorite()/export_vcard()/contact_at_point()/group_at_point()/get_visible_count(), Widget trait impl handle_event MouseDown/MouseMove/Scroll/KeyDown, render com sidebar groups/favorites/all contacts filter, contact list com alphabet sections/selection/hover, detail view com full contact info/action buttons, edit view com form fields, dark theme, init()) |
| 2026-01-18 | [Apps] Notes (gui/apps/notes.rs: NOVO ~1500 linhas - TextStyle enum Normal/Bold/Italic/BoldItalic/Strikethrough/Code com toggle_bold()/toggle_italic(), ListType enum None/Bullet/Numbered/Checkbox/CheckboxChecked com prefix()/cycle(), HeadingLevel enum None/H1/H2/H3 com font_size()/prefix()/cycle(), TextBlock struct content/style/list_type/heading/indent_level com to_markdown()/to_plain_text(), NoteAttachment struct id/filename/mime_type/size/data com format_size(), NoteColor enum None-Gray 9 colors com to_color()/name()/all(), Note struct id/notebook_id/title/blocks/tags/color/is_pinned/is_locked/is_archived/is_trashed/attachments/created/modified/word_count/character_count com plain_text()/markdown()/html() export formats/preview()/update_stats()/add_tag()/remove_tag()/has_tag()/search_text()/format_date(), Notebook struct id/name/color/icon/is_default/is_locked/parent_id/note_count, Tag struct name/color/usage_count, SortOrder enum ModifiedDesc/Asc/CreatedDesc/Asc/TitleAsc/Desc, ViewMode enum NoteList/NoteView/NoteEdit/NotebookList/TagList/Search/Settings, FilterType enum All/Notebook/Tag/Pinned/Archived/Trash/Recent, ExportFormat enum PlainText/Markdown/Html com extension()/name(), NotesApp Widget struct notes/notebooks/tags/next_ids/view_mode/filter/sort_order/search_query/selected_note_id/selected_notebook_id/sidebar_width/scroll_offset/hovered_index/editing_block/cursor_position/current_style/current_list/show_formatting_bar com add_sample_data()/create_note()/delete_note()/restore_note()/archive_note()/pin_note()/move_note()/get_note()/get_note_mut()/create_notebook()/delete_notebook()/add_tag()/remove_tag()/filtered_notes()/set_filter()/set_sort_order()/set_search_query()/export_note()/note_at_point()/sidebar_item_at_point()/get_visible_count()/select_note()/edit_selected_note(), Widget trait impl handle_event, render com three-pane layout sidebar/note list/content, rich text rendering, checkbox lists, dark theme, init()) |
| 2026-01-18 | [Apps] Webcam (gui/apps/webcam.rs: NOVO ~1300 linhas - DeviceCapabilities struct name/resolutions/formats/has_autofocus/has_zoom/has_pan_tilt/has_flash, Resolution struct width/height/fps com format()/aspect_ratio()/megapixels()/vga()/hd720()/hd1080()/uhd4k() presets, gcd() helper, PixelFormat enum Yuyv/Mjpeg/Rgb24/Rgb32/Nv12/I420 com name()/bytes_per_pixel(), VideoDevice struct id/name/path/capabilities/is_connected/is_open, CaptureMode enum Photo/Video/Timelapse/Burst com name()/icon(), PhotoQuality enum Low-Maximum com jpeg_quality(), VideoQuality enum Low/Medium/High/UltraHd com resolution()/bitrate_kbps(), VideoCodec enum H264/H265/Vp8/Vp9/Av1/Raw com extension(), CameraState enum Idle/Previewing/Capturing/Recording/Processing/Error com can_capture(), TimerSetting enum Off/Seconds3/5/10, FlashMode enum Off/On/Auto/RedEyeReduction, WhiteBalance enum Auto/Daylight/Cloudy/Incandescent/Fluorescent/Flash/Custom, CameraSettings struct resolution/photo_quality/video_quality/video_codec/timer/mirror_preview/sound_enabled/grid_enabled/date_stamp/location_stamp/auto_brightness/auto_focus/flash_mode/white_balance/exposure/zoom/burst_count/timelapse_interval/output_directory Default trait impl, MediaItem struct id/path/filename/is_video/timestamp/size/duration_ms/resolution/thumbnail com format_size()/format_duration(), RecordingStats struct duration_seconds/frames_recorded/frames_dropped/bytes_written/current_fps/avg_fps/bitrate_kbps, WebcamApp Widget struct devices/selected_device_id/state/capture_mode/settings/recording_stats/error_message/gallery/next_media_id/selected_media_id/show_settings/show_gallery/timer_countdown/preview_frame/hovered_button com detect_devices()/select_device()/start_preview()/stop_preview()/capture_photo()/start_recording()/stop_recording()/toggle_recording()/set_capture_mode()/set_resolution()/set_timer()/toggle_mirror()/toggle_grid()/toggle_settings()/toggle_gallery()/select_media()/delete_selected_media(), Widget trait impl handle_event keyboard shortcuts Space/Esc/G/S, render com toolbar/preview area/grid overlay/recording indicator/bottom controls/mode buttons/capture button/settings panel/gallery panel, dark theme, init()) |
| 2026-01-18 | [Apps] Network Shares (gui/apps/networkshares.rs: NOVO ~1200 linhas - ShareProtocol enum Smb/Smb2/Smb3/Nfs3/Nfs4/Afp/WebDav/Ftp/Sftp com name()/default_port()/uri_scheme(), AuthMethod enum Anonymous/UserPassword/Kerberos/NtlmV2/PublicKey, ShareCredentials struct auth_method/username/password/domain/key_path/save_password com with_user_password(), ConnectionState enum Disconnected/Connecting/Connected/Authenticating/Error/Timeout com is_connected(), NetworkServer struct id/hostname/ip_address/port/protocol/workgroup/shares/connection_state/last_seen/is_favorite com display_name()/uri(), ShareType enum Disk/Printer/Device/Ipc/Admin com icon(), SharePermissions struct can_read/can_write/can_execute/can_delete, NetworkShare struct name/path/share_type/is_hidden/comment/permissions/size_total/size_free com format_size(), RemoteFile struct name/path/is_directory/size/modified/created/permissions/is_hidden/is_symlink com format_size()/icon(), MountPoint struct id/server_id/share_name/local_path/is_mounted/auto_mount/credentials, SavedConnection struct id/name/uri/protocol/hostname/port/share_path/credentials/auto_connect/last_used, ViewMode enum Browse/Servers/Saved/MountPoints, ShareError enum ConnectionFailed/AuthenticationFailed/PermissionDenied/ShareNotFound/NetworkError/Timeout/MountFailed/ProtocolError com message(), NetworkSharesBrowser Widget struct servers/saved_connections/mount_points/next_ids/current_server_id/current_share/current_path/files/view_mode/selected_index/scroll_offset/hovered_index/sidebar_width/show_hidden/show_connect_dialog/connect_uri/error_message/is_loading com add_sample_data()/discover_servers()/connect_to_server()/disconnect_from_server()/toggle_favorite()/open_share()/navigate_to()/go_up()/load_directory()/mount_share()/unmount()/save_connection()/delete_saved_connection()/get_visible_count()/item_at_point(), Widget trait impl handle_event Esc/Enter/H keys, render com sidebar navigation/favorites/content list/status bar, SMB/NFS browser interface, dark theme, init()) |
| 2026-01-18 | [Apps] Printer Settings (gui/apps/printersettings.rs: NOVO ~1150 linhas - ConnectionType enum Usb/Network/Bluetooth/Serial/Parallel/Virtual com icon(), PrinterType enum Laser/Inkjet/DotMatrix/Thermal/Label/ThreeD/Virtual, PrinterState enum Idle/Printing/Paused/Error/Offline/OutOfPaper/OutOfInk/Jammed/Warming com is_ready()/color(), PaperSize enum Letter/Legal/A4/A3/A5/B5/Tabloid/Envelope/Photo4x6/Photo5x7/Custom com dimensions_mm(), PaperType enum Plain/Photo/Glossy/Matte/Cardstock/Labels/Transparent/Recycled, PrintQuality enum Draft/Normal/High/Best com dpi(), ColorMode enum Color/Grayscale/BlackWhite, DuplexMode enum None/LongEdge/ShortEdge, PaperTray struct id/name/paper_size/paper_type/capacity/level com percentage(), CartridgeColor enum Black/Cyan/Magenta/Yellow/Photo/LightCyan/LightMagenta, InkCartridge struct id/name/color/level/is_low/is_empty com status()/display_color(), JobStatus enum Pending/Processing/Printing/Paused/Completed/Cancelled/Error com is_active(), PrintJob struct id/printer_id/document_name/owner/status/pages_total/pages_printed/copies/submitted/started/completed/size_bytes/priority/error_message com progress()/format_size(), PrinterCapabilities struct supports_color/supports_duplex/max_dpi/paper_sizes/paper_types/max_paper_width_mm/max_paper_height_mm/pages_per_minute/supports_borderless/supports_stapling/supports_hole_punch Default trait impl, PrintSettings struct paper_size/paper_type/quality/color_mode/duplex/copies/collate/reverse_order/borderless/scale_to_fit/scale_percentage/orientation_landscape/pages_per_sheet/selected_tray Default trait impl, Printer struct id/name/description/location/manufacturer/model/driver_name/connection_type/printer_type/state/is_default/is_shared/capabilities/settings/trays/cartridges/uri/serial_number/pages_printed_total com display_name()/status_summary(), ViewMode enum Printers/PrintQueue/Settings/AddPrinter, PrinterSettingsApp Widget struct printers/print_queue/next_ids/view_mode/selected_printer_id/selected_job_id/scroll_offset/hovered_index/sidebar_width/show_supplies/show_advanced com add_sample_data()/add_printer()/remove_printer()/set_default_printer()/get_printer()/get_printer_mut()/cancel_job()/pause_job()/resume_job()/clear_completed_jobs()/pause_printer()/resume_printer()/get_visible_count()/jobs_for_printer(), Widget trait impl, render com sidebar printer list/status indicators/content area views/supplies level bars/paper tray info/print queue table, CUPS-style interface, dark theme, init()) |
| 2026-01-18 | [Apps] Terminal Tabs (gui/apps/terminaltabs.rs: NOVO ~1100 linhas - TabId struct com atomic counter, TabState enum Active/Background/HasActivity/Closing, SessionState struct cwd/env/history/scrollback_saved, TerminalProfile struct name/foreground/background/cursor_color/selection_color/colors[16]/font_size/opacity com dark()/light()/solarized_dark()/monokai() preset themes, SplitDirection enum Horizontal/Vertical, TerminalPane struct para split views, TerminalTab struct id/name/custom_name/state/terminal/session/profile/has_activity/pid/icon/pinned com display_name()/set_name()/clear_custom_name()/update_from_terminal(), TabViewMode enum Single/Grid/Split, DragState struct para tab reordering, TerminalTabs Widget struct tabs/active_tab/view_mode/profiles/tab_bar_height/tab_width/max_visible_tabs/tab_scroll_offset/hovered_tab/hovered_close/drag_state/renaming_tab/rename_buffer/show_new_tab/shortcuts_enabled/closed_tabs/max_closed_tabs com new_tab()/close_tab()/close_active_tab()/reopen_closed_tab()/switch_to_tab()/next_tab()/prev_tab()/move_tab()/toggle_pin()/start_rename()/finish_rename()/cancel_rename()/ensure_tab_visible()/tab_at_position()/is_over_close_button()/is_over_new_tab_button()/handle_shortcut() Ctrl+T/W/Tab/Shift+T/1-9 + Alt+Left/Right/write()/write_to_tab(), Widget trait impl, render_tab_bar() com tab backgrounds/active indicator/activity indicator/pin indicator/close buttons/new tab button/scroll indicators, render_rename_input() inline rename, drag and drop indicator, dark theme Dracula-style, init()) |
| 2026-01-18 | [Apps] Syntax Highlighting (gui/apps/syntax.rs: NOVO ~1750 linhas - Language enum 32 languages PlainText/Rust/Python/JavaScript/TypeScript/C/Cpp/CSharp/Java/Go/Html/Css/Scss/Json/Yaml/Toml/Xml/Markdown/Shell/Sql/Php/Ruby/Swift/Kotlin/Lua/Zig/Haskell/Ocaml/Elixir/Makefile/Dockerfile/Gitignore com from_extension()/name()/line_comment()/block_comment(), TokenType enum 35 types Normal/Keyword/ControlFlow/Type/Builtin/Function/Method/Macro/String/Char/Number/Boolean/Null/Comment/BlockComment/DocComment/Operator/Delimiter/Punctuation/Attribute/Namespace/Variable/Parameter/Property/Constant/Label/Escape/FormatPlaceholder/Regex/TagName/TagAttribute/Error/Special, Token struct start/end/token_type, ColorScheme struct 40+ colors name/background/foreground/cursor/selection/line_highlight/line_numbers + token colors com color_for()/monokai()/dracula()/one_dark()/solarized_dark()/nord()/github_light()/gruvbox_dark()/vscode_dark() 8 preset themes, HighlighterState enum Normal/MultiLineString/BlockComment/DocComment/RawString para multi-line tokens, HighlightedLine struct tokens/end_state, SyntaxHighlighter struct language/scheme com highlight_line()/highlight_rust()/highlight_python()/highlight_javascript()/highlight_c_like()/highlight_go()/highlight_html()/highlight_css()/highlight_json()/highlight_shell()/highlight_markdown()/highlight_generic(), is_rust_operator/is_python_operator/is_js_operator/is_c_operator helpers, suporte completo Rust/Python/JS/TS/C/C++/C#/Java/Go/HTML/CSS/JSON/Shell/Markdown com keywords/types/strings/comments/numbers/operators highlighting, color schemes Monokai/Dracula/One Dark/Solarized/Nord/GitHub/Gruvbox/VS Code) |
| 2026-01-18 | [Apps] Git Integration (gui/apps/git.rs: NOVO ~1200 linhas - ObjectType enum Blob/Tree/Commit/Tag com name(), FileStatus enum Unmodified/Modified/Staged/StagedModified/Added/Deleted/Renamed/Copied/Untracked/Ignored/Conflicted com char()/color()/name(), FileChange struct path/old_path/status/staged/lines_added/lines_removed com display_name(), Commit struct hash/short_hash/author_name/author_email/timestamp/message/summary/parent_hashes/is_merge com format_date(), Branch struct name/is_remote/is_current/upstream/ahead/behind/last_commit com display_name()/has_upstream()/sync_status(), Tag struct name/commit_hash/message/is_annotated/tagger/timestamp, Remote struct name/fetch_url/push_url/default_branch/is_origin, DiffLine struct content/line_type/old_line_no/new_line_no, DiffHunk struct old_start/old_count/new_start/new_count/header/lines, FileDiff struct path/old_path/hunks/is_binary/additions/deletions, StashEntry struct index/message/branch/timestamp, MergeConflict struct path/ours/theirs/base/resolution, RepoState enum Clean/Merging/Rebasing/CherryPicking/Reverting/Bisecting, Repository struct path/state/head/head_commit/branches/tags/remotes/stash_entries/conflicts/is_bare/is_shallow/work_dir com local_branches()/remote_branches()/current_branch()/add_sample_data(), GitError enum/GitResult type alias, GitViewMode enum Status/Commits/Branches, GitPanel Widget struct repository/changes/commits/selected_file/selected_commit/selected_branch/view_mode/bounds/visible/focused/hovered_item/scroll_offset/show_staged_only/show_diff com draw_text()/render_header()/render_status()/render_branches()/render_commits(), Widget trait impl handle_event Up/Down/Tab/Enter/S/Space keys, status tracking staged/unstaged changes, commit history display, branch management local/remote, dark theme Dracula-style, sample data demo) |
| 2026-01-18 | [Browser] Bookmarks (gui/apps/browser/bookmarks.rs: NOVO ~850 linhas - BookmarkId/FolderId/TagId structs IDs únicos, BookmarkType enum Bookmark/Separator/FolderRef, Bookmark struct id/title/url/folder_id/favicon_url/description/keywords/tags/created/modified/last_visited/visit_count/position com domain()/display_title()/is_separator()/record_visit(), BookmarkFolder struct id/name/parent_id/position/created/modified/is_expanded/is_toolbar/icon com toolbar() factory, BookmarkTag struct id/name/color/bookmark_count, SpecialFolder enum Root/Toolbar/Other/Mobile/Recent/Frequent, SortOrder enum Manual/TitleAsc/TitleDesc/UrlAsc/UrlDesc/DateAddedDesc/DateAddedAsc/LastVisitedDesc/VisitCountDesc, BookmarkFormat enum Html/Json/ChromeJson/FirefoxJson com extension()/name(), BookmarkSearchResult struct bookmark/folder_path/match_score/matched_in_*, BookmarkError enum NotFound/DuplicateUrl/InvalidFolder/CircularReference/Import-ExportFailed/SyncFailed, BookmarkManager struct bookmarks/folders/tags BTreeMaps + special folder IDs com create_special_folders()/add_bookmark()/add_to_toolbar()/add_to_folder()/create_folder()/create_tag()/add_tag_to_bookmark()/remove_bookmark()/remove_folder()/move_bookmark()/move_folder()/is_descendant_of()/search()/get_folder_path()/find_by_url()/is_bookmarked()/get_recent()/get_frequent()/get_by_tag()/export_html() Netscape format, toolbar/favicons/confirm_delete settings, sample data demo) |
| 2026-01-18 | [Browser] History (gui/apps/browser/history.rs: NOVO ~750 linhas - HistoryEntryId struct, VisitType enum Link/Typed/Bookmark/AutoComplete/Embed/Redirect/Download/FormSubmit/Reload com is_user_initiated(), Visit struct timestamp/visit_type/referrer_url/transition_type/duration_ms, TransitionType enum Normal/NewTab/ForwardBack/AddressBar/External/Internal, HistoryEntry struct id/url/title/favicon_url/first_visit/last_visit/visit_count/typed_count/visits/is_hidden com domain()/display_title()/record_visit()/frecency_score() Firefox-style, TimeRange enum LastHour/Today/Yesterday/LastWeek/LastMonth/Last3Months/LastYear/AllTime/Custom com seconds(), HistorySearchResult struct, HistoryStats struct total_entries/visits/typed/unique_domains/oldest/newest/most_visited_domain/count, HistoryByDate/HistoryByDomain structs para agrupamento, HistorySortOrder enum DateDesc/DateAsc/VisitCountDesc/Frecency/TitleAsc/TitleDesc, HistoryManager struct entries/url_to_id BTreeMaps + settings com record_visit()/update_title()/delete_entry()/delete_by_url()/delete_range()/delete_domain()/clear_all()/search()/get_recent()/get_most_visited()/get_by_frecency()/get_for_range()/group_by_date()/group_by_domain()/get_stats()/enforce_limits()/get_suggestions() autocomplete, privacy settings enable_history/remember_search/form_data/clear_on_exit, sample data demo) |
| 2026-01-18 | [Browser] Password Manager (gui/apps/browser/passwords.rs: NOVO ~900 linhas - CredentialId/FolderId structs IDs únicos, CredentialType enum Login/CreditCard/Address/SecureNote/Custom com icon(), PasswordStrength enum VeryWeak-VeryStrong com calculate() scoring, LoginCredential struct id/site_name/url/username/password/totp_secret/notes/timestamps/use_count/favorite/folder_id com domain()/display_name()/password_strength()/record_use(), CreditCardCredential struct com masked_number()/card_type()/is_expired(), CardType enum Visa/Mastercard/Amex/Discover/DinersClub/Jcb/UnionPay com detect(), AddressInfo struct com format_address(), SecureNote struct, PasswordFolder struct, BreachInfo/BreachSeverity structs, PasswordGeneratorOptions struct length/use_upper/lower/digits/symbols/avoid_ambiguous/min_*, PasswordSortOrder enum, VaultStatus enum Locked/Unlocked/NoMasterPassword, PasswordManager struct logins/cards/addresses/notes/folders BTreeMaps + security state com set_master_password()/verify/unlock()/lock()/is_unlocked()/check_auto_lock()/add_login()/update_login()/delete_login()/find_logins_for_url()/search()/get_favorites()/check_password_health()/generate_password(), PasswordHealthReport struct weak/reused/old/breached passwords + score, PasswordStats struct, sample data demo com weak/reused password examples) |
| 2026-01-18 | [Browser] WebRTC (gui/apps/browser/webrtc.rs: NOVO ~1020 linhas - PeerConnectionId/MediaStreamId/DataChannelId/TrackId structs IDs únicos, IceConnectionState/IceGatheringState/SignalingState/PeerConnectionState enums para estados de conexão, MediaKind Audio/Video, TrackState Live/Ended, IceCandidateType Host/Srflx/Prflx/Relay, IceProtocol Udp/Tcp, IceCandidate struct com to_sdp(), IceServer STUN/TURN config, SdpType Offer/Answer/Pranswer/Rollback, SessionDescription struct com create_offer()/create_answer() SDP generation, MediaStreamTrack struct id/kind/label/state/muted/enabled/constraints com is_audio()/is_video()/stop(), MediaTrackConstraints struct áudio echo_cancellation/noise_suppression/auto_gain_control + vídeo width/height/frame_rate/facing_mode, FacingMode User/Environment, MediaStream struct com add_track()/remove_track()/get_audio_tracks()/get_video_tracks()/stop(), DataChannelState Connecting/Open/Closing/Closed, DataChannel struct com is_open()/close(), RtcConfiguration IceServers/IceTransportPolicy/BundlePolicy/RtcpMuxPolicy, IceTransportPolicy All/Relay, BundlePolicy Balanced/MaxCompat/MaxBundle, RtcpMuxPolicy Negotiate/Require, RtcCertificate struct, PeerConnection struct com create_offer()/create_answer()/set_local_description()/set_remote_description()/add_ice_candidate()/add_track()/create_data_channel()/close()/simulate_connection(), WebRtcError enum InvalidState/InvalidSdp/IceConnectionFailed/PermissionDenied/DeviceNotFound/DataChannelError/NetworkError, RtcStats struct bytes_sent/received/packets/jitter/round_trip_time, WebRtcManager struct com create_peer_connection()/get_user_media()/get_display_media()/create_data_channel()/grant_camera/microphone_permission()/active_connections()) |
| 2026-01-18 | [Net] Firewall Module (net/firewall.rs: NOVO ~720 linhas - RuleId/ZoneId structs IDs únicos, Protocol enum Any/Tcp/Udp/Icmp/Icmpv6/Gre/Esp/Ah, IpAddress enum Any/Ipv4/Ipv6/Ipv4Cidr/Ipv6Cidr, Port enum Any/Single/Range/List, Action enum Accept/Drop/Reject/Log/LogAccept/LogDrop/Mark/Redirect/Masquerade/Snat/Dnat, Direction Inbound/Outbound/Forward, ConnState New/Established/Related/Invalid, Rule struct com id/name/description/enabled/direction/protocol/src_addr/dst_addr/src_port/dst_port/action/log/conn_states/interface/zone/priority/hit_count, Zone struct com id/name/interfaces/default_action/allow_icmp/masquerade/forward + public()/home()/work()/trusted() presets, Service struct + builtin_services() SSH/HTTP/HTTPS/DNS/DHCP/NTP/SMTP/IMAP/FTP/SMB/RDP/VNC/WireGuard/OpenVPN, ConnTrackEntry struct, FirewallStats struct, FirewallManager struct com add_rule/remove_rule/set_rule_enabled/add_zone/allow_service/conntrack_entries/cleanup_conntrack) + [Apps] Firewall GUI (gui/apps/firewall.rs: NOVO ~600 linhas - FirewallView enum Rules/Zones/Services/Connections/Logs/Settings, RuleFilter enum All/Inbound/Outbound/Forward/Enabled/Disabled, LogEntry struct, QuickAction enum, FirewallApp Widget struct com view/filter/selection/dialogs/search, filtered_rules()/toggle_enabled()/delete_selected_rule/start_add_rule/confirm_add_rule/allow_service/navigate/format_ip/format_port/format_connection/action_color/render_demo) |
| 2026-01-18 | [Net] OpenVPN (net/openvpn.rs: NOVO ~900 linhas - ProtocolVersion V2/V3, Transport Udp/Tcp com default_port(), Cipher enum Aes128Cbc/Aes256Cbc/Aes128Gcm/Aes256Gcm/ChaCha20Poly1305/None com key_size()/iv_size()/is_aead(), Auth enum Sha1/Sha256/Sha384/Sha512/None com digest_size(), TlsAuthMode enum TlsAuth/TlsCrypt/TlsCryptV2/None, Compression enum Lzo/Lz4/Lz4V2/Stub/None, ConnectionState enum 11 states Disconnected/Connecting/WaitingForServer/Authenticating/GettingConfig/AssigningAddress/AddingRoutes/Connected/Reconnecting/Disconnecting/Failed, Opcode enum 11 tipos ControlHardResetClient/Server V1-V3/ControlV1/AckV1/DataV1-V2/etc com from_u8()/is_control()/is_data(), PacketHeader struct, ServerConfig struct ifconfig_local/remote/netmask/routes/dns_servers/domain/redirect_gateway/mtu, Route struct, ClientConfig struct 25+ campos remote/port/transport/cipher/auth/compression/tls_auth_mode/ca_cert/client_cert/username/password/keepalive/mtu/etc com from_ovpn() parser .ovpn, ConnectionStats struct com compression_ratio_out/in(), OpenVpnConnection struct config/state/session_id/key_id/packet_ids/encrypt_key/decrypt_key/hmac_keys/tun_ip com connect()/disconnect()/process_packet()/create_packet()/send()/needs_keepalive()/create_keepalive()/is_timed_out()/simulate_connection(), VpnProfile struct, OpenVpnManager struct com add_profile()/import_ovpn()/connect()/disconnect()/profiles()) |
| 2026-01-18 | [Net] IPsec/IKEv2 (net/ipsec.rs: NOVO ~1000 linhas - IkeVersion V1/V2, EncryptionAlgorithm enum Aes128/192/256/Aes128Gcm16/Aes256Gcm16/ChaCha20Poly1305/Des3/Null com key_size()/is_aead(), IntegrityAlgorithm enum HmacSha1-96/256-128/384-192/512-256/Aes128-256Gmac/None com digest_size(), DhGroup enum Modp768-8192/Ecp256-521/Curve25519-448 com group_number()/key_size(), PrfAlgorithm enum, AuthMethod enum Psk/RsaSig/EcdsaSig256-512/Eap, IpsecProtocol Esp/Ah com protocol_number(), IpsecMode Transport/Tunnel, IkeSaState enum 10 states Idle/InitSent/InitReceived/AuthSent/AuthReceived/Established/RekeyInit/RekeySent/Deleting/Deleted, ChildSaState enum, ExchangeType enum IkeSaInit/IkeAuth/CreateChildSa/Informational, PayloadType enum 16 tipos, IkeHeader struct 28 bytes, TrafficSelector struct com ipv4_any()/ipv4_subnet(), IkeProposal struct com default_aead()/default_cbc(), ChildProposal struct, SecurityPolicy struct, IkeSa struct com SPIs/state/proposal/keys sk_d/ai/ar/ei/er/pi/pr/nonces/lifetime com needs_rekey()/is_expired(), ChildSa struct com SPIs/keys/seq_nums/anti_replay, ConnectionConfig struct com add_connection()/with_psk(), IpsecConnection struct com connect()/disconnect()/process_packet()/create_esp_packet()/simulate_connection(), IpsecManager struct com add_connection/remove_connection/connect/disconnect/configs/get_state) |
| 2026-01-18 | [Power] Battery Health (drivers/battery.rs: ADICIONADO ~350 linhas - HealthStatus enum Excellent/Good/Fair/Poor/Critical/Unknown com from_percentage(), BatteryHealth struct com status/health_percentage/design_capacity/full_charge_capacity/cycle_count/estimated_cycles_remaining/manufacture_date/first_use_date/total_energy_consumed/avg_discharge_rate/max_temperature/current_temperature/deep_discharge_count/overcharge_count/last_calibration/needs_calibration com update()/wear_level()/estimated_lifespan_months()/should_replace(), HealthHistoryEntry struct, BatteryHealthTracker struct com get_health()/update_health()/get_history()/degradation_rate()/add_sample_data(), global HEALTH_TRACKER, APIs init_health_tracking()/get_battery_health()/health_status()/wear_level()/should_replace_battery()/estimated_lifespan_months()/degradation_rate()/update_health_tracking()) |
| 2026-01-18 | [Power] Charge Limit (drivers/battery.rs: ADICIONADO ~490 linhas - ChargeLimitMode enum Full/Conservation/MaxLongevity/Custom com default_thresholds(), ChargeThreshold struct start/stop, ChargeLimitSettings struct enabled/mode/custom_limit/scheduled/schedule_start/schedule_end/bypass_until/bypass_requested/express_charge_requested com is_in_schedule()/is_bypass_active()/clear_bypass()/set_bypass(), ChargeLimitController enum ThinkPad/Asus/Dell/Hp/Lenovo/Samsung/Apple/Generic/EcBased com supports_custom_limit()/supports_scheduling()/supports_bypass(), ChargingDecision enum Allow/StopCharging/Discharging/Unknown, ChargeLimitManager struct batteries BTreeMap/controller/current_time/initialized com detect_controller()/init_battery()/get_settings()/get_settings_mut()/enable()/disable()/set_limit()/process()/apply_to_hardware(), global CHARGE_LIMIT_MANAGER, APIs init_charge_limit()/charge_limit_supported()/enable_charge_limit()/disable_charge_limit()/set_charge_limit()/get_charge_limit_settings()/process_charge_limit()/set_charge_limit_bypass()/clear_charge_limit_bypass()) |
| 2026-01-18 | [WiFi] WPA3-Enterprise (net/wifi/wpa3.rs: ADICIONADO ~1420 linhas - EapCode enum Request/Response/Success/Failure, EapMethod enum 16 tipos Identity/Notification/Nak/Md5Challenge/Otp/Gtc/Tls/Sim/Ttls/Aka/Peap/MsChapV2/AkaPrime/Fast/Pwd/Expanded com name()/supports_192bit(), EapPacket struct code/identifier/length/method/data com parse()/to_bytes()/identity_response()/nak_response(), EapolType enum 9 tipos EapPacket/Start/Logoff/Key/Alert/Mka/AnnouncementGeneric/Specific/Req, EapolFrame struct com parse()/to_bytes()/start()/logoff()/wrap_eap(), TlsContentType enum ChangeCipherSpec/Alert/Handshake/ApplicationData, TlsAlertLevel enum Warning/Fatal, eap_tls_flags module LENGTH_INCLUDED/MORE_FRAGMENTS/START/OUTER_TLV_LENGTH, EapTlsState enum 7 estados Idle/WaitingServerHello/ProcessingCertificate/SendingCertificate/TlsComplete/Success/Failed, Tls192BitCipherSuite enum EcdheEcdsaAes256GcmSha384/EcdheRsaAes256GcmSha384/DheRsaAes256GcmSha384 com to_bytes()/name(), Certificate struct data/subject_cn/issuer_cn/not_before/not_after com from_der()/from_pem(), PrivateKey struct com from_pem(), PrivateKeyType enum Rsa/EcdsaP256/P384/P521, EnterpriseCredentials struct identity/password/client_cert/client_key/ca_cert/allow_insecure/anonymous_identity/domain_constraint com tls()/password_based(), Eap8021xState enum 8 estados Disconnected/Started/WaitingIdentityRequest/IdentitySent/Negotiating/Authenticating/Authenticated/Failed, TlsSession struct client_random/server_random/pre_master_secret/master_secret/handshake_messages/cipher_suite/session_id, Wpa3EnterpriseSupplicant struct com start()/process_eapol()/handle_eap_tls()/build_tls_client_hello()/process_tls_handshake()/build_tls_key_exchange()/derive_msk()/get_msk()/get_pmk()/is_complete()/status(), base64_decode(), APIs create_wpa3_enterprise()/create_wpa3_enterprise_192bit()/supports_wpa3_enterprise()/supports_wpa3_enterprise_192bit()) |
| 2026-01-18 | [WiFi] Hotspot Mode (net/wifi/hotspot.rs: NOVO ~1100 linhas - HotspotSecurity enum Open/Wpa2Personal/Wpa3Personal/Wpa2Wpa3Transition/Wpa2Enterprise/Wpa3Enterprise com name()/requires_password()/supports_pmf(), WifiBand enum Band2_4GHz/Band5GHz/Band6GHz/DualBand/TriBand com name()/channels()/default_channel(), ChannelWidth enum Mhz20/40/80/160/320 com mhz(), HotspotConfig struct ssid/password/security/band/channel/channel_width/hidden_ssid/max_clients/client_isolation/pmf_required/beacon_interval/dtim_period/wmm_enabled/fast_bss_transition/country_code/bandwidth_limit/inactivity_timeout/mac_filter/mac_filter_list com open()/wpa2()/wpa3()/with_band()/with_channel()/with_max_clients()/with_client_isolation()/validate(), MacFilterMode enum Disabled/Whitelist/Blacklist, ClientState enum Authenticating/Associating/Handshaking/Connected/Disconnecting/Disconnected/Blocked, ClientInfo struct mac/state/ip_address/hostname/connected_at/last_activity/signal_strength/tx_rate/rx_rate/tx_bytes/rx_bytes/tx_packets/rx_packets/aid/capabilities/ptk com connection_duration()/idle_time()/touch()/add_tx()/add_rx(), ClientCapabilities struct ht/vht/he/eht/short_gi_20/40/80/ldpc/tx_stbc/rx_stbc/spatial_streams/max_ampdu_exp/width_40_2ghz/80/160/320/power_save/wmm/mfp_capable/mfp_required, HotspotState enum Disabled/Starting/Running/Stopping/Error, HotspotEvent enum Started/Stopped/ClientConnected/Disconnected/Authenticated/AuthFailed/ChannelChanged/Error, HotspotError enum 10 tipos, HotspotManager struct com start()/stop()/handle_auth_request()/handle_assoc_request()/complete_auth()/disconnect_client()/allocate_dhcp_ip()/clients()/build_beacon()/build_rsn_ie()/process_frame(), HotspotStats struct, global HOTSPOT_MANAGER, APIs init()/start()/stop()/is_running()/client_count()/stats()/disconnect_client()/process_timeout()) |
| 2026-01-18 | [WiFi] WiFi 6E/6GHz (net/wifi/wifi6e.rs: NOVO ~700 linhas - Channel6Ghz struct number/frequency/width/operating_class/is_psc com channel_20/40/80/160/320mhz()/psc_channels()/unii5/6/7/8_channels()/all_20mhz_channels()/all_160mhz_channels()/all_320mhz_channels()/bandwidth_mhz()/is_unii5/6/7/8()/unii_band(), PSC_CHANNELS_20MHZ const 15 channels, ChannelWidth6Ghz enum Mhz20/40/80/160/320 com mhz()/operating_class(), RegulatoryDomain6Ghz enum Fcc/Etsi/Mic/Ic/Acma/Other com country_code()/frequency_range()/requires_afc()/max_eirp_lpi()/max_eirp_sp(), PowerType6Ghz enum LowPowerIndoor/StandardPower/VeryLowPower com name()/requires_afc(), AfcRequest struct serial_number/latitude/longitude/altitude/uncertainty/requested_channels/min_eirp/operating_class com new()/with_altitude()/with_uncertainty()/with_channels(), AfcResponse struct response_time/expiry_time/available_channels/response_code, AvailableChannel struct, AfcResponseCode enum 6 tipos, DiscoveryMethod enum Passive/PscOnly/OutOfBand/Fils/UnsolicitedProbe, ReducedNeighborReport struct com parse()/sixghz_neighbors(), RnrNeighbor struct, He6GhzCapabilities struct min_mpdu_start_spacing/max_ampdu_length_exp/max_mpdu_length/sm_power_save/rd_responder/rx_antenna_pattern/tx_antenna_pattern com parse()/to_bytes(), SmPowerSave enum, Operation6Ghz struct com parse()/center_frequency(), Wifi6GhzManager struct com init()/set_power_type()/process_afc_response()/psc_channels()/process_rnr()/add_discovered()/discovered_networks()/supported_channels()/max_eirp()/is_enabled()/build_6ghz_caps_element(), Discovered6GhzNetwork struct, global WIFI6E_MANAGER, APIs init()/is_available()/psc_channels()/supported_channels()/process_rnr()/discovered_networks()/set_power_type()/max_eirp()) |
| 2026-01-18 | [Rede] Network Profiles (net/profiles.rs: NOVO ~800 linhas - ProfileType enum Wifi/Vpn/Ethernet, WifiCredentials enum None/Psk/Enterprise com Psk{psk,sae}/Enterprise{identity,password,client_cert,client_key,ca_cert,eap_method}, WifiProfile struct id/name/ssid/bssid/credentials/hidden/auto_connect/priority/metered/proxy/dns_servers/created/last_connected com psk()/enterprise()/open(), ProxyConfig struct proxy_type/host/port/username/password/bypass_list, ProxyType enum None/Manual/Auto, VpnProfile struct id/name/vpn_type/server/username/password/config/auto_connect/reconnect/created/last_connected com openvpn()/wireguard()/ipsec(), VpnType enum OpenVpn/WireGuard/IpsecIkev2/L2tp/Pptp, OpenVpnConfig struct com from_ovpn(), WireGuardConfig struct private_key/address/dns/peers com parse(), WireGuardPeer struct public_key/allowed_ips/endpoint/persistent_keepalive, EthernetProfile struct id/name/auto_connect/dhcp/static_ip/gateway/dns_servers/vlan_id/mtu/dot1x/created, Dot1xConfig struct identity/password/client_cert/client_key/ca_cert/eap_method/phase2_method, EapMethod enum Tls/Ttls/Peap/Pwd/Leap/Md5/Fast, Phase2Method enum MsChapV2/Gtc/Pap/MsChap/None, ProfileManager struct wifi_profiles/vpn_profiles/ethernet_profiles/next_wifi_id/next_vpn_id/next_ethernet_id com add_wifi()/remove_wifi()/get_wifi()/get_wifi_by_ssid()/update_wifi()/add_vpn()/remove_vpn()/get_vpn()/update_vpn()/add_ethernet()/remove_ethernet()/get_ethernet()/update_ethernet()/all_wifi()/all_vpn()/all_ethernet()/profile_count(), global PROFILE_MANAGER, APIs init()/add_wifi_profile()/get_wifi_profile()/get_wifi_profile_by_ssid()/remove_wifi_profile()/add_vpn_profile()/get_vpn_profile()/remove_vpn_profile()/add_ethernet_profile()/get_ethernet_profile()/remove_ethernet_profile()/profile_count()) |
| 2026-01-18 | [Power] Core Parking (arch/x86_64_arch/core_parking.rs: NOVO ~850 linhas - ParkingPolicy enum Disabled/Conservative/Balanced/Aggressive/Custom com unpark_threshold()/park_threshold()/min_unparked_percent()/max_parked_percent()/from_str()/as_str(), CState enum C0/C1/C1E/C3/C6/C7/C8/C10 com exit_latency_us()/power_savings_percent()/as_str(), CoreState enum Active/Parking/Parked/Unparking/NotParkable, CoreInfo struct cpu_id/state/cstate/load/is_bsp/parked_time/active_time/transitions/park_order, ParkingConfig struct policy/custom_unpark_threshold/custom_park_threshold/custom_min_unparked/target_cstate/max_cstate/sample_interval_ms/hysteresis_samples/thermal_aware/thermal_threshold/allow_parking_pcores/prefer_parking_ecores, ParkingStats struct total_parks/total_unparks/total_parked_time/avg_parked_cores/power_saved/failures/thermal_parks, CoreParkingManager struct com init()/detect_max_cstate()/update_load()/process()/get_thermal_pressure()/try_park_cores()/try_unpark_cores()/select_core_to_park()/select_core_to_unpark()/park_core()/unpark_core()/enter_cstate()/exit_cstate()/update_stats()/set_enabled()/set_policy()/set_target_cstate()/get_config()/get_stats()/get_core_info()/get_all_cores()/parked_count()/active_count()/is_enabled()/format_status(), global PARKING_MANAGER, APIs init()/process()/update_load()/set_enabled()/is_enabled()/set_policy()/get_policy()/set_target_cstate()/parked_count()/active_count()/get_core_info()/get_all_cores()/get_stats()/get_config()/format_status()/park_core()/unpark_core()/unpark_all(), + wake_cpu() adicionado ao ipi.rs) |
| 2026-01-18 | [Power] Resume Speed (power/resume_speed.rs: NOVO ~750 linhas - ResumePhase enum Critical/Core/UserVisible/Network/Other/Background com as_str()/parallel_allowed(), DeviceResumeInfo struct name/phase/priority/resume_callback/suspend_callback/dependencies/async_capable/quick_resume/last_resume_us/avg_resume_us/resume_count/suspended/skip_resume com new()/with_dependencies()/with_async()/with_quick_resume(), ResumeConfig struct parallel_resume/max_parallel/state_caching/skip_unused/target_resume_ms/profiling_enabled/defer_noncritical/defer_time_ms, ResumeStats struct resume_count/avg_resume_ms/best_resume_ms/worst_resume_ms/last_resume_ms/slow_devices/parallel_savings_ms/skip_savings_ms/quick_savings_ms, SlowDevice struct, ResumeTiming struct device_name/phase/start_us/end_us/duration_us/parallel, ResumeSpeedManager struct com init()/register_system_devices()/register_device()/unregister_device()/update_device_callback()/mark_suspended()/mark_resumed()/resume_all()/resume_phase()/resume_sequential()/resume_parallel()/group_by_dependencies()/wait_for_dependencies()/quick_resume_device()/record_timing()/update_device_stats()/update_stats()/analyze_slow_devices()/get_timestamp_us()/get_config()/set_config()/get_stats()/get_timings()/get_devices()/format_status()/prepare_suspend()/set_parallel_resume()/set_target_time()/set_device_skippable(), global RESUME_MANAGER, APIs init()/register_device()/unregister_device()/resume_all()/prepare_suspend()/get_stats()/get_config()/set_config()/get_devices()/get_timings()/format_status()/mark_suspended()/mark_resumed()/set_parallel_resume()/set_target_time()/set_device_skippable()) |
| 2026-01-18 | [Segurança] MOK Manager (security/mok.rs: NOVO ~950 linhas - MokOperation enum Enroll/Delete/Reset/Import/Export, MokState enum Enrolled/PendingEnroll/PendingDelete/Rejected, MokKeyType enum X509/X509Pem/Rsa2048/Sha256 com as_str()/to_signature_type(), MokEntry struct id/key_type/data/common_name/issuer/subject/not_before/not_after/fingerprint/state/enrolled_at/description/owner com from_certificate()/fingerprint_hex()/is_expired()/is_not_yet_valid(), PendingOperation struct operation/entry_id/certificate/password_hash/requested_at/expires_at, MokConfig struct max_entries/pending_timeout_secs/require_password/min_password_length/allow_self_signed/audit_logging, MokStats struct enrolled_count/pending_enroll_count/pending_delete_count/operations_count/failed_operations/rejected_count, MokResult enum Success/Pending/Failed/PasswordRequired/InvalidCertificate/AlreadyEnrolled/NotFound/MaxEntriesReached, MokManager struct com init()/load_mok_list()/enroll()/delete()/confirm_pending()/cancel_pending()/reset()/validate_certificate()/save_mok_list()/get_entry()/get_enrolled()/get_pending()/get_all()/contains_hash()/export_der()/export_pem()/get_config()/set_config()/get_stats()/has_shim()/is_available()/format_status()/cleanup_expired(), helper functions calculate_sha256()/parse_certificate_info()/validate_x509_der()/validate_x509_pem()/base64_encode()/get_timestamp(), global MOK_MANAGER, APIs init()/enroll()/delete()/confirm_pending()/cancel_pending()/reset()/get_enrolled()/get_pending()/get_all()/get_entry()/contains_hash()/export_der()/export_pem()/get_config()/set_config()/get_stats()/is_available()/has_shim()/format_status()/cleanup_expired()) |
| 2026-01-18 | [Segurança] Measured Boot (security/measured_boot.rs: NOVO ~950 linhas - pcr_index module com PCR_FIRMWARE/PLATFORM_CONFIG/OPTION_ROMS/BOOTLOADER/BOOTLOADER_CONFIG/KERNEL/KERNEL_CMDLINE/IMA/SECUREBOOT_POLICY/RESUME/CUSTOM_START/CUSTOM_END, BootComponent enum Firmware/PlatformConfig/OptionRom/Bootloader/BootloaderConfig/Kernel/KernelCmdline/Initrd/KernelModule/SecureBootPolicy/Custom com pcr_index()/name(), EventType enum 40+ tipos TCG event log format EfiPlatformFirmwareBlob/EfiBootServicesApplication/EfiVariableAuthority/etc, MeasurementEntry struct pcr_index/event_type/digest/description/component/data_size/timestamp_ms/sequence, MeasurementResult enum Success/TpmNotAvailable/ExtendFailed/DigestFailed/ComponentNotFound/AlreadyMeasured, MeasurementPolicy enum MeasureAll/MeasureCritical/Disabled, ExpectedPcrValue struct para verificação, MeasuredBootConfig struct policy/hash_algorithm/enable_logging/max_log_entries/enable_sealed_secrets/expected_values/enforce_expected/log_to_console, MeasuredBootStats struct total_measurements/successful_measurements/failed_measurements/pcr_extends/verification_failures/last_measurement_ms, SealedSecret struct id/sealed_data/pcr_policy/created_ms/description, MeasuredBootManager struct com init()/sync_pcr_shadow()/measure()/measure_kernel()/measure_cmdline()/measure_initrd()/measure_module()/measure_secureboot_policy()/compute_sha256()/extend_shadow_pcr()/component_to_event_type()/verify_boot()/get_pcr()/read_pcr_from_tpm()/event_log()/stats()/is_boot_verified()/set_expected_pcr()/seal_secret()/unseal_secret()/export_event_log()/config()/set_config()/format_status(), SHA-256 implementation sha256_hash() com SHA256_K/SHA256_H constants, global MEASURED_BOOT, APIs init()/measure_kernel()/measure_cmdline()/measure_initrd()/measure_module()/verify_boot()/is_verified()/get_pcr()/seal_secret()/unseal_secret()/stats()/status()) |
| 2026-01-18 | [Segurança] Fingerprint Login PAM (security/pam_fprintd.rs: NOVO ~700 linhas - FprintdConfig struct max_attempts/timeout_secs/show_guidance/allow_fallback/min_quality/require_user_match/debug/poll_interval_ms, AuthAttemptResult enum Success/NoMatch/Timeout/PoorQuality/NoDevice/NoEnrollment/Cancelled/Error, FprintdStats struct total_attempts/successful_auths/failed_auths/timeouts/poor_quality_scans/fallbacks/last_auth_time, EnrolledUser struct user_id/username/enrolled_fingers/finger_positions/template_ids/enrolled_at/last_auth/auth_count, PamFprintd struct com init()/load_enrollments()/register_user()/get_user_id()/has_enrollment()/has_enrollment_by_name()/authenticate_user()/find_user_for_template()/enroll()/delete_enrollment()/get_enrollment()/list_enrolled_users()/config()/set_config()/stats()/format_status(), PamModule impl com authenticate()/setcred()/acct_mgmt()/open_session()/close_session(), EnrollSessionState enum Idle/WaitingForFinger/Capturing/Processing/NeedMore/Complete/Failed/Cancelled, EnrollSession struct com new()/start()/step()/cancel(), PamFprintdWrapper trait object wrapper, global PAM_FPRINTD, APIs init()/has_enrollment()/start_enrollment()/delete_enrollment()/get_enrollment()/list_enrolled()/stats()/status()/pam_module()) |
| 2026-01-18 | [Input] Touchscreen Support (drivers/touchscreen.rs: NOVO ~1000 linhas - TouchEventType enum Down/Up/Move/Cancel/Proximity, TouchPointState enum Inactive/Down/Move/Up, TouchToolType enum Finger/Pen/Eraser/Brush, TouchPoint struct slot/tracking_id/x/y/pressure/touch_major/minor/orientation/state/tool_type/timestamp, TouchEvent struct event_type/point/touch_count/timestamp, GestureType enum None/Tap/DoubleTap/LongPress/SwipeUp/Down/Left/Right/TwoFingerSwipe*/ThreeFingerSwipe*/FourFingerSwipe*/PinchIn/Out/Rotate/Scroll, GestureState enum Begin/Update/End/Cancel, GestureEvent struct gesture_type/state/delta_x/y/scale/rotation/finger_count/timestamp, TouchInterface enum I2C/Usb/Spi/Ps2, TouchTechnology enum Capacitive/Resistive/Infrared/Optical/Saw/Acoustic, TouchscreenCapabilities struct max_contacts/x_max/y_max/pressure_max/has_pressure/has_touch_major/minor/orientation/has_stylus/has_proximity/technology, CalibrationData struct offset_x/y/scale_x/y/swap_xy/invert_x/y, TouchscreenConfig struct swipe_threshold/tap_timeout_ms/double_tap_timeout_ms/long_press_timeout_ms/edge_margin/palm_rejection/calibration, TouchscreenDevice struct id/vendor_id/product_id/name/interface/capabilities/calibration/config/initialized/multi_touch_enabled, TouchscreenManager struct devices/touch_points/active_touches/gesture_state/on_touch/on_gesture com init()/register_device()/process_touch()/process_gesture()/emit_touch_event()/emit_gesture()/calculate_finger_distance/angle()/calculate_center()/determine_swipe_direction()/apply_calibration()/get_device()/list_devices()/set_calibration()/configure(), helper functions sqrt_f32() Newton's method/atan2_f32() software implementation for no_std, gesture recognition tap/double-tap/long-press/swipe/pinch/rotate/scroll, multi-touch up to 10 points, palm rejection support, global TOUCHSCREEN_MANAGER singleton) |
| 2026-01-18 | [Input] Stylus/Pen Support (drivers/stylus.rs: NOVO ~950 linhas - StylusToolType enum Pen/Eraser/Brush/Pencil/Airbrush/Finger/Mouse/Lens/Unknown com from_usb_id(), StylusButtons struct tip/barrel1/barrel2/eraser/in_range/inverted/touch, StylusEventType enum ProximityIn/Out/PenDown/PenUp/Move/ButtonDown/Up/ToolChange/PressureChange/TiltChange, StylusEvent struct event_type/x/y/pressure/tilt_x/tilt_y/rotation/distance/tool_type/buttons/tool_serial/timestamp, StylusCapabilities struct x/y_max/resolution/pressure_max/levels/tilt_max/has_pressure/tilt/rotation/distance/eraser/barrel1/barrel2/num_tools/has_tool_serial com preset methods default_tablet/simple_pen/surface_pen/wacom_intuos, StylusInterface enum UsbHid/Bluetooth/I2cHid/Serial/Integrated, StylusVendor enum Wacom/Microsoft/Huion/XpPen/Gaomon/Ugee/Veikk/Apple/Samsung/Lenovo/Dell/Hp/Unknown com from_vendor_id(), StylusConfig struct pressure_curve/threshold/sensitivity/tilt_sensitivity/palm_rejection/mapping_mode/map_to_display/aspect_ratio_correction/active_area bounds, MappingMode enum Absolute/Relative/Pen, StylusState struct x/y/pressure/tilt/rotation/distance/tool_type/buttons/in_proximity/in_contact/tool_serial, StylusStats struct events_processed/strokes/distance_traveled/peak_pressure/avg_pressure/contact_time_ms/tool_changes, StylusDevice struct id/vendor_id/product_id/name/vendor/interface/capabilities/config/state/stats com detect_capabilities/apply_pressure_curve/map_to_display/process_input, StylusRawInput struct, StylusManager struct devices/next_device_id/on_event/active_device com init/register_device/unregister_device/process_input/set_event_callback/configure_device/set_pressure_curve/set_active_area/set_mapping_mode/map_to_display/format_status, helper math functions pow_f32/ln_f32/exp_f32/sqrt_u64 for no_std, global STYLUS_MANAGER, Wacom/Surface Pen/Huion tablet support, pressure curve customization) |
| 2026-01-18 | [Input] Game Controllers (drivers/gamepad.rs: NOVO ~1200 linhas - GamepadButton enum South/East/West/North/Bumpers/Select/Start/Guide/Thumbs/DPad/Triggers/Touchpad/Capture/Mute/Paddles com xbox_name/playstation_name/nintendo_name, GamepadAxis enum LeftStickX/Y/RightStickX/Y/Triggers/DPad/Gyro/Accel, GamepadType enum Xbox/Xbox360/XboxElite/DualShock4/DualSense/SwitchPro/JoyCon/Steam/GenericUsb/Bluetooth/EightBitDo com from_vendor_product, ConnectionType enum Usb/Bluetooth/ProprietaryWireless, GamepadEventType enum ButtonPressed/Released/AxisMoved/Connected/Disconnected/BatteryChanged, GamepadEvent struct, RumbleEffect struct strong_magnitude/weak_magnitude/duration_ms com presets off/light/medium/heavy, LightbarColor struct para DualShock/DualSense com player_color(), AdaptiveTriggerEffect enum Off/Continuous/Section/Vibration/Weapon com to_bytes() para DualSense, BatteryStatus struct level/charging/powered, ButtonState struct com bitmap, AxisState struct sticks/triggers/gyro/accel com get_axis() normalized, GamepadConfig struct deadzone/threshold/invert_y/rumble_enabled/scale/gyro_enabled/sensitivity/player_number, GamepadStats struct, GamepadDevice struct com process_input/apply_deadzone/build_rumble_command device-specific Xbox/DS4/DualSense/SwitchPro, GamepadRawInput struct, GamepadManager struct com init/register_gamepad/unregister_gamepad/process_input/set_rumble/stop_rumble/set_event_callback/configure/set_deadzone/format_status, global GAMEPAD_MANAGER, suporte Xbox One/Series/360/Elite + DualShock 4 + DualSense + Switch Pro + Joy-Con + Steam Controller + 8BitDo + genérico, rumble/vibration, gyro/motion, adaptive triggers, lightbar colors) |
| 2026-01-18 | [Accessibility] Screen Reader (gui/accessibility/screen_reader.rs: NOVO ~750 linhas - AccessibleRole enum Window/Dialog/Alert/MenuBar/Menu/MenuItem/Toolbar/Button/CheckBox/RadioButton/TextField/TextArea/Label/Image/Link/List/ListItem/Tree/TreeItem/Table/TableRow/TableCell/Tab/TabPanel/ProgressBar/Slider/ScrollBar/Tooltip/StatusBar/Heading/Paragraph/Article/Group/Separator/Unknown com to_string(), AccessibleState struct focused/selected/disabled/expanded/checked/editable/multiselectable/readonly/required/invalid flags, AccessibleElement struct role/states/name/description/value/bounds/children/parent com add_child/role_name/state_string, SpeechPriority enum Message/Text/Alert/System com to_queue_order, SpeechUtterance struct text/priority/rate/pitch/volume/language/interruptible/timestamp_ms, VerbosityLevel enum Minimal/Normal/Verbose com announce_role/states/position, NavigationMode enum Element/Container/Heading/Link/Button/FormField/Table com to_string, ScreenReaderConfig struct enabled/verbosity/rate/pitch/volume/language/echo_typed_chars/words/auto_read_page/silence_delay_ms/punctuation_level/announce_caps/caps_as_word, ScreenReaderStats struct utterances_spoken/chars_spoken/elements_navigated/interruptions/session_start_ms/total_speaking_time_ms, TtsCallback type, ScreenReader struct enabled/config/stats/speech_queue/current_focus/focus_history/navigation_mode/tts_callback/speaking/last_spoken_time com init/enable/disable/is_enabled/set_focus/announce_element/build_announcement/announce_state_changes/speak/queue_speech/interrupt/process_queue/navigate_next/prev/navigate_to_mode/read_all/echo_character/echo_word/set_verbosity/config/stats/format_status, helper functions phonetic_char/describe_punctuation/number_to_words, global SCREEN_READER, accessibility API init/enable/disable/set_focus/speak/interrupt/navigate/read_all/echo/set_verbosity/is_enabled/stats + gui/accessibility/mod.rs: module exports e init()) |
| 2026-01-18 | [Accessibility] High Contrast Mode (gui/accessibility/high_contrast.rs: NOVO ~1040 linhas - ContrastScheme enum BlackOnWhite/WhiteOnBlack/YellowOnBlack/GreenOnBlack/Custom com name()/description(), Color struct RGBA com rgb/hex constructors, contrast_ratio()/relative_luminance() WCAG formulas, meets_wcag_aa/aaa(), invert()/mix(), ContrastPalette struct 24 colors background/text/link/button/input/selection/error/warning/success etc com presets black_on_white/white_on_black/yellow_on_black/green_on_black, ElementType enum 30 UI types Window/Dialog/Button/TextInput/Checkbox/RadioButton/Link/List/Menu/Tab/Scrollbar/etc, ElementState enum Normal/Hovered/Pressed/Focused/Disabled/Selected/Checked/Error, ElementStyle struct bg/fg/border/outline colors/widths, HighContrastConfig struct enabled/scheme/custom_palette/min_border_width/focus_width/focus_offset/remove_backgrounds/transparency/disable_animations/bold_text/underline_links/show_button_borders/system_ui_only, HighContrastStats struct, HighContrastManager struct com init/enable/disable/toggle/is_enabled/set_scheme/scheme/set_custom_palette/palette/get_style()/get_focus_style()/should_remove_backgrounds/transparency/disable_animations/use_bold_text/underline_links/config/set_config/stats/format_status, helper math functions pow_f32/ln_f32/exp_f32/floor_f32 for no_std luminance calculation, global HIGH_CONTRAST_MANAGER singleton, APIs init/enable/disable/toggle/is_enabled/set_scheme/get_scheme/get_style/get_palette/stats/status) |
| 2026-01-18 | [Accessibility] Large Text (gui/accessibility/large_text.rs: NOVO ~600 linhas - TextScale enum Normal/Large/Larger/Largest/ExtraLarge/Custom com percentage()/multiplier()/name()/description()/from_percentage()/presets(), TextCategory enum Body/Label/Heading/Menu/Button/Input/WindowTitle/Tooltip/StatusBar/Tab/ListItem/Table/Code/Caption/SmallPrint com name()/base_size()/min_size()/all(), FontWeight enum Normal/Medium/SemiBold/Bold com value()/name(), TextProperties struct size/line_height/letter_spacing/word_spacing/weight com default_for_size()/line_height_px(), CategoryScales struct per-category scale overrides com get()/set(), LargeTextConfig struct enabled/scale/category_scales/increased_line_height/line_height_factor/letter_spacing_factor/word_spacing_factor/bold_text/min_font_size/max_font_size/ui_only, LargeTextStats struct, LargeTextManager struct com init/enable/disable/toggle/is_enabled/set_scale/scale/set_category_scale/get_category_scale/calculate_size/calculate_properties/scaled_size/set_bold_text/set_line_height/set_letter_spacing/set_word_spacing/set_min_size/set_max_size/config/set_config/effective_multiplier/stats/format_status, global LARGE_TEXT_MANAGER singleton, APIs init/enable/disable/toggle/is_enabled/set_scale/get_scale/scaled_size/get_properties/multiplier/stats/status) |
| 2026-01-18 | [Accessibility] Screen Magnifier (gui/accessibility/magnifier.rs: NOVO ~950 linhas - MagnificationMode enum FullScreen/Lens/Docked/Split/PictureInPicture com name()/description()/all(), DockPosition enum Top/Bottom/Left/Right, LensShape enum Rectangle/Square/Circle, TrackingMode enum Centered/Proportional/Push/Comfortable com name()/description(), ZoomLevel enum X1_5/X2/X3/X4/X6/X8/X10/X16/Custom com percentage()/multiplier()/name()/from_percentage()/presets(), Point struct x/y, Rect struct x/y/width/height com contains()/center(), MagnificationView struct source_rect/dest_rect/cursor_pos/zoom com new()/calculate_source_rect()/center_on_cursor()/set_cursor()/set_zoom(), LensView struct width/height/shape/position/zoom com bounds()/source_bounds(), MagnifierConfig struct enabled/mode/zoom/zoom_increment/tracking/dock_position/dock_size_percent/lens_shape/lens_width/height/invert_colors/smooth_scrolling/scroll_speed/follow_caret/focus/show_cursor/cursor_magnification, MagnifierStats struct, Magnifier struct com init/set_screen_size/enable/disable/toggle/is_enabled/initialize_view/set_mode/mode/set_zoom/zoom/zoom_in/zoom_out/update_cursor/apply_tracking_to_view/request_redraw/fullscreen_view/lens_view/set_tracking/set_lens_size/set_lens_shape/set_invert_colors/config/set_config/stats/format_status, global MAGNIFIER singleton, APIs init/enable/disable/toggle/is_enabled/set_zoom/get_zoom/zoom_in/zoom_out/update_cursor/set_screen_size/stats/status) |
| 2026-01-18 | [Accessibility] Keyboard Accessibility (gui/accessibility/keyboard.rs: NOVO ~950 linhas - ModifierKey enum Shift/Ctrl/Alt/Super/AltGr com name()/all(), StickyState enum Off/Latched/Locked com name(), StickyKeysConfig struct enabled/lock_on_double_press/double_press_timeout_ms/off_on_two_modifiers/sound_on_modifier/show_indicator, ModifierState struct state/last_press_ms/use_count, SlowKeysConfig struct enabled/acceptance_delay_ms/sound_on_accept/sound_on_reject, SlowKeyState struct key_code/press_start_ms/accepted, BounceKeysConfig struct enabled/debounce_delay_ms, MouseKeysConfig struct enabled/initial_speed/max_speed/acceleration/acceleration_delay_ms/use_numpad, MouseKeysState struct direction/movement_start_ms/current_speed/held_button/moving, MouseButton enum Left/Middle/Right, KeyEventType enum Press/Release/Repeat, KeyEventResult enum PassThrough/Suppress/Modified/MouseMove/MouseButton, KeyboardAccessibilityStats struct, KeyboardAccessibility struct com init/enable_sticky_keys/disable_sticky_keys/is_sticky_keys_enabled/process_modifier/process_key_with_sticky/get_sticky_state/get_active_modifiers/enable_slow_keys/disable_slow_keys/is_slow_keys_enabled/set_slow_keys_delay/process_slow_key/enable_bounce_keys/disable_bounce_keys/is_bounce_keys_enabled/set_bounce_keys_delay/process_bounce_key/enable_mouse_keys/disable_mouse_keys/is_mouse_keys_enabled/process_mouse_key numpad directions/click/button select/drag, sticky/slow/bounce/mouse_config getters/setters, callbacks for sticky_change/mouse_move/mouse_button, stats()/format_status(), global KEYBOARD_ACCESSIBILITY singleton, APIs init/enable_sticky_keys/disable_sticky_keys/is_sticky_keys_enabled/get_sticky_state/enable_slow_keys/disable_slow_keys/is_slow_keys_enabled/enable_bounce_keys/disable_bounce_keys/is_bounce_keys_enabled/enable_mouse_keys/disable_mouse_keys/is_mouse_keys_enabled/status()/stats()) |
| 2026-01-18 | [Accessibility] On-Screen Keyboard (gui/accessibility/osk.rs: NOVO ~1600 linhas - KeyType enum 15 types Character/Space/Backspace/Enter/Shift/CapsLock/Tab/Ctrl/Alt/Super/Function/Arrow/Delete/Escape/NumberToggle/LanguageSwitch/Close/Minimize/Settings, KeyDefinition struct key_type/normal/shifted/label/shifted_label/width/key_code com char()/special()/display_label(), KeyboardLayout enum QwertyUs/QwertyUk/Azerty/Qwertz/Dvorak/Colemak/Abnt2 com name()/code(), KeyboardMode enum Standard/Compact/Split/Numeric/Phone, KeyState enum Normal/Hovered/Pressed/Locked/Disabled, KeyboardPosition enum Bottom/Top/Floating/Left/Right, OskColor struct RGBA, OskTheme struct 10 colors com light()/dark()/high_contrast(), OskConfig struct 24 settings enabled/layout/mode/position/theme/key_height/spacing/border_radius/suggestions/sound/haptic/dwell/auto_show/hide/opacity/float_pos/size, KeyVisual struct key/state/x/y/width/height/row/col com contains(), Prediction struct word/confidence/frequency, OskStats struct, OnScreenKeyboard struct com init/set_screen_size/show/hide/toggle/is_visible/enable/disable/is_enabled/load_layout/load_qwerty_layout/load_azerty_layout/load_qwertz_layout/load_dvorak_layout/load_colemak_layout/load_abnt2_layout/add_key_row/recalculate_positions/calculate_keyboard_bounds/calculate_total_height/load_dictionary/update_predictions/process_input/process_hover/find_key_at/handle_key_press/emit_key_event/accept_prediction/set_layout/layout/set_mode/mode/set_position/position/set_theme/theme/set_dwell_enabled/set_dwell_time/set_key_press_callback/set_special_key_callback/config/set_config/stats/get_keys/get_predictions/get_bounds/is_shift_active/format_status, KeyEventOutput enum Character/Special, global OSK singleton, APIs init/enable/disable/is_enabled/show/hide/toggle/is_visible/set_layout/get_layout/status/stats, 6 full keyboard layouts com suporte a múltiplos idiomas, word prediction, dwell clicking) |
| 2026-01-18 | [Accessibility] Reduce Motion (gui/accessibility/reduce_motion.rs: NOVO ~750 linhas - AnimationType enum 23 types WindowTransitions/MenuAnimations/ScrollAnimations/ButtonEffects/ProgressBars/Spinners/ParallaxScrolling/ZoomOnHover/BackgroundMotion/PageTransitions/NotificationAnimations/CursorEffects/WindowMinMax/WorkspaceTransitions/TooltipAnimations/DropdownAnimations/TabTransitions/IconAnimations/AutoPlayVideo/AutoPlayGifs/Carousels/TextAnimations/All com name()/description()/is_motion()/all_types(), MotionReduction enum None/Reduced/Crossfade/Disabled com name()/description()/duration_multiplier(), AnimationTiming struct original_duration_ms/effective_duration_ms/disabled/use_crossfade com new()/should_skip(), ReduceMotionConfig struct 14 settings enabled/global_mode/overrides/min_duration_ms/max_duration_ms/disable_parallax/pause_auto_play/disable_blinking/scrolling_text/zoom_effects/reduce_transparency/instant_visibility/prefer_static_backgrounds/system_ui_only com default/minimal_motion()/reduced_motion()/crossfade_only() presets, ReduceMotionStats struct, AnimationRequest struct animation_type/duration_ms/essential/source com new()/essential()/from_source(), ReduceMotionManager struct com init/enable/disable/toggle/is_enabled/set_mode/mode/set_override/get_override/get_reduction/calculate_timing/should_animate/should_disable_parallax/pause_auto_play/blinking/scrolling_text/zoom/reduce_transparency/instant_visibility/prefers_static_backgrounds/record_auto_play_paused/apply_preset/callbacks/config/set_config/stats/prefers_reduced_motion() CSS media query/format_status, global REDUCE_MOTION singleton, APIs init/enable/disable/toggle/is_enabled/set_mode/get_mode/should_animate/calculate_timing/should_disable_parallax/should_pause_auto_play/prefers_reduced_motion/apply_preset/status/stats, presets minimal/reduced/crossfade/default) |

---

## Métricas de Progresso

### Por Seção
| Seção | Total | Concluído | % |
|-------|-------|-----------|---|
| 1. Instalador | 24 | 12 | 50% |
| 2. Hardware Real | 31 | 22 | 71% |
| 3. GPU e Gráficos | 35 | 17 | 49% |
| 4. Desktop Environment | 38 | 38 | 100% |
| 5. Aplicações | 38 | 18 | 47% |
| 6. Package Manager | 17 | 14 | 82% |
| 7. Áudio | 17 | 12 | 71% |
| 8. Rede | 30 | 6 | 20% |
| 9. Power Management | 25 | 19 | 76% |
| 10. Segurança | 20 | 7 | 35% |
| 11. Input/Accessibility | 23 | 2 | 9% |
| 12. i18n | 18 | 7 | 39% |
| 13. Performance | 16 | 0 | 0% |
| 14. Cloud/Virt | 11 | 0 | 0% |
| 15. Hardware Test | 14 | 0 | 0% |
| 16. Ecossistema | 18 | 0 | 0% |
| **TOTAL** | **375** | **175** | **46.7%** |

### Por Prioridade
| Prioridade | Total | Concluído | % |
|------------|-------|-----------|---|
| 🔴 Crítico | ~120 | 101 | 84% |
| 🟡 Importante | ~160 | 57 | 36% |
| 🟢 Nice-to-have | ~94 | 0 | 0% |

---

## Fases de Desenvolvimento Sugeridas

### Fase 1: Bootável em Hardware Real (MVP)
**Objetivo:** Bootar e usar básico em 1 laptop de referência

Itens prioritários:
- [ ] Live USB Boot
- [ ] Instalador básico (particionamento, cópia, bootloader)
- [ ] NVMe/AHCI real
- [ ] USB real
- [ ] ACPI básico (power button, lid, bateria)
- [ ] Intel/AMD GPU básico (framebuffer funcional)
- [ ] WiFi básico (1 chipset Intel)
- [ ] Touchpad funcional
- [ ] Suspend/Resume S3

### Fase 2: Usável no Dia-a-Dia
**Objetivo:** Substituir Linux para uso básico

Itens prioritários:
- [ ] Desktop Environment funcional
- [ ] File Manager
- [ ] Terminal
- [ ] Text Editor
- [ ] Web Browser básico
- [ ] Package Manager
- [ ] Settings App
- [ ] Bluetooth funcional
- [ ] Áudio completo
- [ ] Multi-monitor

### Fase 3: Feature Complete
**Objetivo:** Paridade com Linux para maioria dos casos

Itens prioritários:
- [ ] Todos os drivers de GPU (Intel, AMD)
- [ ] Todos os drivers de WiFi comuns
- [ ] Todas as aplicações essenciais
- [ ] Flatpak/AppImage support
- [ ] Encryption (LUKS)
- [ ] Secure Boot
- [ ] i18n completo
- [ ] Accessibility

### Fase 4: Polish e Comunidade
**Objetivo:** Pronto para usuários finais

Itens prioritários:
- [ ] Performance otimizada
- [ ] Boot rápido
- [ ] Documentação completa
- [ ] Website e comunidade
- [ ] Hardware certification
- [ ] App Store

---

## Notas

1. **Foco inicial:** Hardware Intel (mais comum em laptops) antes de AMD
2. **Browser:** Considerar portar WebKitGTK ou integrar Chromium/Firefox
3. **Apps:** Priorizar portar apps GTK/Qt existentes antes de criar do zero
4. **Drivers:** Começar com drivers mais simples e expandir
5. **Testes:** Cada feature deve ser testada em QEMU primeiro, depois em hardware real

---

## Como Contribuir

1. Escolha um item ⬜ Pendente
2. Crie uma branch: `feature/v2-nome-do-item`
3. Implemente e teste
4. Atualize este documento marcando como 🔄 ou ✅
5. Faça um PR

---

*Documento gerado em 2026-01-17*
*Stenzel OS - Rumo à produção!*

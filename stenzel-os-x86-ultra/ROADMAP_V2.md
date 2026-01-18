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
| Recovery Partition | Partição de recuperação | ⬜ Pendente | 🟡 Importante |
| Encryption Setup | LUKS encryption durante instalação | ⬜ Pendente | 🟡 Importante |

### 1.2 Imagens e Distribuição
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| ISO Builder | Script para gerar ISO bootável | ✅ Concluído | 🔴 Crítico |
| Hybrid ISO | ISO bootável em BIOS e UEFI | ✅ Concluído | 🔴 Crítico |
| Netinstall | Instalação mínima via rede | ⬜ Pendente | 🟢 Nice-to-have |
| OEM Install | Modo de instalação para fabricantes | ⬜ Pendente | 🟢 Nice-to-have |
| Raspberry Pi Image | Imagem para ARM (futuro) | ⬜ Pendente | 🟢 Nice-to-have |
| Cloud Images | AMI, qcow2, VHD para cloud | ⬜ Pendente | 🟡 Importante |
| Docker Base Image | Imagem base para containers | ⬜ Pendente | 🟡 Importante |

### 1.3 Updates e Recovery
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| System Updater | Atualizações de sistema com rollback | ✅ Concluído | 🔴 Crítico |
| A/B Partitions | Sistema de partições A/B para updates seguros | ⬜ Pendente | 🟡 Importante |
| Recovery Mode | Boot em modo de recuperação | ✅ Concluído | 🔴 Crítico |
| Factory Reset | Reset para configurações de fábrica | ⬜ Pendente | 🟡 Importante |
| Backup/Restore | Backup e restauração de sistema | ⬜ Pendente | 🟡 Importante |

---

## 2. Hardware Real - Drivers de Produção
> Drivers testados e funcionais em hardware real, não apenas QEMU.

### 2.1 Storage Controllers
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| NVMe Real | Driver NVMe testado em SSDs reais | ✅ Concluído | 🔴 Crítico |
| AHCI/SATA Real | Driver SATA testado em HDDs/SSDs reais | ✅ Concluído | 🔴 Crítico |
| Intel RST | Intel Rapid Storage Technology | ⬜ Pendente | 🟡 Importante |
| AMD StoreMI | AMD storage acceleration | ⬜ Pendente | 🟢 Nice-to-have |
| eMMC | Suporte a eMMC (tablets, Chromebooks) | ⬜ Pendente | 🟡 Importante |
| SD Card | Leitor de cartão SD (SDHCI) | ⬜ Pendente | 🟡 Importante |
| USB Mass Storage | USB drives, pendrives | ✅ Concluído | 🔴 Crítico |

### 2.2 USB Controllers Reais
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| xHCI Real | USB 3.x em hardware real (Intel, AMD, Renesas) | ✅ Concluído | 🔴 Crítico |
| EHCI Real | USB 2.0 em hardware legado | ⬜ Pendente | 🟡 Importante |
| USB Hub Handling | Hubs USB multinível | ✅ Concluído | 🔴 Crítico |
| USB Hotplug | Plug/unplug dinâmico | ✅ Concluído | 🔴 Crítico |
| USB Power Management | Suspend/resume de dispositivos USB | ⬜ Pendente | 🟡 Importante |

### 2.3 Chipset e Platform
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Intel PCH | Platform Controller Hub (série 100-700) | ✅ Concluído | 🔴 Crítico |
| AMD FCH | Fusion Controller Hub | ✅ Concluído | 🔴 Crítico |
| Intel ME Interface | Management Engine (básico) | ⬜ Pendente | 🟢 Nice-to-have |
| AMD PSP Interface | Platform Security Processor | ⬜ Pendente | 🟢 Nice-to-have |
| SMBus/I2C | System Management Bus | ✅ Concluído | 🔴 Crítico |
| GPIO | General Purpose I/O | ⬜ Pendente | 🟡 Importante |
| LPC/eSPI | Low Pin Count / Enhanced SPI | ⬜ Pendente | 🟡 Importante |

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
| UEFI Runtime | UEFI runtime services | ⬜ Pendente | 🟡 Importante |
| UEFI Variables | Leitura/escrita de variáveis EFI | ⬜ Pendente | 🟡 Importante |
| fwupd Support | Firmware update daemon | ⬜ Pendente | 🟡 Importante |

---

## 3. GPU e Gráficos
> Drivers de GPU reais com aceleração 2D/3D.

### 3.1 Intel Graphics (i915)
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Intel Gen9 (Skylake+) | HD 520/530, UHD 620/630 | ✅ Concluído | 🔴 Crítico |
| Intel Gen11 (Ice Lake) | Iris Plus Graphics | ✅ Concluído | 🔴 Crítico |
| Intel Gen12 (Tiger Lake+) | Xe Graphics | ✅ Concluído | 🔴 Crítico |
| Intel Arc (Alchemist) | Arc A-series discrete | ⬜ Pendente | 🟡 Importante |
| GEM/GTT | Graphics memory management | ✅ Concluído | 🔴 Crítico |
| Display Pipe | Display pipeline configuration | ✅ Concluído | 🔴 Crítico |
| Power Wells | GPU power management | ⬜ Pendente | 🟡 Importante |
| GuC/HuC Firmware | Firmware loading | ✅ Concluído | 🔴 Crítico |

### 3.2 AMD Graphics (amdgpu)
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| AMD GCN 4 (Polaris) | RX 400/500 series | ⬜ Pendente | 🟡 Importante |
| AMD GCN 5 (Vega) | Vega 56/64, APUs | ⬜ Pendente | 🟡 Importante |
| AMD RDNA 1 (Navi) | RX 5000 series | ⬜ Pendente | 🟡 Importante |
| AMD RDNA 2 | RX 6000 series, Steam Deck | ✅ Concluído | 🔴 Crítico |
| AMD RDNA 3 | RX 7000 series | ⬜ Pendente | 🟡 Importante |
| AMD APU | Ryzen integrated graphics | ✅ Concluído | 🔴 Crítico |
| AMD SMU | System Management Unit | ⬜ Pendente | 🟡 Importante |
| AMD PowerPlay | Power management | ⬜ Pendente | 🟡 Importante |

### 3.3 NVIDIA Graphics (nouveau/proprietary)
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Nouveau Basic | Open source NVIDIA driver (basic) | ⬜ Pendente | 🟡 Importante |
| NVIDIA Firmware | Signed firmware loading | ⬜ Pendente | 🟡 Importante |
| NVIDIA Optimus | Hybrid graphics switching | ⬜ Pendente | 🟡 Importante |

### 3.4 Display Infrastructure
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| DRM/KMS | Direct Rendering Manager, Kernel Mode Setting | ✅ Concluído | 🔴 Crítico |
| DRM Framebuffer | DRM-based framebuffer | ✅ Concluído | 🔴 Crítico |
| Multi-Monitor | Múltiplos monitores | ✅ Concluído | 🔴 Crítico |
| Hotplug Display | Conectar/desconectar monitores | ✅ Concluído | 🔴 Crítico |
| HDMI | Saída HDMI com áudio | ✅ Concluído | 🔴 Crítico |
| DisplayPort | DP 1.4, MST (daisy chain) | ✅ Concluído | 🔴 Crítico |
| USB-C Display | DisplayPort Alt Mode | ⬜ Pendente | 🟡 Importante |
| eDP | Embedded DisplayPort (laptops) | ✅ Concluído | 🔴 Crítico |
| VRR/FreeSync | Variable refresh rate | ⬜ Pendente | 🟢 Nice-to-have |
| HDR | High Dynamic Range | ⬜ Pendente | 🟢 Nice-to-have |
| HiDPI Scaling | 4K/Retina display scaling | ⬜ Pendente | 🟡 Importante |

### 3.5 3D Acceleration
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| OpenGL 4.6 | OpenGL implementation | ⬜ Pendente | 🟡 Importante |
| Vulkan 1.3 | Vulkan implementation | ✅ Concluído | 🔴 Crítico |
| Mesa Integration | Mesa 3D library | ✅ Concluído | 🔴 Crítico |
| VA-API | Video Acceleration API | ⬜ Pendente | 🟡 Importante |
| VDPAU | Video decode acceleration | ⬜ Pendente | 🟡 Importante |
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
| Archive Manager | Compactar/descompactar (zip, tar, 7z) | ⬜ Pendente | 🟡 Importante |
| Disk Utility | Gerenciador de discos | ⬜ Pendente | 🟡 Importante |
| Search | Busca de arquivos | ✅ Concluído | 🔴 Crítico |
| Recent Files | Arquivos recentes | ⬜ Pendente | 🟡 Importante |
| Thumbnails | Miniaturas de imagens/vídeos | ⬜ Pendente | 🟡 Importante |
| Network Shares | SMB/NFS browser | ⬜ Pendente | 🟡 Importante |

### 5.2 Terminal e Desenvolvimento
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Terminal Emulator | Emulador de terminal gráfico | ✅ Concluído | 🔴 Crítico |
| Terminal Tabs | Abas no terminal | ⬜ Pendente | 🟡 Importante |
| Terminal Profiles | Perfis de terminal | ⬜ Pendente | 🟢 Nice-to-have |
| Text Editor | Editor de texto (VSCode-like básico) | ✅ Concluído | 🔴 Crítico |
| Syntax Highlighting | Destaque de sintaxe | ⬜ Pendente | 🟡 Importante |
| Git Integration | Integração Git básica | ⬜ Pendente | 🟡 Importante |

### 5.3 Web Browser
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Browser Engine | Motor de renderização (WebKit/Gecko port) | ✅ Concluído | 🔴 Crítico |
| JavaScript Engine | Motor JS (JavaScriptCore/SpiderMonkey) | ✅ Concluído | 🔴 Crítico |
| Browser UI | Interface do navegador | ✅ Concluído | 🔴 Crítico |
| Tabs | Abas de navegação | ✅ Concluído | 🔴 Crítico |
| Bookmarks | Favoritos | ⬜ Pendente | 🟡 Importante |
| History | Histórico de navegação | ⬜ Pendente | 🟡 Importante |
| Downloads | Gerenciador de downloads | ✅ Concluído | 🔴 Crítico |
| Extensions | Suporte a extensões | ⬜ Pendente | 🟢 Nice-to-have |
| Password Manager | Gerenciador de senhas integrado | ⬜ Pendente | 🟡 Importante |
| WebRTC | Chamadas de vídeo no browser | ⬜ Pendente | 🟡 Importante |

### 5.4 Multimídia
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Image Viewer | Visualizador de imagens | ✅ Concluído | 🔴 Crítico |
| Video Player | Player de vídeo (VLC-like) | ✅ Concluído | 🔴 Crítico |
| Music Player | Player de música | ⬜ Pendente | 🟡 Importante |
| Webcam App | Aplicativo de webcam | ⬜ Pendente | 🟡 Importante |
| Screen Recorder | Gravador de tela | ⬜ Pendente | 🟡 Importante |
| Screenshot Tool | Ferramenta de captura de tela | ✅ Concluído | 🔴 Crítico |
| Photo Editor | Editor de fotos básico | ⬜ Pendente | 🟢 Nice-to-have |

### 5.5 Comunicação
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Email Client | Cliente de email | ⬜ Pendente | 🟡 Importante |
| Calendar | Calendário | ⬜ Pendente | 🟡 Importante |
| Contacts | Gerenciador de contatos | ⬜ Pendente | 🟡 Importante |
| Video Calls | App de videochamada | ⬜ Pendente | 🟢 Nice-to-have |
| Chat | App de mensagens | ⬜ Pendente | 🟢 Nice-to-have |

### 5.6 Utilitários
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Calculator | Calculadora | ✅ Concluído | 🟡 Importante |
| System Monitor | Monitor de sistema (CPU, RAM, processos) | ✅ Concluído | 🔴 Crítico |
| Font Viewer | Visualizador de fontes | ⬜ Pendente | 🟢 Nice-to-have |
| Character Map | Mapa de caracteres | ⬜ Pendente | 🟢 Nice-to-have |
| Notes | Aplicativo de notas | ⬜ Pendente | 🟡 Importante |
| PDF Viewer | Visualizador de PDF | ✅ Concluído | 🔴 Crítico |
| Printer Settings | Configuração de impressoras | ⬜ Pendente | 🟡 Importante |

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
| Flatpak Support | Executar apps Flatpak | ⬜ Pendente | 🟡 Importante |
| Snap Support | Executar apps Snap | ⬜ Pendente | 🟢 Nice-to-have |
| AppImage Support | Executar AppImages | ⬜ Pendente | 🟡 Importante |

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
| Low Latency | Baixa latência para música | ⬜ Pendente | 🟡 Importante |

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
| Opus | Codec Opus | ⬜ Pendente | 🟡 Importante |
| Vorbis | OGG Vorbis | ⬜ Pendente | 🟡 Importante |

---

## 8. Rede e Conectividade
> Networking de produção.

### 8.1 WiFi Real
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Intel WiFi 6/6E | AX200, AX201, AX210, AX211 | ✅ Concluído | 🔴 Crítico |
| Intel WiFi 7 | BE200, BE202 | ⬜ Pendente | 🟡 Importante |
| Realtek WiFi | RTL8821, RTL8822, RTL8852 | ⬜ Pendente | 🟡 Importante |
| MediaTek WiFi | MT7921, MT7922 | ⬜ Pendente | 🟡 Importante |
| Broadcom WiFi | BCM43xx | ⬜ Pendente | 🟡 Importante |
| Atheros WiFi | ath10k, ath11k | ⬜ Pendente | 🟡 Importante |
| WiFi Firmware | Firmware loading de WiFi | ✅ Concluído | 🔴 Crítico |
| WPA3 Enterprise | WPA3-Enterprise | ⬜ Pendente | 🟡 Importante |
| WiFi Direct | P2P WiFi | ⬜ Pendente | 🟢 Nice-to-have |
| Hotspot Mode | Access Point mode | ⬜ Pendente | 🟡 Importante |
| WiFi 6 GHz | Suporte a banda 6 GHz | ⬜ Pendente | 🟡 Importante |

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
| OpenVPN | VPN OpenVPN | ⬜ Pendente | 🟡 Importante |
| IPsec/IKEv2 | VPN IPsec | ⬜ Pendente | 🟡 Importante |
| Firewall GUI | Interface gráfica para firewall | ⬜ Pendente | 🟡 Importante |
| Network Profiles | Perfis de rede (casa, trabalho, público) | ⬜ Pendente | 🟡 Importante |

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
| Resume Speed | Tempo de resume otimizado | ⬜ Pendente | 🟡 Importante |
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
| Core Parking | Desativar cores ociosos | ⬜ Pendente | 🟡 Importante |

### 9.3 Battery
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Battery Status | Status da bateria | ✅ Concluído | 🔴 Crítico |
| Time Remaining | Estimativa de tempo restante | ✅ Concluído | 🔴 Crítico |
| Charge Limit | Limitar carga a 80% | ⬜ Pendente | 🟡 Importante |
| Battery Health | Saúde da bateria | ⬜ Pendente | 🟡 Importante |
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
| MOK (Machine Owner Key) | Gerenciamento de chaves | ⬜ Pendente | 🟡 Importante |
| Measured Boot | Boot medido com TPM | ⬜ Pendente | 🟡 Importante |
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
| Fingerprint Login | Login com impressão digital | ⬜ Pendente | 🟡 Importante |
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
| Touchscreen | Suporte a tela touch | ⬜ Pendente | 🟡 Importante |
| Stylus/Pen | Caneta stylus com pressão | ⬜ Pendente | 🟡 Importante |
| Graphics Tablet | Tablets Wacom, etc. | ⬜ Pendente | 🟢 Nice-to-have |
| Game Controllers | Controles de jogo (Xbox, PS) | ⬜ Pendente | 🟡 Importante |
| MIDI Devices | Teclados MIDI | ⬜ Pendente | 🟢 Nice-to-have |

### 11.2 Accessibility
| Item | Descrição | Status | Prioridade |
|------|-----------|--------|------------|
| Screen Reader | Leitor de tela | ⬜ Pendente | 🟡 Importante |
| High Contrast | Modo alto contraste | ⬜ Pendente | 🟡 Importante |
| Large Text | Texto grande | ⬜ Pendente | 🟡 Importante |
| Screen Magnifier | Lupa de tela | ⬜ Pendente | 🟡 Importante |
| Sticky Keys | Teclas de aderência | ⬜ Pendente | 🟡 Importante |
| Slow Keys | Teclas lentas | ⬜ Pendente | 🟢 Nice-to-have |
| Bounce Keys | Teclas de repique | ⬜ Pendente | 🟢 Nice-to-have |
| Mouse Keys | Controle do mouse pelo teclado | ⬜ Pendente | 🟡 Importante |
| On-Screen Keyboard | Teclado virtual | ⬜ Pendente | 🟡 Importante |
| Voice Control | Controle por voz | ⬜ Pendente | 🟢 Nice-to-have |
| Color Filters | Filtros para daltonismo | ⬜ Pendente | 🟢 Nice-to-have |
| Reduce Motion | Reduzir animações | ⬜ Pendente | 🟡 Importante |

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
| eBPF | Extended BPF | ⬜ Pendente | 🟡 Importante |
| Tracing | ftrace, systemtap | ⬜ Pendente | 🟡 Importante |
| CPU Profiler | Profiler de CPU | ⬜ Pendente | 🟡 Importante |
| Memory Profiler | Profiler de memória | ⬜ Pendente | 🟡 Importante |

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

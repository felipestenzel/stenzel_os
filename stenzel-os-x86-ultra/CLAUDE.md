# Stenzel OS - Instruções para o Claude

## Contexto do Projeto
Este é o **Stenzel OS**, um sistema operacional x86_64 escrito em Rust do zero. O objetivo é criar um OS completo capaz de rodar em hardware real com interface gráfica, rede, WiFi, e capacidade de instalar software.

---

## REGRAS OBRIGATÓRIAS DE TRABALHO

### 1. Fluxo de Trabalho: Lotes de 50 Itens

**SEMPRE** trabalhar em lotes de até 50 itens por sessão, seguindo esta ordem de prioridade:

1. **🔴 Crítico** - Fazer TODOS os críticos primeiro
2. **🟡 Importante** - Depois os importantes
3. **🟢 Nice-to-have** - Por último os opcionais

**Processo para cada item:**
1. Implementar o item completamente
2. Compilar e verificar: `cargo build --release -p stenzel_kernel --target x86_64-unknown-none`
3. Marcar como ✅ no ROADMAP_V2.md
4. Adicionar entrada no Histórico de Atualizações
5. Passar para o próximo item

**NÃO pular para o próximo item até que o atual esteja 100% completo e o build passe!**

### ⚠️ REGRA CRÍTICA: NUNCA PARAR PARA PERGUNTAR

**É ESTRITAMENTE PROIBIDO:**
- Parar para perguntar "Posso continuar?"
- Parar para perguntar "Devo prosseguir?"
- Parar para pedir confirmação do usuário
- Fazer resumos parciais e aguardar resposta

**VOCÊ SÓ PARA QUANDO:**
- Completar 100% dos 50 itens do lote
- Cada item testado (build passou)
- Cada item documentado no ROADMAP_V2.md

**Se encontrar um erro:** Corrija e continue.
**Se precisar de uma decisão técnica:** Tome a melhor decisão e continue.
**Se um item já existe:** Verifique, marque como completo, documente e continue.

### 2. Consultar o ROADMAP_V2.md

Antes de começar qualquer tarefa:
- **Leia o arquivo `ROADMAP_V2.md`** para entender o estado atual
- Verifique quais itens estão ✅ (concluídos), 🔄 (em progresso) ou ⬜ (pendentes)
- Identifique o próximo lote de itens 🔴 Críticos pendentes
- Identifique dependências entre tarefas

### 3. Atualizar o ROADMAP_V2.md Após CADA Conclusão

**OBRIGATÓRIO** após completar cada item:

1. **Marcar o item como ✅** na tabela correspondente
2. **Adicionar entrada no Histórico de Atualizações** com o formato:

```markdown
| YYYY-MM-DD | [Seção] Item implementado (arquivo.rs: descrição técnica detalhada do que foi feito, structs, enums, funções principais, ~X linhas) |
```

**Exemplo:**
```markdown
| 2026-01-17 | [Instalador] Live USB Boot implementado (installer/live.rs: LiveUsbBuilder struct com create_iso()/create_usb(), initramfs generation, squashfs compression, GRUB/systemd-boot config, ~800 linhas) |
```

### 4. Formato do Histórico de Atualizações

O Histórico deve conter:
- **Data** no formato YYYY-MM-DD
- **Seção** entre colchetes [Nome da Seção]
- **Nome do item** implementado
- **Arquivo(s)** criado(s) ou modificado(s)
- **Descrição técnica** detalhada incluindo:
  - Structs e enums criados
  - Funções principais implementadas
  - Número aproximado de linhas
  - Integrações com outros módulos

### 5. Adicionar Novos Itens Descobertos

Durante o desenvolvimento, se encontrar algo que:
- Não estava no ROADMAP mas é necessário → **Adicione na seção apropriada**
- Precisa de mais detalhes → **Expanda a descrição**
- Tem nova prioridade → **Atualize a prioridade**
- É bloqueador de outro item → **Documente a dependência**

### 6. Verificação de Build

**SEMPRE** após cada implementação:
```bash
cargo build --release -p stenzel_kernel --target x86_64-unknown-none
```

- Se houver erros → Corrigir ANTES de marcar como concluído
- Se houver warnings → Aceitável, mas documentar se relevante
- Só marcar ✅ após build bem-sucedido

---

## Arquitetura do Projeto

```
stenzel-os-x86-ultra/
├── kernel/           # Kernel em Rust
│   └── src/
│       ├── arch/     # Código específico x86_64
│       ├── mm/       # Gerenciamento de memória
│       ├── sched/    # Scheduler
│       ├── syscall/  # System calls
│       ├── drivers/  # Drivers de dispositivos
│       ├── fs/       # Sistemas de arquivo
│       ├── net/      # Networking
│       ├── gui/      # Interface gráfica
│       ├── compat/   # Camadas de compatibilidade
│       └── ...
├── userland/         # Programas userspace
│   ├── init/         # Processo init (PID 1)
│   ├── sh/           # Shell
│   └── libc/         # Biblioteca C
├── ROADMAP.md        # Roadmap V1 (completado)
├── ROADMAP_V2.md     # Roadmap V2 (ATUAL - usar este!)
└── CLAUDE.md         # Este arquivo
```

---

## Comandos Úteis

```bash
# Compilar o kernel (OBRIGATÓRIO após cada implementação)
cargo build --release -p stenzel_kernel --target x86_64-unknown-none

# Rodar no QEMU para teste
cargo run --release --bin stenzel

# Limpar build cache (se necessário forçar recompilação)
rm -rf target/x86_64-unknown-none/

# Ver warnings detalhados
cargo build --release -p stenzel_kernel --target x86_64-unknown-none 2>&1 | head -100
```

---

## Padrões de Código

- **Linguagem:** Rust (no_std para kernel)
- **Imports obrigatórios para collections:**
  ```rust
  use alloc::vec::Vec;
  use alloc::vec;  // Para macro vec![]
  use alloc::string::String;
  use alloc::collections::BTreeMap;
  ```
- **Logs/Debug:** Usar `crate::kprintln!()` no kernel
- **Documentação:** Doc comments `///` para funções públicas
- **Módulos:** Adicionar `pub mod nome;` no mod.rs pai
- **Init:** Criar função `pub fn init()` para inicialização

---

## Estado Atual

### Completado (ROADMAP V1):
- ✅ Boot (BIOS e UEFI)
- ✅ Memória virtual, heap, paging
- ✅ Scheduler preemptivo com SMP
- ✅ Syscalls (200+ implementados)
- ✅ VFS com ext2, ext4, FAT32, NTFS, tmpfs, procfs, devfs
- ✅ Networking (TCP/IP, WiFi, TLS)
- ✅ GUI com compositor, transparência, animações
- ✅ Compatibilidade Windows/Linux/POSIX
- ✅ Containers, cgroups, namespaces
- ✅ USB, NVMe, AHCI, Bluetooth, Audio

### Em Progresso (ROADMAP V2):
- ⬜ Instalador para hardware real
- ⬜ Drivers de GPU reais (Intel/AMD)
- ⬜ Desktop Environment completo
- ⬜ Aplicações essenciais
- ⬜ Package Manager
- ⬜ Testes em hardware real

---

## Checklist de Início de Sessão

Ao iniciar uma sessão de trabalho:

1. [ ] Ler o ROADMAP_V2.md
2. [ ] Identificar próximos itens 🔴 Críticos pendentes
3. [ ] Planejar lote de até 50 itens
4. [ ] Começar pelo primeiro item crítico
5. [ ] Implementar → Build → Marcar ✅ → Histórico → Próximo

---

## Checklist de Fim de Item

Após completar cada item:

1. [ ] Código implementado e funcional
2. [ ] Build passa sem erros
3. [ ] Item marcado como ✅ no ROADMAP_V2.md
4. [ ] Entrada adicionada no Histórico de Atualizações
5. [ ] Métricas de progresso atualizadas (se aplicável)

---

## Exemplo de Sessão de Trabalho

```
Sessão: 2026-01-18

Lote planejado (50 itens críticos da Fase 1):
1. [1.1] Live USB Boot
2. [1.1] Detecção de Hardware
3. [1.1] Particionamento
...

Item 1: Live USB Boot
- Criar kernel/src/installer/live.rs
- Implementar LiveUsbBuilder
- Build: ✅ Passou
- ROADMAP_V2.md: ✅ Marcado
- Histórico: ✅ Adicionado

Item 2: Detecção de Hardware
- Criar kernel/src/installer/hwdetect.rs
...
```

---

## Lembrete Final

**A cada item completado:**
1. ✅ Build passou
2. ✅ ROADMAP_V2.md atualizado (item marcado)
3. ✅ Histórico de Atualizações atualizado (entrada detalhada)

**NÃO avançar para próximo item sem completar estes 3 passos!**

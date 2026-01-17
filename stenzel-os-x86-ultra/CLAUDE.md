# Stenzel OS - Instruções para o Claude

## Contexto do Projeto
Este é o **Stenzel OS**, um sistema operacional x86_64 escrito em Rust do zero. O objetivo é criar um OS completo capaz de rodar em hardware real com interface gráfica, rede, WiFi, e capacidade de instalar software.

## Regras Obrigatórias

### 1. Sempre Consultar o ROADMAP
Antes de começar qualquer tarefa:
- **Leia o arquivo `ROADMAP.md`** para entender o estado atual do projeto
- Verifique quais itens estão ✅ (concluídos), 🔄 (em progresso) ou ⬜ (pendentes)
- Identifique dependências entre tarefas

### 2. Atualizar o ROADMAP Após Cada Conclusão
Quando completar uma tarefa:
- Marque o item como ✅ no ROADMAP.md
- Adicione a data no "Histórico de Atualizações"
- Se descobrir sub-tarefas que não estavam listadas, adicione-as

### 3. Adicionar Novos Itens Descobertos
Durante o desenvolvimento, se encontrar algo que:
- Não estava no ROADMAP mas é necessário → **Adicione**
- Precisa de mais detalhes → **Expanda a descrição**
- Tem nova prioridade → **Atualize a prioridade**

### 4. Entender o Próximo Passo
Sempre pergunte ou sugira:
- Qual é o próximo item de **alta prioridade** a ser feito?
- Há algum bloqueio ou dependência?
- O que o usuário quer atacar agora?

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
│       └── ...
├── userland/         # Programas userspace
│   ├── init/         # Processo init (PID 1)
│   └── sh/           # Shell
├── ROADMAP.md        # Plano mestre do projeto
└── CLAUDE.md         # Este arquivo
```

## Comandos Úteis

```bash
# Compilar o kernel
cargo build --release -p stenzel_kernel --target x86_64-unknown-none

# Rodar no QEMU
cargo run --release --bin stenzel

# Limpar build cache (forçar recompilação)
rm -rf target/x86_64-unknown-none/
```

## Padrões de Código

- **Linguagem:** Rust (kernel e userspace)
- **Comentários:** Em português ou inglês
- **Logs/Debug:** Usar `crate::kprintln!()` no kernel
- **Testes:** Rodar sempre no QEMU antes de considerar completo

## Estado Atual (Resumo)

O que funciona:
- Boot (BIOS e UEFI)
- Memória virtual e heap
- Scheduler preemptivo com context switch
- Syscalls básicos (fork, execve, exit, wait, read, write)
- Shell básico rodando
- Teclado PS/2
- VirtIO-blk e ext2 (leitura)

Próximos passos prioritários:
1. Limpar debug output verboso
2. Corrigir blocking I/O no shell
3. Implementar pipes
4. procfs/sysfs
5. APIC (para hardware real e SMP)

## Lembrete Final

**Sempre que iniciar uma sessão de trabalho:**
1. Leia o ROADMAP.md
2. Pergunte ao usuário o que ele quer fazer
3. Verifique se há itens bloqueados
4. Ao terminar, atualize o ROADMAP.md

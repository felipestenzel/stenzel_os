# Memória

## Inicialização
1. Criar mapper (OffsetPageTable) a partir do CR3 e `physical_memory_offset`.
2. Inicializar alocador físico **early** (linear) para mapear heap e estruturas mínimas.
3. Mapear heap do kernel.
4. Inicializar heap allocator (LockedHeap).
5. Construir alocador físico definitivo (BitmapFrameAllocator) marcando regiões usadas/Reservadas.

## Alocador físico (bitmap)
- Representa cada frame 4KiB com 1 bit.
- Marca frames `Usable` como livres e o restante como ocupado.
- Mantém estatísticas e permite debug.

## Paging
- Mapear heap: páginas 4KiB com flags PRESENT | WRITABLE
- No futuro:
  - mapear user pages com USER_ACCESSIBLE
  - copy-on-write, etc.

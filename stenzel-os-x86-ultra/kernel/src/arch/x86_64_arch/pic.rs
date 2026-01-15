    #![allow(dead_code)]

    use x86_64::instructions::port::Port;

    const PIC1_CMD: u16 = 0x20;
    const PIC1_DATA: u16 = 0x21;
    const PIC2_CMD: u16 = 0xA0;
    const PIC2_DATA: u16 = 0xA1;

    const ICW1_INIT: u8 = 0x10;
    const ICW1_ICW4: u8 = 0x01;
    const ICW4_8086: u8 = 0x01;

    /// Inicializa PIC 8259 (legacy). Em PCs modernos o ideal é migrar para APIC/IOAPIC,
    /// mas isso nos permite subir o kernel rapidamente em QEMU/hardware real.
    ///
    /// # Safety
    /// Acesso a portas I/O e reprogramação de controladores de interrupção.
    pub unsafe fn init(offset1: u8, offset2: u8) {
        crate::kprintln!("pic: init offsets={}, {}", offset1, offset2);
        let mut pic1_cmd = Port::<u8>::new(PIC1_CMD);
        let mut pic1_data = Port::<u8>::new(PIC1_DATA);
        let mut pic2_cmd = Port::<u8>::new(PIC2_CMD);
        let mut pic2_data = Port::<u8>::new(PIC2_DATA);

        let a1 = pic1_data.read();
        let a2 = pic2_data.read();
        crate::kprintln!("pic: initial masks: master={:#04x}, slave={:#04x}", a1, a2);

        pic1_cmd.write(ICW1_INIT | ICW1_ICW4);
        io_wait();
        pic2_cmd.write(ICW1_INIT | ICW1_ICW4);
        io_wait();

        pic1_data.write(offset1);
        io_wait();
        pic2_data.write(offset2);
        io_wait();

        pic1_data.write(4); // PIC2 em IRQ2
        io_wait();
        pic2_data.write(2);
        io_wait();

        pic1_data.write(ICW4_8086);
        io_wait();
        pic2_data.write(ICW4_8086);
        io_wait();

        // Restaura máscaras, depois habilita timer/keyboard explicitamente.
        pic1_data.write(a1);
        pic2_data.write(a2);

        // Habilita IRQ0 (timer) e IRQ1 (keyboard) no master.
        unmask_irq(0);
        unmask_irq(1);
    }

    #[inline]
    unsafe fn io_wait() {
        let mut port = Port::<u8>::new(0x80);
        port.write(0);
    }

    /// Habilita IRQ (0..15)
    pub fn unmask_irq(irq: u8) {
        unsafe {
            if irq < 8 {
                let mut data = Port::<u8>::new(PIC1_DATA);
                let mask = data.read();
                let new_mask = mask & !(1 << irq);
                crate::kprintln!("pic: unmask IRQ{} mask={:#04x} -> {:#04x}", irq, mask, new_mask);
                data.write(new_mask);
            } else {
                let mut data = Port::<u8>::new(PIC2_DATA);
                let mask = data.read();
                let new_mask = mask & !(1 << (irq - 8));
                crate::kprintln!("pic: unmask IRQ{} mask={:#04x} -> {:#04x}", irq, mask, new_mask);
                data.write(new_mask);
            }
        }
    }

    /// Desabilita IRQ (0..15)
    pub fn mask_irq(irq: u8) {
        unsafe {
            if irq < 8 {
                let mut data = Port::<u8>::new(PIC1_DATA);
                let mask = data.read();
                data.write(mask | (1 << irq));
            } else {
                let mut data = Port::<u8>::new(PIC2_DATA);
                let mask = data.read();
                data.write(mask | (1 << (irq - 8)));
            }
        }
    }

    /// Notifica fim de interrupção (EOI).
    pub fn eoi(irq: u8) {
        unsafe {
            if irq >= 8 {
                let mut pic2_cmd = Port::<u8>::new(PIC2_CMD);
                pic2_cmd.write(0x20);
            }
            let mut pic1_cmd = Port::<u8>::new(PIC1_CMD);
            pic1_cmd.write(0x20);
        }
    }

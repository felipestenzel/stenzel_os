    use x86_64::instructions::port::Port;

    const PIT_CH0: u16 = 0x40;
    const PIT_CMD: u16 = 0x43;

    const PIT_FREQUENCY_HZ: u32 = 1_193_182;

    /// Inicializa PIT para gerar IRQ0 em `hz`.
    pub fn init(hz: u32) {
        let divisor = (PIT_FREQUENCY_HZ / hz).max(1).min(0xFFFF) as u16;
        crate::kprintln!("pit: init hz={} divisor={} ({:#06x})", hz, divisor, divisor);
        unsafe {
            let mut cmd = Port::<u8>::new(PIT_CMD);
            let mut ch0 = Port::<u8>::new(PIT_CH0);

            // channel 0, lobyte/hibyte, mode 2 (rate generator), binary
            cmd.write(0b0011_0100);
            ch0.write((divisor & 0xFF) as u8);
            ch0.write((divisor >> 8) as u8);
        }
        crate::kprintln!("pit: configured");
    }

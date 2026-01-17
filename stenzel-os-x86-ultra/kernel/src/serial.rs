    use core::fmt;
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    // COM1 base
    const COM1: u16 = 0x3F8;

    pub struct Serial {
        data: Port<u8>,
        int_en: Port<u8>,
        fifo_ctrl: Port<u8>,
        line_ctrl: Port<u8>,
        modem_ctrl: Port<u8>,
        line_status: Port<u8>,
    }

    impl Serial {
        /// # Safety
        /// Acesso direto a portas I/O.
        pub const unsafe fn new(base: u16) -> Self {
            Self {
                data: Port::new(base),
                int_en: Port::new(base + 1),
                fifo_ctrl: Port::new(base + 2),
                line_ctrl: Port::new(base + 3),
                modem_ctrl: Port::new(base + 4),
                line_status: Port::new(base + 5),
            }
        }

        /// Inicializa UART 16550 (115200 8N1, FIFO ligado).
        ///
        /// # Safety
        /// Acesso a portas I/O.
        pub unsafe fn init(&mut self) {
            // Desabilita interrupções
            self.int_en.write(0x00);

            // Habilita DLAB
            self.line_ctrl.write(0x80);

            // Divisor para 115200: 1 (LSB=1, MSB=0)
            self.data.write(0x01);
            self.int_en.write(0x00);

            // 8 bits, sem paridade, 1 stop
            self.line_ctrl.write(0x03);

            // Habilita FIFO, limpa, threshold 14 bytes
            self.fifo_ctrl.write(0xC7);

            // IRQs, RTS/DSR set
            self.modem_ctrl.write(0x0B);
        }

        #[inline]
        fn can_write(&mut self) -> bool {
            unsafe { self.line_status.read() & 0x20 != 0 }
        }

        #[inline]
        fn can_read(&mut self) -> bool {
            unsafe { self.line_status.read() & 0x01 != 0 }
        }

        pub fn write_byte(&mut self, b: u8) {
            while !self.can_write() {
                core::hint::spin_loop();
            }
            unsafe { self.data.write(b) };
        }

        pub fn read_byte(&mut self) -> Option<u8> {
            if !self.can_read() {
                return None;
            }
            Some(unsafe { self.data.read() })
        }
    }

    impl fmt::Write for Serial {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            for &b in s.as_bytes() {
                if b == b'\n' {
                    self.write_byte(b'\r');
                }
                self.write_byte(b);
            }
            Ok(())
        }
    }

    static SERIAL: Mutex<Option<Serial>> = Mutex::new(None);

    pub fn init() {
        let mut guard = SERIAL.lock();
        if guard.is_some() {
            return;
        }
        let mut s = unsafe { Serial::new(COM1) };
        unsafe { s.init() };
        *guard = Some(s);
    }

    pub fn read_byte() -> Option<u8> {
        let mut guard = SERIAL.lock();
        guard.as_mut().and_then(|s| s.read_byte())
    }

    /// Verifica se há dados disponíveis para leitura SEM consumir.
    /// Usado por poll/select.
    pub fn has_data() -> bool {
        let mut guard = SERIAL.lock();
        guard.as_mut().map_or(false, |s| s.can_read())
    }

    pub fn print(args: fmt::Arguments) {
        use core::fmt::Write;
        let mut guard = SERIAL.lock();
        if let Some(s) = guard.as_mut() {
            let _ = s.write_fmt(args);
        }
    }

    /// Escreve um byte diretamente na serial.
    /// Usado por syscalls para output de processos user.
    pub fn write_byte(b: u8) {
        let mut guard = SERIAL.lock();
        if let Some(s) = guard.as_mut() {
            s.write_byte(b);
        }
    }

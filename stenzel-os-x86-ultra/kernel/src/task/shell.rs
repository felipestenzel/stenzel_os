    use alloc::string::{String, ToString};
    use alloc::vec::Vec;

    use crate::security;
    use crate::util::{KError, KResult};

    pub fn shell_thread(_arg: usize) {
        let mut cred = crate::task::current_cred();
        let mut line = String::new();

        crate::kprintln!("ksh: Stenzel OS kernel shell");
        crate::kprintln!("digite 'help' para comandos.\n");

        loop {
            prompt(&cred);

            line.clear();
            read_line(&mut line);

            if line.trim().is_empty() {
                continue;
            }

            match exec_line(&line, &mut cred) {
                Ok(()) => {}
                Err(e) => crate::kprintln!("erro: {:?}", e),
            }
        }
    }

    fn prompt(cred: &security::Cred) {
        let user = if cred.uid.0 == 0 { "root" } else { "user" };
        crate::kprint!("{}@stenzel:/# ", user);
    }

    fn read_line(out: &mut String) {
        loop {
            // Usa console unificado (teclado PS/2 + serial)
            if let Some(b) = crate::console::read_byte() {
                match b {
                    b'\r' | b'\n' => {
                        crate::kprintln!("");
                        break;
                    }
                    8 | 127 => {
                        // backspace
                        if !out.is_empty() {
                            out.pop();
                            crate::kprint!("\x08 \x08");
                        }
                    }
                    3 => {
                        // Ctrl+C - cancela linha
                        crate::kprintln!("^C");
                        out.clear();
                        break;
                    }
                    b => {
                        if b.is_ascii_graphic() || b == b' ' {
                            out.push(b as char);
                            crate::kprint!("{}", b as char);
                        }
                    }
                }
            } else {
                // Sem input: libera CPU.
                crate::task::yield_now();
            }
        }
    }

    fn exec_line(line: &str, cred: &mut security::Cred) -> KResult<()> {
        let parts = split_command(line);
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0].as_str() {
            "help" => help(),
            "whoami" => whoami(cred),
            "su" => {
                if parts.len() < 2 {
                    return Err(KError::Invalid);
                }
                su(&parts[1], cred)
            }
            "ls" => {
                let path = parts.get(1).map(|s| s.as_str()).unwrap_or("/");
                ls(path, cred)
            }
            "cat" => {
                let path = parts.get(1).ok_or(KError::Invalid)?;
                cat(path, cred)
            }
            "mkdir" => {
                let path = parts.get(1).ok_or(KError::Invalid)?;
                mkdir(path, cred)
            }
            "write" => {
                if parts.len() < 3 {
                    return Err(KError::Invalid);
                }
                let path = &parts[1];
                let text = parts[2..].join(" ");
                write_file(path, &text, cred)
            }
            "meminfo" => meminfo(),
            "ticks" => ticks(),
            _ => {
                crate::kprintln!("comando desconhecido: {}", parts[0]);
                Ok(())
            }
        }
    }

    fn split_command(line: &str) -> Vec<String> {
        line.split_whitespace().map(|s| s.to_string()).collect()
    }

    fn help() -> KResult<()> {
        crate::kprintln!("comandos:");
        crate::kprintln!("  help                 - mostra ajuda");
        crate::kprintln!("  whoami               - mostra usuário atual");
        crate::kprintln!("  su <user>            - troca usuário (root/user)");
        crate::kprintln!("  ls [path]            - lista diretório");
        crate::kprintln!("  cat <path>           - mostra arquivo");
        crate::kprintln!("  mkdir <path>         - cria diretório (mkdir -p)");
        crate::kprintln!("  write <path> <texto> - escreve arquivo (sobrescreve)");
        crate::kprintln!("  meminfo              - stats do alocador físico");
        crate::kprintln!("  ticks                - ticks do timer");
        Ok(())
    }

    fn whoami(cred: &security::Cred) -> KResult<()> {
        crate::kprintln!("uid={} gid={}", cred.uid.0, cred.gid.0);
        Ok(())
    }

    fn su(user: &str, cred: &mut security::Cred) -> KResult<()> {
        let db = security::user_db();
        let new = db.login(user)?;
        *cred = new.clone();
        crate::task::set_current_cred(new);
        Ok(())
    }

    fn ls(path: &str, cred: &security::Cred) -> KResult<()> {
        let mut vfs = crate::fs::vfs_lock();
        let entries = vfs.list_dir(path, cred)?;
        for e in entries {
            let kind = match e.kind {
                crate::fs::InodeKind::Dir => "d",
                crate::fs::InodeKind::File => "f",
                crate::fs::InodeKind::Symlink => "l",
                _ => "?",
            };
            crate::kprintln!("{} {}", kind, e.name);
        }
        Ok(())
    }

    fn cat(path: &str, cred: &security::Cred) -> KResult<()> {
        let mut vfs = crate::fs::vfs_lock();
        let data = vfs.read_file(path, cred)?;
        if let Ok(s) = core::str::from_utf8(&data) {
            crate::kprintln!("{}", s);
        } else {
            crate::kprintln!("{:02x?}", data);
        }
        Ok(())
    }

    fn mkdir(path: &str, cred: &security::Cred) -> KResult<()> {
        let mut vfs = crate::fs::vfs_lock();
        vfs.mkdir_all(path, cred, crate::fs::Mode::from_octal(0o755))?;
        Ok(())
    }

    fn write_file(path: &str, text: &str, cred: &security::Cred) -> KResult<()> {
        let mut vfs = crate::fs::vfs_lock();
        vfs.write_file(path, cred, crate::fs::Mode::from_octal(0o644), text.as_bytes())?;
        Ok(())
    }

    fn meminfo() -> KResult<()> {
        let stats = crate::mm::frame_allocator_stats();
        crate::kprintln!("frames: total={} used={} free={}", stats.total, stats.used, stats.free);
        Ok(())
    }

    fn ticks() -> KResult<()> {
        let t = crate::arch::x86_64_arch::interrupts::ticks();
        crate::kprintln!("ticks={}", t);
        Ok(())
    }

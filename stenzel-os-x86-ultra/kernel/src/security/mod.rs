    #![allow(dead_code)]

    use alloc::collections::BTreeMap;
    use alloc::string::String;

    use spin::Once;
    use bitflags::bitflags;

    use crate::util::{KError, KResult};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Uid(pub u32);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Gid(pub u32);

    bitflags! {
        /// Capabilities do processo (modelo simplificado).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Caps: u32 {
            const CAP_FS    = 1 << 0;  // Acesso ao filesystem
            const CAP_PROC  = 1 << 1;  // Gerenciamento de processos
            const CAP_NET   = 1 << 2;  // Operações de rede
            const CAP_ADMIN = 1 << 3;  // Operações administrativas
            const ALL = Self::CAP_FS.bits() | Self::CAP_PROC.bits() | Self::CAP_NET.bits() | Self::CAP_ADMIN.bits();
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Cred {
        pub uid: Uid,
        pub gid: Gid,
        pub caps: Caps,
    }

    impl Cred {
        pub fn is_root(&self) -> bool {
            self.uid.0 == 0
        }

        pub fn root() -> Self {
            Cred {
                uid: Uid(0),
                gid: Gid(0),
                caps: Caps::ALL,
            }
        }

        pub fn user(uid: u32, gid: u32) -> Self {
            Cred {
                uid: Uid(uid),
                gid: Gid(gid),
                caps: Caps::empty(),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct User {
        pub name: String,
        pub uid: Uid,
        pub gid: Gid,
        pub home: String,
        pub shell: String,
    }

    pub struct UserDb {
        users: BTreeMap<String, User>,
    }

    impl UserDb {
        pub fn new() -> Self {
            Self {
                users: BTreeMap::new(),
            }
        }

        pub fn add(&mut self, user: User) {
            self.users.insert(user.name.clone(), user);
        }

        pub fn get(&self, name: &str) -> Option<&User> {
            self.users.get(name)
        }

        pub fn login(&self, name: &str) -> KResult<Cred> {
            let u = self.get(name).ok_or(KError::NotFound)?;
            let caps = if u.uid.0 == 0 { Caps::ALL } else { Caps::empty() };
            Ok(Cred {
                uid: u.uid,
                gid: u.gid,
                caps,
            })
        }

        pub fn passwd_text(&self) -> String {
            let mut out = String::new();
            for u in self.users.values() {
                // formato simples: name:uid:gid:home:shell
                use alloc::fmt::Write;
                let _ = writeln!(
                    out,
                    "{}:{}:{}:{}:{}",
                    u.name,
                    u.uid.0,
                    u.gid.0,
                    u.home,
                    u.shell
                );
            }
            out
        }
    }

    static USER_DB: Once<UserDb> = Once::new();

    pub fn init() {
        USER_DB.call_once(|| {
            let mut db = UserDb::new();

            db.add(User {
                name: "root".into(),
                uid: Uid(0),
                gid: Gid(0),
                home: "/root".into(),
                shell: "/bin/ksh".into(),
            });

            db.add(User {
                name: "user".into(),
                uid: Uid(1000),
                gid: Gid(1000),
                home: "/home/user".into(),
                shell: "/bin/ksh".into(),
            });

            db
        });
    }

    pub fn user_db() -> &'static UserDb {
        USER_DB.call_once(|| panic!("USER_DB não inicializado"))
    }

    #![allow(dead_code)]

    use alloc::collections::BTreeMap;
    use alloc::string::String;

    use spin::Once;
    use bitflags::bitflags;

    use crate::util::{KError, KResult};

    pub mod caps;
    pub mod seccomp;
    pub mod keyring;
    pub use caps::{ProcessCaps, CapSet, Cap, FileCaps};
    pub use seccomp::SeccompState;

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

    /// Process credentials with real, effective, and saved UIDs/GIDs
    /// Following Unix semantics:
    /// - Real UID/GID: the actual user who started the process
    /// - Effective UID/GID: used for permission checks (set by setuid binaries)
    /// - Saved UID/GID: for temporarily dropping/regaining privileges
    #[derive(Debug, Clone, Copy)]
    pub struct Cred {
        /// Real user ID (who actually started the process)
        pub uid: Uid,
        /// Real group ID
        pub gid: Gid,
        /// Effective user ID (used for permission checks)
        pub euid: Uid,
        /// Effective group ID
        pub egid: Gid,
        /// Saved set-user-ID (for seteuid to restore)
        pub suid: Uid,
        /// Saved set-group-ID
        pub sgid: Gid,
        /// Supplementary group IDs
        pub groups: [Gid; MAX_GROUPS],
        /// Number of supplementary groups
        pub ngroups: usize,
        /// Process capabilities
        pub caps: Caps,
    }

    /// Maximum number of supplementary groups
    pub const MAX_GROUPS: usize = 32;

    impl Cred {
        /// Check if process has effective root privileges
        pub fn is_root(&self) -> bool {
            self.euid.0 == 0
        }

        /// Check if process has real root identity
        pub fn is_real_root(&self) -> bool {
            self.uid.0 == 0
        }

        /// Create root credentials
        pub fn root() -> Self {
            Cred {
                uid: Uid(0),
                gid: Gid(0),
                euid: Uid(0),
                egid: Gid(0),
                suid: Uid(0),
                sgid: Gid(0),
                groups: [Gid(0); MAX_GROUPS],
                ngroups: 0,
                caps: Caps::ALL,
            }
        }

        /// Create user credentials
        pub fn user(uid: u32, gid: u32) -> Self {
            Cred {
                uid: Uid(uid),
                gid: Gid(gid),
                euid: Uid(uid),
                egid: Gid(gid),
                suid: Uid(uid),
                sgid: Gid(gid),
                groups: [Gid(0); MAX_GROUPS],
                ngroups: 0,
                caps: Caps::empty(),
            }
        }

        /// Create credentials with specific effective IDs (for setuid binaries)
        pub fn with_setuid(uid: u32, gid: u32, euid: u32, egid: u32) -> Self {
            let caps = if euid == 0 { Caps::ALL } else { Caps::empty() };
            Cred {
                uid: Uid(uid),
                gid: Gid(gid),
                euid: Uid(euid),
                egid: Gid(egid),
                suid: Uid(euid),
                sgid: Gid(egid),
                groups: [Gid(0); MAX_GROUPS],
                ngroups: 0,
                caps,
            }
        }

        /// Set effective UID
        /// Returns error if not permitted
        pub fn setuid(&mut self, new_uid: u32) -> KResult<()> {
            // Root can set to any UID
            if self.euid.0 == 0 {
                self.uid = Uid(new_uid);
                self.euid = Uid(new_uid);
                self.suid = Uid(new_uid);
                // Drop caps if becoming non-root
                if new_uid != 0 {
                    self.caps = Caps::empty();
                }
                return Ok(());
            }

            // Non-root can only set to real, effective, or saved UID
            if new_uid == self.uid.0 || new_uid == self.euid.0 || new_uid == self.suid.0 {
                self.euid = Uid(new_uid);
                return Ok(());
            }

            Err(KError::PermissionDenied)
        }

        /// Set effective GID
        pub fn setgid(&mut self, new_gid: u32) -> KResult<()> {
            // Root can set to any GID
            if self.euid.0 == 0 {
                self.gid = Gid(new_gid);
                self.egid = Gid(new_gid);
                self.sgid = Gid(new_gid);
                return Ok(());
            }

            // Non-root can only set to real, effective, or saved GID
            if new_gid == self.gid.0 || new_gid == self.egid.0 || new_gid == self.sgid.0 {
                self.egid = Gid(new_gid);
                return Ok(());
            }

            Err(KError::PermissionDenied)
        }

        /// Set real and effective UID
        pub fn setreuid(&mut self, ruid: i32, euid: i32) -> KResult<()> {
            let is_root = self.euid.0 == 0;

            // -1 means "don't change"
            let new_ruid = if ruid == -1 { self.uid.0 } else { ruid as u32 };
            let new_euid = if euid == -1 { self.euid.0 } else { euid as u32 };

            // Check permissions
            if !is_root {
                // Non-root can only set ruid to ruid or euid
                if ruid != -1 && new_ruid != self.uid.0 && new_ruid != self.euid.0 {
                    return Err(KError::PermissionDenied);
                }
                // Non-root can only set euid to ruid, euid, or suid
                if euid != -1 && new_euid != self.uid.0 && new_euid != self.euid.0 && new_euid != self.suid.0 {
                    return Err(KError::PermissionDenied);
                }
            }

            // If ruid is set or euid is different from current ruid, save euid to suid
            if ruid != -1 || (euid != -1 && new_euid != self.uid.0) {
                self.suid = self.euid;
            }

            self.uid = Uid(new_ruid);
            self.euid = Uid(new_euid);

            // Update caps
            if self.euid.0 == 0 {
                self.caps = Caps::ALL;
            } else {
                self.caps = Caps::empty();
            }

            Ok(())
        }

        /// Set real and effective GID
        pub fn setregid(&mut self, rgid: i32, egid: i32) -> KResult<()> {
            let is_root = self.euid.0 == 0;

            let new_rgid = if rgid == -1 { self.gid.0 } else { rgid as u32 };
            let new_egid = if egid == -1 { self.egid.0 } else { egid as u32 };

            if !is_root {
                if rgid != -1 && new_rgid != self.gid.0 && new_rgid != self.egid.0 {
                    return Err(KError::PermissionDenied);
                }
                if egid != -1 && new_egid != self.gid.0 && new_egid != self.egid.0 && new_egid != self.sgid.0 {
                    return Err(KError::PermissionDenied);
                }
            }

            if rgid != -1 || (egid != -1 && new_egid != self.gid.0) {
                self.sgid = self.egid;
            }

            self.gid = Gid(new_rgid);
            self.egid = Gid(new_egid);

            Ok(())
        }

        /// Set supplementary groups
        pub fn setgroups(&mut self, groups: &[Gid]) -> KResult<()> {
            // Only root can set groups
            if self.euid.0 != 0 {
                return Err(KError::PermissionDenied);
            }

            if groups.len() > MAX_GROUPS {
                return Err(KError::Invalid);
            }

            self.ngroups = groups.len();
            for (i, &g) in groups.iter().enumerate() {
                self.groups[i] = g;
            }

            Ok(())
        }

        /// Get supplementary groups
        pub fn getgroups(&self) -> &[Gid] {
            &self.groups[..self.ngroups]
        }

        /// Check if user is in a specific group
        pub fn in_group(&self, gid: Gid) -> bool {
            if self.egid == gid || self.gid == gid {
                return true;
            }
            for i in 0..self.ngroups {
                if self.groups[i] == gid {
                    return true;
                }
            }
            false
        }

        /// Get effective UID
        pub fn geteuid(&self) -> Uid {
            self.euid
        }

        /// Get effective GID
        pub fn getegid(&self) -> Gid {
            self.egid
        }

        /// Get real UID
        pub fn getuid(&self) -> Uid {
            self.uid
        }

        /// Get real GID
        pub fn getgid(&self) -> Gid {
            self.gid
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
                euid: u.uid,
                egid: u.gid,
                suid: u.uid,
                sgid: u.gid,
                groups: [Gid(0); MAX_GROUPS],
                ngroups: 0,
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

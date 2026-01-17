    use crate::security::Cred;

    use super::vfs::{Metadata, Mode};

    #[derive(Clone, Copy)]
    enum Class {
        User,
        Group,
        Other,
    }

    /// Check if credential is in the file's group (primary or supplementary)
    fn in_group(meta: &Metadata, cred: &Cred) -> bool {
        // Check primary group
        if cred.gid == meta.gid {
            return true;
        }
        // Check effective group
        if cred.egid == meta.gid {
            return true;
        }
        // Check supplementary groups
        for i in 0..cred.ngroups as usize {
            if cred.groups[i] == meta.gid {
                return true;
            }
        }
        false
    }

    fn class(meta: &Metadata, cred: &Cred) -> Class {
        if cred.is_root() {
            return Class::User; // root bypass tratado externamente
        }
        // Use effective UID for permission checks
        if cred.euid == meta.uid {
            Class::User
        } else if in_group(meta, cred) {
            Class::Group
        } else {
            Class::Other
        }
    }

    fn allow(meta: &Metadata, cred: &Cred, need_r: bool, need_w: bool, need_x: bool) -> bool {
        if cred.is_root() {
            return true;
        }

        let c = class(meta, cred);
        let m = meta.mode;

        let (r, w, x) = match c {
            Class::User => (m.contains(Mode::UR), m.contains(Mode::UW), m.contains(Mode::UX)),
            Class::Group => (m.contains(Mode::GR), m.contains(Mode::GW), m.contains(Mode::GX)),
            Class::Other => (m.contains(Mode::OR), m.contains(Mode::OW), m.contains(Mode::OX)),
        };

        (!need_r || r) && (!need_w || w) && (!need_x || x)
    }

    pub fn can_read_dir(meta: &Metadata, cred: &Cred) -> bool {
        allow(meta, cred, true, false, false)
    }

    pub fn can_write_dir(meta: &Metadata, cred: &Cred) -> bool {
        allow(meta, cred, false, true, false)
    }

    pub fn can_exec_dir(meta: &Metadata, cred: &Cred) -> bool {
        // Em diretórios, X == "travessia"
        allow(meta, cred, false, false, true)
    }

    pub fn can_read_file(meta: &Metadata, cred: &Cred) -> bool {
        allow(meta, cred, true, false, false)
    }

    pub fn can_write_file(meta: &Metadata, cred: &Cred) -> bool {
        allow(meta, cred, false, true, false)
    }

    /// Generic write check (works for files and directories)
    pub fn can_write(meta: &Metadata, cred: &Cred) -> bool {
        allow(meta, cred, false, true, false)
    }

    /// Generic execute check (works for files and directory traversal)
    pub fn can_exec(meta: &Metadata, cred: &Cred) -> bool {
        allow(meta, cred, false, false, true)
    }

    /// Generic read check
    pub fn can_read(meta: &Metadata, cred: &Cred) -> bool {
        allow(meta, cred, true, false, false)
    }

    /// Check read+write permission (for O_RDWR)
    pub fn can_read_write(meta: &Metadata, cred: &Cred) -> bool {
        allow(meta, cred, true, true, false)
    }

    // ==================== access() support ====================
    // access() uses REAL uid/gid, not effective

    /// Check permission using real UID/GID (for access() syscall)
    fn allow_real(meta: &Metadata, cred: &Cred, need_r: bool, need_w: bool, need_x: bool) -> bool {
        // Root always passes (using real UID)
        if cred.uid.0 == 0 {
            return true;
        }

        // Determine class using real UID/GID
        let c = if cred.uid == meta.uid {
            Class::User
        } else if cred.gid == meta.gid {
            // Check supplementary groups with real gid
            Class::Group
        } else {
            // Check supplementary groups
            let mut in_grp = false;
            for i in 0..cred.ngroups as usize {
                if cred.groups[i] == meta.gid {
                    in_grp = true;
                    break;
                }
            }
            if in_grp { Class::Group } else { Class::Other }
        };

        let m = meta.mode;
        let (r, w, x) = match c {
            Class::User => (m.contains(Mode::UR), m.contains(Mode::UW), m.contains(Mode::UX)),
            Class::Group => (m.contains(Mode::GR), m.contains(Mode::GW), m.contains(Mode::GX)),
            Class::Other => (m.contains(Mode::OR), m.contains(Mode::OW), m.contains(Mode::OX)),
        };

        (!need_r || r) && (!need_w || w) && (!need_x || x)
    }

    /// Check access permissions using real UID/GID (for access()/faccessat())
    /// mode: bitmask of R_OK (4), W_OK (2), X_OK (1), F_OK (0)
    pub fn check_access(meta: &Metadata, cred: &Cred, mode: u32) -> bool {
        const F_OK: u32 = 0; // Test for existence
        const X_OK: u32 = 1; // Test for execute permission
        const W_OK: u32 = 2; // Test for write permission
        const R_OK: u32 = 4; // Test for read permission

        // F_OK: just check existence (always true if we have metadata)
        if mode == F_OK {
            return true;
        }

        let need_r = (mode & R_OK) != 0;
        let need_w = (mode & W_OK) != 0;
        let need_x = (mode & X_OK) != 0;

        allow_real(meta, cred, need_r, need_w, need_x)
    }

    /// Check if user can execute a file (for execve)
    pub fn can_exec_file(meta: &Metadata, cred: &Cred) -> bool {
        // Must be a regular file and have execute permission
        allow(meta, cred, false, false, true)
    }

    use crate::security::Cred;

    use super::vfs::{Metadata, Mode};

    #[derive(Clone, Copy)]
    enum Class {
        User,
        Group,
        Other,
    }

    fn class(meta: &Metadata, cred: &Cred) -> Class {
        if cred.is_root() {
            return Class::User; // root bypass tratado externamente
        }
        if cred.uid == meta.uid {
            Class::User
        } else if cred.gid == meta.gid {
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

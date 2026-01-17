//! Session Management
//!
//! Manages user login sessions:
//! - Track who is logged in
//! - Session creation/destruction
//! - TTY/PTY association
//! - Login/logout timestamps
//! - utmp/wtmp equivalent functionality

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use spin::RwLock;

use super::passwd::{Uid, Gid};

/// Session ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(pub u64);

impl SessionId {
    /// Create a new unique session ID
    pub fn new() -> Self {
        static NEXT_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);
        SessionId(NEXT_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
    }
}

/// Session type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    /// Local console login
    Console,
    /// TTY login
    Tty,
    /// Pseudo-terminal (SSH, terminal emulator)
    Pty,
    /// Graphical session (GUI login)
    Graphical,
    /// Background/system session
    System,
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is active
    Active,
    /// Session is idle (no activity)
    Idle,
    /// Session is locked (screen lock)
    Locked,
    /// Session is closing
    Closing,
}

/// A user session
#[derive(Debug, Clone)]
pub struct Session {
    /// Session ID
    pub id: SessionId,
    /// Username
    pub username: String,
    /// User ID
    pub uid: Uid,
    /// Primary group ID
    pub gid: Gid,
    /// Session type
    pub session_type: SessionType,
    /// Session state
    pub state: SessionState,
    /// TTY/terminal name (e.g., "tty1", "pts/0")
    pub tty: Option<String>,
    /// Remote host (for SSH sessions)
    pub remote_host: Option<String>,
    /// Process ID of session leader
    pub leader_pid: Option<u32>,
    /// Login timestamp (seconds since epoch)
    pub login_time: u64,
    /// Last activity timestamp
    pub last_activity: u64,
    /// Display (for graphical sessions, e.g., ":0")
    pub display: Option<String>,
    /// Seat (for multi-seat systems)
    pub seat: Option<String>,
}

impl Session {
    /// Create a new session
    pub fn new(username: &str, uid: Uid, gid: Gid, session_type: SessionType) -> Self {
        let now = crate::time::uptime_secs();

        Self {
            id: SessionId::new(),
            username: String::from(username),
            uid,
            gid,
            session_type,
            state: SessionState::Active,
            tty: None,
            remote_host: None,
            leader_pid: None,
            login_time: now,
            last_activity: now,
            display: None,
            seat: None,
        }
    }

    /// Set TTY
    pub fn with_tty(mut self, tty: &str) -> Self {
        self.tty = Some(String::from(tty));
        self
    }

    /// Set remote host
    pub fn with_remote_host(mut self, host: &str) -> Self {
        self.remote_host = Some(String::from(host));
        self
    }

    /// Set leader PID
    pub fn with_leader_pid(mut self, pid: u32) -> Self {
        self.leader_pid = Some(pid);
        self
    }

    /// Set display
    pub fn with_display(mut self, display: &str) -> Self {
        self.display = Some(String::from(display));
        self
    }

    /// Update activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = crate::time::uptime_secs();
        if self.state == SessionState::Idle {
            self.state = SessionState::Active;
        }
    }

    /// Lock the session
    pub fn lock(&mut self) {
        self.state = SessionState::Locked;
    }

    /// Unlock the session
    pub fn unlock(&mut self) {
        self.state = SessionState::Active;
        self.touch();
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        self.state == SessionState::Active
    }

    /// Get session duration in seconds
    pub fn duration(&self) -> u64 {
        crate::time::uptime_secs().saturating_sub(self.login_time)
    }

    /// Get idle time in seconds
    pub fn idle_time(&self) -> u64 {
        crate::time::uptime_secs().saturating_sub(self.last_activity)
    }
}

/// Session manager
pub struct SessionManager {
    /// Active sessions by ID
    sessions: BTreeMap<SessionId, Session>,
    /// Sessions by user
    user_sessions: BTreeMap<String, Vec<SessionId>>,
    /// Sessions by TTY
    tty_sessions: BTreeMap<String, SessionId>,
    /// Idle timeout in seconds (0 = disabled)
    idle_timeout: u64,
}

impl SessionManager {
    /// Create a new session manager
    pub const fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            user_sessions: BTreeMap::new(),
            tty_sessions: BTreeMap::new(),
            idle_timeout: 0,
        }
    }

    /// Create a new session
    pub fn create_session(&mut self, session: Session) -> SessionId {
        let id = session.id;
        let username = session.username.clone();
        let tty = session.tty.clone();

        // Add to sessions map
        self.sessions.insert(id, session);

        // Add to user sessions
        self.user_sessions
            .entry(username)
            .or_insert_with(Vec::new)
            .push(id);

        // Add to tty sessions
        if let Some(tty) = tty {
            self.tty_sessions.insert(tty, id);
        }

        crate::kprintln!("session: created session {} for user", id.0);

        id
    }

    /// Destroy a session (logout)
    pub fn destroy_session(&mut self, id: SessionId) -> Option<Session> {
        let session = self.sessions.remove(&id)?;

        // Remove from user sessions
        if let Some(user_sessions) = self.user_sessions.get_mut(&session.username) {
            user_sessions.retain(|&sid| sid != id);
            if user_sessions.is_empty() {
                self.user_sessions.remove(&session.username);
            }
        }

        // Remove from tty sessions
        if let Some(ref tty) = session.tty {
            self.tty_sessions.remove(tty);
        }

        crate::kprintln!("session: destroyed session {} for {}", id.0, session.username);

        Some(session)
    }

    /// Get a session by ID
    pub fn get_session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.get(&id)
    }

    /// Get a mutable session by ID
    pub fn get_session_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.get_mut(&id)
    }

    /// Get session by TTY
    pub fn get_session_by_tty(&self, tty: &str) -> Option<&Session> {
        self.tty_sessions.get(tty).and_then(|id| self.sessions.get(id))
    }

    /// Get all sessions for a user
    pub fn get_user_sessions(&self, username: &str) -> Vec<&Session> {
        self.user_sessions
            .get(username)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.sessions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all active sessions
    pub fn active_sessions(&self) -> Vec<&Session> {
        self.sessions.values().filter(|s| s.is_active()).collect()
    }

    /// Get all sessions
    pub fn all_sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    /// Check if user is logged in
    pub fn is_user_logged_in(&self, username: &str) -> bool {
        self.user_sessions
            .get(username)
            .map(|ids| !ids.is_empty())
            .unwrap_or(false)
    }

    /// Count active sessions
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Count user sessions
    pub fn user_session_count(&self, username: &str) -> usize {
        self.user_sessions
            .get(username)
            .map(|ids| ids.len())
            .unwrap_or(0)
    }

    /// Update idle status for all sessions
    pub fn update_idle_status(&mut self) {
        if self.idle_timeout == 0 {
            return;
        }

        let now = crate::time::uptime_secs();
        for session in self.sessions.values_mut() {
            if session.state == SessionState::Active {
                if now - session.last_activity > self.idle_timeout {
                    session.state = SessionState::Idle;
                }
            }
        }
    }

    /// Set idle timeout
    pub fn set_idle_timeout(&mut self, seconds: u64) {
        self.idle_timeout = seconds;
    }

    /// Get sessions that should be timed out
    pub fn get_timed_out_sessions(&self, timeout: u64) -> Vec<SessionId> {
        let now = crate::time::uptime_secs();
        self.sessions
            .iter()
            .filter(|(_, s)| now - s.last_activity > timeout)
            .map(|(id, _)| *id)
            .collect()
    }
}

/// Global session manager
static SESSION_MANAGER: RwLock<SessionManager> = RwLock::new(SessionManager::new());

/// Initialize session management
pub fn init() {
    crate::kprintln!("session: initialized");
}

/// Login - create a new session
pub fn login(
    username: &str,
    uid: Uid,
    gid: Gid,
    session_type: SessionType,
    tty: Option<&str>,
) -> SessionId {
    let mut session = Session::new(username, uid, gid, session_type);

    if let Some(tty) = tty {
        session = session.with_tty(tty);
    }

    let mut manager = SESSION_MANAGER.write();
    manager.create_session(session)
}

/// Logout - destroy a session
pub fn logout(session_id: SessionId) -> Option<Session> {
    let mut manager = SESSION_MANAGER.write();
    manager.destroy_session(session_id)
}

/// Logout all sessions for a user
pub fn logout_user(username: &str) -> Vec<Session> {
    let mut manager = SESSION_MANAGER.write();

    let session_ids: Vec<SessionId> = manager
        .user_sessions
        .get(username)
        .cloned()
        .unwrap_or_default();

    session_ids
        .into_iter()
        .filter_map(|id| manager.destroy_session(id))
        .collect()
}

/// Get current session for a TTY
pub fn get_tty_session(tty: &str) -> Option<Session> {
    let manager = SESSION_MANAGER.read();
    manager.get_session_by_tty(tty).cloned()
}

/// Check if user is logged in
pub fn is_logged_in(username: &str) -> bool {
    let manager = SESSION_MANAGER.read();
    manager.is_user_logged_in(username)
}

/// Get all active sessions
pub fn list_sessions() -> Vec<Session> {
    let manager = SESSION_MANAGER.read();
    manager.all_sessions().into_iter().cloned().collect()
}

/// Update session activity
pub fn touch_session(session_id: SessionId) {
    let mut manager = SESSION_MANAGER.write();
    if let Some(session) = manager.get_session_mut(session_id) {
        session.touch();
    }
}

/// Lock a session
pub fn lock_session(session_id: SessionId) {
    let mut manager = SESSION_MANAGER.write();
    if let Some(session) = manager.get_session_mut(session_id) {
        session.lock();
    }
}

/// Unlock a session
pub fn unlock_session(session_id: SessionId) {
    let mut manager = SESSION_MANAGER.write();
    if let Some(session) = manager.get_session_mut(session_id) {
        session.unlock();
    }
}

/// Get session count
pub fn session_count() -> usize {
    let manager = SESSION_MANAGER.read();
    manager.session_count()
}

/// Get logged in users
pub fn logged_in_users() -> Vec<String> {
    let manager = SESSION_MANAGER.read();
    manager.user_sessions.keys().cloned().collect()
}

/// Format session info (like `who` command)
pub fn who() -> Vec<String> {
    let manager = SESSION_MANAGER.read();
    let mut lines = Vec::new();

    for session in manager.all_sessions() {
        let tty = session.tty.as_deref().unwrap_or("?");
        let state = match session.state {
            SessionState::Active => "",
            SessionState::Idle => " (idle)",
            SessionState::Locked => " (locked)",
            SessionState::Closing => " (closing)",
        };

        let mut line = session.username.clone();
        line.push_str("    ");
        line.push_str(tty);
        line.push_str(state);

        if let Some(ref host) = session.remote_host {
            line.push_str(" (");
            line.push_str(host);
            line.push(')');
        }

        lines.push(line);
    }

    lines
}

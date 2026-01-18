//! Git Integration
//!
//! Git version control integration for the text editor and file manager.
//! Provides repository management, commit, branch, diff, and status operations.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use crate::drivers::framebuffer::Color;
use crate::drivers::font::DEFAULT_FONT;
use crate::gui::surface::Surface;
use crate::gui::widgets::{Widget, WidgetId, WidgetEvent, Bounds, MouseButton};

/// Git object type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
    Tag,
}

impl ObjectType {
    pub fn name(&self) -> &'static str {
        match self {
            ObjectType::Blob => "blob",
            ObjectType::Tree => "tree",
            ObjectType::Commit => "commit",
            ObjectType::Tag => "tag",
        }
    }
}

/// Git file status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// No changes
    Unmodified,
    /// Modified in working directory
    Modified,
    /// Staged for commit
    Staged,
    /// Both modified and staged (with different changes)
    StagedModified,
    /// Added to staging (new file)
    Added,
    /// Deleted
    Deleted,
    /// Renamed
    Renamed,
    /// Copied
    Copied,
    /// Untracked (not in git)
    Untracked,
    /// Ignored by .gitignore
    Ignored,
    /// Conflicted (merge conflict)
    Conflicted,
}

impl FileStatus {
    pub fn char(&self) -> char {
        match self {
            FileStatus::Unmodified => ' ',
            FileStatus::Modified => 'M',
            FileStatus::Staged => 'A',
            FileStatus::StagedModified => 'M',
            FileStatus::Added => 'A',
            FileStatus::Deleted => 'D',
            FileStatus::Renamed => 'R',
            FileStatus::Copied => 'C',
            FileStatus::Untracked => '?',
            FileStatus::Ignored => '!',
            FileStatus::Conflicted => 'U',
        }
    }

    pub fn color(&self) -> Color {
        match self {
            FileStatus::Unmodified => Color::new(150, 150, 150),
            FileStatus::Modified => Color::new(255, 200, 100),
            FileStatus::Staged | FileStatus::Added => Color::new(80, 250, 123),
            FileStatus::StagedModified => Color::new(255, 200, 100),
            FileStatus::Deleted => Color::new(255, 85, 85),
            FileStatus::Renamed | FileStatus::Copied => Color::new(139, 233, 253),
            FileStatus::Untracked => Color::new(189, 147, 249),
            FileStatus::Ignored => Color::new(100, 100, 100),
            FileStatus::Conflicted => Color::new(255, 121, 198),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FileStatus::Unmodified => "Unmodified",
            FileStatus::Modified => "Modified",
            FileStatus::Staged => "Staged",
            FileStatus::StagedModified => "Staged + Modified",
            FileStatus::Added => "Added",
            FileStatus::Deleted => "Deleted",
            FileStatus::Renamed => "Renamed",
            FileStatus::Copied => "Copied",
            FileStatus::Untracked => "Untracked",
            FileStatus::Ignored => "Ignored",
            FileStatus::Conflicted => "Conflict",
        }
    }
}

/// File change entry
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub staged: bool,
    pub lines_added: usize,
    pub lines_removed: usize,
}

impl FileChange {
    pub fn new(path: &str, status: FileStatus) -> Self {
        Self {
            path: String::from(path),
            old_path: None,
            status,
            staged: false,
            lines_added: 0,
            lines_removed: 0,
        }
    }

    pub fn display_name(&self) -> &str {
        if let Some(ref old_path) = self.old_path {
            // Would show "old -> new" but can't allocate in const
            &self.path
        } else {
            &self.path
        }
    }
}

/// Git commit
#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: String,
    pub timestamp: u64,
    pub message: String,
    pub summary: String,
    pub parent_hashes: Vec<String>,
    pub is_merge: bool,
}

impl Commit {
    pub fn new(hash: &str, message: &str) -> Self {
        let short_hash = if hash.len() >= 7 {
            String::from(&hash[..7])
        } else {
            String::from(hash)
        };
        let summary = String::from(message.lines().next().unwrap_or(""));

        Self {
            hash: String::from(hash),
            short_hash,
            author_name: String::new(),
            author_email: String::new(),
            timestamp: 0,
            message: String::from(message),
            summary,
            parent_hashes: Vec::new(),
            is_merge: false,
        }
    }

    pub fn format_date(&self) -> String {
        // Simple timestamp formatting
        format!("{}", self.timestamp)
    }
}

/// Git branch
#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub is_remote: bool,
    pub is_current: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub last_commit: Option<String>,
}

impl Branch {
    pub fn new(name: &str, is_remote: bool) -> Self {
        Self {
            name: String::from(name),
            is_remote,
            is_current: false,
            upstream: None,
            ahead: 0,
            behind: 0,
            last_commit: None,
        }
    }

    pub fn display_name(&self) -> &str {
        &self.name
    }

    pub fn has_upstream(&self) -> bool {
        self.upstream.is_some()
    }

    pub fn sync_status(&self) -> String {
        if self.ahead > 0 && self.behind > 0 {
            format!("+{} -{}", self.ahead, self.behind)
        } else if self.ahead > 0 {
            format!("+{}", self.ahead)
        } else if self.behind > 0 {
            format!("-{}", self.behind)
        } else {
            String::new()
        }
    }
}

/// Git tag
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub commit_hash: String,
    pub message: Option<String>,
    pub is_annotated: bool,
    pub tagger: Option<String>,
    pub timestamp: Option<u64>,
}

impl Tag {
    pub fn new(name: &str, commit_hash: &str) -> Self {
        Self {
            name: String::from(name),
            commit_hash: String::from(commit_hash),
            message: None,
            is_annotated: false,
            tagger: None,
            timestamp: None,
        }
    }
}

/// Git remote
#[derive(Debug, Clone)]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

impl Remote {
    pub fn new(name: &str, url: &str) -> Self {
        Self {
            name: String::from(name),
            fetch_url: String::from(url),
            push_url: String::from(url),
        }
    }
}

/// Git stash entry
#[derive(Debug, Clone)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    pub branch: String,
    pub timestamp: u64,
}

impl StashEntry {
    pub fn display(&self) -> String {
        format!("stash@{{{}}}: {}", self.index, self.message)
    }
}

/// Diff hunk
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// Diff line
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
    pub content: String,
}

/// Diff line type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
    Header,
    Binary,
}

impl DiffLineType {
    pub fn color(&self) -> Color {
        match self {
            DiffLineType::Context => Color::new(200, 200, 200),
            DiffLineType::Addition => Color::new(80, 250, 123),
            DiffLineType::Deletion => Color::new(255, 85, 85),
            DiffLineType::Header => Color::new(139, 233, 253),
            DiffLineType::Binary => Color::new(189, 147, 249),
        }
    }

    pub fn prefix(&self) -> char {
        match self {
            DiffLineType::Context => ' ',
            DiffLineType::Addition => '+',
            DiffLineType::Deletion => '-',
            DiffLineType::Header => '@',
            DiffLineType::Binary => 'B',
        }
    }
}

/// File diff
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub is_binary: bool,
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub deletions: usize,
}

impl FileDiff {
    pub fn new(path: &str) -> Self {
        Self {
            path: String::from(path),
            old_path: None,
            status: FileStatus::Modified,
            is_binary: false,
            hunks: Vec::new(),
            additions: 0,
            deletions: 0,
        }
    }
}

/// Merge conflict
#[derive(Debug, Clone)]
pub struct MergeConflict {
    pub path: String,
    pub ours: String,
    pub theirs: String,
    pub base: Option<String>,
    pub resolved: bool,
}

/// Repository state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoState {
    Normal,
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
    ApplyingMailbox,
}

impl RepoState {
    pub fn name(&self) -> &'static str {
        match self {
            RepoState::Normal => "Normal",
            RepoState::Merging => "Merging",
            RepoState::Rebasing => "Rebasing",
            RepoState::CherryPicking => "Cherry-picking",
            RepoState::Reverting => "Reverting",
            RepoState::Bisecting => "Bisecting",
            RepoState::ApplyingMailbox => "Applying mailbox",
        }
    }

    pub fn is_in_progress(&self) -> bool {
        *self != RepoState::Normal
    }
}

/// Git repository
#[derive(Debug, Clone)]
pub struct Repository {
    pub path: String,
    pub git_dir: String,
    pub work_dir: String,
    pub state: RepoState,
    pub head: Option<String>,
    pub head_commit: Option<String>,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
    pub tags: Vec<Tag>,
    pub stashes: Vec<StashEntry>,
    pub submodules: Vec<String>,
    pub is_bare: bool,
    pub is_shallow: bool,
}

impl Repository {
    pub fn new(path: &str) -> Self {
        Self {
            path: String::from(path),
            git_dir: format!("{}/.git", path),
            work_dir: String::from(path),
            state: RepoState::Normal,
            head: None,
            head_commit: None,
            branches: Vec::new(),
            remotes: Vec::new(),
            tags: Vec::new(),
            stashes: Vec::new(),
            submodules: Vec::new(),
            is_bare: false,
            is_shallow: false,
        }
    }

    pub fn current_branch(&self) -> Option<&str> {
        self.head.as_deref()
    }

    pub fn is_detached(&self) -> bool {
        self.head.as_ref().map_or(true, |h| h.starts_with("refs/"))
    }

    pub fn find_branch(&self, name: &str) -> Option<&Branch> {
        self.branches.iter().find(|b| b.name == name)
    }

    pub fn find_remote(&self, name: &str) -> Option<&Remote> {
        self.remotes.iter().find(|r| r.name == name)
    }

    pub fn local_branches(&self) -> impl Iterator<Item = &Branch> {
        self.branches.iter().filter(|b| !b.is_remote)
    }

    pub fn remote_branches(&self) -> impl Iterator<Item = &Branch> {
        self.branches.iter().filter(|b| b.is_remote)
    }
}

/// Git operation result
#[derive(Debug, Clone)]
pub enum GitResult<T> {
    Ok(T),
    Err(GitError),
}

impl<T> GitResult<T> {
    pub fn is_ok(&self) -> bool {
        matches!(self, GitResult::Ok(_))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, GitResult::Err(_))
    }

    pub fn ok(self) -> Option<T> {
        match self {
            GitResult::Ok(v) => Some(v),
            GitResult::Err(_) => None,
        }
    }

    pub fn err(self) -> Option<GitError> {
        match self {
            GitResult::Ok(_) => None,
            GitResult::Err(e) => Some(e),
        }
    }
}

/// Git error
#[derive(Debug, Clone)]
pub enum GitError {
    NotARepository,
    InvalidRef,
    AmbiguousRef,
    RefNotFound,
    FileNotFound,
    NotStaged,
    NothingToCommit,
    MergeConflict,
    DetachedHead,
    DirtyWorkingTree,
    UpstreamNotSet,
    RemoteNotFound,
    NetworkError,
    AuthenticationFailed,
    PermissionDenied,
    LockFailed,
    ObjectNotFound,
    InvalidObject,
    Other(String),
}

impl GitError {
    pub fn message(&self) -> &str {
        match self {
            GitError::NotARepository => "Not a git repository",
            GitError::InvalidRef => "Invalid reference",
            GitError::AmbiguousRef => "Ambiguous reference",
            GitError::RefNotFound => "Reference not found",
            GitError::FileNotFound => "File not found",
            GitError::NotStaged => "Changes not staged",
            GitError::NothingToCommit => "Nothing to commit",
            GitError::MergeConflict => "Merge conflict",
            GitError::DetachedHead => "HEAD is detached",
            GitError::DirtyWorkingTree => "Working tree is dirty",
            GitError::UpstreamNotSet => "Upstream not set",
            GitError::RemoteNotFound => "Remote not found",
            GitError::NetworkError => "Network error",
            GitError::AuthenticationFailed => "Authentication failed",
            GitError::PermissionDenied => "Permission denied",
            GitError::LockFailed => "Lock failed",
            GitError::ObjectNotFound => "Object not found",
            GitError::InvalidObject => "Invalid object",
            GitError::Other(msg) => msg,
        }
    }
}

/// View mode for Git panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitViewMode {
    Status,
    Branches,
    Commits,
    Stashes,
    Tags,
    Diff,
}

/// Git panel widget
pub struct GitPanel {
    id: WidgetId,
    bounds: Bounds,

    /// Current repository
    repository: Option<Repository>,

    /// File changes
    changes: Vec<FileChange>,

    /// Commit history
    commits: Vec<Commit>,

    /// Current diff
    current_diff: Option<FileDiff>,

    /// View mode
    view_mode: GitViewMode,

    /// Selected file index
    selected_file: usize,

    /// Selected commit index
    selected_commit: usize,

    /// Selected branch index
    selected_branch: usize,

    /// Scroll offset
    scroll_offset: usize,

    /// Commit message buffer
    commit_message: String,

    /// Is expanded
    expanded: bool,

    /// Width when expanded
    panel_width: usize,

    /// Hover state
    hovered_item: Option<usize>,

    /// Show staged only
    show_staged_only: bool,

    /// Show untracked files
    show_untracked: bool,

    /// Widget state
    visible: bool,
    focused: bool,
}

impl GitPanel {
    const MIN_WIDTH: usize = 250;
    const MAX_WIDTH: usize = 400;
    const ITEM_HEIGHT: usize = 24;
    const HEADER_HEIGHT: usize = 32;
    const BUTTON_HEIGHT: usize = 28;

    pub fn new(x: isize, y: isize, width: usize, height: usize) -> Self {
        let mut panel = Self {
            id: WidgetId::new(),
            bounds: Bounds::new(x, y, width, height),
            repository: None,
            changes: Vec::new(),
            commits: Vec::new(),
            current_diff: None,
            view_mode: GitViewMode::Status,
            selected_file: 0,
            selected_commit: 0,
            selected_branch: 0,
            scroll_offset: 0,
            commit_message: String::new(),
            expanded: true,
            panel_width: width,
            hovered_item: None,
            show_staged_only: false,
            show_untracked: true,
            visible: true,
            focused: false,
        };

        // Add sample data
        panel.add_sample_data();
        panel
    }

    fn add_sample_data(&mut self) {
        // Create sample repository
        let mut repo = Repository::new("/home/user/project");
        repo.head = Some(String::from("main"));
        repo.head_commit = Some(String::from("abc123"));

        // Add branches
        let mut main = Branch::new("main", false);
        main.is_current = true;
        main.upstream = Some(String::from("origin/main"));
        repo.branches.push(main);

        let mut develop = Branch::new("develop", false);
        develop.upstream = Some(String::from("origin/develop"));
        develop.ahead = 2;
        repo.branches.push(develop);

        let feature = Branch::new("feature/new-ui", false);
        repo.branches.push(feature);

        let remote_main = Branch::new("origin/main", true);
        repo.branches.push(remote_main);

        // Add remotes
        repo.remotes.push(Remote::new("origin", "git@github.com:user/project.git"));

        // Add tags
        repo.tags.push(Tag::new("v1.0.0", "abc111"));
        repo.tags.push(Tag::new("v1.1.0", "abc222"));

        self.repository = Some(repo);

        // Add sample changes
        let mut change1 = FileChange::new("src/main.rs", FileStatus::Modified);
        change1.staged = true;
        change1.lines_added = 10;
        change1.lines_removed = 3;
        self.changes.push(change1);

        let mut change2 = FileChange::new("src/lib.rs", FileStatus::Modified);
        change2.lines_added = 5;
        change2.lines_removed = 2;
        self.changes.push(change2);

        let change3 = FileChange::new("README.md", FileStatus::Untracked);
        self.changes.push(change3);

        let mut change4 = FileChange::new("Cargo.toml", FileStatus::Added);
        change4.staged = true;
        self.changes.push(change4);

        // Add sample commits
        let mut commit1 = Commit::new("abc123def456789012345678901234567890abcd", "Fix: Resolve memory leak in allocator");
        commit1.author_name = String::from("John Doe");
        commit1.author_email = String::from("john@example.com");
        commit1.timestamp = 1705500000;
        self.commits.push(commit1);

        let mut commit2 = Commit::new("bcd234efg567890123456789012345678901bcde", "Add: New GUI widget for file browser");
        commit2.author_name = String::from("Jane Smith");
        commit2.author_email = String::from("jane@example.com");
        commit2.timestamp = 1705400000;
        self.commits.push(commit2);

        let mut commit3 = Commit::new("cde345fgh678901234567890123456789012cdef", "Refactor: Clean up networking code");
        commit3.author_name = String::from("John Doe");
        commit3.author_email = String::from("john@example.com");
        commit3.timestamp = 1705300000;
        self.commits.push(commit3);
    }

    /// Open repository
    pub fn open_repository(&mut self, path: &str) {
        self.repository = Some(Repository::new(path));
        self.refresh_status();
    }

    /// Close repository
    pub fn close_repository(&mut self) {
        self.repository = None;
        self.changes.clear();
        self.commits.clear();
        self.current_diff = None;
    }

    /// Refresh status
    pub fn refresh_status(&mut self) {
        // In a real implementation, this would read the git index
    }

    /// Stage file
    pub fn stage_file(&mut self, index: usize) {
        if let Some(change) = self.changes.get_mut(index) {
            change.staged = true;
            if change.status == FileStatus::Untracked {
                change.status = FileStatus::Added;
            } else if change.status == FileStatus::Modified {
                change.status = FileStatus::Staged;
            }
        }
    }

    /// Unstage file
    pub fn unstage_file(&mut self, index: usize) {
        if let Some(change) = self.changes.get_mut(index) {
            change.staged = false;
            if change.status == FileStatus::Added {
                change.status = FileStatus::Untracked;
            } else if change.status == FileStatus::Staged {
                change.status = FileStatus::Modified;
            }
        }
    }

    /// Stage all files
    pub fn stage_all(&mut self) {
        for change in &mut self.changes {
            change.staged = true;
            if change.status == FileStatus::Untracked {
                change.status = FileStatus::Added;
            } else if change.status == FileStatus::Modified {
                change.status = FileStatus::Staged;
            }
        }
    }

    /// Unstage all files
    pub fn unstage_all(&mut self) {
        for change in &mut self.changes {
            change.staged = false;
            if change.status == FileStatus::Added {
                change.status = FileStatus::Untracked;
            } else if change.status == FileStatus::Staged {
                change.status = FileStatus::Modified;
            }
        }
    }

    /// Commit staged changes
    pub fn commit(&mut self, message: &str) -> GitResult<Commit> {
        let staged_count = self.changes.iter().filter(|c| c.staged).count();
        if staged_count == 0 {
            return GitResult::Err(GitError::NothingToCommit);
        }

        // Create commit
        let commit = Commit::new("new123commit456hash", message);

        // Remove staged files from changes
        self.changes.retain(|c| !c.staged);

        // Add to commit history
        self.commits.insert(0, commit.clone());

        GitResult::Ok(commit)
    }

    /// Get visible items count
    fn visible_items(&self) -> usize {
        let content_height = self.bounds.height
            .saturating_sub(Self::HEADER_HEIGHT + Self::BUTTON_HEIGHT * 2);
        content_height / Self::ITEM_HEIGHT
    }

    /// Get filtered changes
    fn filtered_changes(&self) -> Vec<(usize, &FileChange)> {
        self.changes.iter()
            .enumerate()
            .filter(|(_, c)| {
                if self.show_staged_only && !c.staged {
                    return false;
                }
                if !self.show_untracked && c.status == FileStatus::Untracked {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Draw text
    fn draw_text(&self, surface: &mut Surface, x: usize, y: usize, text: &str, color: Color) {
        let mut cx = x;
        for c in text.chars() {
            if let Some(glyph) = DEFAULT_FONT.get_glyph(c) {
                for row in 0..DEFAULT_FONT.height {
                    let byte = glyph[row];
                    for col in 0..DEFAULT_FONT.width {
                        if (byte >> (DEFAULT_FONT.width - 1 - col)) & 1 != 0 {
                            surface.set_pixel(cx + col, y + row, color);
                        }
                    }
                }
            }
            cx += DEFAULT_FONT.width;
        }
    }

    /// Render status view
    fn render_status(&self, surface: &mut Surface) {
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;

        // Section: Staged Changes
        let staged: Vec<_> = self.changes.iter().filter(|c| c.staged).collect();
        let mut cy = y + Self::HEADER_HEIGHT;

        if !staged.is_empty() {
            // Staged header
            self.draw_text(surface, x + 8, cy + 4, "STAGED CHANGES", Color::new(139, 233, 253));
            cy += Self::ITEM_HEIGHT;

            for change in &staged {
                // Status indicator
                let status_color = change.status.color();
                for py in 0..12 {
                    for px in 0..12 {
                        surface.set_pixel(x + 8 + px, cy + 6 + py, status_color);
                    }
                }

                // File name
                let text_color = Color::new(248, 248, 242);
                let filename = change.path.rsplit('/').next().unwrap_or(&change.path);
                self.draw_text(surface, x + 28, cy + 4, filename, text_color);

                cy += Self::ITEM_HEIGHT;
                if cy > y + self.bounds.height - Self::BUTTON_HEIGHT * 2 {
                    break;
                }
            }

            cy += 8; // Spacing
        }

        // Section: Unstaged Changes
        let unstaged: Vec<_> = self.changes.iter().filter(|c| !c.staged).collect();

        if !unstaged.is_empty() {
            self.draw_text(surface, x + 8, cy + 4, "CHANGES", Color::new(255, 184, 108));
            cy += Self::ITEM_HEIGHT;

            for change in &unstaged {
                if cy > y + self.bounds.height - Self::BUTTON_HEIGHT * 2 {
                    break;
                }

                // Status indicator
                let status_color = change.status.color();
                for py in 0..12 {
                    for px in 0..12 {
                        surface.set_pixel(x + 8 + px, cy + 6 + py, status_color);
                    }
                }

                // File name
                let text_color = Color::new(248, 248, 242);
                let filename = change.path.rsplit('/').next().unwrap_or(&change.path);
                self.draw_text(surface, x + 28, cy + 4, filename, text_color);

                cy += Self::ITEM_HEIGHT;
            }
        }

        if staged.is_empty() && unstaged.is_empty() {
            self.draw_text(surface, x + 8, cy + 4, "No changes", Color::new(100, 100, 100));
        }
    }

    /// Render branches view
    fn render_branches(&self, surface: &mut Surface) {
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;

        if let Some(repo) = &self.repository {
            let mut cy = y + Self::HEADER_HEIGHT;

            // Local branches
            self.draw_text(surface, x + 8, cy + 4, "LOCAL", Color::new(139, 233, 253));
            cy += Self::ITEM_HEIGHT;

            for branch in repo.local_branches() {
                if cy > y + self.bounds.height - Self::BUTTON_HEIGHT {
                    break;
                }

                let text_color = if branch.is_current {
                    Color::new(80, 250, 123)
                } else {
                    Color::new(248, 248, 242)
                };

                // Current indicator
                if branch.is_current {
                    self.draw_text(surface, x + 8, cy + 4, "*", text_color);
                }

                self.draw_text(surface, x + 20, cy + 4, &branch.name, text_color);

                // Sync status
                let sync = branch.sync_status();
                if !sync.is_empty() {
                    self.draw_text(
                        surface,
                        x + self.bounds.width - sync.len() * 8 - 8,
                        cy + 4,
                        &sync,
                        Color::new(189, 147, 249),
                    );
                }

                cy += Self::ITEM_HEIGHT;
            }

            cy += 8;

            // Remote branches
            self.draw_text(surface, x + 8, cy + 4, "REMOTE", Color::new(255, 184, 108));
            cy += Self::ITEM_HEIGHT;

            for branch in repo.remote_branches() {
                if cy > y + self.bounds.height - Self::BUTTON_HEIGHT {
                    break;
                }

                self.draw_text(surface, x + 20, cy + 4, &branch.name, Color::new(150, 150, 150));
                cy += Self::ITEM_HEIGHT;
            }
        }
    }

    /// Render commits view
    fn render_commits(&self, surface: &mut Surface) {
        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;

        let mut cy = y + Self::HEADER_HEIGHT;

        for (i, commit) in self.commits.iter().enumerate() {
            if cy > y + self.bounds.height - Self::BUTTON_HEIGHT {
                break;
            }

            let is_selected = i == self.selected_commit;
            let is_hovered = self.hovered_item == Some(i);

            // Background
            if is_selected {
                let bg = Color::new(68, 71, 90);
                for py in 0..Self::ITEM_HEIGHT {
                    for px in 0..self.bounds.width - 4 {
                        surface.set_pixel(x + 2 + px, cy + py, bg);
                    }
                }
            } else if is_hovered {
                let bg = Color::new(55, 57, 70);
                for py in 0..Self::ITEM_HEIGHT {
                    for px in 0..self.bounds.width - 4 {
                        surface.set_pixel(x + 2 + px, cy + py, bg);
                    }
                }
            }

            // Short hash
            self.draw_text(surface, x + 8, cy + 4, &commit.short_hash, Color::new(255, 184, 108));

            // Message summary (truncated)
            let max_len = (self.bounds.width - 80) / 8;
            let summary = if commit.summary.len() > max_len {
                let truncated: String = commit.summary.chars().take(max_len - 2).collect();
                format!("{}..", truncated)
            } else {
                commit.summary.clone()
            };
            self.draw_text(surface, x + 72, cy + 4, &summary, Color::new(248, 248, 242));

            cy += Self::ITEM_HEIGHT;
        }
    }
}

impl Widget for GitPanel {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn set_position(&mut self, x: isize, y: isize) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: usize, height: usize) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {}

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn handle_event(&mut self, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Focus => {
                self.focused = true;
                true
            }
            WidgetEvent::Blur => {
                self.focused = false;
                true
            }
            WidgetEvent::KeyDown { key, .. } => {
                match *key {
                    0x48 => { // Up
                        match self.view_mode {
                            GitViewMode::Status => {
                                if self.selected_file > 0 {
                                    self.selected_file -= 1;
                                }
                            }
                            GitViewMode::Commits => {
                                if self.selected_commit > 0 {
                                    self.selected_commit -= 1;
                                }
                            }
                            GitViewMode::Branches => {
                                if self.selected_branch > 0 {
                                    self.selected_branch -= 1;
                                }
                            }
                            _ => {}
                        }
                        true
                    }
                    0x50 => { // Down
                        match self.view_mode {
                            GitViewMode::Status => {
                                if self.selected_file + 1 < self.changes.len() {
                                    self.selected_file += 1;
                                }
                            }
                            GitViewMode::Commits => {
                                if self.selected_commit + 1 < self.commits.len() {
                                    self.selected_commit += 1;
                                }
                            }
                            GitViewMode::Branches => {
                                if let Some(repo) = &self.repository {
                                    if self.selected_branch + 1 < repo.branches.len() {
                                        self.selected_branch += 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                        true
                    }
                    0x39 => { // Space - toggle stage
                        if self.view_mode == GitViewMode::Status {
                            if let Some(change) = self.changes.get(self.selected_file) {
                                if change.staged {
                                    self.unstage_file(self.selected_file);
                                } else {
                                    self.stage_file(self.selected_file);
                                }
                            }
                        }
                        true
                    }
                    0x1C => { // Enter - commit
                        if self.view_mode == GitViewMode::Status && !self.commit_message.is_empty() {
                            let msg = self.commit_message.clone();
                            let _ = self.commit(&msg);
                            self.commit_message.clear();
                        }
                        true
                    }
                    _ => false,
                }
            }
            WidgetEvent::MouseDown { button: MouseButton::Left, x, y } => {
                let local_y = (*y - self.bounds.y) as usize;

                if local_y < Self::HEADER_HEIGHT {
                    // Clicked on header - cycle view mode
                    self.view_mode = match self.view_mode {
                        GitViewMode::Status => GitViewMode::Branches,
                        GitViewMode::Branches => GitViewMode::Commits,
                        GitViewMode::Commits => GitViewMode::Status,
                        _ => GitViewMode::Status,
                    };
                    return true;
                }

                // Item click
                let item_y = local_y.saturating_sub(Self::HEADER_HEIGHT);
                let item_index = item_y / Self::ITEM_HEIGHT + self.scroll_offset;

                match self.view_mode {
                    GitViewMode::Status => {
                        if item_index < self.changes.len() {
                            self.selected_file = item_index;
                        }
                    }
                    GitViewMode::Commits => {
                        if item_index < self.commits.len() {
                            self.selected_commit = item_index;
                        }
                    }
                    GitViewMode::Branches => {
                        if let Some(repo) = &self.repository {
                            if item_index < repo.branches.len() {
                                self.selected_branch = item_index;
                            }
                        }
                    }
                    _ => {}
                }

                true
            }
            WidgetEvent::MouseMove { x, y } => {
                let local_y = (*y - self.bounds.y) as usize;
                if local_y >= Self::HEADER_HEIGHT {
                    let item_y = local_y - Self::HEADER_HEIGHT;
                    let item_index = item_y / Self::ITEM_HEIGHT + self.scroll_offset;
                    self.hovered_item = Some(item_index);
                } else {
                    self.hovered_item = None;
                }
                true
            }
            WidgetEvent::Scroll { delta_y, .. } => {
                if *delta_y < 0 && self.scroll_offset < 100 {
                    self.scroll_offset += 3;
                } else if *delta_y > 0 && self.scroll_offset > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
                true
            }
            _ => false,
        }
    }

    fn render(&self, surface: &mut Surface) {
        if !self.visible {
            return;
        }

        let x = self.bounds.x.max(0) as usize;
        let y = self.bounds.y.max(0) as usize;
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Background
        let bg = Color::new(40, 42, 54);
        for py in 0..h {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, bg);
            }
        }

        // Header
        let header_bg = Color::new(68, 71, 90);
        for py in 0..Self::HEADER_HEIGHT {
            for px in 0..w {
                surface.set_pixel(x + px, y + py, header_bg);
            }
        }

        // Title
        let title = match self.view_mode {
            GitViewMode::Status => "SOURCE CONTROL",
            GitViewMode::Branches => "BRANCHES",
            GitViewMode::Commits => "COMMITS",
            GitViewMode::Stashes => "STASHES",
            GitViewMode::Tags => "TAGS",
            GitViewMode::Diff => "DIFF",
        };
        self.draw_text(surface, x + 8, y + 8, title, Color::new(139, 233, 253));

        // Branch name
        if let Some(repo) = &self.repository {
            if let Some(branch) = repo.current_branch() {
                let branch_x = x + w - branch.len() * 8 - 8;
                self.draw_text(surface, branch_x, y + 8, branch, Color::new(80, 250, 123));
            }
        }

        // Content
        match self.view_mode {
            GitViewMode::Status => self.render_status(surface),
            GitViewMode::Branches => self.render_branches(surface),
            GitViewMode::Commits => self.render_commits(surface),
            _ => self.render_status(surface),
        }

        // Bottom buttons
        let btn_y = y + h - Self::BUTTON_HEIGHT;
        let btn_bg = Color::new(55, 57, 70);

        // Commit button
        for py in 0..Self::BUTTON_HEIGHT - 4 {
            for px in 4..w / 2 - 4 {
                surface.set_pixel(x + px, btn_y + py + 2, btn_bg);
            }
        }
        self.draw_text(surface, x + 8, btn_y + 6, "Commit", Color::new(80, 250, 123));

        // Refresh button
        for py in 0..Self::BUTTON_HEIGHT - 4 {
            for px in w / 2 + 4..w - 4 {
                surface.set_pixel(x + px, btn_y + py + 2, btn_bg);
            }
        }
        self.draw_text(surface, x + w / 2 + 8, btn_y + 6, "Refresh", Color::new(139, 233, 253));

        // Border
        let border = Color::new(68, 71, 90);
        for py in 0..h {
            surface.set_pixel(x + w - 1, y + py, border);
        }
    }
}

/// Initialize git module
pub fn init() {
    // Nothing to initialize
}

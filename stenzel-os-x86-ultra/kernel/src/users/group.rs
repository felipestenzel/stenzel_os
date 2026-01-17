//! Group Database (/etc/group)
//!
//! Format: groupname:password:gid:members
//!
//! Example:
//! root:x:0:
//! wheel:x:10:alice,bob
//! users:x:100:alice,bob,charlie

use alloc::string::String;
use alloc::vec::Vec;

use super::passwd::Gid;

/// Group entry (from /etc/group)
#[derive(Debug, Clone)]
pub struct Group {
    /// Group name
    pub name: String,
    /// Group password (usually 'x' or empty)
    pub password: String,
    /// Group ID
    pub gid: Gid,
    /// List of member usernames
    pub members: Vec<String>,
}

impl Group {
    /// Create a new group
    pub fn new(name: &str, gid: Gid) -> Self {
        Self {
            name: String::from(name),
            password: String::from("x"),
            gid,
            members: Vec::new(),
        }
    }

    /// Create with members
    pub fn with_members(name: &str, gid: Gid, members: Vec<String>) -> Self {
        Self {
            name: String::from(name),
            password: String::from("x"),
            gid,
            members,
        }
    }

    /// Add a member
    pub fn add_member(&mut self, username: &str) {
        if !self.members.contains(&String::from(username)) {
            self.members.push(String::from(username));
        }
    }

    /// Remove a member
    pub fn remove_member(&mut self, username: &str) {
        self.members.retain(|m| m != username);
    }

    /// Check if user is a member
    pub fn has_member(&self, username: &str) -> bool {
        self.members.iter().any(|m| m == username)
    }

    /// Parse from /etc/group line
    pub fn from_group_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 4 {
            return None;
        }

        let gid: u32 = parts[2].parse().ok()?;

        let members: Vec<String> = if parts[3].is_empty() {
            Vec::new()
        } else {
            parts[3].split(',').map(String::from).collect()
        };

        Some(Self {
            name: String::from(parts[0]),
            password: String::from(parts[1]),
            gid: Gid(gid),
            members,
        })
    }

    /// Format as /etc/group line
    pub fn to_group_line(&self) -> String {
        use alloc::string::ToString;
        let mut s = self.name.clone();
        s.push(':');
        s.push_str(&self.password);
        s.push(':');
        s.push_str(&self.gid.0.to_string());
        s.push(':');

        for (i, member) in self.members.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(member);
        }

        s
    }
}

/// Group database
pub struct GroupDatabase {
    groups: Vec<Group>,
}

impl GroupDatabase {
    /// Create a new empty database
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    /// Load from /etc/group content
    pub fn from_group(content: &str) -> Self {
        let mut db = Self::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(group) = Group::from_group_line(line) {
                db.groups.push(group);
            }
        }

        db
    }

    /// Export to /etc/group format
    pub fn to_group(&self) -> String {
        let mut content = String::new();
        for group in &self.groups {
            content.push_str(&group.to_group_line());
            content.push('\n');
        }
        content
    }

    /// Get all groups
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// Get group by GID
    pub fn get_by_gid(&self, gid: Gid) -> Option<&Group> {
        self.groups.iter().find(|g| g.gid == gid)
    }

    /// Get mutable group by GID
    pub fn get_by_gid_mut(&mut self, gid: Gid) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| g.gid == gid)
    }

    /// Get group by name
    pub fn get_by_name(&self, name: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.name == name)
    }

    /// Get mutable group by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| g.name == name)
    }

    /// Add a group
    pub fn add_group(&mut self, group: Group) {
        // Remove existing group with same name or GID if any
        self.groups.retain(|g| g.name != group.name && g.gid != group.gid);
        self.groups.push(group);
    }

    /// Remove a group by name
    pub fn remove_group(&mut self, name: &str) {
        self.groups.retain(|g| g.name != name);
    }

    /// Remove a group by GID
    pub fn remove_group_by_gid(&mut self, gid: Gid) {
        self.groups.retain(|g| g.gid != gid);
    }

    /// Count groups
    pub fn count(&self) -> usize {
        self.groups.len()
    }

    /// Check if group exists
    pub fn exists(&self, name: &str) -> bool {
        self.groups.iter().any(|g| g.name == name)
    }

    /// Check if GID is in use
    pub fn gid_exists(&self, gid: Gid) -> bool {
        self.groups.iter().any(|g| g.gid == gid)
    }

    /// Get groups that user is member of
    pub fn groups_for_user(&self, username: &str) -> Vec<&Group> {
        self.groups
            .iter()
            .filter(|g| g.has_member(username))
            .collect()
    }

    /// Remove user from all groups
    pub fn remove_user_from_all_groups(&mut self, username: &str) {
        for group in &mut self.groups {
            group.remove_member(username);
        }
    }

    /// Get all group names
    pub fn group_names(&self) -> Vec<&str> {
        self.groups.iter().map(|g| g.name.as_str()).collect()
    }

    /// Validate database (check for duplicates, etc.)
    pub fn validate(&self) -> Result<(), &'static str> {
        // Check for duplicate names
        let mut names: Vec<&str> = self.groups.iter().map(|g| g.name.as_str()).collect();
        names.sort();
        for i in 1..names.len() {
            if names[i] == names[i - 1] {
                return Err("Duplicate group name");
            }
        }

        // Check for duplicate GIDs
        let mut gids: Vec<u32> = self.groups.iter().map(|g| g.gid.0).collect();
        gids.sort();
        for i in 1..gids.len() {
            if gids[i] == gids[i - 1] {
                return Err("Duplicate GID");
            }
        }

        // Check that root group exists
        if !self.exists("root") {
            return Err("No root group");
        }

        // Check that root group has GID 0
        if let Some(root) = self.get_by_name("root") {
            if root.gid.0 != 0 {
                return Err("root group must have GID 0");
            }
        }

        Ok(())
    }
}

impl Default for GroupDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parse_group_line() {
        let line = "wheel:x:10:alice,bob";
        let group = Group::from_group_line(line).unwrap();
        assert_eq!(group.name, "wheel");
        assert_eq!(group.gid.0, 10);
        assert_eq!(group.members.len(), 2);
        assert!(group.has_member("alice"));
        assert!(group.has_member("bob"));
    }

    fn test_to_group_line() {
        let group = Group::with_members(
            "developers",
            Gid(200),
            vec![String::from("alice"), String::from("bob")],
        );
        let line = group.to_group_line();
        assert_eq!(line, "developers:x:200:alice,bob");
    }

    fn test_empty_members() {
        let line = "nogroup:x:65534:";
        let group = Group::from_group_line(line).unwrap();
        assert_eq!(group.members.len(), 0);
    }
}

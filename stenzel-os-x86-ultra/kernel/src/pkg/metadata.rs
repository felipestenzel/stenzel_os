//! Package Metadata
//!
//! Package metadata is stored in TOML-like format in SPKG-INFO file.

use alloc::string::String;
use alloc::vec::Vec;
use crate::util::{KResult, KError};

/// Semantic version
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub prerelease: Option<String>,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
        }
    }

    /// Parse version from string (e.g., "1.2.3" or "1.2.3-beta1")
    pub fn parse(s: &str) -> Option<Self> {
        let (version_part, prerelease) = if let Some(idx) = s.find('-') {
            (&s[..idx], Some(String::from(&s[idx + 1..])))
        } else {
            (s, None)
        };

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() < 2 {
            return None;
        }

        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);

        Some(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        let base = alloc::format!("{}.{}.{}", self.major, self.minor, self.patch);
        if let Some(pre) = &self.prerelease {
            alloc::format!("{}-{}", base, pre)
        } else {
            base
        }
    }
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.prerelease {
            write!(f, "-{}", pre)?;
        }
        Ok(())
    }
}

/// Dependency specification
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Package name
    pub name: String,
    /// Version constraint
    pub version_constraint: VersionConstraint,
    /// Is this an optional dependency?
    pub optional: bool,
}

/// Version constraint types
#[derive(Debug, Clone)]
pub enum VersionConstraint {
    /// Any version
    Any,
    /// Exact version (=1.0.0)
    Exact(Version),
    /// Greater than or equal (>=1.0.0)
    GreaterOrEqual(Version),
    /// Less than (<2.0.0)
    LessThan(Version),
    /// Range (>=1.0.0,<2.0.0)
    Range(Version, Version),
}

impl VersionConstraint {
    /// Check if a version satisfies this constraint
    pub fn satisfies(&self, version: &Version) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(v) => version == v,
            Self::GreaterOrEqual(v) => version >= v,
            Self::LessThan(v) => version < v,
            Self::Range(min, max) => version >= min && version < max,
        }
    }

    /// Parse from string
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() || s == "*" {
            return Some(Self::Any);
        }

        if s.starts_with(">=") {
            let v = Version::parse(&s[2..])?;
            return Some(Self::GreaterOrEqual(v));
        }

        if s.starts_with('>') {
            let v = Version::parse(&s[1..])?;
            // Convert >X to >=X.0.1 (approximately)
            return Some(Self::GreaterOrEqual(Version::new(
                v.major,
                v.minor,
                v.patch + 1,
            )));
        }

        if s.starts_with("<=") {
            let v = Version::parse(&s[2..])?;
            return Some(Self::LessThan(Version::new(
                v.major,
                v.minor,
                v.patch + 1,
            )));
        }

        if s.starts_with('<') {
            let v = Version::parse(&s[1..])?;
            return Some(Self::LessThan(v));
        }

        if s.starts_with('=') {
            let v = Version::parse(&s[1..])?;
            return Some(Self::Exact(v));
        }

        // Range format: >=1.0.0,<2.0.0
        if s.contains(',') {
            let parts: Vec<&str> = s.split(',').collect();
            if parts.len() == 2 {
                if let (Some(Self::GreaterOrEqual(min)), Some(Self::LessThan(max))) =
                    (Self::parse(parts[0]), Self::parse(parts[1]))
                {
                    return Some(Self::Range(min, max));
                }
            }
        }

        // Plain version = exact match
        let v = Version::parse(s)?;
        Some(Self::Exact(v))
    }
}

/// Package metadata
#[derive(Debug, Clone)]
pub struct PackageMetadata {
    /// Package name
    pub name: String,
    /// Package version
    pub version: Version,
    /// Package description
    pub description: String,
    /// Package author(s)
    pub authors: Vec<String>,
    /// Package license
    pub license: String,
    /// Homepage URL
    pub homepage: Option<String>,
    /// Source repository URL
    pub repository: Option<String>,
    /// Runtime dependencies
    pub dependencies: Vec<Dependency>,
    /// Build dependencies
    pub build_dependencies: Vec<Dependency>,
    /// Optional dependencies
    pub optional_dependencies: Vec<Dependency>,
    /// Provides (virtual packages)
    pub provides: Vec<String>,
    /// Conflicts with
    pub conflicts: Vec<String>,
    /// Replaces
    pub replaces: Vec<String>,
    /// Installed size in bytes
    pub installed_size: u64,
    /// Architecture
    pub arch: String,
    /// Build date (Unix timestamp)
    pub build_date: u64,
}

impl PackageMetadata {
    /// Create new empty metadata
    pub fn new(name: &str, version: Version) -> Self {
        Self {
            name: String::from(name),
            version,
            description: String::new(),
            authors: Vec::new(),
            license: String::from("Unknown"),
            homepage: None,
            repository: None,
            dependencies: Vec::new(),
            build_dependencies: Vec::new(),
            optional_dependencies: Vec::new(),
            provides: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
            installed_size: 0,
            arch: String::from("x86_64"),
            build_date: 0,
        }
    }

    /// Parse metadata from TOML-like string
    pub fn parse(content: &str) -> KResult<Self> {
        let mut meta = Self::new("unknown", Version::new(0, 0, 0));

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match key {
                    "name" => meta.name = String::from(value),
                    "version" => {
                        meta.version = Version::parse(value)
                            .ok_or(KError::Invalid)?;
                    }
                    "description" => meta.description = String::from(value),
                    "license" => meta.license = String::from(value),
                    "homepage" => meta.homepage = Some(String::from(value)),
                    "repository" => meta.repository = Some(String::from(value)),
                    "arch" => meta.arch = String::from(value),
                    "installed_size" => {
                        meta.installed_size = value.parse().unwrap_or(0);
                    }
                    "build_date" => {
                        meta.build_date = value.parse().unwrap_or(0);
                    }
                    "authors" => {
                        meta.authors = value.split(',')
                            .map(|s| String::from(s.trim()))
                            .collect();
                    }
                    "depends" => {
                        for dep_str in value.split(',') {
                            if let Some(dep) = parse_dependency(dep_str.trim()) {
                                meta.dependencies.push(dep);
                            }
                        }
                    }
                    "optdepends" => {
                        for dep_str in value.split(',') {
                            if let Some(mut dep) = parse_dependency(dep_str.trim()) {
                                dep.optional = true;
                                meta.optional_dependencies.push(dep);
                            }
                        }
                    }
                    "provides" => {
                        meta.provides = value.split(',')
                            .map(|s| String::from(s.trim()))
                            .collect();
                    }
                    "conflicts" => {
                        meta.conflicts = value.split(',')
                            .map(|s| String::from(s.trim()))
                            .collect();
                    }
                    "replaces" => {
                        meta.replaces = value.split(',')
                            .map(|s| String::from(s.trim()))
                            .collect();
                    }
                    _ => {} // Ignore unknown keys
                }
            }
        }

        if meta.name == "unknown" {
            return Err(KError::Invalid);
        }

        Ok(meta)
    }

    /// Serialize to string
    pub fn serialize(&self) -> String {
        let mut s = String::new();

        s.push_str(&alloc::format!("name=\"{}\"\n", self.name));
        s.push_str(&alloc::format!("version=\"{}\"\n", self.version));
        s.push_str(&alloc::format!("description=\"{}\"\n", self.description));
        s.push_str(&alloc::format!("license=\"{}\"\n", self.license));
        s.push_str(&alloc::format!("arch=\"{}\"\n", self.arch));
        s.push_str(&alloc::format!("installed_size={}\n", self.installed_size));
        s.push_str(&alloc::format!("build_date={}\n", self.build_date));

        if !self.authors.is_empty() {
            s.push_str(&alloc::format!("authors=\"{}\"\n", self.authors.join(", ")));
        }

        if let Some(ref homepage) = self.homepage {
            s.push_str(&alloc::format!("homepage=\"{}\"\n", homepage));
        }

        if let Some(ref repo) = self.repository {
            s.push_str(&alloc::format!("repository=\"{}\"\n", repo));
        }

        if !self.dependencies.is_empty() {
            let deps: Vec<String> = self.dependencies.iter()
                .map(|d| d.name.clone())
                .collect();
            s.push_str(&alloc::format!("depends=\"{}\"\n", deps.join(", ")));
        }

        if !self.provides.is_empty() {
            s.push_str(&alloc::format!("provides=\"{}\"\n", self.provides.join(", ")));
        }

        if !self.conflicts.is_empty() {
            s.push_str(&alloc::format!("conflicts=\"{}\"\n", self.conflicts.join(", ")));
        }

        s
    }
}

/// Parse a dependency string (e.g., "foo>=1.0.0" or "bar")
fn parse_dependency(s: &str) -> Option<Dependency> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Find version constraint start
    let constraint_start = s.find(|c: char| c == '>' || c == '<' || c == '=');

    let (name, constraint) = if let Some(idx) = constraint_start {
        (&s[..idx], VersionConstraint::parse(&s[idx..]).unwrap_or(VersionConstraint::Any))
    } else {
        (s, VersionConstraint::Any)
    };

    Some(Dependency {
        name: String::from(name),
        version_constraint: constraint,
        optional: false,
    })
}

/// Package information (summary for display)
#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: Version,
    pub description: String,
    pub installed_size: u64,
    pub repository: Option<String>,
}

impl From<&PackageMetadata> for PackageInfo {
    fn from(meta: &PackageMetadata) -> Self {
        Self {
            name: meta.name.clone(),
            version: meta.version.clone(),
            description: meta.description.clone(),
            installed_size: meta.installed_size,
            repository: meta.repository.clone(),
        }
    }
}

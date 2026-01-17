//! Help System and User Manual
//!
//! Built-in documentation and user assistance.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

pub mod manual;
pub mod commands;
pub mod topics;

/// Help entry
#[derive(Debug, Clone)]
pub struct HelpEntry {
    pub name: String,
    pub category: HelpCategory,
    pub short_desc: String,
    pub long_desc: String,
    pub usage: Option<String>,
    pub examples: Vec<String>,
    pub see_also: Vec<String>,
}

/// Help category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpCategory {
    Command,
    Syscall,
    Concept,
    Tutorial,
    Faq,
}

impl HelpCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            HelpCategory::Command => "Commands",
            HelpCategory::Syscall => "System Calls",
            HelpCategory::Concept => "Concepts",
            HelpCategory::Tutorial => "Tutorials",
            HelpCategory::Faq => "FAQ",
        }
    }
}

/// Help system
pub struct HelpSystem {
    entries: Vec<HelpEntry>,
}

impl HelpSystem {
    pub fn new() -> Self {
        let mut system = Self {
            entries: Vec::new(),
        };
        system.load_builtin_help();
        system
    }

    /// Load built-in help entries
    fn load_builtin_help(&mut self) {
        // Register command help
        commands::register_help(self);

        // Register manual pages
        manual::register_help(self);

        // Register topic help
        topics::register_help(self);
    }

    /// Add a help entry
    pub fn add_entry(&mut self, entry: HelpEntry) {
        self.entries.push(entry);
    }

    /// Get help for a specific topic
    pub fn get(&self, name: &str) -> Option<&HelpEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Search help entries
    pub fn search(&self, query: &str) -> Vec<&HelpEntry> {
        let query_lower = query.to_lowercase();
        self.entries.iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&query_lower) ||
                e.short_desc.to_lowercase().contains(&query_lower) ||
                e.long_desc.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// List entries by category
    pub fn list_category(&self, category: HelpCategory) -> Vec<&HelpEntry> {
        self.entries.iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Format help entry for display
    pub fn format_entry(entry: &HelpEntry) -> String {
        let mut output = String::new();

        output.push_str(&format!("\n{}\n", entry.name.to_uppercase()));
        output.push_str(&format!("{}\n\n", "=".repeat(entry.name.len())));

        output.push_str("DESCRIPTION\n");
        output.push_str(&format!("    {}\n\n", entry.short_desc));

        if !entry.long_desc.is_empty() {
            output.push_str("DETAILS\n");
            for line in entry.long_desc.lines() {
                output.push_str(&format!("    {}\n", line));
            }
            output.push_str("\n");
        }

        if let Some(usage) = &entry.usage {
            output.push_str("USAGE\n");
            output.push_str(&format!("    {}\n\n", usage));
        }

        if !entry.examples.is_empty() {
            output.push_str("EXAMPLES\n");
            for example in &entry.examples {
                output.push_str(&format!("    {}\n", example));
            }
            output.push_str("\n");
        }

        if !entry.see_also.is_empty() {
            output.push_str("SEE ALSO\n");
            output.push_str(&format!("    {}\n", entry.see_also.join(", ")));
        }

        output
    }

    /// Get overview of all help topics
    pub fn get_overview(&self) -> String {
        let mut output = String::new();

        output.push_str("\nSTENZEL OS HELP\n");
        output.push_str("===============\n\n");

        output.push_str("Welcome to Stenzel OS! Type 'help <topic>' for detailed help.\n\n");

        // List categories
        for category in &[
            HelpCategory::Command,
            HelpCategory::Concept,
            HelpCategory::Tutorial,
            HelpCategory::Faq,
        ] {
            let entries = self.list_category(*category);
            if !entries.is_empty() {
                output.push_str(&format!("{}:\n", category.as_str()));
                for entry in entries {
                    output.push_str(&format!("    {:<20} {}\n", entry.name, entry.short_desc));
                }
                output.push_str("\n");
            }
        }

        output
    }
}

/// Create a help entry
pub fn help_entry(
    name: &str,
    category: HelpCategory,
    short_desc: &str,
    long_desc: &str,
) -> HelpEntry {
    HelpEntry {
        name: String::from(name),
        category,
        short_desc: String::from(short_desc),
        long_desc: String::from(long_desc),
        usage: None,
        examples: Vec::new(),
        see_also: Vec::new(),
    }
}

/// Create a command help entry
pub fn command_help(
    name: &str,
    short_desc: &str,
    long_desc: &str,
    usage: &str,
    examples: &[&str],
    see_also: &[&str],
) -> HelpEntry {
    HelpEntry {
        name: String::from(name),
        category: HelpCategory::Command,
        short_desc: String::from(short_desc),
        long_desc: String::from(long_desc),
        usage: Some(String::from(usage)),
        examples: examples.iter().map(|s| String::from(*s)).collect(),
        see_also: see_also.iter().map(|s| String::from(*s)).collect(),
    }
}

/// Initialize help system
pub fn init() -> HelpSystem {
    HelpSystem::new()
}

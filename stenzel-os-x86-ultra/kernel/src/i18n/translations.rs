//! Translation catalog management

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Translation key type
pub type TranslationKey = String;

/// Translation catalog for a single locale
#[derive(Debug, Clone)]
pub struct Translations {
    /// Key-value pairs
    entries: BTreeMap<String, String>,
    /// Plural forms
    plurals: BTreeMap<String, PluralForms>,
}

/// Plural forms for a translation
#[derive(Debug, Clone)]
pub struct PluralForms {
    /// Zero items
    pub zero: Option<String>,
    /// One item (singular)
    pub one: String,
    /// Two items (for languages with dual)
    pub two: Option<String>,
    /// Few items (for languages with special few form)
    pub few: Option<String>,
    /// Many items
    pub many: Option<String>,
    /// Other (default plural)
    pub other: String,
}

impl Translations {
    /// Create a new empty translation catalog
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            plurals: BTreeMap::new(),
        }
    }

    /// Add a simple translation
    pub fn add(&mut self, key: &str, value: &str) {
        self.entries.insert(key.to_string(), value.to_string());
    }

    /// Add a plural translation
    pub fn add_plural(&mut self, key: &str, one: &str, other: &str) {
        self.plurals.insert(key.to_string(), PluralForms {
            zero: None,
            one: one.to_string(),
            two: None,
            few: None,
            many: None,
            other: other.to_string(),
        });
    }

    /// Add a plural translation with all forms
    pub fn add_plural_full(&mut self, key: &str, forms: PluralForms) {
        self.plurals.insert(key.to_string(), forms);
    }

    /// Get a translation
    pub fn get(&self, key: &str) -> Option<&String> {
        self.entries.get(key)
    }

    /// Get a plural translation
    pub fn get_plural(&self, key: &str, count: usize) -> Option<String> {
        self.plurals.get(key).map(|forms| {
            // English-style plural rules
            if count == 0 {
                forms.zero.clone().unwrap_or_else(|| forms.other.clone())
            } else if count == 1 {
                forms.one.clone()
            } else {
                forms.other.clone()
            }
        })
    }

    /// Get all keys
    pub fn keys(&self) -> Vec<&String> {
        self.entries.keys().collect()
    }

    /// Check if key exists
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key) || self.plurals.contains_key(key)
    }

    /// Merge another catalog into this one
    pub fn merge(&mut self, other: &Translations) {
        for (key, value) in &other.entries {
            self.entries.insert(key.clone(), value.clone());
        }
        for (key, forms) in &other.plurals {
            self.plurals.insert(key.clone(), forms.clone());
        }
    }

    /// Remove a translation
    pub fn remove(&mut self, key: &str) {
        self.entries.remove(key);
        self.plurals.remove(key);
    }

    /// Number of entries
    pub fn len(&self) -> usize {
        self.entries.len() + self.plurals.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.plurals.is_empty()
    }
}

impl Default for Translations {
    fn default() -> Self {
        Self::new()
    }
}

/// Translation context for interpolation
#[derive(Debug, Clone)]
pub struct TranslationContext {
    /// Variable substitutions
    vars: BTreeMap<String, String>,
}

impl TranslationContext {
    /// Create new context
    pub fn new() -> Self {
        Self {
            vars: BTreeMap::new(),
        }
    }

    /// Set a variable
    pub fn set(&mut self, name: &str, value: &str) -> &mut Self {
        self.vars.insert(name.to_string(), value.to_string());
        self
    }

    /// Apply substitutions to a string
    pub fn apply(&self, text: &str) -> String {
        let mut result = text.to_string();
        for (name, value) in &self.vars {
            result = result.replace(&alloc::format!("{{{}}}", name), value);
        }
        result
    }
}

impl Default for TranslationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Plural rule categories (based on CLDR)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

/// Get plural category for a language
pub fn get_plural_category(language_code: &str, count: usize) -> PluralCategory {
    match language_code {
        // English, German, Spanish, Portuguese, Italian, etc. (simple singular/plural)
        "en" | "de" | "es" | "pt" | "it" | "nl" | "sv" | "da" | "no" | "fi" => {
            if count == 1 { PluralCategory::One } else { PluralCategory::Other }
        }

        // French (0 and 1 are singular)
        "fr" => {
            if count == 0 || count == 1 { PluralCategory::One } else { PluralCategory::Other }
        }

        // Russian, Ukrainian, Polish (complex)
        "ru" | "uk" => {
            let mod10 = count % 10;
            let mod100 = count % 100;
            if mod10 == 1 && mod100 != 11 {
                PluralCategory::One
            } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
                PluralCategory::Few
            } else {
                PluralCategory::Many
            }
        }

        "pl" => {
            let mod10 = count % 10;
            let mod100 = count % 100;
            if count == 1 {
                PluralCategory::One
            } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
                PluralCategory::Few
            } else {
                PluralCategory::Many
            }
        }

        // Arabic (6 forms)
        "ar" => {
            let mod100 = count % 100;
            if count == 0 {
                PluralCategory::Zero
            } else if count == 1 {
                PluralCategory::One
            } else if count == 2 {
                PluralCategory::Two
            } else if (3..=10).contains(&mod100) {
                PluralCategory::Few
            } else if (11..=99).contains(&mod100) {
                PluralCategory::Many
            } else {
                PluralCategory::Other
            }
        }

        // Chinese, Japanese, Korean, Vietnamese (no plural)
        "zh" | "ja" | "ko" | "vi" | "th" | "id" | "ms" => {
            PluralCategory::Other
        }

        // Default to simple singular/plural
        _ => {
            if count == 1 { PluralCategory::One } else { PluralCategory::Other }
        }
    }
}

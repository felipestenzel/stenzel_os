//! Internationalization (i18n) Module
//!
//! Provides locale system, translations framework, and regional formats.
//!
//! Features:
//! - Locale detection and management
//! - Message translations with interpolation
//! - Date/time formatting
//! - Number formatting
//! - Currency formatting

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;
use core::sync::atomic::{AtomicUsize, Ordering};

use spin::Once;

use crate::sync::IrqSafeMutex;
use crate::util::KResult;

pub mod locale;
pub mod translations;
pub mod formats;

pub use locale::{Locale, LocaleId, Language, Country};
pub use translations::{TranslationKey, Translations};
pub use formats::{DateFormat, TimeFormat, NumberFormat, CurrencyFormat};

/// Global i18n manager
static I18N_MANAGER: Once<I18nManager> = Once::new();

/// Initialize the i18n subsystem
pub fn init() {
    I18N_MANAGER.call_once(|| {
        let mut mgr = I18nManager::new();
        mgr.register_default_locales();
        mgr.load_default_translations();
        mgr
    });
    crate::kprintln!("i18n: Internationalization subsystem initialized");
}

/// Get the i18n manager
pub fn manager() -> &'static I18nManager {
    I18N_MANAGER.get().expect("i18n not initialized")
}

/// Main i18n manager
pub struct I18nManager {
    /// Available locales
    locales: IrqSafeMutex<BTreeMap<LocaleId, Locale>>,
    /// Current locale
    current_locale: IrqSafeMutex<LocaleId>,
    /// Fallback locale
    fallback_locale: LocaleId,
    /// Translation catalogs
    translations: IrqSafeMutex<BTreeMap<LocaleId, Translations>>,
}

impl I18nManager {
    pub fn new() -> Self {
        Self {
            locales: IrqSafeMutex::new(BTreeMap::new()),
            current_locale: IrqSafeMutex::new(LocaleId::EN_US),
            fallback_locale: LocaleId::EN_US,
            translations: IrqSafeMutex::new(BTreeMap::new()),
        }
    }

    /// Register a locale
    pub fn register_locale(&self, locale: Locale) {
        self.locales.lock().insert(locale.id, locale);
    }

    /// Get current locale
    pub fn current_locale(&self) -> LocaleId {
        *self.current_locale.lock()
    }

    /// Set current locale
    pub fn set_locale(&self, locale_id: LocaleId) -> bool {
        if self.locales.lock().contains_key(&locale_id) {
            *self.current_locale.lock() = locale_id;
            crate::kprintln!("i18n: Locale set to {:?}", locale_id);
            true
        } else {
            false
        }
    }

    /// Get locale by ID
    pub fn get_locale(&self, id: LocaleId) -> Option<Locale> {
        self.locales.lock().get(&id).cloned()
    }

    /// List available locales
    pub fn available_locales(&self) -> Vec<LocaleId> {
        self.locales.lock().keys().cloned().collect()
    }

    /// Translate a key
    pub fn translate(&self, key: &str) -> String {
        let current = *self.current_locale.lock();
        let translations = self.translations.lock();

        // Try current locale
        if let Some(catalog) = translations.get(&current) {
            if let Some(text) = catalog.get(key) {
                return text.clone();
            }
        }

        // Try fallback
        if let Some(catalog) = translations.get(&self.fallback_locale) {
            if let Some(text) = catalog.get(key) {
                return text.clone();
            }
        }

        // Return key if no translation found
        key.to_string()
    }

    /// Translate with arguments
    pub fn translate_with_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.translate(key);
        for (name, value) in args {
            text = text.replace(&format!("{{{}}}", name), value);
        }
        text
    }

    /// Add translation catalog
    pub fn add_translations(&self, locale: LocaleId, catalog: Translations) {
        self.translations.lock().insert(locale, catalog);
    }

    /// Register default locales
    fn register_default_locales(&mut self) {
        // English (US)
        self.register_locale(Locale {
            id: LocaleId::EN_US,
            language: Language::English,
            country: Country::UnitedStates,
            name: "English (US)".to_string(),
            native_name: "English (US)".to_string(),
            date_format: DateFormat::MDY,
            time_format: TimeFormat::H12,
            first_day_of_week: 0, // Sunday
            decimal_separator: '.',
            thousands_separator: ',',
            currency_symbol: "$".to_string(),
            currency_position: CurrencyPosition::Before,
        });

        // Portuguese (Brazil)
        self.register_locale(Locale {
            id: LocaleId::PT_BR,
            language: Language::Portuguese,
            country: Country::Brazil,
            name: "Portuguese (Brazil)".to_string(),
            native_name: "Português (Brasil)".to_string(),
            date_format: DateFormat::DMY,
            time_format: TimeFormat::H24,
            first_day_of_week: 1, // Monday
            decimal_separator: ',',
            thousands_separator: '.',
            currency_symbol: "R$".to_string(),
            currency_position: CurrencyPosition::Before,
        });

        // Spanish
        self.register_locale(Locale {
            id: LocaleId::ES_ES,
            language: Language::Spanish,
            country: Country::Spain,
            name: "Spanish (Spain)".to_string(),
            native_name: "Español (España)".to_string(),
            date_format: DateFormat::DMY,
            time_format: TimeFormat::H24,
            first_day_of_week: 1,
            decimal_separator: ',',
            thousands_separator: '.',
            currency_symbol: "€".to_string(),
            currency_position: CurrencyPosition::After,
        });

        // German
        self.register_locale(Locale {
            id: LocaleId::DE_DE,
            language: Language::German,
            country: Country::Germany,
            name: "German (Germany)".to_string(),
            native_name: "Deutsch (Deutschland)".to_string(),
            date_format: DateFormat::DMY,
            time_format: TimeFormat::H24,
            first_day_of_week: 1,
            decimal_separator: ',',
            thousands_separator: '.',
            currency_symbol: "€".to_string(),
            currency_position: CurrencyPosition::After,
        });

        // French
        self.register_locale(Locale {
            id: LocaleId::FR_FR,
            language: Language::French,
            country: Country::France,
            name: "French (France)".to_string(),
            native_name: "Français (France)".to_string(),
            date_format: DateFormat::DMY,
            time_format: TimeFormat::H24,
            first_day_of_week: 1,
            decimal_separator: ',',
            thousands_separator: ' ',
            currency_symbol: "€".to_string(),
            currency_position: CurrencyPosition::After,
        });

        // Japanese
        self.register_locale(Locale {
            id: LocaleId::JA_JP,
            language: Language::Japanese,
            country: Country::Japan,
            name: "Japanese".to_string(),
            native_name: "日本語".to_string(),
            date_format: DateFormat::YMD,
            time_format: TimeFormat::H24,
            first_day_of_week: 0,
            decimal_separator: '.',
            thousands_separator: ',',
            currency_symbol: "¥".to_string(),
            currency_position: CurrencyPosition::Before,
        });

        // Chinese (Simplified)
        self.register_locale(Locale {
            id: LocaleId::ZH_CN,
            language: Language::Chinese,
            country: Country::China,
            name: "Chinese (Simplified)".to_string(),
            native_name: "简体中文".to_string(),
            date_format: DateFormat::YMD,
            time_format: TimeFormat::H24,
            first_day_of_week: 1,
            decimal_separator: '.',
            thousands_separator: ',',
            currency_symbol: "¥".to_string(),
            currency_position: CurrencyPosition::Before,
        });

        // Korean
        self.register_locale(Locale {
            id: LocaleId::KO_KR,
            language: Language::Korean,
            country: Country::SouthKorea,
            name: "Korean".to_string(),
            native_name: "한국어".to_string(),
            date_format: DateFormat::YMD,
            time_format: TimeFormat::H12,
            first_day_of_week: 0,
            decimal_separator: '.',
            thousands_separator: ',',
            currency_symbol: "₩".to_string(),
            currency_position: CurrencyPosition::Before,
        });
    }

    /// Load default translations
    fn load_default_translations(&mut self) {
        // English translations
        let mut en = Translations::new();
        en.add("app.name", "Stenzel OS");
        en.add("welcome", "Welcome to Stenzel OS");
        en.add("login.username", "Username");
        en.add("login.password", "Password");
        en.add("login.submit", "Log In");
        en.add("login.failed", "Invalid username or password");
        en.add("logout", "Log Out");
        en.add("settings", "Settings");
        en.add("settings.display", "Display");
        en.add("settings.sound", "Sound");
        en.add("settings.network", "Network");
        en.add("settings.bluetooth", "Bluetooth");
        en.add("settings.users", "Users & Accounts");
        en.add("settings.privacy", "Privacy & Security");
        en.add("settings.keyboard", "Keyboard");
        en.add("settings.mouse", "Mouse & Touchpad");
        en.add("settings.power", "Power");
        en.add("settings.about", "About");
        en.add("file.open", "Open");
        en.add("file.save", "Save");
        en.add("file.save_as", "Save As...");
        en.add("file.close", "Close");
        en.add("file.new", "New");
        en.add("file.delete", "Delete");
        en.add("file.rename", "Rename");
        en.add("file.copy", "Copy");
        en.add("file.paste", "Paste");
        en.add("file.cut", "Cut");
        en.add("edit.undo", "Undo");
        en.add("edit.redo", "Redo");
        en.add("edit.select_all", "Select All");
        en.add("error", "Error");
        en.add("warning", "Warning");
        en.add("info", "Information");
        en.add("confirm", "Confirm");
        en.add("cancel", "Cancel");
        en.add("ok", "OK");
        en.add("yes", "Yes");
        en.add("no", "No");
        en.add("search", "Search");
        en.add("help", "Help");
        en.add("quit", "Quit");
        en.add("shutdown", "Shut Down");
        en.add("restart", "Restart");
        en.add("suspend", "Suspend");
        en.add("lock", "Lock Screen");
        self.add_translations(LocaleId::EN_US, en);

        // Portuguese (Brazil) translations
        let mut pt_br = Translations::new();
        pt_br.add("app.name", "Stenzel OS");
        pt_br.add("welcome", "Bem-vindo ao Stenzel OS");
        pt_br.add("login.username", "Nome de usuário");
        pt_br.add("login.password", "Senha");
        pt_br.add("login.submit", "Entrar");
        pt_br.add("login.failed", "Nome de usuário ou senha inválidos");
        pt_br.add("logout", "Sair");
        pt_br.add("settings", "Configurações");
        pt_br.add("settings.display", "Tela");
        pt_br.add("settings.sound", "Som");
        pt_br.add("settings.network", "Rede");
        pt_br.add("settings.bluetooth", "Bluetooth");
        pt_br.add("settings.users", "Usuários e Contas");
        pt_br.add("settings.privacy", "Privacidade e Segurança");
        pt_br.add("settings.keyboard", "Teclado");
        pt_br.add("settings.mouse", "Mouse e Touchpad");
        pt_br.add("settings.power", "Energia");
        pt_br.add("settings.about", "Sobre");
        pt_br.add("file.open", "Abrir");
        pt_br.add("file.save", "Salvar");
        pt_br.add("file.save_as", "Salvar como...");
        pt_br.add("file.close", "Fechar");
        pt_br.add("file.new", "Novo");
        pt_br.add("file.delete", "Excluir");
        pt_br.add("file.rename", "Renomear");
        pt_br.add("file.copy", "Copiar");
        pt_br.add("file.paste", "Colar");
        pt_br.add("file.cut", "Recortar");
        pt_br.add("edit.undo", "Desfazer");
        pt_br.add("edit.redo", "Refazer");
        pt_br.add("edit.select_all", "Selecionar tudo");
        pt_br.add("error", "Erro");
        pt_br.add("warning", "Aviso");
        pt_br.add("info", "Informação");
        pt_br.add("confirm", "Confirmar");
        pt_br.add("cancel", "Cancelar");
        pt_br.add("ok", "OK");
        pt_br.add("yes", "Sim");
        pt_br.add("no", "Não");
        pt_br.add("search", "Buscar");
        pt_br.add("help", "Ajuda");
        pt_br.add("quit", "Sair");
        pt_br.add("shutdown", "Desligar");
        pt_br.add("restart", "Reiniciar");
        pt_br.add("suspend", "Suspender");
        pt_br.add("lock", "Bloquear tela");
        self.add_translations(LocaleId::PT_BR, pt_br);
    }
}

impl Default for I18nManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Currency symbol position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrencyPosition {
    Before,
    After,
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Translate a key using current locale
pub fn t(key: &str) -> String {
    manager().translate(key)
}

/// Translate with arguments
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    manager().translate_with_args(key, args)
}

/// Get current locale
pub fn current_locale() -> LocaleId {
    manager().current_locale()
}

/// Set current locale
pub fn set_locale(id: LocaleId) -> bool {
    manager().set_locale(id)
}

/// Format a date using current locale
pub fn format_date(year: i32, month: u32, day: u32) -> String {
    let locale = manager().get_locale(current_locale()).unwrap_or_else(|| {
        manager().get_locale(LocaleId::EN_US).unwrap()
    });

    match locale.date_format {
        DateFormat::DMY => format!("{:02}/{:02}/{}", day, month, year),
        DateFormat::MDY => format!("{:02}/{:02}/{}", month, day, year),
        DateFormat::YMD => format!("{}-{:02}-{:02}", year, month, day),
    }
}

/// Format a time using current locale
pub fn format_time(hour: u32, minute: u32, second: u32) -> String {
    let locale = manager().get_locale(current_locale()).unwrap_or_else(|| {
        manager().get_locale(LocaleId::EN_US).unwrap()
    });

    match locale.time_format {
        TimeFormat::H12 => {
            let (h, period) = if hour == 0 {
                (12, "AM")
            } else if hour < 12 {
                (hour, "AM")
            } else if hour == 12 {
                (12, "PM")
            } else {
                (hour - 12, "PM")
            };
            format!("{}:{:02}:{:02} {}", h, minute, second, period)
        }
        TimeFormat::H24 => {
            format!("{:02}:{:02}:{:02}", hour, minute, second)
        }
    }
}

/// Format a number using current locale
pub fn format_number(n: i64) -> String {
    let locale = manager().get_locale(current_locale()).unwrap_or_else(|| {
        manager().get_locale(LocaleId::EN_US).unwrap()
    });

    let is_negative = n < 0;
    let abs_n = n.abs() as u64;
    let s = abs_n.to_string();

    // Insert thousands separators
    let mut result = String::new();
    let len = s.len();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(locale.thousands_separator);
        }
        result.push(c);
    }

    if is_negative {
        result.insert(0, '-');
    }

    result
}

/// Format a decimal number using current locale
pub fn format_decimal(n: f64, decimals: usize) -> String {
    let locale = manager().get_locale(current_locale()).unwrap_or_else(|| {
        manager().get_locale(LocaleId::EN_US).unwrap()
    });

    let formatted = format!("{:.prec$}", n.abs(), prec = decimals);
    let parts: Vec<&str> = formatted.split('.').collect();

    let integer_part = if parts[0].len() > 3 {
        // Add thousands separator
        let s = parts[0];
        let mut result = String::new();
        let len = s.len();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (len - i) % 3 == 0 {
                result.push(locale.thousands_separator);
            }
            result.push(c);
        }
        result
    } else {
        parts[0].to_string()
    };

    let result = if decimals > 0 && parts.len() > 1 {
        format!("{}{}{}", integer_part, locale.decimal_separator, parts[1])
    } else {
        integer_part
    };

    if n < 0.0 {
        format!("-{}", result)
    } else {
        result
    }
}

/// Format currency using current locale
pub fn format_currency(amount: f64) -> String {
    let locale = manager().get_locale(current_locale()).unwrap_or_else(|| {
        manager().get_locale(LocaleId::EN_US).unwrap()
    });

    let formatted = format_decimal(amount.abs(), 2);
    let result = match locale.currency_position {
        CurrencyPosition::Before => format!("{}{}", locale.currency_symbol, formatted),
        CurrencyPosition::After => format!("{} {}", formatted, locale.currency_symbol),
    };

    if amount < 0.0 {
        format!("-{}", result)
    } else {
        result
    }
}

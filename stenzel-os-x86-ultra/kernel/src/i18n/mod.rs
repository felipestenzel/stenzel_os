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
pub mod ibus;
pub mod pinyin;
pub mod japanese;
pub mod hangul;
pub mod arabic;
pub mod emoji;

pub use locale::{Locale, LocaleId, Language, Country};
pub use translations::{TranslationKey, Translations};
pub use formats::{DateFormat, TimeFormat, NumberFormat, CurrencyFormat};
pub use ibus::{
    IBusManager, IBusConfig, IBusStats, InputMethodType, InputMethodState,
    Candidate, InputEvent, KeyModifiers, InputResult, InputMethodEngine,
};
pub use pinyin::{PinyinEngine, PinyinConfig};
pub use japanese::{JapaneseEngine, JapaneseConfig, JapaneseMode};
pub use hangul::{HangulEngine, HangulConfig};
pub use arabic::{ArabicEngine, ArabicConfig, ArabicLayout};
pub use emoji::{EmojiPicker, EmojiPickerConfig, EmojiCategory, Emoji, SkinTone, PickerState};

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
    ibus::init();
    emoji::init();
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

        // Spanish (Spain) translations
        let mut es = Translations::new();
        es.add("app.name", "Stenzel OS");
        es.add("welcome", "Bienvenido a Stenzel OS");
        es.add("login.username", "Nombre de usuario");
        es.add("login.password", "Contraseña");
        es.add("login.submit", "Iniciar sesión");
        es.add("login.failed", "Nombre de usuario o contraseña no válidos");
        es.add("logout", "Cerrar sesión");
        es.add("settings", "Configuración");
        es.add("settings.display", "Pantalla");
        es.add("settings.sound", "Sonido");
        es.add("settings.network", "Red");
        es.add("settings.bluetooth", "Bluetooth");
        es.add("settings.users", "Usuarios y cuentas");
        es.add("settings.privacy", "Privacidad y seguridad");
        es.add("settings.keyboard", "Teclado");
        es.add("settings.mouse", "Ratón y touchpad");
        es.add("settings.power", "Energía");
        es.add("settings.about", "Acerca de");
        es.add("file.open", "Abrir");
        es.add("file.save", "Guardar");
        es.add("file.save_as", "Guardar como...");
        es.add("file.close", "Cerrar");
        es.add("file.new", "Nuevo");
        es.add("file.delete", "Eliminar");
        es.add("file.rename", "Renombrar");
        es.add("file.copy", "Copiar");
        es.add("file.paste", "Pegar");
        es.add("file.cut", "Cortar");
        es.add("edit.undo", "Deshacer");
        es.add("edit.redo", "Rehacer");
        es.add("edit.select_all", "Seleccionar todo");
        es.add("error", "Error");
        es.add("warning", "Advertencia");
        es.add("info", "Información");
        es.add("confirm", "Confirmar");
        es.add("cancel", "Cancelar");
        es.add("ok", "Aceptar");
        es.add("yes", "Sí");
        es.add("no", "No");
        es.add("search", "Buscar");
        es.add("help", "Ayuda");
        es.add("quit", "Salir");
        es.add("shutdown", "Apagar");
        es.add("restart", "Reiniciar");
        es.add("suspend", "Suspender");
        es.add("lock", "Bloquear pantalla");
        self.add_translations(LocaleId::ES_ES, es);

        // French (France) translations
        let mut fr = Translations::new();
        fr.add("app.name", "Stenzel OS");
        fr.add("welcome", "Bienvenue sur Stenzel OS");
        fr.add("login.username", "Nom d'utilisateur");
        fr.add("login.password", "Mot de passe");
        fr.add("login.submit", "Connexion");
        fr.add("login.failed", "Nom d'utilisateur ou mot de passe invalide");
        fr.add("logout", "Déconnexion");
        fr.add("settings", "Paramètres");
        fr.add("settings.display", "Affichage");
        fr.add("settings.sound", "Son");
        fr.add("settings.network", "Réseau");
        fr.add("settings.bluetooth", "Bluetooth");
        fr.add("settings.users", "Utilisateurs et comptes");
        fr.add("settings.privacy", "Confidentialité et sécurité");
        fr.add("settings.keyboard", "Clavier");
        fr.add("settings.mouse", "Souris et pavé tactile");
        fr.add("settings.power", "Alimentation");
        fr.add("settings.about", "À propos");
        fr.add("file.open", "Ouvrir");
        fr.add("file.save", "Enregistrer");
        fr.add("file.save_as", "Enregistrer sous...");
        fr.add("file.close", "Fermer");
        fr.add("file.new", "Nouveau");
        fr.add("file.delete", "Supprimer");
        fr.add("file.rename", "Renommer");
        fr.add("file.copy", "Copier");
        fr.add("file.paste", "Coller");
        fr.add("file.cut", "Couper");
        fr.add("edit.undo", "Annuler");
        fr.add("edit.redo", "Rétablir");
        fr.add("edit.select_all", "Tout sélectionner");
        fr.add("error", "Erreur");
        fr.add("warning", "Avertissement");
        fr.add("info", "Information");
        fr.add("confirm", "Confirmer");
        fr.add("cancel", "Annuler");
        fr.add("ok", "OK");
        fr.add("yes", "Oui");
        fr.add("no", "Non");
        fr.add("search", "Rechercher");
        fr.add("help", "Aide");
        fr.add("quit", "Quitter");
        fr.add("shutdown", "Éteindre");
        fr.add("restart", "Redémarrer");
        fr.add("suspend", "Veille");
        fr.add("lock", "Verrouiller l'écran");
        self.add_translations(LocaleId::FR_FR, fr);

        // German (Germany) translations
        let mut de = Translations::new();
        de.add("app.name", "Stenzel OS");
        de.add("welcome", "Willkommen bei Stenzel OS");
        de.add("login.username", "Benutzername");
        de.add("login.password", "Passwort");
        de.add("login.submit", "Anmelden");
        de.add("login.failed", "Ungültiger Benutzername oder Passwort");
        de.add("logout", "Abmelden");
        de.add("settings", "Einstellungen");
        de.add("settings.display", "Anzeige");
        de.add("settings.sound", "Ton");
        de.add("settings.network", "Netzwerk");
        de.add("settings.bluetooth", "Bluetooth");
        de.add("settings.users", "Benutzer und Konten");
        de.add("settings.privacy", "Datenschutz und Sicherheit");
        de.add("settings.keyboard", "Tastatur");
        de.add("settings.mouse", "Maus und Touchpad");
        de.add("settings.power", "Energie");
        de.add("settings.about", "Über");
        de.add("file.open", "Öffnen");
        de.add("file.save", "Speichern");
        de.add("file.save_as", "Speichern unter...");
        de.add("file.close", "Schließen");
        de.add("file.new", "Neu");
        de.add("file.delete", "Löschen");
        de.add("file.rename", "Umbenennen");
        de.add("file.copy", "Kopieren");
        de.add("file.paste", "Einfügen");
        de.add("file.cut", "Ausschneiden");
        de.add("edit.undo", "Rückgängig");
        de.add("edit.redo", "Wiederherstellen");
        de.add("edit.select_all", "Alles auswählen");
        de.add("error", "Fehler");
        de.add("warning", "Warnung");
        de.add("info", "Information");
        de.add("confirm", "Bestätigen");
        de.add("cancel", "Abbrechen");
        de.add("ok", "OK");
        de.add("yes", "Ja");
        de.add("no", "Nein");
        de.add("search", "Suchen");
        de.add("help", "Hilfe");
        de.add("quit", "Beenden");
        de.add("shutdown", "Herunterfahren");
        de.add("restart", "Neustart");
        de.add("suspend", "Standby");
        de.add("lock", "Bildschirm sperren");
        self.add_translations(LocaleId::DE_DE, de);

        // Chinese (Simplified) translations
        let mut zh = Translations::new();
        zh.add("app.name", "Stenzel OS");
        zh.add("welcome", "欢迎使用 Stenzel OS");
        zh.add("login.username", "用户名");
        zh.add("login.password", "密码");
        zh.add("login.submit", "登录");
        zh.add("login.failed", "用户名或密码无效");
        zh.add("logout", "注销");
        zh.add("settings", "设置");
        zh.add("settings.display", "显示");
        zh.add("settings.sound", "声音");
        zh.add("settings.network", "网络");
        zh.add("settings.bluetooth", "蓝牙");
        zh.add("settings.users", "用户与账户");
        zh.add("settings.privacy", "隐私与安全");
        zh.add("settings.keyboard", "键盘");
        zh.add("settings.mouse", "鼠标与触控板");
        zh.add("settings.power", "电源");
        zh.add("settings.about", "关于");
        zh.add("file.open", "打开");
        zh.add("file.save", "保存");
        zh.add("file.save_as", "另存为...");
        zh.add("file.close", "关闭");
        zh.add("file.new", "新建");
        zh.add("file.delete", "删除");
        zh.add("file.rename", "重命名");
        zh.add("file.copy", "复制");
        zh.add("file.paste", "粘贴");
        zh.add("file.cut", "剪切");
        zh.add("edit.undo", "撤销");
        zh.add("edit.redo", "重做");
        zh.add("edit.select_all", "全选");
        zh.add("error", "错误");
        zh.add("warning", "警告");
        zh.add("info", "信息");
        zh.add("confirm", "确认");
        zh.add("cancel", "取消");
        zh.add("ok", "确定");
        zh.add("yes", "是");
        zh.add("no", "否");
        zh.add("search", "搜索");
        zh.add("help", "帮助");
        zh.add("quit", "退出");
        zh.add("shutdown", "关机");
        zh.add("restart", "重启");
        zh.add("suspend", "休眠");
        zh.add("lock", "锁定屏幕");
        self.add_translations(LocaleId::ZH_CN, zh);

        // Japanese translations
        let mut ja = Translations::new();
        ja.add("app.name", "Stenzel OS");
        ja.add("welcome", "Stenzel OSへようこそ");
        ja.add("login.username", "ユーザー名");
        ja.add("login.password", "パスワード");
        ja.add("login.submit", "ログイン");
        ja.add("login.failed", "ユーザー名またはパスワードが無効です");
        ja.add("logout", "ログアウト");
        ja.add("settings", "設定");
        ja.add("settings.display", "ディスプレイ");
        ja.add("settings.sound", "サウンド");
        ja.add("settings.network", "ネットワーク");
        ja.add("settings.bluetooth", "Bluetooth");
        ja.add("settings.users", "ユーザーとアカウント");
        ja.add("settings.privacy", "プライバシーとセキュリティ");
        ja.add("settings.keyboard", "キーボード");
        ja.add("settings.mouse", "マウスとタッチパッド");
        ja.add("settings.power", "電源");
        ja.add("settings.about", "このシステムについて");
        ja.add("file.open", "開く");
        ja.add("file.save", "保存");
        ja.add("file.save_as", "名前を付けて保存...");
        ja.add("file.close", "閉じる");
        ja.add("file.new", "新規");
        ja.add("file.delete", "削除");
        ja.add("file.rename", "名前を変更");
        ja.add("file.copy", "コピー");
        ja.add("file.paste", "貼り付け");
        ja.add("file.cut", "切り取り");
        ja.add("edit.undo", "元に戻す");
        ja.add("edit.redo", "やり直し");
        ja.add("edit.select_all", "すべて選択");
        ja.add("error", "エラー");
        ja.add("warning", "警告");
        ja.add("info", "情報");
        ja.add("confirm", "確認");
        ja.add("cancel", "キャンセル");
        ja.add("ok", "OK");
        ja.add("yes", "はい");
        ja.add("no", "いいえ");
        ja.add("search", "検索");
        ja.add("help", "ヘルプ");
        ja.add("quit", "終了");
        ja.add("shutdown", "シャットダウン");
        ja.add("restart", "再起動");
        ja.add("suspend", "スリープ");
        ja.add("lock", "画面をロック");
        self.add_translations(LocaleId::JA_JP, ja);

        // Korean translations
        let mut ko = Translations::new();
        ko.add("app.name", "Stenzel OS");
        ko.add("welcome", "Stenzel OS에 오신 것을 환영합니다");
        ko.add("login.username", "사용자 이름");
        ko.add("login.password", "비밀번호");
        ko.add("login.submit", "로그인");
        ko.add("login.failed", "사용자 이름 또는 비밀번호가 올바르지 않습니다");
        ko.add("logout", "로그아웃");
        ko.add("settings", "설정");
        ko.add("settings.display", "디스플레이");
        ko.add("settings.sound", "소리");
        ko.add("settings.network", "네트워크");
        ko.add("settings.bluetooth", "블루투스");
        ko.add("settings.users", "사용자 및 계정");
        ko.add("settings.privacy", "개인정보 및 보안");
        ko.add("settings.keyboard", "키보드");
        ko.add("settings.mouse", "마우스 및 터치패드");
        ko.add("settings.power", "전원");
        ko.add("settings.about", "정보");
        ko.add("file.open", "열기");
        ko.add("file.save", "저장");
        ko.add("file.save_as", "다른 이름으로 저장...");
        ko.add("file.close", "닫기");
        ko.add("file.new", "새로 만들기");
        ko.add("file.delete", "삭제");
        ko.add("file.rename", "이름 바꾸기");
        ko.add("file.copy", "복사");
        ko.add("file.paste", "붙여넣기");
        ko.add("file.cut", "잘라내기");
        ko.add("edit.undo", "실행 취소");
        ko.add("edit.redo", "다시 실행");
        ko.add("edit.select_all", "모두 선택");
        ko.add("error", "오류");
        ko.add("warning", "경고");
        ko.add("info", "정보");
        ko.add("confirm", "확인");
        ko.add("cancel", "취소");
        ko.add("ok", "확인");
        ko.add("yes", "예");
        ko.add("no", "아니오");
        ko.add("search", "검색");
        ko.add("help", "도움말");
        ko.add("quit", "종료");
        ko.add("shutdown", "시스템 종료");
        ko.add("restart", "다시 시작");
        ko.add("suspend", "절전");
        ko.add("lock", "화면 잠금");
        self.add_translations(LocaleId::KO_KR, ko);
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

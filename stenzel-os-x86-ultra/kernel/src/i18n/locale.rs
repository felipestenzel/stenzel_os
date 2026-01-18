//! Locale definitions

use alloc::string::String;
use super::formats::{DateFormat, TimeFormat};
use super::CurrencyPosition;

/// Locale identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocaleId(pub u32);

impl LocaleId {
    // Common locale IDs
    pub const EN_US: LocaleId = LocaleId(0x0409);  // English (United States)
    pub const EN_GB: LocaleId = LocaleId(0x0809);  // English (United Kingdom)
    pub const PT_BR: LocaleId = LocaleId(0x0416);  // Portuguese (Brazil)
    pub const PT_PT: LocaleId = LocaleId(0x0816);  // Portuguese (Portugal)
    pub const ES_ES: LocaleId = LocaleId(0x0C0A);  // Spanish (Spain)
    pub const ES_MX: LocaleId = LocaleId(0x080A);  // Spanish (Mexico)
    pub const FR_FR: LocaleId = LocaleId(0x040C);  // French (France)
    pub const FR_CA: LocaleId = LocaleId(0x0C0C);  // French (Canada)
    pub const DE_DE: LocaleId = LocaleId(0x0407);  // German (Germany)
    pub const IT_IT: LocaleId = LocaleId(0x0410);  // Italian (Italy)
    pub const JA_JP: LocaleId = LocaleId(0x0411);  // Japanese (Japan)
    pub const ZH_CN: LocaleId = LocaleId(0x0804);  // Chinese (Simplified, China)
    pub const ZH_TW: LocaleId = LocaleId(0x0404);  // Chinese (Traditional, Taiwan)
    pub const KO_KR: LocaleId = LocaleId(0x0412);  // Korean (Korea)
    pub const RU_RU: LocaleId = LocaleId(0x0419);  // Russian (Russia)
    pub const AR_SA: LocaleId = LocaleId(0x0401);  // Arabic (Saudi Arabia)
    pub const HE_IL: LocaleId = LocaleId(0x040D);  // Hebrew (Israel)
    pub const TH_TH: LocaleId = LocaleId(0x041E);  // Thai (Thailand)
    pub const VI_VN: LocaleId = LocaleId(0x042A);  // Vietnamese (Vietnam)
    pub const NL_NL: LocaleId = LocaleId(0x0413);  // Dutch (Netherlands)
    pub const PL_PL: LocaleId = LocaleId(0x0415);  // Polish (Poland)
    pub const TR_TR: LocaleId = LocaleId(0x041F);  // Turkish (Turkey)
    pub const CS_CZ: LocaleId = LocaleId(0x0405);  // Czech (Czech Republic)
    pub const HU_HU: LocaleId = LocaleId(0x040E);  // Hungarian (Hungary)
    pub const SV_SE: LocaleId = LocaleId(0x041D);  // Swedish (Sweden)
    pub const DA_DK: LocaleId = LocaleId(0x0406);  // Danish (Denmark)
    pub const FI_FI: LocaleId = LocaleId(0x040B);  // Finnish (Finland)
    pub const NO_NO: LocaleId = LocaleId(0x0414);  // Norwegian (Norway)
    pub const EL_GR: LocaleId = LocaleId(0x0408);  // Greek (Greece)
    pub const UK_UA: LocaleId = LocaleId(0x0422);  // Ukrainian (Ukraine)
    pub const RO_RO: LocaleId = LocaleId(0x0418);  // Romanian (Romania)
    pub const ID_ID: LocaleId = LocaleId(0x0421);  // Indonesian (Indonesia)
    pub const MS_MY: LocaleId = LocaleId(0x043E);  // Malay (Malaysia)
    pub const HI_IN: LocaleId = LocaleId(0x0439);  // Hindi (India)

    /// Get the language code (e.g., "en", "pt")
    pub fn language_code(&self) -> &'static str {
        match *self {
            LocaleId::EN_US | LocaleId::EN_GB => "en",
            LocaleId::PT_BR | LocaleId::PT_PT => "pt",
            LocaleId::ES_ES | LocaleId::ES_MX => "es",
            LocaleId::FR_FR | LocaleId::FR_CA => "fr",
            LocaleId::DE_DE => "de",
            LocaleId::IT_IT => "it",
            LocaleId::JA_JP => "ja",
            LocaleId::ZH_CN | LocaleId::ZH_TW => "zh",
            LocaleId::KO_KR => "ko",
            LocaleId::RU_RU => "ru",
            LocaleId::AR_SA => "ar",
            LocaleId::HE_IL => "he",
            LocaleId::TH_TH => "th",
            LocaleId::VI_VN => "vi",
            LocaleId::NL_NL => "nl",
            LocaleId::PL_PL => "pl",
            LocaleId::TR_TR => "tr",
            LocaleId::CS_CZ => "cs",
            LocaleId::HU_HU => "hu",
            LocaleId::SV_SE => "sv",
            LocaleId::DA_DK => "da",
            LocaleId::FI_FI => "fi",
            LocaleId::NO_NO => "no",
            LocaleId::EL_GR => "el",
            LocaleId::UK_UA => "uk",
            LocaleId::RO_RO => "ro",
            LocaleId::ID_ID => "id",
            LocaleId::MS_MY => "ms",
            LocaleId::HI_IN => "hi",
            _ => "en",
        }
    }

    /// Get the country code (e.g., "US", "BR")
    pub fn country_code(&self) -> &'static str {
        match *self {
            LocaleId::EN_US => "US",
            LocaleId::EN_GB => "GB",
            LocaleId::PT_BR => "BR",
            LocaleId::PT_PT => "PT",
            LocaleId::ES_ES => "ES",
            LocaleId::ES_MX => "MX",
            LocaleId::FR_FR => "FR",
            LocaleId::FR_CA => "CA",
            LocaleId::DE_DE => "DE",
            LocaleId::IT_IT => "IT",
            LocaleId::JA_JP => "JP",
            LocaleId::ZH_CN => "CN",
            LocaleId::ZH_TW => "TW",
            LocaleId::KO_KR => "KR",
            LocaleId::RU_RU => "RU",
            LocaleId::AR_SA => "SA",
            LocaleId::HE_IL => "IL",
            LocaleId::TH_TH => "TH",
            LocaleId::VI_VN => "VN",
            LocaleId::NL_NL => "NL",
            LocaleId::PL_PL => "PL",
            LocaleId::TR_TR => "TR",
            LocaleId::CS_CZ => "CZ",
            LocaleId::HU_HU => "HU",
            LocaleId::SV_SE => "SE",
            LocaleId::DA_DK => "DK",
            LocaleId::FI_FI => "FI",
            LocaleId::NO_NO => "NO",
            LocaleId::EL_GR => "GR",
            LocaleId::UK_UA => "UA",
            LocaleId::RO_RO => "RO",
            LocaleId::ID_ID => "ID",
            LocaleId::MS_MY => "MY",
            LocaleId::HI_IN => "IN",
            _ => "US",
        }
    }

    /// Get POSIX-style locale string (e.g., "en_US", "pt_BR")
    pub fn posix_name(&self) -> String {
        alloc::format!("{}_{}", self.language_code(), self.country_code())
    }
}

/// Language enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Portuguese,
    Spanish,
    French,
    German,
    Italian,
    Japanese,
    Chinese,
    Korean,
    Russian,
    Arabic,
    Hebrew,
    Thai,
    Vietnamese,
    Dutch,
    Polish,
    Turkish,
    Czech,
    Hungarian,
    Swedish,
    Danish,
    Finnish,
    Norwegian,
    Greek,
    Ukrainian,
    Romanian,
    Indonesian,
    Malay,
    Hindi,
    Other,
}

impl Language {
    /// ISO 639-1 code
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Portuguese => "pt",
            Language::Spanish => "es",
            Language::French => "fr",
            Language::German => "de",
            Language::Italian => "it",
            Language::Japanese => "ja",
            Language::Chinese => "zh",
            Language::Korean => "ko",
            Language::Russian => "ru",
            Language::Arabic => "ar",
            Language::Hebrew => "he",
            Language::Thai => "th",
            Language::Vietnamese => "vi",
            Language::Dutch => "nl",
            Language::Polish => "pl",
            Language::Turkish => "tr",
            Language::Czech => "cs",
            Language::Hungarian => "hu",
            Language::Swedish => "sv",
            Language::Danish => "da",
            Language::Finnish => "fi",
            Language::Norwegian => "no",
            Language::Greek => "el",
            Language::Ukrainian => "uk",
            Language::Romanian => "ro",
            Language::Indonesian => "id",
            Language::Malay => "ms",
            Language::Hindi => "hi",
            Language::Other => "xx",
        }
    }

    /// Check if RTL (right-to-left)
    pub fn is_rtl(&self) -> bool {
        matches!(self, Language::Arabic | Language::Hebrew)
    }
}

/// Country enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Country {
    UnitedStates,
    UnitedKingdom,
    Brazil,
    Portugal,
    Spain,
    Mexico,
    France,
    Canada,
    Germany,
    Italy,
    Japan,
    China,
    Taiwan,
    SouthKorea,
    Russia,
    SaudiArabia,
    Israel,
    Thailand,
    Vietnam,
    Netherlands,
    Poland,
    Turkey,
    CzechRepublic,
    Hungary,
    Sweden,
    Denmark,
    Finland,
    Norway,
    Greece,
    Ukraine,
    Romania,
    Indonesia,
    Malaysia,
    India,
    Other,
}

impl Country {
    /// ISO 3166-1 alpha-2 code
    pub fn code(&self) -> &'static str {
        match self {
            Country::UnitedStates => "US",
            Country::UnitedKingdom => "GB",
            Country::Brazil => "BR",
            Country::Portugal => "PT",
            Country::Spain => "ES",
            Country::Mexico => "MX",
            Country::France => "FR",
            Country::Canada => "CA",
            Country::Germany => "DE",
            Country::Italy => "IT",
            Country::Japan => "JP",
            Country::China => "CN",
            Country::Taiwan => "TW",
            Country::SouthKorea => "KR",
            Country::Russia => "RU",
            Country::SaudiArabia => "SA",
            Country::Israel => "IL",
            Country::Thailand => "TH",
            Country::Vietnam => "VN",
            Country::Netherlands => "NL",
            Country::Poland => "PL",
            Country::Turkey => "TR",
            Country::CzechRepublic => "CZ",
            Country::Hungary => "HU",
            Country::Sweden => "SE",
            Country::Denmark => "DK",
            Country::Finland => "FI",
            Country::Norway => "NO",
            Country::Greece => "GR",
            Country::Ukraine => "UA",
            Country::Romania => "RO",
            Country::Indonesia => "ID",
            Country::Malaysia => "MY",
            Country::India => "IN",
            Country::Other => "XX",
        }
    }
}

/// Complete locale definition
#[derive(Debug, Clone)]
pub struct Locale {
    /// Locale identifier
    pub id: LocaleId,
    /// Primary language
    pub language: Language,
    /// Country
    pub country: Country,
    /// English name
    pub name: String,
    /// Native language name
    pub native_name: String,
    /// Date format preference
    pub date_format: DateFormat,
    /// Time format preference
    pub time_format: TimeFormat,
    /// First day of week (0=Sunday, 1=Monday)
    pub first_day_of_week: u8,
    /// Decimal separator
    pub decimal_separator: char,
    /// Thousands separator
    pub thousands_separator: char,
    /// Currency symbol
    pub currency_symbol: String,
    /// Currency position
    pub currency_position: CurrencyPosition,
}

impl Locale {
    /// Get week day names in this locale
    pub fn weekday_names(&self) -> [&'static str; 7] {
        match self.language {
            Language::Portuguese => [
                "domingo", "segunda-feira", "terça-feira", "quarta-feira",
                "quinta-feira", "sexta-feira", "sábado"
            ],
            Language::Spanish => [
                "domingo", "lunes", "martes", "miércoles",
                "jueves", "viernes", "sábado"
            ],
            Language::French => [
                "dimanche", "lundi", "mardi", "mercredi",
                "jeudi", "vendredi", "samedi"
            ],
            Language::German => [
                "Sonntag", "Montag", "Dienstag", "Mittwoch",
                "Donnerstag", "Freitag", "Samstag"
            ],
            _ => [
                "Sunday", "Monday", "Tuesday", "Wednesday",
                "Thursday", "Friday", "Saturday"
            ],
        }
    }

    /// Get short week day names
    pub fn weekday_names_short(&self) -> [&'static str; 7] {
        match self.language {
            Language::Portuguese => ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"],
            Language::Spanish => ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"],
            Language::French => ["dim", "lun", "mar", "mer", "jeu", "ven", "sam"],
            Language::German => ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
            _ => ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        }
    }

    /// Get month names in this locale
    pub fn month_names(&self) -> [&'static str; 12] {
        match self.language {
            Language::Portuguese => [
                "janeiro", "fevereiro", "março", "abril", "maio", "junho",
                "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"
            ],
            Language::Spanish => [
                "enero", "febrero", "marzo", "abril", "mayo", "junio",
                "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"
            ],
            Language::French => [
                "janvier", "février", "mars", "avril", "mai", "juin",
                "juillet", "août", "septembre", "octobre", "novembre", "décembre"
            ],
            Language::German => [
                "Januar", "Februar", "März", "April", "Mai", "Juni",
                "Juli", "August", "September", "Oktober", "November", "Dezember"
            ],
            _ => [
                "January", "February", "March", "April", "May", "June",
                "July", "August", "September", "October", "November", "December"
            ],
        }
    }

    /// Get short month names
    pub fn month_names_short(&self) -> [&'static str; 12] {
        match self.language {
            Language::Portuguese => [
                "jan", "fev", "mar", "abr", "mai", "jun",
                "jul", "ago", "set", "out", "nov", "dez"
            ],
            Language::Spanish => [
                "ene", "feb", "mar", "abr", "may", "jun",
                "jul", "ago", "sep", "oct", "nov", "dic"
            ],
            Language::French => [
                "janv", "févr", "mars", "avr", "mai", "juin",
                "juil", "août", "sept", "oct", "nov", "déc"
            ],
            Language::German => [
                "Jan", "Feb", "Mär", "Apr", "Mai", "Jun",
                "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"
            ],
            _ => [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun",
                "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"
            ],
        }
    }
}

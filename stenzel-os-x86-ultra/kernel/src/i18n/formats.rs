//! Regional formats for dates, times, numbers, and currencies

use alloc::string::String;
use alloc::format;

/// Helper function to compute 10^n without std library
fn pow10(n: usize) -> u64 {
    const POWERS: [u64; 20] = [
        1, 10, 100, 1000, 10000, 100000, 1000000, 10000000, 100000000, 1000000000,
        10000000000, 100000000000, 1000000000000, 10000000000000, 100000000000000,
        1000000000000000, 10000000000000000, 100000000000000000, 1000000000000000000,
        10000000000000000000,
    ];
    POWERS.get(n).copied().unwrap_or(1)
}

/// Date format style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// Day/Month/Year (most of the world)
    DMY,
    /// Month/Day/Year (US)
    MDY,
    /// Year/Month/Day (ISO, East Asia)
    YMD,
}

impl DateFormat {
    /// Get format pattern
    pub fn pattern(&self) -> &'static str {
        match self {
            DateFormat::DMY => "dd/MM/yyyy",
            DateFormat::MDY => "MM/dd/yyyy",
            DateFormat::YMD => "yyyy-MM-dd",
        }
    }

    /// Get short format pattern
    pub fn short_pattern(&self) -> &'static str {
        match self {
            DateFormat::DMY => "dd/MM/yy",
            DateFormat::MDY => "MM/dd/yy",
            DateFormat::YMD => "yy-MM-dd",
        }
    }

    /// Format a date
    pub fn format(&self, year: i32, month: u32, day: u32) -> String {
        match self {
            DateFormat::DMY => format!("{:02}/{:02}/{}", day, month, year),
            DateFormat::MDY => format!("{:02}/{:02}/{}", month, day, year),
            DateFormat::YMD => format!("{}-{:02}-{:02}", year, month, day),
        }
    }

    /// Format a date with short year
    pub fn format_short(&self, year: i32, month: u32, day: u32) -> String {
        let short_year = year % 100;
        match self {
            DateFormat::DMY => format!("{:02}/{:02}/{:02}", day, month, short_year),
            DateFormat::MDY => format!("{:02}/{:02}/{:02}", month, day, short_year),
            DateFormat::YMD => format!("{:02}-{:02}-{:02}", short_year, month, day),
        }
    }
}

/// Time format style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormat {
    /// 12-hour format with AM/PM
    H12,
    /// 24-hour format
    H24,
}

impl TimeFormat {
    /// Get format pattern
    pub fn pattern(&self) -> &'static str {
        match self {
            TimeFormat::H12 => "hh:mm:ss a",
            TimeFormat::H24 => "HH:mm:ss",
        }
    }

    /// Get short format pattern (no seconds)
    pub fn short_pattern(&self) -> &'static str {
        match self {
            TimeFormat::H12 => "hh:mm a",
            TimeFormat::H24 => "HH:mm",
        }
    }

    /// Format a time
    pub fn format(&self, hour: u32, minute: u32, second: u32) -> String {
        match self {
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
                format!("{:02}:{:02}:{:02} {}", h, minute, second, period)
            }
            TimeFormat::H24 => {
                format!("{:02}:{:02}:{:02}", hour, minute, second)
            }
        }
    }

    /// Format time without seconds
    pub fn format_short(&self, hour: u32, minute: u32) -> String {
        match self {
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
                format!("{:02}:{:02} {}", h, minute, period)
            }
            TimeFormat::H24 => {
                format!("{:02}:{:02}", hour, minute)
            }
        }
    }
}

/// Number format configuration
#[derive(Debug, Clone)]
pub struct NumberFormat {
    /// Decimal separator
    pub decimal_separator: char,
    /// Thousands separator
    pub thousands_separator: char,
    /// Group size (usually 3)
    pub grouping: u8,
    /// Minimum integer digits
    pub min_integer_digits: u8,
    /// Minimum fraction digits
    pub min_fraction_digits: u8,
    /// Maximum fraction digits
    pub max_fraction_digits: u8,
}

impl NumberFormat {
    /// US number format (1,234.56)
    pub fn us() -> Self {
        Self {
            decimal_separator: '.',
            thousands_separator: ',',
            grouping: 3,
            min_integer_digits: 1,
            min_fraction_digits: 0,
            max_fraction_digits: 3,
        }
    }

    /// European number format (1.234,56)
    pub fn european() -> Self {
        Self {
            decimal_separator: ',',
            thousands_separator: '.',
            grouping: 3,
            min_integer_digits: 1,
            min_fraction_digits: 0,
            max_fraction_digits: 3,
        }
    }

    /// French number format (1 234,56)
    pub fn french() -> Self {
        Self {
            decimal_separator: ',',
            thousands_separator: ' ',
            grouping: 3,
            min_integer_digits: 1,
            min_fraction_digits: 0,
            max_fraction_digits: 3,
        }
    }

    /// Format an integer
    pub fn format_integer(&self, n: i64) -> String {
        let is_negative = n < 0;
        let abs_n = n.abs() as u64;
        let s = format!("{}", abs_n);

        // Insert thousands separators
        let mut result = String::new();
        let len = s.len();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (len - i) % (self.grouping as usize) == 0 {
                result.push(self.thousands_separator);
            }
            result.push(c);
        }

        if is_negative {
            result.insert(0, '-');
        }

        result
    }

    /// Format a decimal number
    pub fn format_decimal(&self, n: f64, decimals: usize) -> String {
        let is_negative = n < 0.0;
        let abs_n = if n < 0.0 { -n } else { n };

        // Round to specified decimal places using integer math
        let factor = pow10(decimals);
        let rounded = ((abs_n * factor as f64) + 0.5) as u64;
        let int_part = (rounded / factor) as i64;
        let frac_part = rounded % factor;

        // Format integer part with thousands separators
        let formatted_int = self.format_integer(int_part);

        // Build result
        let result = if decimals > 0 {
            format!("{}{}{:0>width$}", formatted_int, self.decimal_separator, frac_part, width = decimals)
        } else {
            formatted_int
        };

        if is_negative {
            format!("-{}", result)
        } else {
            result
        }
    }

    /// Format a percentage
    pub fn format_percent(&self, n: f64, decimals: usize) -> String {
        format!("{}%", self.format_decimal(n * 100.0, decimals))
    }
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self::us()
    }
}

/// Currency format configuration
#[derive(Debug, Clone)]
pub struct CurrencyFormat {
    /// Currency symbol
    pub symbol: String,
    /// Symbol position
    pub symbol_position: CurrencySymbolPosition,
    /// Space between symbol and number
    pub space_between: bool,
    /// Number format to use
    pub number_format: NumberFormat,
    /// Decimal places (usually 2)
    pub decimal_places: u8,
}

/// Currency symbol position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrencySymbolPosition {
    /// Symbol before number ($100)
    Before,
    /// Symbol after number (100€)
    After,
}

impl CurrencyFormat {
    /// US Dollar format ($1,234.56)
    pub fn usd() -> Self {
        Self {
            symbol: "$".into(),
            symbol_position: CurrencySymbolPosition::Before,
            space_between: false,
            number_format: NumberFormat::us(),
            decimal_places: 2,
        }
    }

    /// Euro format (€1.234,56 or 1.234,56 €)
    pub fn eur() -> Self {
        Self {
            symbol: "€".into(),
            symbol_position: CurrencySymbolPosition::After,
            space_between: true,
            number_format: NumberFormat::european(),
            decimal_places: 2,
        }
    }

    /// British Pound format (£1,234.56)
    pub fn gbp() -> Self {
        Self {
            symbol: "£".into(),
            symbol_position: CurrencySymbolPosition::Before,
            space_between: false,
            number_format: NumberFormat::us(),
            decimal_places: 2,
        }
    }

    /// Brazilian Real format (R$ 1.234,56)
    pub fn brl() -> Self {
        Self {
            symbol: "R$".into(),
            symbol_position: CurrencySymbolPosition::Before,
            space_between: true,
            number_format: NumberFormat::european(),
            decimal_places: 2,
        }
    }

    /// Japanese Yen format (¥1,234)
    pub fn jpy() -> Self {
        Self {
            symbol: "¥".into(),
            symbol_position: CurrencySymbolPosition::Before,
            space_between: false,
            number_format: NumberFormat::us(),
            decimal_places: 0, // Yen has no decimals
        }
    }

    /// Format a currency amount
    pub fn format(&self, amount: f64) -> String {
        let is_negative = amount < 0.0;
        let abs_amount = amount.abs();

        let formatted_number = self.number_format.format_decimal(
            abs_amount,
            self.decimal_places as usize,
        );

        let space = if self.space_between { " " } else { "" };

        let result = match self.symbol_position {
            CurrencySymbolPosition::Before => {
                format!("{}{}{}", self.symbol, space, formatted_number)
            }
            CurrencySymbolPosition::After => {
                format!("{}{}{}", formatted_number, space, self.symbol)
            }
        };

        if is_negative {
            format!("-{}", result)
        } else {
            result
        }
    }
}

impl Default for CurrencyFormat {
    fn default() -> Self {
        Self::usd()
    }
}

/// Relative time format (e.g., "3 days ago", "in 2 hours")
#[derive(Debug, Clone, Copy)]
pub enum RelativeTimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// Format relative time
pub fn format_relative_time(value: i64, unit: RelativeTimeUnit, language: &str) -> String {
    let (singular, plural) = match (unit, language) {
        // English
        (RelativeTimeUnit::Second, "en") => ("second", "seconds"),
        (RelativeTimeUnit::Minute, "en") => ("minute", "minutes"),
        (RelativeTimeUnit::Hour, "en") => ("hour", "hours"),
        (RelativeTimeUnit::Day, "en") => ("day", "days"),
        (RelativeTimeUnit::Week, "en") => ("week", "weeks"),
        (RelativeTimeUnit::Month, "en") => ("month", "months"),
        (RelativeTimeUnit::Year, "en") => ("year", "years"),

        // Portuguese
        (RelativeTimeUnit::Second, "pt") => ("segundo", "segundos"),
        (RelativeTimeUnit::Minute, "pt") => ("minuto", "minutos"),
        (RelativeTimeUnit::Hour, "pt") => ("hora", "horas"),
        (RelativeTimeUnit::Day, "pt") => ("dia", "dias"),
        (RelativeTimeUnit::Week, "pt") => ("semana", "semanas"),
        (RelativeTimeUnit::Month, "pt") => ("mês", "meses"),
        (RelativeTimeUnit::Year, "pt") => ("ano", "anos"),

        // Spanish
        (RelativeTimeUnit::Second, "es") => ("segundo", "segundos"),
        (RelativeTimeUnit::Minute, "es") => ("minuto", "minutos"),
        (RelativeTimeUnit::Hour, "es") => ("hora", "horas"),
        (RelativeTimeUnit::Day, "es") => ("día", "días"),
        (RelativeTimeUnit::Week, "es") => ("semana", "semanas"),
        (RelativeTimeUnit::Month, "es") => ("mes", "meses"),
        (RelativeTimeUnit::Year, "es") => ("año", "años"),

        // Default to English
        (RelativeTimeUnit::Second, _) => ("second", "seconds"),
        (RelativeTimeUnit::Minute, _) => ("minute", "minutes"),
        (RelativeTimeUnit::Hour, _) => ("hour", "hours"),
        (RelativeTimeUnit::Day, _) => ("day", "days"),
        (RelativeTimeUnit::Week, _) => ("week", "weeks"),
        (RelativeTimeUnit::Month, _) => ("month", "months"),
        (RelativeTimeUnit::Year, _) => ("year", "years"),
    };

    let abs_value = value.abs();
    let unit_str = if abs_value == 1 { singular } else { plural };

    match (value.cmp(&0), language) {
        (core::cmp::Ordering::Less, "pt") => format!("há {} {}", abs_value, unit_str),
        (core::cmp::Ordering::Greater, "pt") => format!("em {} {}", abs_value, unit_str),
        (core::cmp::Ordering::Equal, "pt") => "agora".into(),

        (core::cmp::Ordering::Less, "es") => format!("hace {} {}", abs_value, unit_str),
        (core::cmp::Ordering::Greater, "es") => format!("en {} {}", abs_value, unit_str),
        (core::cmp::Ordering::Equal, "es") => "ahora".into(),

        (core::cmp::Ordering::Less, _) => format!("{} {} ago", abs_value, unit_str),
        (core::cmp::Ordering::Greater, _) => format!("in {} {}", abs_value, unit_str),
        (core::cmp::Ordering::Equal, _) => "now".into(),
    }
}

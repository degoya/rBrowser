//! `hreflang` values: `x-default`, or an ISO 639-1 language, optionally an ISO 15924 script
//! and an ISO 3166-1 alpha-2 region (`de`, `de-AT`, `zh-Hant-TW`), in any case. A value that is
//! none gets a correction when the mistake is a known one: `_` for `-` (`de_DE`), `UK` for `GB`,
//! a country code for a language (`jp`, `cz`).

use std::fmt;

/// ISO 639-1, sorted.
pub const LANGUAGES: &[&str] = &[
    "aa", "ab", "ae", "af", "ak", "am", "an", "ar", "as", "av", "ay", "az", "ba", "be", "bg", "bh",
    "bi", "bm", "bn", "bo", "br", "bs", "ca", "ce", "ch", "co", "cr", "cs", "cu", "cv", "cy", "da",
    "de", "dv", "dz", "ee", "el", "en", "eo", "es", "et", "eu", "fa", "ff", "fi", "fj", "fo", "fr",
    "fy", "ga", "gd", "gl", "gn", "gu", "gv", "ha", "he", "hi", "ho", "hr", "ht", "hu", "hy", "hz",
    "ia", "id", "ie", "ig", "ii", "ik", "io", "is", "it", "iu", "ja", "jv", "ka", "kg", "ki", "kj",
    "kk", "kl", "km", "kn", "ko", "kr", "ks", "ku", "kv", "kw", "ky", "la", "lb", "lg", "li", "ln",
    "lo", "lt", "lu", "lv", "mg", "mh", "mi", "mk", "ml", "mn", "mr", "ms", "mt", "my", "na", "nb",
    "nd", "ne", "ng", "nl", "nn", "no", "nr", "nv", "ny", "oc", "oj", "om", "or", "os", "pa", "pi",
    "pl", "ps", "pt", "qu", "rm", "rn", "ro", "ru", "rw", "sa", "sc", "sd", "se", "sg", "si", "sk",
    "sl", "sm", "sn", "so", "sq", "sr", "ss", "st", "su", "sv", "sw", "ta", "te", "tg", "th", "ti",
    "tk", "tl", "tn", "to", "tr", "ts", "tt", "tw", "ty", "ug", "uk", "ur", "uz", "ve", "vi", "vo",
    "wa", "wo", "xh", "yi", "yo", "za", "zh", "zu",
];

/// ISO 3166-1 alpha-2, sorted.
pub const REGIONS: &[&str] = &[
    "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AO", "AQ", "AR", "AS", "AT", "AU", "AW", "AX", "AZ",
    "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ", "BR", "BS",
    "BT", "BV", "BW", "BY", "BZ", "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK", "CL", "CM", "CN",
    "CO", "CR", "CU", "CV", "CW", "CX", "CY", "CZ", "DE", "DJ", "DK", "DM", "DO", "DZ", "EC", "EE",
    "EG", "EH", "ER", "ES", "ET", "FI", "FJ", "FK", "FM", "FO", "FR", "GA", "GB", "GD", "GE", "GF",
    "GG", "GH", "GI", "GL", "GM", "GN", "GP", "GQ", "GR", "GS", "GT", "GU", "GW", "GY", "HK", "HM",
    "HN", "HR", "HT", "HU", "ID", "IE", "IL", "IM", "IN", "IO", "IQ", "IR", "IS", "IT", "JE", "JM",
    "JO", "JP", "KE", "KG", "KH", "KI", "KM", "KN", "KP", "KR", "KW", "KY", "KZ", "LA", "LB", "LC",
    "LI", "LK", "LR", "LS", "LT", "LU", "LV", "LY", "MA", "MC", "MD", "ME", "MF", "MG", "MH", "MK",
    "ML", "MM", "MN", "MO", "MP", "MQ", "MR", "MS", "MT", "MU", "MV", "MW", "MX", "MY", "MZ", "NA",
    "NC", "NE", "NF", "NG", "NI", "NL", "NO", "NP", "NR", "NU", "NZ", "OM", "PA", "PE", "PF", "PG",
    "PH", "PK", "PL", "PM", "PN", "PR", "PS", "PT", "PW", "PY", "QA", "RE", "RO", "RS", "RU", "RW",
    "SA", "SB", "SC", "SD", "SE", "SG", "SH", "SI", "SJ", "SK", "SL", "SM", "SN", "SO", "SR", "SS",
    "ST", "SV", "SX", "SY", "SZ", "TC", "TD", "TF", "TG", "TH", "TJ", "TK", "TL", "TM", "TN", "TO",
    "TR", "TT", "TV", "TW", "TZ", "UA", "UG", "UM", "US", "UY", "UZ", "VA", "VC", "VE", "VG", "VI",
    "VN", "VU", "WF", "WS", "YE", "YT", "ZA", "ZM", "ZW",
];

/// Country codes and retired codes written for a language, and the language meant.
const LANGUAGE_FIXES: &[(&str, &str)] = &[
    ("cn", "zh"),
    ("cz", "cs"),
    ("dk", "da"),
    ("gr", "el"),
    ("in", "id"),
    ("iw", "he"),
    ("ji", "yi"),
    ("jp", "ja"),
    ("ua", "uk"),
];

/// Regions that are no ISO 3166-1 code, and the one meant.
const REGION_FIXES: &[(&str, &str)] = &[("UK", "GB")];

/// The value `x-default`.
pub const X_DEFAULT: &str = "x-default";

/// A valid language code.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Code {
    /// Lower case.
    pub language: String,
    /// Title case (`Hant`).
    pub script: Option<String>,
    /// Upper case.
    pub region: Option<String>,
}

impl Code {
    /// Whether a page's `lang` (or one `Content-Language` value) names this language: the
    /// primary tags are equal, and so are the regions when both name one.
    #[must_use]
    pub fn matches_lang(&self, lang: &str) -> bool {
        let mut parts = lang.trim().split(['-', '_']);
        let primary = parts.next().unwrap_or_default().to_ascii_lowercase();
        if primary != self.language {
            return false;
        }
        let region = parts
            .find(|part| {
                part.len() == 2 || part.len() == 3 && part.chars().all(|c| c.is_ascii_digit())
            })
            .map(str::to_ascii_uppercase);
        match (&self.region, region) {
            (Some(own), Some(other)) => *own == other,
            _ => true,
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.language)?;
        if let Some(script) = &self.script {
            write!(formatter, "-{script}")?;
        }
        if let Some(region) = &self.region {
            write!(formatter, "-{region}")?;
        }
        Ok(())
    }
}

/// What a `hreflang` value is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checked {
    Valid(Code),
    XDefault,
    /// No valid value; the correction when the mistake is a known one.
    Invalid {
        suggestion: Option<String>,
    },
}

fn title_case(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut chars = lower.chars();
    chars
        .next()
        .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

/// `parts` as a code; `fix` swaps in the known corrections.
fn parse(parts: &[&str], fix: bool) -> Option<Code> {
    let (first, rest) = parts.split_first()?;
    let mut language = first.to_ascii_lowercase();
    if fix && let Some((_, meant)) = LANGUAGE_FIXES.iter().find(|(wrong, _)| *wrong == language) {
        (*meant).clone_into(&mut language);
    }
    if LANGUAGES.binary_search(&language.as_str()).is_err() {
        return None;
    }
    let mut code = Code {
        language,
        script: None,
        region: None,
    };
    let mut rest = rest.iter();
    let mut next = rest.next();
    if let Some(script) = next.filter(|part| part.len() == 4) {
        if !script.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        code.script = Some(title_case(script));
        next = rest.next();
    }
    if let Some(region) = next {
        let mut region = region.to_ascii_uppercase();
        if fix && let Some((_, meant)) = REGION_FIXES.iter().find(|(wrong, _)| *wrong == region) {
            (*meant).clone_into(&mut region);
        }
        if REGIONS.binary_search(&region.as_str()).is_err() {
            return None;
        }
        code.region = Some(region);
    }
    rest.next().is_none().then_some(code)
}

/// `text` checked as a `hreflang` value.
#[must_use]
pub fn check(text: &str) -> Checked {
    let text = text.trim();
    if text.eq_ignore_ascii_case(X_DEFAULT) {
        return Checked::XDefault;
    }
    let dashed: Vec<&str> = text.split('-').collect();
    if !text.contains('_')
        && let Some(code) = parse(&dashed, false)
    {
        return Checked::Valid(code);
    }
    let suggestion = if text.replace('_', "-").eq_ignore_ascii_case(X_DEFAULT) {
        Some(X_DEFAULT.to_owned())
    } else {
        let parts: Vec<&str> = text.split(['-', '_']).collect();
        parse(&parts, true).map(|code| code.to_string())
    };
    Checked::Invalid { suggestion }
}

#[cfg(test)]
#[path = "code_tests.rs"]
mod tests;

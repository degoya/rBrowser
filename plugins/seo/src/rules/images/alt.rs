//! What makes an alt text poor, as pure functions: a file name, a placeholder alone, a prefix
//! that says it is an image, and a language other than the page's.

/// Extensions of image files an alt text should not end in.
const EXTENSIONS: [&str; 10] = [
    "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "bmp", "tif", "tiff",
];

/// Prefixes of camera and screenshot file names (`IMG_1234`, `DSC01234`).
const CAMERA: [&str; 7] = ["img", "dsc", "dscn", "dscf", "pxl", "screenshot", "dcim"];

/// Prefixes that only say the image is one.
const PREFIXES: [&str; 14] = [
    "image of",
    "an image of",
    "picture of",
    "a picture of",
    "photo of",
    "a photo of",
    "graphic of",
    "bild von",
    "ein bild von",
    "foto von",
    "ein foto von",
    "grafik von",
    "abbildung von",
    "abbildung:",
];

/// The fewest characters an alt text needs before its language is guessed.
const LANGUAGE_MIN_CHARS: usize = 24;

/// `text` lower case with its white space collapsed and outer punctuation removed.
#[must_use]
pub fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c.is_ascii_punctuation() && c != '_')
        .to_lowercase()
}

fn stem(src: &str) -> Option<String> {
    let name = src
        .split(['?', '#'])
        .next()?
        .rsplit('/')
        .next()?
        .to_lowercase();
    let stem = name
        .rsplit_once('.')
        .map_or(name.as_str(), |(stem, _)| stem);
    (!stem.is_empty()).then(|| stem.to_owned())
}

/// Whether `alt` is a file name: it ends in an image extension, names a camera file
/// (`IMG_1234`), equals the stem of `src`, or is one hyphenated or underscored token with a
/// digit or with more than one separator (`hero-final-v2`).
#[must_use]
pub fn is_filename(alt: &str, src: &str) -> bool {
    let alt = alt.trim().to_lowercase();
    if alt.is_empty() {
        return false;
    }
    if let Some((_, extension)) = alt.rsplit_once('.')
        && EXTENSIONS.contains(&extension)
    {
        return true;
    }
    if stem(src).is_some_and(|stem| stem == alt && alt.contains(['-', '_'])) {
        return true;
    }
    if alt.contains(char::is_whitespace) {
        return false;
    }
    let camera = CAMERA.iter().any(|prefix| {
        alt.strip_prefix(prefix).is_some_and(|rest| {
            let rest = rest.trim_start_matches(['_', '-']);
            !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '_')
        })
    });
    let separators = alt.matches(['-', '_']).count();
    camera || (separators > 0 && (alt.contains(|c: char| c.is_ascii_digit()) || separators > 1))
}

/// Whether `alt` is nothing but a placeholder word of `words` (lower case).
#[must_use]
pub fn is_placeholder(alt: &str, words: &[String]) -> bool {
    let alt = normalize(alt);
    words.contains(&alt)
}

/// The prefix `alt` starts with that only says it is an image (`image of`).
#[must_use]
pub fn redundant_prefix(alt: &str) -> Option<&'static str> {
    let alt = normalize(alt);
    PREFIXES
        .iter()
        .find(|prefix| {
            alt.strip_prefix(**prefix).is_some_and(|rest| {
                rest.is_empty() || rest.starts_with(' ') || prefix.ends_with(':')
            })
        })
        .copied()
}

/// The ISO 639-3 code `whatlang` names the language with for a page's `lang` (`de-AT`).
fn page_language(lang: &str) -> Option<whatlang::Lang> {
    let primary = lang.split(['-', '_']).next()?.trim().to_ascii_lowercase();
    let code = match primary.as_str() {
        "en" => "eng",
        "de" => "deu",
        "fr" => "fra",
        "es" => "spa",
        "it" => "ita",
        "pt" => "por",
        "nl" => "nld",
        "pl" => "pol",
        "sv" => "swe",
        "da" => "dan",
        "nb" | "no" => "nob",
        "fi" => "fin",
        "cs" => "ces",
        "sk" => "slk",
        "sl" => "slv",
        "hr" => "hrv",
        "hu" => "hun",
        "ro" => "ron",
        "bg" => "bul",
        "ru" => "rus",
        "uk" => "ukr",
        "tr" => "tur",
        "el" => "ell",
        "et" => "est",
        "lv" => "lav",
        "lt" => "lit",
        "ca" => "cat",
        "id" => "ind",
        "vi" => "vie",
        "ja" => "jpn",
        "zh" => "cmn",
        "ko" => "kor",
        "ar" => "ara",
        "he" => "heb",
        "hi" => "hin",
        "th" => "tha",
        other if other.len() == 3 => other,
        _ => return None,
    };
    whatlang::Lang::from_code(code)
}

/// The language `alt` is reliably written in when it is not the page's `lang`: its English
/// name; `None` when it matches, is too short to tell, or either language is unknown.
#[must_use]
pub fn foreign_language(alt: &str, lang: &str) -> Option<&'static str> {
    if alt.trim().chars().count() < LANGUAGE_MIN_CHARS {
        return None;
    }
    let page = page_language(lang)?;
    let info = whatlang::detect(alt)?;
    (info.is_reliable() && info.lang() != page).then(|| info.lang().eng_name())
}

#[cfg(test)]
#[path = "alt_tests.rs"]
mod tests;

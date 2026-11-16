//! sRGB colours as the page's computed style writes them, blended and compared after WCAG 2.x:
//! relative luminance and the contrast ratio (`(L1 + 0.05) / (L2 + 0.05)`).
//!
//! A computed colour in Chromium is `rgb(r, g, b)` or `rgba(r, g, b, a)`; the page script turns
//! the other colour spaces into one of those before they come here. What is no sRGB colour
//! (`oklch(...)`, `color(display-p3 ...)`, a keyword) is refused, and the contrast stays unknown.

/// The longest colour text read; anything longer is refused.
const MAX_COLOR_CHARS: usize = 96;

/// An sRGB colour: channels 0-255 (not rounded, so a blend keeps its precision), alpha 0-1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const WHITE: Self = Self {
        r: 255.0,
        g: 255.0,
        b: 255.0,
        a: 1.0,
    };

    /// Whether nothing shows through.
    #[must_use]
    pub fn is_opaque(self) -> bool {
        self.a >= 1.0
    }

    /// `self` painted over `below` (source-over).
    #[must_use]
    pub fn over(self, below: Self) -> Self {
        let a = self.a + below.a * (1.0 - self.a);
        if a <= 0.0 {
            return Self::TRANSPARENT;
        }
        let mix = |top: f64, bottom: f64| (top * self.a + bottom * below.a * (1.0 - self.a)) / a;
        Self {
            r: mix(self.r, below.r),
            g: mix(self.g, below.g),
            b: mix(self.b, below.b),
            a,
        }
    }

    /// The relative luminance of WCAG 2.x (alpha ignored).
    #[must_use]
    pub fn luminance(self) -> f64 {
        let linear = |channel: f64| {
            let c = channel / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    /// `#rrggbb`, or `#rrggbbaa` when not opaque.
    #[must_use]
    pub fn hex(self) -> String {
        let byte = |value: f64| value.round().clamp(0.0, 255.0) as u8;
        let rgb = format!(
            "#{:02x}{:02x}{:02x}",
            byte(self.r),
            byte(self.g),
            byte(self.b)
        );
        if self.is_opaque() {
            rgb
        } else {
            format!("{rgb}{:02x}", byte(self.a * 255.0))
        }
    }
}

/// The WCAG contrast ratio of two colours, 1 to 21, in either order.
#[must_use]
pub fn contrast_ratio(one: Rgba, other: Rgba) -> f64 {
    let (a, b) = (one.luminance(), other.luminance());
    let (light, dark) = if a >= b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

/// `value` rounded to two decimals, as the tooltip and the tool show a ratio.
#[must_use]
pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Reads `rgb()`/`rgba()` (comma or space syntax), `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
/// `transparent` and `color(srgb r g b [/ a])`; anything else is `None`.
#[must_use]
pub fn parse(text: &str) -> Option<Rgba> {
    if text.len() > MAX_COLOR_CHARS {
        return None;
    }
    let text = text.trim().to_ascii_lowercase();
    if text == "transparent" {
        return Some(Rgba::TRANSPARENT);
    }
    if let Some(hex) = text.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(inner) = text.strip_prefix("color(srgb") {
        let numbers = numbers(inner.strip_suffix(')')?)?;
        let channel = |value: f64| value.clamp(0.0, 1.0) * 255.0;
        return match numbers.as_slice() {
            [r, g, b] => Some(rgba(channel(*r), channel(*g), channel(*b), 1.0)),
            [r, g, b, a] => Some(rgba(channel(*r), channel(*g), channel(*b), *a)),
            _ => None,
        };
    }
    let inner = text
        .strip_prefix("rgba(")
        .or_else(|| text.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    match numbers(inner)?.as_slice() {
        [r, g, b] => Some(rgba(*r, *g, *b, 1.0)),
        [r, g, b, a] => Some(rgba(*r, *g, *b, *a)),
        _ => None,
    }
}

fn rgba(r: f64, g: f64, b: f64, a: f64) -> Rgba {
    let channel = |value: f64| value.clamp(0.0, 255.0);
    Rgba {
        r: channel(r),
        g: channel(g),
        b: channel(b),
        a: a.clamp(0.0, 1.0),
    }
}

/// The numbers of `1, 2, 3, 0.5` or `1 2 3 / 50%`; a percentage is a fraction (alpha only).
fn numbers(inner: &str) -> Option<Vec<f64>> {
    inner
        .split([',', ' ', '/'])
        .filter(|part| !part.is_empty())
        .map(|part| match part.strip_suffix('%') {
            Some(percent) => percent.parse::<f64>().ok().map(|value| value / 100.0),
            None => part.parse::<f64>().ok(),
        })
        .map(|value| value.filter(|value| value.is_finite()))
        .collect()
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |index: usize| u8::from_str_radix(&hex[index..=index], 16).ok();
    let pair = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).ok();
    let short = |index: usize| digit(index).map(|value| f64::from(value * 17));
    let long = |index: usize| pair(index).map(f64::from);
    match hex.len() {
        3 => Some(rgba(short(0)?, short(1)?, short(2)?, 1.0)),
        4 => Some(rgba(short(0)?, short(1)?, short(2)?, short(3)? / 255.0)),
        6 => Some(rgba(long(0)?, long(2)?, long(4)?, 1.0)),
        8 => Some(rgba(long(0)?, long(2)?, long(4)?, long(6)? / 255.0)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "color_tests.rs"]
mod tests;

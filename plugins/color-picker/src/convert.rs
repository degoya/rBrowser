//! A colour in the four formats the panel copies - `#rrggbb`, `rgb()`, `hsl()` and `oklch()` -
//! and back, and its OKLab coordinates for grouping near colours.
//!
//! The texts are the CSS forms every browser reads (`rgb(30, 136, 229)`, `hsl(208, 79.3%,
//! 50.8%)`, `oklch(61.79% 0.16678 250.87)`) with as many decimals as an 8-bit colour needs to
//! read back as itself; OKLab is Bjorn Ottosson's (the matrices of CSS Color 4).

use serde_json::{Value, json};

use crate::color::Rgba;

/// A format the person picks (`color-picker.defaultFormat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Hex,
    Rgb,
    Hsl,
    Oklch,
}

impl Format {
    /// The format of a setting's value; anything unknown is hex.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "rgb" => Self::Rgb,
            "hsl" => Self::Hsl,
            "oklch" => Self::Oklch,
            _ => Self::Hex,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Hex => "hex",
            Self::Rgb => "rgb",
            Self::Hsl => "hsl",
            Self::Oklch => "oklch",
        }
    }
}

/// One colour in every format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formats {
    pub hex: String,
    pub rgb: String,
    pub hsl: String,
    pub oklch: String,
}

impl Formats {
    #[must_use]
    pub fn of(color: Rgba) -> Self {
        Self {
            hex: color.hex(),
            rgb: rgb_text(color),
            hsl: hsl_text(color),
            oklch: oklch_text(color),
        }
    }

    #[must_use]
    pub fn get(&self, format: Format) -> &str {
        match format {
            Format::Hex => &self.hex,
            Format::Rgb => &self.rgb,
            Format::Hsl => &self.hsl,
            Format::Oklch => &self.oklch,
        }
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({ "hex": self.hex, "rgb": self.rgb, "hsl": self.hsl, "oklch": self.oklch })
    }
}

/// `value` with at most `decimals` decimals, without trailing zeros (`-0` is `0`).
fn number(value: f64, decimals: i32) -> String {
    let factor = 10f64.powi(decimals);
    let rounded = (value * factor).round() / factor;
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    let text = format!("{rounded:.*}", usize::try_from(decimals).unwrap_or(0));
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

fn byte(value: f64) -> u8 {
    // Clamped to 0-255 first, so the cast cannot truncate.
    value.round().clamp(0.0, 255.0) as u8
}

/// `rgb(r, g, b)`, or `rgba(r, g, b, a)` when not opaque.
#[must_use]
pub fn rgb_text(color: Rgba) -> String {
    let (r, g, b) = (byte(color.r), byte(color.g), byte(color.b));
    if color.is_opaque() {
        format!("rgb({r}, {g}, {b})")
    } else {
        format!("rgba({r}, {g}, {b}, {})", number(color.a, 3))
    }
}

/// Hue (degrees), saturation and lightness (percent) of the 8-bit colour.
fn hsl(color: Rgba) -> (f64, f64, f64) {
    let [r, g, b] = [color.r, color.g, color.b].map(|c| f64::from(byte(c)) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = f64::midpoint(max, min);
    let delta = max - min;
    if delta == 0.0 {
        return (0.0, 0.0, lightness * 100.0);
    }
    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let sector = if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    (sector * 60.0, saturation * 100.0, lightness * 100.0)
}

/// `hsl(h, s%, l%)`, or `hsla(...)` with alpha; one decimal reads back as the same colour.
#[must_use]
pub fn hsl_text(color: Rgba) -> String {
    let (h, s, l) = hsl(color);
    let body = format!("{}, {}%, {}%", number(h, 1), number(s, 1), number(l, 1));
    if color.is_opaque() {
        format!("hsl({body})")
    } else {
        format!("hsla({body}, {})", number(color.a, 3))
    }
}

/// The colour of `hue` (degrees), `saturation` and `lightness` (0-1) in 0-255 channels.
#[must_use]
pub fn from_hsl(hue: f64, saturation: f64, lightness: f64, alpha: f64) -> Rgba {
    let saturation = saturation.clamp(0.0, 1.0);
    let lightness = lightness.clamp(0.0, 1.0);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue.rem_euclid(360.0) / 60.0;
    let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let m = lightness - chroma / 2.0;
    let (r, g, b) = match sector.floor() as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    Rgba {
        r: (r + m) * 255.0,
        g: (g + m) * 255.0,
        b: (b + m) * 255.0,
        a: alpha.clamp(0.0, 1.0),
    }
}

fn linear(channel: f64) -> f64 {
    let c = channel / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn gamma(linear: f64) -> f64 {
    let c = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).clamp(0.0, 255.0)
}

/// OKLab `(L, a, b)` of the colour (alpha ignored), L from 0 to 1.
#[must_use]
pub fn oklab(color: Rgba) -> (f64, f64, f64) {
    let (r, g, b) = (linear(color.r), linear(color.g), linear(color.b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    (
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    )
}

/// The distance of two OKLab colours (deltaE OK).
#[must_use]
pub fn distance(one: (f64, f64, f64), other: (f64, f64, f64)) -> f64 {
    ((one.0 - other.0).powi(2) + (one.1 - other.1).powi(2) + (one.2 - other.2).powi(2)).sqrt()
}

/// `oklch(L% C H)`, with ` / a` when not opaque; a grey has chroma and hue 0.
#[must_use]
pub fn oklch_text(color: Rgba) -> String {
    let (l, a, b) = oklab(color);
    let chroma = a.hypot(b);
    let (chroma, hue) = if chroma < 0.000_005 {
        (0.0, 0.0)
    } else {
        (chroma, b.atan2(a).to_degrees().rem_euclid(360.0))
    };
    let body = format!(
        "{}% {} {}",
        number(l * 100.0, 2),
        number(chroma, 5),
        number(hue, 2)
    );
    if color.is_opaque() {
        format!("oklch({body})")
    } else {
        format!("oklch({body} / {})", number(color.a, 3))
    }
}

/// The colour of OKLCH `lightness` (0-1), `chroma` and `hue` (degrees), clamped into sRGB.
#[must_use]
pub fn from_oklch(lightness: f64, chroma: f64, hue: f64, alpha: f64) -> Rgba {
    let (a, b) = (
        chroma * hue.to_radians().cos(),
        chroma * hue.to_radians().sin(),
    );
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    Rgba {
        r: gamma(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        g: gamma(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        b: gamma(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s),
        a: alpha.clamp(0.0, 1.0),
    }
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;

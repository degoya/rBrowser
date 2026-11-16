//! The catalogue: the fifteen simulations, their strength and how each is drawn.
//!
//! Chromium renders six of them itself (`Emulation.setEmulatedVisionDeficiency`,
//! [`Simulation::native`]); every simulation also has a [`Spec`] the page script turns into a CSS
//! `filter` on the root element (an SVG `feColorMatrix` and CSS filter functions) and, for the
//! ones that hide part of the view, an overlay. The colour matrices work in linear RGB, as
//! `feColorMatrix` does by default: Machado, Oliveira and Fernandes (2009) at full severity for
//! the dichromacies - the matrices Chromium's own emulation uses - and the anomalies a linear
//! blend between normal vision and them by the severity.

use serde::{Deserialize, Serialize};

/// The picker's three groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Group {
    /// Colour vision deficiencies.
    Color,
    /// Vision impairments.
    Vision,
    /// Surroundings: sunlight, a dimmed screen.
    Environment,
}

/// A simulation, as MCP's `simulator_set` names it (`type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Simulation {
    None,
    Protanopia,
    Protanomaly,
    Deuteranopia,
    Deuteranomaly,
    Tritanopia,
    Tritanomaly,
    Achromatopsia,
    Achromatomaly,
    Myopia,
    Cataract,
    Glaucoma,
    MacularDegeneration,
    LowContrast,
    Sunlight,
    Dimmed,
}

/// Every simulation but [`Simulation::None`], in the picker's order.
pub const ALL: [Simulation; 15] = [
    Simulation::Protanopia,
    Simulation::Protanomaly,
    Simulation::Deuteranopia,
    Simulation::Deuteranomaly,
    Simulation::Tritanopia,
    Simulation::Tritanomaly,
    Simulation::Achromatopsia,
    Simulation::Achromatomaly,
    Simulation::Myopia,
    Simulation::Cataract,
    Simulation::Glaucoma,
    Simulation::MacularDegeneration,
    Simulation::LowContrast,
    Simulation::Sunlight,
    Simulation::Dimmed,
];

type Matrix = [[f64; 3]; 3];

const IDENTITY: Matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const PROTANOPIA: Matrix = [
    [0.152_286, 1.052_583, -0.204_868],
    [0.114_503, 0.786_281, 0.099_216],
    [-0.003_882, -0.048_116, 1.051_998],
];
const DEUTERANOPIA: Matrix = [
    [0.367_322, 0.860_646, -0.227_968],
    [0.280_085, 0.672_501, 0.047_413],
    [-0.011_820, 0.042_940, 0.968_881],
];
const TRITANOPIA: Matrix = [
    [1.255_528, -0.076_749, -0.178_779],
    [-0.078_411, 0.930_809, 0.147_602],
    [0.004_733, 0.691_367, 0.303_900],
];
/// Luminance only, with Chromium's weights for its `achromatopsia`.
const ACHROMATOPSIA: Matrix = [
    [0.213, 0.715, 0.072],
    [0.213, 0.715, 0.072],
    [0.213, 0.715, 0.072],
];
/// A clouded lens: yellowed, the blue channel muted.
const CATARACT: Matrix = [[1.0, 0.0, 0.0], [0.0, 0.95, 0.0], [0.0, 0.0, 0.6]];

impl Simulation {
    /// The id MCP, the settings and the state file use.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Protanopia => "protanopia",
            Self::Protanomaly => "protanomaly",
            Self::Deuteranopia => "deuteranopia",
            Self::Deuteranomaly => "deuteranomaly",
            Self::Tritanopia => "tritanopia",
            Self::Tritanomaly => "tritanomaly",
            Self::Achromatopsia => "achromatopsia",
            Self::Achromatomaly => "achromatomaly",
            Self::Myopia => "myopia",
            Self::Cataract => "cataract",
            Self::Glaucoma => "glaucoma",
            Self::MacularDegeneration => "macular-degeneration",
            Self::LowContrast => "low-contrast",
            Self::Sunlight => "sunlight",
            Self::Dimmed => "dimmed",
        }
    }

    /// The picker's group; none for [`Simulation::None`].
    #[must_use]
    pub fn group(self) -> Option<Group> {
        match self {
            Self::None => None,
            Self::Protanopia
            | Self::Protanomaly
            | Self::Deuteranopia
            | Self::Deuteranomaly
            | Self::Tritanopia
            | Self::Tritanomaly
            | Self::Achromatopsia
            | Self::Achromatomaly => Some(Group::Color),
            Self::Myopia
            | Self::Cataract
            | Self::Glaucoma
            | Self::MacularDegeneration
            | Self::LowContrast => Some(Group::Vision),
            Self::Sunlight | Self::Dimmed => Some(Group::Environment),
        }
    }

    /// The `type` of CDP `Emulation.setEmulatedVisionDeficiency` when Chromium renders this
    /// simulation itself.
    #[must_use]
    pub fn native(self) -> Option<&'static str> {
        match self {
            Self::Protanopia => Some("protanopia"),
            Self::Deuteranopia => Some("deuteranopia"),
            Self::Tritanopia => Some("tritanopia"),
            Self::Achromatopsia => Some("achromatopsia"),
            Self::Myopia => Some("blurredVision"),
            Self::LowContrast => Some("reducedContrast"),
            _ => None,
        }
    }

    /// The strength (0 to 1) without one given; none for a simulation without a strength.
    #[must_use]
    pub fn default_severity(self) -> Option<f64> {
        match self {
            Self::None
            | Self::Protanopia
            | Self::Deuteranopia
            | Self::Tritanopia
            | Self::Achromatopsia => None,
            Self::Myopia | Self::LowContrast => Some(1.0),
            Self::Cataract | Self::Sunlight => Some(0.7),
            Self::Protanomaly
            | Self::Deuteranomaly
            | Self::Tritanomaly
            | Self::Achromatomaly
            | Self::Glaucoma
            | Self::MacularDegeneration
            | Self::Dimmed => Some(0.6),
        }
    }
}

/// One step of the root element's CSS `filter`, applied in order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Step {
    /// An SVG `feColorMatrix` (`values`, 4 x 5, row by row) in linear RGB.
    Matrix { values: [f64; 20] },
    /// CSS `blur()` in CSS px.
    Blur { px: f64 },
    /// CSS `contrast()`.
    Contrast { amount: f64 },
    /// CSS `brightness()`.
    Brightness { amount: f64 },
    /// CSS `saturate()`.
    Saturate { amount: f64 },
}

/// A layer over the view that hides part of it; it takes no pointer events.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Overlay {
    /// Tunnel vision: clear within `clear` of the half diagonal from the centre, dark after a
    /// further `fade`, at `opacity`.
    Tunnel { clear: f64, fade: f64, opacity: f64 },
    /// A blurred, dark spot in the centre, `size` of the shorter side across.
    #[serde(rename_all = "camelCase")]
    Spot {
        size: f64,
        blur_px: f64,
        opacity: f64,
    },
    /// Glare: a white haze, brightest in the centre.
    Glare { opacity: f64 },
}

/// How the page script draws a simulation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Spec {
    /// The root element's filter, in order; empty for none.
    pub filter: Vec<Step>,
    /// The layer over the view, if any.
    pub overlay: Option<Overlay>,
}

/// Four places: the page sees the same numbers every time, and a test can name them.
fn round(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn blend(target: &Matrix, severity: f64) -> Step {
    let mut values = [0.0; 20];
    for row in 0..3 {
        for column in 0..3 {
            let normal = IDENTITY[row][column];
            values[row * 5 + column] = round(normal + (target[row][column] - normal) * severity);
        }
    }
    values[18] = 1.0;
    Step::Matrix { values }
}

/// How the page script draws `simulation` at `severity` (0 to 1, clamped; a dichromacy ignores
/// it), with `blur_px` the blur of [`Simulation::Myopia`] at full strength; none for
/// [`Simulation::None`].
#[must_use]
pub fn spec(simulation: Simulation, severity: f64, blur_px: f64) -> Option<Spec> {
    let s = if severity.is_nan() {
        0.0
    } else {
        severity.clamp(0.0, 1.0)
    };
    let filter = |filter: Vec<Step>| {
        Some(Spec {
            filter,
            overlay: None,
        })
    };
    match simulation {
        Simulation::None => None,
        Simulation::Protanopia => filter(vec![blend(&PROTANOPIA, 1.0)]),
        Simulation::Deuteranopia => filter(vec![blend(&DEUTERANOPIA, 1.0)]),
        Simulation::Tritanopia => filter(vec![blend(&TRITANOPIA, 1.0)]),
        Simulation::Achromatopsia => filter(vec![blend(&ACHROMATOPSIA, 1.0)]),
        Simulation::Protanomaly => filter(vec![blend(&PROTANOPIA, s)]),
        Simulation::Deuteranomaly => filter(vec![blend(&DEUTERANOPIA, s)]),
        Simulation::Tritanomaly => filter(vec![blend(&TRITANOPIA, s)]),
        Simulation::Achromatomaly => filter(vec![blend(&ACHROMATOPSIA, s)]),
        Simulation::Myopia => filter(vec![Step::Blur {
            px: round(blur_px.max(0.0) * s),
        }]),
        Simulation::LowContrast => filter(vec![Step::Contrast {
            amount: round(1.0 - 0.5 * s),
        }]),
        Simulation::Dimmed => filter(vec![Step::Brightness {
            amount: round(1.0 - 0.6 * s),
        }]),
        Simulation::Sunlight => filter(vec![
            Step::Brightness {
                amount: round(1.0 + 0.4 * s),
            },
            Step::Contrast {
                amount: round(1.0 - 0.5 * s),
            },
            Step::Saturate {
                amount: round(1.0 - 0.5 * s),
            },
        ]),
        Simulation::Cataract => Some(Spec {
            // The CSS functions first: after an SVG reference Chromium computes the functions
            // that follow in linear RGB, before it in sRGB as everywhere else.
            filter: vec![
                Step::Blur { px: round(2.0 * s) },
                Step::Contrast {
                    amount: round(1.0 - 0.4 * s),
                },
                blend(&CATARACT, s),
            ],
            overlay: Some(Overlay::Glare {
                opacity: round(0.35 * s),
            }),
        }),
        Simulation::Glaucoma => Some(Spec {
            filter: Vec::new(),
            overlay: Some(Overlay::Tunnel {
                clear: round(0.45 - 0.35 * s),
                fade: 0.25,
                opacity: 0.95,
            }),
        }),
        Simulation::MacularDegeneration => Some(Spec {
            filter: Vec::new(),
            overlay: Some(Overlay::Spot {
                size: round(0.1 + 0.25 * s),
                blur_px: round(6.0 + 10.0 * s),
                opacity: 0.85,
            }),
        }),
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;

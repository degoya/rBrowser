//! The scale of an external view's page into its grid cell (RB-080-40). A Chromium cell renders
//! the breakpoint's width at `cell width / breakpoint width` and fills the cell's height
//! (`rbrowser::fit`); a Firefox or WebKit window in its cell gets the same through its engine:
//! WebKit through the Playwright context's `deviceScaleFactor`, Firefox through the profile pref
//! `layout.css.devPixelsPerPx` (one factor for every Firefox view, ADR 0003). Everything here is
//! pure and tested; the app computes the cells in pixels and the engines apply the result
//! ([`crate::ViewHandle::set_cell`]).

/// The smallest factor a cell asks for: a narrower cell shows the page at this and cuts it.
pub const MIN_FACTOR: f64 = 0.25;
/// The step factors are held to ([`quantize`]): a resize by a few pixels changes nothing, and
/// a changed factor - a new Firefox, a new WebKit context - is rare.
pub const FACTOR_STEP: f64 = 0.02;

/// A cell's view area in physical pixels, as the window placed it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellPixels {
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
}

/// How an external view's page is scaled into its cell: the viewport in CSS px (the
/// breakpoint's width, the cell's height at `factor`) and `factor`, CSS px to device px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellScale {
    /// CSS px to device px: `cell width / breakpoint width`, at most 1.
    pub factor: f64,
    /// The viewport width in CSS px: the breakpoint's.
    pub width: u32,
    /// The viewport height in CSS px: the cell's height at `factor`.
    pub height: u32,
}

/// The factor that fits `viewport_width` CSS px into `cell`: `cell width / viewport width`, at
/// most 1 (a wider cell shows the page at 100 %, like a Chromium cell), at least
/// [`MIN_FACTOR`]. A viewport without width is shown as it is.
pub fn cell_factor(cell: CellPixels, viewport_width: u32) -> f64 {
    if viewport_width == 0 {
        return 1.0;
    }
    let factor = f64::from(cell.width) / f64::from(viewport_width);
    clamp(factor)
}

/// `factor` rounded down to a multiple of [`FACTOR_STEP`] and held to the range: the page is
/// never wider than its cell, and a cell a few pixels wider or narrower asks for nothing new.
pub fn quantize(factor: f64) -> f64 {
    let steps = 1.0 / FACTOR_STEP;
    let down = (clamp(factor) * steps + 1e-9).floor() / steps;
    clamp(down)
}

/// The one factor several views share (every Firefox view of one profile): the smallest, so
/// every page fits its cell; `None` without views.
pub fn shared_factor(factors: impl IntoIterator<Item = f64>) -> Option<f64> {
    factors
        .into_iter()
        .filter(|factor| factor.is_finite())
        .fold(None, |least: Option<f64>, factor| {
            Some(least.map_or(factor, |least| least.min(factor)))
        })
}

/// The scale of a view of `viewport_width` CSS px in `cell` at `factor`: the breakpoint's width
/// and the cell's height at that factor, each at least 1.
pub fn cell_scale(cell: CellPixels, viewport_width: u32, factor: f64) -> CellScale {
    let factor = clamp(factor);
    // A few thousand pixels over a factor of at least MIN_FACTOR: always inside u32.
    let height = (f64::from(cell.height) / factor).round().max(1.0) as u32;
    CellScale {
        factor,
        width: viewport_width.max(1),
        height,
    }
}

/// `factor` as the value of `layout.css.devPixelsPerPx`: two decimals.
pub fn pref_value(factor: f64) -> String {
    format!("{:.2}", clamp(factor))
}

fn clamp(factor: f64) -> f64 {
    if factor.is_finite() {
        factor.clamp(MIN_FACTOR, 1.0)
    } else {
        1.0
    }
}

#[cfg(test)]
#[path = "scale_tests.rs"]
mod tests;

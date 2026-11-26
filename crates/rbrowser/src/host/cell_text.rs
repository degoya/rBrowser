//! The text of a cell's label strip (split from `host/mod.rs`, RB-099-19).

use super::state::ViewState;

/// The text of a cell's label strip: the breakpoint, its size and - when it is set and not 1 -
/// its pixel ratio, as in `Mobile  393 x 852 @3x` with a multiplication sign (RB-010-07).
pub(super) fn cell_label(entry: &ViewState) -> String {
    let name = entry
        .breakpoint
        .as_ref()
        .map(|breakpoint| {
            breakpoint
                .label
                .clone()
                .unwrap_or_else(|| breakpoint.id.clone())
        })
        .unwrap_or_default();
    let emulation = &entry.emulation;
    // 0 leaves the pixel ratio to the device: no suffix, as for 1.
    let ratio = if emulation.dpr > 0.0 && (emulation.dpr - 1.0).abs() > f64::EPSILON {
        format!(" @{}x", emulation.dpr)
    } else {
        String::new()
    };
    // U+00D7, the multiplication sign.
    format!(
        "{name}  {} \u{d7} {}{ratio}",
        emulation.width, emulation.height
    )
}

//! The footer under every view (RB-080-44): a strip in the breakpoint's colour that names the
//! widths the page's step holds for and the page's class for that width - `1024-1279px | md`
//! by default (RB-080-78: the name and the size stand in the label strip above). The
//! breakpoint, the view's size and the step's ends as media queries can be chosen too. Without
//! an analysis of the page the range is the config's (RB-080-47). A footer never takes more
//! than half of its view's height: a second line, then the footer itself, goes first.
//!
//! Below the breakpoint bar sits the vitals bar (RB-090-71, `ui.viewFooter.vitals`): the view's
//! Core Web Vitals with the shapes of their rating, a click marks the element of the worst. Both
//! are one strip, so a view gives its footer the height of every line it shows and no view
//! overlaps another in any layout; in a low view the breakpoint bar's first line stays longest,
//! then the vitals bar, then the breakpoint bar's second line.
//!
//! Below the vitals bar sits the Lighthouse bar (RB-090-73, `ui.viewFooter.lighthouse`): the four
//! category scores of the newest Lighthouse run of the view's page and breakpoint as gauges, a
//! click opens the run's report; a view without such a run has none. In a low view it comes
//! after the vitals bar and before the breakpoint bar's second line.
//!
//! | Part | Where |
//! | --- | --- |
//! | The pure rules: range, queries, text, colours | [`model`] |
//! | Which footer each view shows, the 18 DIP per line it takes, keeping it current | `crate::host::footer` |
//! | The native strips: grid cells, layout modes, the canvas, a view's own window | `cef_app/cell_footers.rs` |
//! | What the native strip is painted with, again after each theme change | [`paint`] |
//! | One line or two in a narrow view, the width a text takes (RB-080-53) | [`wrap`], `crate::host::footer` (`footer_lines.rs`) |
//! | Reading the queries of the document a view shows (`ui.viewFooter.analyze`) | [`auto`], [`read`] |
//! | The results of those reads, per document | [`results`] |
//! | Settings `ui.viewFooter.visible`, `.fields`, `.analyze`, `.wrap`, `.align` | `rb_config::ViewFooterSettings` |
//! | The vitals bar's words, from the shell state's `metrics` (`crate::cell_info` hands them over) | [`vitals`] |
//! | Setting `ui.viewFooter.vitals` | `rb_config::ViewFooterSettings` |
//! | The Lighthouse bar's words, colours and run, from the plugin's run summaries | [`lighthouse`] |
//! | Reading those summaries from disk, again after each Lighthouse answer; its click | [`lighthouse_runs`] |
//! | Setting `ui.viewFooter.lighthouse` | `rb_config::ViewFooterSettings` |
//! | The colour `breakpoints[].color` | `rb_config::Breakpoint` |
//! | `ui.view_footer.toggle` (`Leader O B`), `ui.vitals_bar.toggle` (`Leader O W`), `ui.lighthouse_bar.toggle` (`Leader O Q`) | [`defs`] |
//! | `views_list` names `range` per view | `crate::mcp::host_tools` |

pub mod auto;
pub mod defs;
pub mod lighthouse;
pub mod lighthouse_runs;
pub mod model;
pub mod paint;
pub mod read;
pub mod results;
pub mod vitals;
pub mod wrap;

pub use defs::{LIGHTHOUSE_TOGGLE, TOGGLE, VITALS_TOGGLE, register};
pub use model::{
    CellFooter, DARK_TEXT, FOOTER_HEIGHT, FooterInput, FooterText, LIGHT_TEXT, PALETTE, Range,
    above_footer, background, contrast, document_of, foreground, framework_at, page_range,
    queries_at, range, text, window_height,
};
pub use paint::StripColors;
pub use results::{PageQueries, Results};

use crate::control::Core;

/// The core is up: the host gets `ui.viewFooter`, the footer's own analysis follows the views'
/// page loads, and the Lighthouse bar reads the runs on disk.
pub fn on_start(core: &Core) {
    core.host()
        .set_footer_settings(core.user_settings().ui.view_footer);
    auto::on_start(core);
    lighthouse_runs::reload(core);
}

/// The user settings changed: the footers follow `ui.viewFooter`; a Lighthouse bar switched on
/// reads the runs.
pub fn on_user_settings_changed(core: &Core) {
    core.host()
        .set_footer_settings(core.user_settings().ui.view_footer);
    auto::on_user_settings_changed(core);
    lighthouse_runs::reload(core);
}

//! The pseudo-localisation of the page (RB-092-02): `pseudo_locale`, the command
//! `overlays.pseudo_locale` once per resolved target. `dir` and `lang` are emulation fields of
//! `emulation_set`.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::TargetArg;

/// The factor's range (`rb_config::PSEUDO_FACTOR_RANGE`).
const FACTOR_RANGE: (f64, f64) = (1.0, 3.0);

/// `pseudo_locale`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PseudoLocaleParams {
    /// `"all"`, a view id or a breakpoint id; the active view when absent.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// On (the default) or off.
    #[serde(default)]
    pub on: Option<bool>,
    /// How much longer each text gets, 1 to 3; the user's `emulation.pseudoLocale.factor`
    /// (1.4) when absent.
    #[serde(default)]
    pub factor: Option<f64>,
    /// Accented letters; the user's setting when absent.
    #[serde(default)]
    pub accents: Option<bool>,
    /// The texts also run right to left; the user's setting when absent.
    #[serde(default)]
    pub rtl: Option<bool>,
}

#[tool_router(router = pseudo_locale_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Pseudo-localise the page's texts in the targeted views, live and without a reload: every text node longer by factor (1 to 3, default the user's 1.4), letters accented, [ ] marking its edges, optionally running right to left - a localisation stress test of the layout in every breakpoint. Input values, script, style, code, pre and editable content keep their text, and the sync keeps working: the selectors read the original texts. on: false puts the original texts back. Chromium views only; Firefox and WebKit views come back as unsupported. dir and lang of <html> are emulation_set fields. target: \"all\", a view id or a breakpoint id; default the active view. Answers { on, views, unsupported }."
    )]
    pub(crate) async fn pseudo_locale(
        &self,
        Parameters(params): Parameters<PseudoLocaleParams>,
    ) -> ToolResult {
        if let Some(factor) = params.factor
            && !(factor.is_finite() && (FACTOR_RANGE.0..=FACTOR_RANGE.1).contains(&factor))
        {
            return respond::<Value>(Err(ToolFailure::new(
                "invalid_arguments",
                "factor is a number from 1 to 3",
            )));
        }
        let mut args = Map::new();
        args.insert("on".to_owned(), json!(params.on.unwrap_or(true)));
        if let Some(factor) = params.factor {
            args.insert("factor".to_owned(), json!(factor));
        }
        if let Some(accents) = params.accents {
            args.insert("accents".to_owned(), json!(accents));
        }
        if let Some(rtl) = params.rtl {
            args.insert("rtl".to_owned(), json!(rtl));
        }
        respond(
            self.per_target(
                "overlays.pseudo_locale",
                params.target.as_ref(),
                Value::Object(args),
            )
            .await,
        )
    }
}

#[cfg(test)]
#[path = "pseudo_locale_tests.rs"]
mod tests;

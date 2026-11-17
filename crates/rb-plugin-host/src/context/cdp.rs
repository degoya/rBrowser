//! `cdp.send` behind the grants and the CDP allowlist ([`crate::cdp_policy`]): the manifest's
//! domains, the allowlisted methods, their parameters, and what needs the view - the plugin's own
//! world for an evaluation in an isolated context, the view's own site for cookies.

use rb_plugin_api::MAX_JSON_CHARS;
use rb_plugin_types::{Capability, Failure, FailureKind, ViewId, codes, rules};
use serde_json::{Value, json};

use super::{PluginContext, bounded_json};
use crate::{
    cdp_policy::{self, Pending},
    failure::too_large,
};

impl PluginContext {
    /// `cdp.send`: only the manifest's domains, only the allowlisted methods, with their
    /// parameters held to the plugin's own world and the view's own site.
    pub(crate) async fn cdp_send(
        &self,
        view: ViewId,
        method: &str,
        params: &str,
    ) -> Result<String, Failure> {
        self.require(Capability::Cdp)?;
        rules::check_cdp_method(method, &self.grants.cdp_domains)?;
        if params.len() > MAX_JSON_CHARS {
            return Err(too_large(MAX_JSON_CHARS));
        }
        let parsed: Value = if params.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(params)
                .map_err(|_| Failure::coded(FailureKind::BadData, codes::INVALID))?
        };
        match cdp_policy::check(&self.plugin_id, method, &parsed)? {
            Pending::None => {}
            // The view's window image without a clip: a permission of its own (review S2 of
            // RB-080-64), granted to the colour picker only.
            Pending::WindowImage => {
                if !self.grants.cdp_window_image {
                    return Err(cdp_policy::denied(method, "window_image"));
                }
            }
            Pending::OwnContext(context) => {
                if self.own_world_context(view).await? != context {
                    return Err(cdp_policy::denied(method, "context"));
                }
            }
            pending => {
                let url = self
                    .services
                    .views()
                    .into_iter()
                    .find(|info| info.id == view)
                    .map(|info| info.url)
                    .unwrap_or_default();
                if !cdp_policy::view_allows(&pending, &url) {
                    return Err(cdp_policy::denied(method, "origin"));
                }
            }
        }
        if method == "Page.captureScreenshot" {
            let wait = self.screenshots.reserve();
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
        }
        // What was checked is what is sent: no second reading of duplicate keys.
        let checked = parsed.to_string();
        bounded_json(
            self.services
                .cdp_send(&self.caller(), view, method, &checked)
                .await?,
        )
    }

    /// The execution context of the plugin's own world `rbplugin-<id>` in `view`'s main frame
    /// now, asked of the page itself: a context id the plugin names is compared with it, so a
    /// guessed id - the sync agent's world, another plugin's - never passes, and neither does an
    /// id of a document the view has left.
    async fn own_world_context(&self, view: ViewId) -> Result<i64, Failure> {
        let failed = |reason: &str| {
            Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param("reason", reason)
        };
        let caller = self.caller();
        let tree = self
            .services
            .cdp_send(&caller, view, "Page.getFrameTree", "{}")
            .await?;
        let tree: Value = serde_json::from_str(&tree).map_err(|_| failed("frame"))?;
        let frame = tree
            .pointer("/frameTree/frame/id")
            .and_then(Value::as_str)
            .ok_or_else(|| failed("frame"))?;
        let world = json!({
            "frameId": frame,
            "worldName": crate::bridge::world_name(&self.plugin_id),
            "grantUniveralAccess": false,
        });
        let made = self
            .services
            .cdp_send(
                &caller,
                view,
                "Page.createIsolatedWorld",
                &world.to_string(),
            )
            .await?;
        let made: Value = serde_json::from_str(&made).map_err(|_| failed("world"))?;
        made.get("executionContextId")
            .and_then(Value::as_i64)
            .ok_or_else(|| failed("world"))
    }
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;

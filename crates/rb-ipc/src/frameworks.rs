//! The frameworks each view's page runs, as the shell offers their DevTools (RB-080-24): the
//! state's `frameworks`, one entry per view with a finding. A framework appears only while its
//! setting (`frameworkDevtools.vue` / `.react`) is not off. Mirror: `ui/src/ipc/frameworks.ts`.

use rb_engine::ViewId;
use rb_sync::FrameworkInfo;
use serde::{Deserialize, Serialize};

/// What a view's page runs of Vue and React.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewFrameworks {
    /// The view.
    pub view: ViewId,
    /// Vue, when the page runs it and Vue DevTools are not off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vue: Option<FrameworkInfo>,
    /// React, when the page runs it and React DevTools are not off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub react: Option<FrameworkInfo>,
}

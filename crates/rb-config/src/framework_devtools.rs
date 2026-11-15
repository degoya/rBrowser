//! `frameworkDevtools` of the user settings (RB-080-24, DevTools & Debugging > Framework
//! DevTools): whether rBrowser's Vue and React DevTools backends go into the views' pages.
//! Read by `crates/rbrowser/src/sync_agent/framework.rs`, which registers the main-world
//! scripts; the panel is the `framework-devtools` plugin.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How a framework's DevTools are offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FrameworkDevtoolsMode {
    /// The probe looks for the framework in every page; once it is found, the view offers the
    /// DevTools and the hook goes into the view's next document (reload the view once).
    #[default]
    Auto,
    /// The hook goes into every document of every view before the page's own scripts, so
    /// the DevTools connect without a reload.
    On,
    /// Nothing is injected and nothing is offered.
    Off,
}

impl FrameworkDevtoolsMode {
    /// Whether the framework is looked for at all.
    pub const fn is_on(self) -> bool {
        !matches!(self, FrameworkDevtoolsMode::Off)
    }
}

/// `frameworkDevtools`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct FrameworkDevtoolsSettings {
    /// Vue DevTools (Vue 3, `@vue/devtools-kit`). A production build shows its components only
    /// when it is built with `__VUE_PROD_DEVTOOLS__`; the panel says so.
    pub vue: FrameworkDevtoolsMode,
    /// React DevTools (`react-devtools-inline`): components, props, hooks and the profiler.
    /// React registers with the hook only when the hook is there before React loads, so a view
    /// opened before the hook needs one reload.
    pub react: FrameworkDevtoolsMode,
}

impl FrameworkDevtoolsSettings {
    /// Whether the probe runs at all: at least one framework is not off.
    pub const fn probes(&self) -> bool {
        self.vue.is_on() || self.react.is_on()
    }
}

#[cfg(test)]
#[path = "framework_devtools_tests.rs"]
mod tests;

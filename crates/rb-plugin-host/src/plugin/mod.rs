//! A verified plugin compiled, linked and ready to call: the exports of every kind its manifest
//! declares, behind its health, its call gate and its limits.
//!
//! Built like rCommander's `ComparePlugin` (`crates/rc-plugin-host/src/compare/mod.rs`): the
//! component compiled with its imports held to the grants, linked by hand, one fresh store per
//! call, the call bounded by fuel, the epoch deadline and the caller's wall clock, its answer
//! checked (`answers.rs`), and a malfunction counted towards suspension. A trap ends the one call
//! and never the host: the next call - of this plugin or any other - starts in a fresh store.

use std::{path::PathBuf, sync::Arc, time::Duration};

use rb_plugin_types::{
    CheckContext, ExportFormat, ExportOutput, Failure, FailureKind, Finding, PluginKind, Proposal,
    RuleInfo, codes,
};
use wasmtime::component::{
    ComponentExportIndex, ComponentNamedList, InstancePre, Lift, Linker, Lower,
};

use crate::{
    PluginHealth, VerifiedPackage,
    bindings::{analyzer_world, check_world, export_world, rbrowser::plugin::types},
    context::PluginContext,
    failure,
    link::{from_wit_finding, from_wit_severity, link},
    manifest::PluginManifest,
    runtime::{PluginStoreState, SandboxEngine},
    services::{HostServices, NoServices},
};

pub(crate) mod answers;
mod exports;

use exports::Exports;

/// How many times its compute budget a call may take on the caller's clock: the epoch deadline
/// ends a busy plugin; this ends one parked in a host call past the time credited back to it.
pub(crate) const WALL_CLOCK_FACTOR: u32 = 3;

type WitResult<T> = Result<T, types::Failure>;

/// What the application decides about a plugin's calls.
#[derive(Clone)]
pub struct PluginOptions {
    /// The services its host functions reach.
    pub services: Arc<dyn HostServices>,
    /// The open project, for `project-read` and `out-write`.
    pub project_root: Option<PathBuf>,
    /// The plugin's settings, JSON text by key.
    pub settings: Vec<(String, String)>,
    /// The plugin's own - shared by every version of it, so a suspension holds across them, and
    /// never by another plugin ([`crate::PluginHealths`], RB-090-59).
    pub health: Arc<PluginHealth>,
}

impl Default for PluginOptions {
    fn default() -> Self {
        Self {
            services: Arc::new(NoServices),
            project_root: None,
            settings: Vec::new(),
            health: Arc::new(PluginHealth::default()),
        }
    }
}

/// A plugin ready to call.
#[derive(Clone)]
pub struct LoadedPlugin {
    shared: Arc<Shared>,
}

struct Shared {
    manifest: PluginManifest,
    sandbox: SandboxEngine,
    pre: InstancePre<PluginStoreState>,
    exports: Exports,
    context: Arc<PluginContext>,
    health: Arc<PluginHealth>,
}

impl LoadedPlugin {
    /// Compiles a verified package's component, checks it exports every declared kind and links
    /// what its manifest grants. Compiling takes Cranelift's time on a cache miss; call it off
    /// the UI thread.
    ///
    /// # Errors
    ///
    /// The component does not compile (`plugins.error.invalid`), imports what is not granted
    /// (`plugins.error.capability_denied`, `plugins.error.denied`) or lacks a declared kind's
    /// exports (`plugins.error.world_missing`).
    pub fn load(package: &VerifiedPackage, options: PluginOptions) -> Result<Self, Failure> {
        let manifest = package.manifest.clone();
        let sandbox = SandboxEngine::new(manifest.limits).map_err(failure::host_failed)?;
        let component = sandbox.compile_component(&package.component, &manifest)?;
        let exports = Exports::find(&component, &manifest.kinds).map_err(world_missing)?;
        let mut linker = Linker::new(sandbox.engine());
        link(&mut linker, &manifest).map_err(failure::host_failed)?;
        let pre = linker.instantiate_pre(&component).map_err(|error| {
            tracing::debug!(%error, "a plugin's component does not link");
            failure::coded(FailureKind::AccessDenied, codes::DENIED)
        })?;
        let context = PluginContext::new(
            &manifest.id,
            &manifest.version.to_string(),
            manifest.grants.clone(),
            options.services,
        )
        .with_project(options.project_root)
        .with_settings(options.settings)
        .with_max_response_bytes(manifest.limits.max_response_bytes)
        .with_timeout(Duration::from_millis(manifest.limits.timeout()));
        Ok(Self {
            shared: Arc::new(Shared {
                manifest,
                sandbox,
                pre,
                exports,
                context: Arc::new(context),
                health: options.health,
            }),
        })
    }

    #[must_use]
    pub fn manifest(&self) -> &PluginManifest {
        &self.shared.manifest
    }

    /// The failure count and suspension; the person's Resume is [`PluginHealth::resume`].
    #[must_use]
    pub fn health(&self) -> &Arc<PluginHealth> {
        &self.shared.health
    }

    /// Whether a call now would wait for a running one: its `limits.max_concurrent` calls run
    /// (a check run, another panel's call). RB-080-90.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.shared
            .health
            .busy(self.shared.sandbox.max_concurrent())
    }

    /// `feature.activate`.
    ///
    /// # Errors
    ///
    /// The plugin is no feature plugin, failed, or broke a limit.
    pub async fn activate(&self) -> Result<(), Failure> {
        let index = self.feature()?.activate;
        let (answer,) = self
            .call::<(), (WitResult<()>,)>("activate", index, ())
            .await?;
        answer.map_err(answers::from_wit_failure)
    }

    /// `feature.run-command` for the manifest's command `command` (its last segment).
    ///
    /// # Errors
    ///
    /// The command is not declared (`plugins.error.unknown_contribution`), failed, broke a limit
    /// or answered no JSON.
    pub async fn run_command(&self, command: &str, args: &str) -> Result<String, Failure> {
        if !self
            .shared
            .manifest
            .commands
            .iter()
            .any(|entry| entry.id == command)
        {
            return Err(unknown(command));
        }
        let index = self.feature()?.run_command;
        let params = (command.to_owned(), args.to_owned());
        let (answer,) = self
            .call::<(String, String), (WitResult<String>,)>("run-command", index, params)
            .await?;
        answers::json(answer.map_err(answers::from_wit_failure)?)
    }

    /// `feature.call-tool` for the manifest's tool `tool` (without the MCP prefix).
    ///
    /// # Errors
    ///
    /// As [`LoadedPlugin::run_command`].
    pub async fn call_tool(&self, tool: &str, args: &str) -> Result<String, Failure> {
        if !self
            .shared
            .manifest
            .tools
            .iter()
            .any(|entry| entry.name == tool)
        {
            return Err(unknown(tool));
        }
        let index = self.feature()?.call_tool;
        let params = (tool.to_owned(), args.to_owned());
        let (answer,) = self
            .call::<(String, String), (WitResult<String>,)>("call-tool", index, params)
            .await?;
        answers::json(answer.map_err(answers::from_wit_failure)?)
    }

    /// A request of the plugin's page script in `view` (RB-060-12): `feature.call-tool` with
    /// [`crate::PAGE_MESSAGE_TOOL`] and `{"view", "message"}`.
    ///
    /// # Errors
    ///
    /// The plugin has no page script (`plugins.error.unknown_contribution`), failed, broke a
    /// limit or answered no JSON.
    pub async fn page_message(
        &self,
        view: u32,
        message: &serde_json::Value,
    ) -> Result<String, Failure> {
        if self.shared.manifest.page_scripts.is_empty() {
            return Err(unknown(crate::PAGE_MESSAGE_TOOL));
        }
        let index = self.feature()?.call_tool;
        let params = (
            crate::PAGE_MESSAGE_TOOL.to_owned(),
            crate::page_request_args(view, message),
        );
        let (answer,) = self
            .call::<(String, String), (WitResult<String>,)>("call-tool", index, params)
            .await?;
        answers::json(answer.map_err(answers::from_wit_failure)?)
    }

    /// The host asks a plugin granted `network-rules` for its declarative interception rules
    /// (RB-070-02): `feature.call-tool` with [`crate::NETWORK_RULES_TOOL`] and `args` (the
    /// host's `$network` state). The answer is the rules document the app validates and applies.
    ///
    /// # Errors
    ///
    /// The plugin is not granted `network-rules` (`plugins.error.unknown_contribution`), failed,
    /// broke a limit or answered no JSON.
    pub async fn network_rules(&self, args: &str) -> Result<String, Failure> {
        if !self
            .shared
            .manifest
            .grants
            .has(rb_plugin_types::Capability::NetworkRules)
        {
            return Err(unknown(crate::NETWORK_RULES_TOOL));
        }
        let index = self.feature()?.call_tool;
        let params = (crate::NETWORK_RULES_TOOL.to_owned(), args.to_owned());
        let (answer,) = self
            .call::<(String, String), (WitResult<String>,)>("call-tool", index, params)
            .await?;
        answers::json(answer.map_err(answers::from_wit_failure)?)
    }

    /// `check.describe`.
    ///
    /// # Errors
    ///
    /// The plugin is no check plugin, broke a limit or answered malformed rules.
    pub async fn describe(&self) -> Result<Vec<RuleInfo>, Failure> {
        let index = self.check_exports()?.describe;
        type Wit = check_world::exports::rbrowser::plugin::check::RuleInfo;
        let (infos,) = self.call::<(), (Vec<Wit>,)>("describe", index, ()).await?;
        let rules = infos
            .into_iter()
            .map(|info| RuleInfo {
                id: info.id,
                title: info.title,
                severity: from_wit_severity(info.severity),
                shared_id: info.shared_id,
                help_url: info.help_url,
            })
            .collect();
        answers::rules(rules, &self.shared.manifest.key_prefix())
    }

    /// `check.run` on `ctx`.
    ///
    /// # Errors
    ///
    /// The plugin is no check plugin, failed, broke a limit or answered malformed findings.
    pub async fn run_check(&self, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
        let index = self.check_exports()?.run;
        type Wit = check_world::exports::rbrowser::plugin::check::CheckContext;
        let params = (Wit {
            view: ctx.view,
            url: ctx.url.clone(),
            rules: ctx.rules.clone(),
            args: ctx.args.clone(),
        },);
        let (answer,) = self
            .call::<(Wit,), (WitResult<Vec<types::Finding>>,)>("run", index, params)
            .await?;
        let found = answer.map_err(answers::from_wit_failure)?;
        answers::findings(
            found.into_iter().map(from_wit_finding).collect(),
            &self.shared.manifest.key_prefix(),
        )
    }

    /// `analyzer.analyze`.
    ///
    /// # Errors
    ///
    /// The plugin is no analyzer, failed, broke a limit or answered malformed proposals.
    pub async fn analyze(&self, args: &str) -> Result<Vec<Proposal>, Failure> {
        let index = self.exports_of(PluginKind::Analyzer)?.analyzer;
        let index = index
            .ok_or_else(|| world_missing(PluginKind::Analyzer))?
            .analyze;
        type Wit = analyzer_world::exports::rbrowser::plugin::analyzer::Proposal;
        let (answer,) = self
            .call::<(String,), (WitResult<Vec<Wit>>,)>("analyze", index, (args.to_owned(),))
            .await?;
        let proposals = answer
            .map_err(answers::from_wit_failure)?
            .into_iter()
            .map(|proposal| Proposal {
                pointer: proposal.pointer,
                value: proposal.value,
                reason: proposal.reason,
                params: proposal.params,
            })
            .collect();
        answers::proposals(proposals, &self.shared.manifest.key_prefix())
    }

    /// `exporter.formats`.
    ///
    /// # Errors
    ///
    /// The plugin is no export plugin, broke a limit or answered malformed formats.
    pub async fn formats(&self) -> Result<Vec<ExportFormat>, Failure> {
        let index = self.exports_of(PluginKind::Export)?.exporter;
        let index = index
            .ok_or_else(|| world_missing(PluginKind::Export))?
            .formats;
        type Wit = export_world::exports::rbrowser::plugin::exporter::ExportFormat;
        let (formats,) = self.call::<(), (Vec<Wit>,)>("formats", index, ()).await?;
        let formats = formats
            .into_iter()
            .map(|format| ExportFormat {
                id: format.id,
                title: format.title,
                extension: format.extension,
            })
            .collect();
        answers::formats(formats, &self.shared.manifest.key_prefix())
    }

    /// `exporter.render`.
    ///
    /// # Errors
    ///
    /// The plugin is no export plugin, failed, broke a limit or answered a malformed file.
    pub async fn render(&self, format: &str, recording: &str) -> Result<ExportOutput, Failure> {
        let index = self.exports_of(PluginKind::Export)?.exporter;
        let index = index
            .ok_or_else(|| world_missing(PluginKind::Export))?
            .render;
        type Wit = export_world::exports::rbrowser::plugin::exporter::ExportOutput;
        let params = (format.to_owned(), recording.to_owned());
        let (answer,) = self
            .call::<(String, String), (WitResult<Wit>,)>("render", index, params)
            .await?;
        let output = answer.map_err(answers::from_wit_failure)?;
        answers::export_output(ExportOutput {
            file_name: output.file_name,
            text: output.text,
        })
    }

    fn check_exports(&self) -> Result<exports::CheckExports, Failure> {
        self.exports_of(PluginKind::Check)?
            .check
            .ok_or_else(|| world_missing(PluginKind::Check))
    }

    fn feature(&self) -> Result<exports::FeatureExports, Failure> {
        self.exports_of(PluginKind::Feature)?
            .feature
            .ok_or_else(|| world_missing(PluginKind::Feature))
    }

    fn exports_of(&self, kind: PluginKind) -> Result<&Exports, Failure> {
        if self.shared.manifest.is(kind) {
            Ok(&self.shared.exports)
        } else {
            Err(world_missing(kind))
        }
    }

    /// The longest the caller waits for one call.
    fn wall_clock(&self) -> Duration {
        self.shared.sandbox.timeout() * WALL_CLOCK_FACTOR
    }

    /// Runs one call unless the plugin is suspended, within its concurrency, and counts a
    /// malfunction towards suspension.
    async fn call<P, R>(
        &self,
        operation: &'static str,
        index: ComponentExportIndex,
        params: P,
    ) -> Result<R, Failure>
    where
        P: ComponentNamedList + Lower + Send + Sync + 'static,
        R: ComponentNamedList + Lift + Send + Sync + 'static,
    {
        let shared = &self.shared;
        shared.health.check()?;
        let _running = shared.health.enter(shared.sandbox.max_concurrent()).await;
        let result = self.call_once(operation, index, params).await;
        if let Err(failure) = &result
            && counts_as_failure(failure)
            && shared.health.record_failure()
        {
            tracing::warn!(
                code = codes::SUSPENDED,
                plugin_id = shared.manifest.id.as_str(),
                plugin_version = %shared.manifest.version,
                "a plugin failed repeatedly and is suspended until it is resumed"
            );
        }
        result
    }

    /// One call in a fresh store: instantiated, the export typed and called, all within the
    /// wall clock.
    async fn call_once<P, R>(
        &self,
        operation: &'static str,
        index: ComponentExportIndex,
        params: P,
    ) -> Result<R, Failure>
    where
        P: ComponentNamedList + Lower + Send + Sync + 'static,
        R: ComponentNamedList + Lift + Send + Sync + 'static,
    {
        let shared = &self.shared;
        let mut store = shared
            .sandbox
            .create_store(Arc::clone(&shared.context))
            .map_err(failure::host_failed)?;
        shared
            .sandbox
            .begin_call(&mut store, operation)
            .map_err(failure::host_failed)?;
        let call = async {
            let instance = shared.pre.instantiate_async(&mut store).await?;
            let function = instance.get_typed_func::<P, R>(&mut store, index)?;
            function.call_async(&mut store, params).await
        };
        match tokio::time::timeout(self.wall_clock(), call).await {
            Ok(Ok(results)) => Ok(results),
            Ok(Err(error)) => Err(failure::stopped(Some(&error))),
            Err(_) => Err(failure::stopped(None)),
        }
    }
}

/// Whether a failure is the plugin's malfunction, which counts towards suspension - not a
/// failure it answered on purpose.
fn counts_as_failure(failure: &Failure) -> bool {
    matches!(
        failure.code(),
        Some(
            codes::CRASHED
                | codes::TIMED_OUT
                | codes::OUT_OF_FUEL
                | codes::OUT_OF_MEMORY
                | codes::BAD_OUTPUT
        )
    )
}

fn world_missing(kind: PluginKind) -> Failure {
    failure::coded(FailureKind::BadData, codes::WORLD_MISSING).with_param("kind", kind.world())
}

fn unknown(name: &str) -> Failure {
    failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", failure::readable(name, 64))
}

#[cfg(test)]
#[path = "plugin_tests.rs"]
mod tests;

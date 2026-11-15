//! Manifests, signed packages and a scripted service backend for tests - this crate's and,
//! through the `test-support` feature, those of the crates above. Never in a shipped build.
//!
//! Adapted from rCommander's `crates/rc-plugin-host/src/test_support.rs` and `test_fixtures.rs`.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use ed25519_dalek::SigningKey;
use rb_plugin_types::{Failure, Finding, HttpRequest, HttpResponse, ViewId, ViewInfo};

use crate::{
    package_plugin, public_key_base64,
    services::{Caller, HostServices},
};

/// The key id every test manifest names.
pub const KEY_ID: &str = "test-v1";

/// A signing key from a fixed seed: the same key in every run.
#[must_use]
pub fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

/// A manifest for tests, `sample` by default: a feature plugin with the MCP prefix `sample`,
/// English locales and a 1 s time limit per call.
#[derive(Clone, Debug)]
pub struct ManifestBuilder {
    pub id: String,
    pub mcp_prefix: Option<String>,
    pub kinds: Vec<String>,
    /// The lines of `[capabilities]`.
    pub capabilities: Vec<String>,
    /// The lines of `[limits]`.
    pub limits: Vec<String>,
    /// More tables after the manifest's own (`[[commands]]`, `[[tools]]`, `[settings]`).
    pub tables: String,
}

impl Default for ManifestBuilder {
    fn default() -> Self {
        Self {
            id: "sample".to_owned(),
            mcp_prefix: Some("sample".to_owned()),
            kinds: vec!["feature-plugin".to_owned()],
            capabilities: Vec::new(),
            limits: vec!["timeout_milliseconds = 1000".to_owned()],
            tables: String::new(),
        }
    }
}

impl ManifestBuilder {
    /// Plugin `id` with MCP prefix `prefix`.
    #[must_use]
    pub fn named(id: &str, prefix: &str) -> Self {
        Self {
            id: id.to_owned(),
            mcp_prefix: Some(prefix.to_owned()),
            ..Self::default()
        }
    }

    /// Of the kinds `kinds` (world names).
    #[must_use]
    pub fn kinds(mut self, kinds: &[&str]) -> Self {
        self.kinds = kinds.iter().map(|kind| (*kind).to_owned()).collect();
        self
    }

    /// Granting `line` (`http = true`).
    #[must_use]
    pub fn capability(mut self, line: &str) -> Self {
        self.capabilities.push(line.to_owned());
        self
    }

    /// With `[limits]` `line` added (`fuel = 1000000`).
    #[must_use]
    pub fn limit(mut self, line: &str) -> Self {
        self.limits.push(line.to_owned());
        self
    }

    /// With the tables `tables` appended.
    #[must_use]
    pub fn tables(mut self, tables: &str) -> Self {
        self.tables.push_str(tables);
        self
    }

    /// The manifest's text, naming `key`'s public half.
    #[must_use]
    pub fn text(&self, key: &SigningKey) -> String {
        let kinds: Vec<String> = self
            .kinds
            .iter()
            .map(|kind| format!("\"{kind}\""))
            .collect();
        let prefix = self
            .mcp_prefix
            .as_ref()
            .map(|prefix| format!("mcp_prefix = \"{prefix}\"\n"))
            .unwrap_or_default();
        format!(
            "manifest_version = 1\nid = \"{id}\"\nname = \"Sample\"\nversion = \"1.0.0\"\n\
             api_version = \"{api}\"\nkinds = [{kinds}]\n{prefix}key_id = \"{KEY_ID}\"\n\
             public_key = \"{public}\"\ndescription = \"A plugin the host's tests run.\"\n\n\
             [locales]\nen = \"locales/en.json\"\n\n[capabilities]\n{capabilities}\n\n\
             [limits]\n{limits}\n{tables}",
            id = self.id,
            api = rb_plugin_api::API_VERSION,
            kinds = kinds.join(", "),
            public = public_key_base64(key),
            capabilities = self.capabilities.join("\n"),
            limits = self.limits.join("\n"),
            tables = self.tables,
        )
    }

    /// The English locale file of the plugin: every text the test beds answer.
    #[must_use]
    pub fn locale(&self) -> (String, Vec<u8>) {
        let texts = serde_json::json!({
            "plugin": { self.id.clone(): {
                "name": "Sample",
                "checks": { "basic": "Basic check" },
                "findings": { "basic": "An image without alt text" },
                "formats": { "text": "Plain text" },
                "commands": { "run": "Run the sample" },
                "tools": { "run": "Runs the sample" },
            }}
        });
        ("locales/en.json".to_owned(), texts.to_string().into_bytes())
    }

    /// A package of `component` under this manifest with its locale and `extra` members, signed
    /// by `key` - or unsigned with `signed` off, which only development mode accepts.
    ///
    /// # Panics
    ///
    /// The packager refuses it, which a test helper may.
    #[must_use]
    pub fn package(
        &self,
        component: &[u8],
        key: &SigningKey,
        signed: bool,
        extra: &[(String, Vec<u8>)],
    ) -> Vec<u8> {
        let mut files = vec![self.locale()];
        files.extend_from_slice(extra);
        package_plugin(
            self.text(key).as_bytes(),
            component,
            &files,
            signed.then_some(key),
        )
        .expect("a valid test package")
    }

    /// The same package signed by hand, without the packager's checks: what an author
    /// bypassing them would publish.
    ///
    /// # Panics
    ///
    /// The archive cannot be written, which a test helper may.
    #[must_use]
    pub fn package_unchecked(&self, component: &[u8], key: &SigningKey) -> Vec<u8> {
        let manifest = self.text(key);
        let files = vec![self.locale()];
        let digest = crate::package_digest(manifest.as_bytes(), component, &files);
        let signature = crate::sign_detached(key, &digest);
        zip_package(&[
            (crate::MANIFEST_MEMBER, manifest.as_bytes()),
            (crate::COMPONENT_MEMBER, component),
            (crate::SIGNATURE_MEMBER, signature.as_bytes()),
            (files[0].0.as_str(), files[0].1.as_slice()),
        ])
    }
}

/// A `.rbplugin` made of exactly `members`, as a tampering hand would make one.
///
/// # Panics
///
/// The archive cannot be written, which a test helper may.
#[must_use]
pub fn zip_package(members: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write as _;

    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in members {
        writer.start_file(*name, options).expect("member");
        writer.write_all(bytes).expect("write member");
    }
    writer.finish().expect("archive").into_inner()
}

/// `package` with the member `name` changed by `change`, every other member as signed.
///
/// # Panics
///
/// The package is no archive, which a test helper may.
#[must_use]
pub fn tampered(package: &[u8], name: &str, change: impl Fn(&mut Vec<u8>)) -> Vec<u8> {
    use std::io::Read as _;

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(package)).expect("archive");
    let names: Vec<String> = archive
        .file_names()
        .map(|name| name.expect("member name").into_owned())
        .collect();
    let mut members = Vec::new();
    for member in names {
        let mut bytes = Vec::new();
        archive
            .by_name(&member)
            .expect("member")
            .read_to_end(&mut bytes)
            .expect("read member");
        if member == name {
            change(&mut bytes);
        }
        members.push((member, bytes));
    }
    let borrowed: Vec<(&str, &[u8])> = members
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    zip_package(&borrowed)
}

/// A service backend answering from memory: two views, `{}` for CDP, `200` for every fetch,
/// `null` for the sidecar and `"ok"` for `evaluate`; it keeps what it was asked.
#[derive(Debug, Default)]
pub struct ScriptedServices {
    /// The caller of every `evaluate`, `cdp` and `sidecar` call: `(plugin id, timeout)`.
    pub callers: Mutex<Vec<(String, Duration)>>,
    /// What `project_root` answers.
    pub project: Mutex<Option<PathBuf>>,
    /// What `setting` answers: `(plugin id, key, JSON text)`.
    pub settings: Mutex<Vec<(String, String, String)>>,
    /// Every request fetched.
    pub fetched: Mutex<Vec<HttpRequest>>,
    /// Every finding reported, with the plugin's id.
    pub findings: Mutex<Vec<(String, Finding)>>,
    /// Every progress reported, as `(plugin id, done, total)`.
    pub progress: Mutex<Vec<(String, u32, u32)>>,
    /// What `workspace_trusted` answers; untrusted by default.
    pub trusted: AtomicBool,
    /// Every CDP command sent, as `(view, method, params)`.
    pub cdp_sent: Mutex<Vec<(ViewId, String, String)>>,
    /// What `cdp_send` answers, by method; `{}` for any other.
    pub cdp_answers: Mutex<Vec<(String, String)>>,
    /// The URL a view shows, by view id; empty for one not named.
    pub urls: Mutex<Vec<(ViewId, String)>>,
    /// What `lighthouse_any_url` answers; held to the views' sites by default.
    pub lighthouse_any_url: AtomicBool,
}

impl ScriptedServices {
    fn called(&self, caller: &Caller<'_>) {
        if let Ok(mut callers) = self.callers.lock() {
            callers.push((caller.plugin_id.to_owned(), caller.timeout));
        }
    }
}

#[async_trait]
impl HostServices for ScriptedServices {
    fn views(&self) -> Vec<ViewInfo> {
        let urls = self
            .urls
            .lock()
            .map(|urls| urls.clone())
            .unwrap_or_default();
        let url = |id: ViewId| {
            urls.iter()
                .find(|(view, _)| *view == id)
                .map(|(_, url)| url.clone())
                .unwrap_or_default()
        };
        vec![
            ViewInfo {
                id: 1,
                label: "Mobile".to_owned(),
                width: 375,
                height: 812,
                dpr: 3.0,
                url: url(1),
                ..ViewInfo::default()
            },
            ViewInfo {
                id: 2,
                label: "Desktop".to_owned(),
                width: 1440,
                height: 900,
                dpr: 1.0,
                url: url(2),
                ..ViewInfo::default()
            },
        ]
    }

    fn active_view(&self) -> Option<ViewId> {
        Some(1)
    }

    fn project_root(&self) -> Option<PathBuf> {
        self.project.lock().ok().and_then(|project| project.clone())
    }

    fn workspace_trusted(&self) -> bool {
        self.trusted.load(Ordering::SeqCst)
    }

    fn lighthouse_any_url(&self) -> bool {
        self.lighthouse_any_url.load(Ordering::SeqCst)
    }

    fn setting(&self, plugin_id: &str, key: &str) -> Option<String> {
        self.settings.lock().ok().and_then(|settings| {
            settings
                .iter()
                .find(|(plugin, name, _)| plugin == plugin_id && name == key)
                .map(|(_, _, value)| value.clone())
        })
    }

    async fn evaluate(
        &self,
        caller: &Caller<'_>,
        _view: ViewId,
        _script: &str,
    ) -> Result<String, Failure> {
        self.called(caller);
        Ok("\"ok\"".to_owned())
    }

    async fn cdp_send(
        &self,
        caller: &Caller<'_>,
        view: ViewId,
        method: &str,
        params: &str,
    ) -> Result<String, Failure> {
        self.called(caller);
        if let Ok(mut sent) = self.cdp_sent.lock() {
            sent.push((view, method.to_owned(), params.to_owned()));
        }
        let answer = self.cdp_answers.lock().ok().and_then(|answers| {
            answers
                .iter()
                .find(|(listed, _)| listed == method)
                .map(|(_, answer)| answer.clone())
        });
        Ok(answer.unwrap_or_else(|| "{}".to_owned()))
    }

    async fn fetch(
        &self,
        request: HttpRequest,
        _max_response_bytes: u64,
    ) -> Result<HttpResponse, Failure> {
        if let Ok(mut fetched) = self.fetched.lock() {
            fetched.push(request);
        }
        Ok(HttpResponse {
            status: 200,
            ..HttpResponse::default()
        })
    }

    async fn sidecar_call(
        &self,
        caller: &Caller<'_>,
        _method: &str,
        _params: &str,
    ) -> Result<String, Failure> {
        self.called(caller);
        Ok("null".to_owned())
    }

    fn report_progress(&self, plugin_id: &str, done: u32, total: u32) {
        if let Ok(mut progress) = self.progress.lock() {
            progress.push((plugin_id.to_owned(), done, total));
        }
    }

    fn report_findings(&self, plugin_id: &str, findings: Vec<Finding>) -> Result<(), Failure> {
        if let Ok(mut reported) = self.findings.lock() {
            reported.extend(
                findings
                    .into_iter()
                    .map(|finding| (plugin_id.to_owned(), finding)),
            );
        }
        Ok(())
    }
}

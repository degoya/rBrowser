use super::*;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, codes};

use crate::state::{Anchor, Blend, Split};

fn host() -> MemoryHost {
    MemoryHost::new()
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
}

#[test]
fn the_shared_layer_comes_from_the_settings() {
    let mut host = host();
    host.settings.insert("opacity".into(), "70".into());
    host.settings
        .insert("blend".into(), "\"difference\"".into());
    host.settings.insert("invert".into(), "true".into());
    host.settings.insert("anchor".into(), "\"viewport\"".into());
    host.settings.insert("autoScaleDpr".into(), "false".into());
    host.settings.insert(
        "designs".into(),
        r#"{"mobile": "designs/mobile.png"}"#.into(),
    );
    let shared = shared(&mut host);
    assert_eq!(shared.opacity, 70);
    assert_eq!(shared.blend, Blend::Difference);
    assert!(shared.invert);
    assert_eq!(shared.anchor, Anchor::Viewport);
    assert!(!shared.auto_scale_dpr);
    assert_eq!(shared.designs["mobile"], "designs/mobile.png");
}

#[test]
fn a_setting_that_does_not_fit_falls_back_and_is_logged() {
    let mut host = host();
    host.settings.insert("opacity".into(), "300".into());
    host.settings.insert("blend".into(), "\"darken\"".into());
    host.settings
        .insert("designs".into(), r#"{"a b": "designs/x.png"}"#.into());
    let shared = shared(&mut host);
    assert_eq!(shared, Shared::default());
    assert_eq!(host.logs.len(), 3);
}

#[test]
fn a_design_setting_outside_the_folder_stays_for_the_view_to_report() {
    // The mapping keeps the entry: the view it names reports it (`plugin.image-overlay.errors.
    // path_outside`) instead of silently showing another image.
    let mut host = host();
    host.settings
        .insert("designs".into(), r#"{"mobile": "../secret.png"}"#.into());
    assert_eq!(shared(&mut host).designs["mobile"], "../secret.png");
}

#[test]
fn the_overrides_round_trip_through_the_state_file() {
    let mut host = host();
    let overrides = Overrides {
        enabled: Some(true),
        split: Some(Split {
            on: true,
            position: 30.0,
        }),
        ..Overrides::default()
    };
    save(&mut host, &overrides).expect("written");
    let written = host.out.get(STATE_FILE).expect("state.json").clone();
    host.project.insert(STATE_PATH.to_owned(), written);
    assert_eq!(load(&mut host), Loaded::Stored(overrides));
}

#[test]
fn a_missing_file_is_no_override_and_a_broken_one_is_logged() {
    let mut host = host();
    assert_eq!(load(&mut host), Loaded::Stored(Overrides::default()));
    host.project.insert(
        STATE_PATH.to_owned(),
        br#"{"version": 1, "overrides": {"opacity": 900}}"#.to_vec(),
    );
    assert_eq!(load(&mut host), Loaded::Stored(Overrides::default()));
    assert_eq!(host.logs.len(), 1);
}

#[test]
fn without_a_project_the_pages_copy_stands_in() {
    let mut host = MemoryHost::new();
    assert_eq!(load(&mut host), Loaded::Unavailable);
    let page = Overrides {
        opacity: Some(10),
        ..Overrides::default()
    };
    assert_eq!(overrides(&mut host, Some(page.clone())), page);
    let bad = Overrides {
        opacity: Some(200),
        ..Overrides::default()
    };
    assert_eq!(overrides(&mut host, Some(bad)), Overrides::default());
}

/// A host whose workspace is not trusted: the protected folders answer
/// `plugins.error.untrusted_workspace`, as the real host does before it looks a path up.
struct Untrusted(MemoryHost);

impl Untrusted {
    fn refuses(path: &str) -> bool {
        [".rbrowser/flows", ".rbrowser/snippets", ".rbrowser/plugins"]
            .iter()
            .any(|dir| path == *dir || path.starts_with(&format!("{dir}/")))
    }

    fn refusal(path: &str) -> Failure {
        Failure::coded(FailureKind::AccessDenied, codes::UNTRUSTED_WORKSPACE)
            .with_param("path", path)
    }
}

impl PluginHost for Untrusted {
    fn log(&mut self, level: &str, message: &str) {
        self.0.log(level, message);
    }
    fn now_unix_seconds(&mut self) -> u64 {
        self.0.now_unix_seconds()
    }
    fn setting(&mut self, key: &str) -> Option<String> {
        self.0.setting(key)
    }
    fn views(&mut self) -> Result<Vec<rb_plugin_api::ViewInfo>, Failure> {
        self.0.views()
    }
    fn active_view(&mut self) -> Result<Option<rb_plugin_api::ViewId>, Failure> {
        self.0.active_view()
    }
    fn evaluate(&mut self, view: rb_plugin_api::ViewId, script: &str) -> Result<String, Failure> {
        self.0.evaluate(view, script)
    }
    fn cdp_send(
        &mut self,
        view: rb_plugin_api::ViewId,
        method: &str,
        params: &str,
    ) -> Result<String, Failure> {
        self.0.cdp_send(view, method, params)
    }
    fn fetch(
        &mut self,
        request: &rb_plugin_api::HttpRequest,
    ) -> Result<rb_plugin_api::HttpResponse, Failure> {
        self.0.fetch(request)
    }
    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        if Self::refuses(path) {
            return Err(Self::refusal(path));
        }
        self.0.project_read(path)
    }
    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        if Self::refuses(dir) {
            return Err(Self::refusal(dir));
        }
        self.0.project_list(dir)
    }
    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.0.out_write(path, bytes)
    }
    fn report_findings(&mut self, items: &[rb_plugin_api::Finding]) -> Result<(), Failure> {
        self.0.report_findings(items)
    }
    fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure> {
        self.0.sidecar_call(method, params)
    }
}

#[test]
fn an_untrusted_workspace_neither_reads_nor_writes_the_state_file() {
    // Charge 8 review L1: a repository that commits `state.json` with the design on at full
    // opacity would lay any image over every page; without trust the file is not read (the
    // page's copy stands in, as without a project) and not overwritten.
    let committed = br#"{"version": 1, "overrides": {"enabled": true, "opacity": 100}}"#;
    let mut host = Untrusted(host().with_file(STATE_PATH, committed));
    assert_eq!(load(&mut host), Loaded::Unavailable);
    assert_eq!(overrides(&mut host, None), Overrides::default());
    let page = Overrides {
        enabled: Some(true),
        ..Overrides::default()
    };
    assert_eq!(overrides(&mut host, Some(page.clone())), page);
    let refused = save(&mut host, &page).expect_err("untrusted");
    assert_eq!(refused.code.as_deref(), Some(codes::UNTRUSTED_WORKSPACE));
    assert!(host.0.out.is_empty());
}

#[test]
fn a_trusted_workspace_reads_the_committed_state_file() {
    let committed = br#"{"version": 1, "overrides": {"enabled": true, "opacity": 100}}"#;
    let mut host = host().with_file(STATE_PATH, committed);
    let Loaded::Stored(stored) = load(&mut host) else {
        panic!("trusted: the file is read");
    };
    assert_eq!(stored.enabled, Some(true));
    assert_eq!(stored.opacity, Some(100));
}

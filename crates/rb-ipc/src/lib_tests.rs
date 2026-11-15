use serde_json::json;

use super::*;

fn to_json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("serializes")
}

#[test]
fn requests_are_tagged_camel_case() {
    assert_eq!(
        to_json(&ShellToCore::Navigate {
            url: "http://localhost:5173".into()
        }),
        json!({ "type": "navigate", "url": "http://localhost:5173" })
    );
    assert_eq!(
        to_json(&ShellToCore::GetState),
        json!({ "type": "getState" })
    );
    assert_eq!(
        to_json(&ShellToCore::SetBreakpoint {
            view: ViewId(2),
            breakpoint: None,
            width: Some(500),
            height: Some(800),
            dpr: None,
        }),
        json!({ "type": "setBreakpoint", "view": 2, "width": 500, "height": 800 })
    );
    assert_eq!(
        to_json(&ShellToCore::SetSetting {
            key: "sync.scroll".into(),
            value: json!(false),
            scope: Scope::Local,
        }),
        json!({ "type": "setSetting", "key": "sync.scroll", "value": false, "scope": "local" })
    );
    assert_eq!(
        to_json(&ShellToCore::TrustDecision {
            prompt_id: 7,
            decision: TrustDecision::Restricted,
        }),
        json!({ "type": "trustDecision", "promptId": 7, "decision": "restricted" })
    );
}

#[test]
fn requests_parse_with_defaults() {
    assert_eq!(
        parse_request(r#"{ "type": "runCommand", "id": "view.zoom_fit_all" }"#),
        Ok(ShellToCore::RunCommand {
            id: "view.zoom_fit_all".into(),
            args: serde_json::Value::Null,
        })
    );
    assert_eq!(
        parse_request(r#"{ "type": "reload" }"#),
        Ok(ShellToCore::Reload {
            ignore_cache: false
        })
    );
    assert_eq!(
        parse_request(r#"{ "type": "setSync", "feature": "scroll", "enabled": false }"#),
        Ok(ShellToCore::SetSync {
            feature: SyncFeature::Scroll,
            enabled: false,
        })
    );
    let error = parse_request(r#"{ "type": "selfDestruct" }"#).expect_err("unknown type");
    assert_eq!(error.code(), IpcErrorCode::BadRequest);
    assert_eq!(IpcErrorCode::BadRequest as i32, 400);
}

#[test]
fn the_state_snapshot_shape() {
    let state = CoreToShell::State(Box::new(StateSnapshot {
        project: Some(ProjectInfo {
            name: "shop".into(),
            root: "C:/work/shop".into(),
            trusted: true,
        }),
        views: Vec::new(),
        active_view: Some(ViewId(1)),
        url: "http://localhost:5173/".into(),
        environment: Some("local".into()),
        environments: Vec::new(),
        layout: LayoutState {
            preset: Some("grid-main".into()),
            presets: vec!["grid-main".into(), "focus".into()],
            mode: rb_config::LayoutMode::Grid,
            focus: None,
            modes: None,
            hidden: Vec::new(),
        },
        sync: SyncState {
            enabled: true,
            features: rb_config::SyncConfig::default(),
            excluded: vec![ViewId(2)],
            desynced: vec![rb_sync::ViewDesync {
                view: ViewId(3),
                reason: rb_sync::DesyncReason::NotFound,
                selector: Some("nav > a".into()),
                event: "click".into(),
            }],
            navigation_decoupled: vec![ViewId(3)],
            browsersync: Some("http://localhost:3000".into()),
            skip_next_click: false,
        },
        breakpoints: vec![BreakpointSummary {
            id: "mobile".into(),
            label: "Mobile".into(),
            width: 393,
            height: 852,
            device: None,
        }],
        sessions: Vec::new(),
        detected: Vec::new(),
        devtools: Some(DevtoolsState {
            view: ViewId(1),
            mode: rb_config::DevtoolsMode::DockedRight,
        }),
        externals: Vec::new(),
        recorder: Some(RecorderStatus {
            phase: RecorderPhase::Paused,
            name: "login".into(),
            steps: 4,
        }),
        bookmarks: Vec::new(),
        sitemap: Some(SitemapStatus {
            run_id: "2026-11-11_10-00-00".into(),
            state: SitemapRunState::Running,
            total: 76,
            done: 38,
            failed: 1,
            eta_seconds: Some(40),
            folder: "C:/work/shop/.rbrowser/out/sitemap-capture/2026-11-11_10-00-00".into(),
            gallery: None,
        }),
        metrics: Vec::new(),
        frameworks: Vec::new(),
    }));
    let json = to_json(&state);
    assert_eq!(json["type"], "state");
    assert_eq!(json["activeView"], 1);
    assert_eq!(json["project"]["trusted"], true);
    assert_eq!(json["layout"]["mode"], "grid");
    assert_eq!(json["sync"]["features"]["leaderOnlySubmit"], true);
    assert_eq!(json["sync"]["features"]["desyncTimeoutMs"], 750);
    assert_eq!(json["sync"]["excluded"], json!([2]));
    assert_eq!(json["sync"]["navigationDecoupled"], json!([3]));
    assert_eq!(json["sync"]["browsersync"], "http://localhost:3000");
    assert_eq!(json["sync"]["features"]["navigationSettleMs"], 1000);
    assert_eq!(
        json["sync"]["desynced"],
        json!([{ "view": 3, "reason": "notFound", "selector": "nav > a", "event": "click" }])
    );
    assert_eq!(json["layout"]["focus"], json!(null));
    assert_eq!(json["breakpoints"][0]["width"], 393);
    assert!(json["breakpoints"][0].get("device").is_none());
    assert_eq!(
        json["devtools"],
        json!({ "view": 1, "mode": "docked-right" })
    );
    assert_eq!(
        json["recorder"],
        json!({ "phase": "paused", "name": "login", "steps": 4 })
    );
    assert_eq!(json["sitemap"]["state"], "running");
    assert_eq!(json["sitemap"]["etaSeconds"], 40);
    assert!(json["sitemap"].get("gallery").is_none());
}

#[test]
fn the_w1b_messages_shape() {
    let message = parse_request(
        r#"{ "type": "applyBreakpoints", "proposalId": 3, "selected": ["md"], "replace": false }"#,
    )
    .expect("parses");
    assert_eq!(
        message,
        ShellToCore::ApplyBreakpoints {
            proposal_id: 3,
            selected: vec!["md".into()],
            replace: false,
        }
    );
    assert_eq!(
        parse_request(r#"{ "type": "analyzeBreakpoints" }"#).expect("defaults"),
        ShellToCore::AnalyzeBreakpoints {
            urls: Vec::new(),
            mode: None,
        }
    );
    let prompt = CoreToShell::TrustPrompt(TrustPrompt {
        prompt_id: 7,
        project_root: "C:/work/shop".into(),
        blocked: vec![BlockedItem {
            kind: ProtectedKind::Flows,
            path: ".rbrowser/flows/a.ts".into(),
        }],
    });
    assert_eq!(
        to_json(&prompt)["blocked"],
        json!([{ "kind": "flows", "path": ".rbrowser/flows/a.ts" }])
    );
    let rejected = CoreToShell::SettingRejected {
        key: "breakpoints".into(),
        issues: vec![SettingIssue {
            path: "/1/width".into(),
            message_key: "config.error.invalid_value".into(),
        }],
    };
    assert_eq!(
        to_json(&rejected),
        json!({
            "type": "settingRejected",
            "key": "breakpoints",
            "issues": [{ "path": "/1/width", "messageKey": "config.error.invalid_value" }]
        })
    );
}

#[test]
fn events_and_answers_shape() {
    assert_eq!(to_json(&CoreToShell::Ack), json!({ "type": "ack" }));
    let notification = CoreToShell::Notification(Notification {
        level: NotificationLevel::Warning,
        message_key: "config.warning.local_only_ignored".into(),
        args: [("key".to_owned(), "network.proxy".to_owned())].into(),
        settings_key: Some("network.proxy".into()),
    });
    assert_eq!(
        to_json(&notification),
        json!({
            "type": "notification",
            "level": "warning",
            "messageKey": "config.warning.local_only_ignored",
            "args": { "key": "network.proxy" },
            "settingsKey": "network.proxy"
        })
    );
    let commands = CoreToShell::Commands {
        leader: "Ctrl+Shift+Space".into(),
        commands: vec![CommandInfo {
            id: "sync.toggle".into(),
            title_key: "commands.sync.toggle".into(),
            group: rb_commands::Group::Sync,
            bindings: vec!["Ctrl+Shift+Space S S".into()],
            default_bindings: vec!["Ctrl+Shift+Space S S".into()],
            changed: false,
            mcp: rb_commands::McpCoverage::Covered("sync_set".into()),
            dropped_defaults: Vec::new(),
        }],
        texts: std::collections::BTreeMap::new(),
    };
    let json = to_json(&commands);
    assert!(json.get("texts").is_none(), "no plugin titles, no field");
    assert!(json["commands"][0].get("droppedDefaults").is_none());
    assert_eq!(json["commands"][0]["group"], "sync");
    assert_eq!(json["commands"][0]["titleKey"], "commands.sync.toggle");
    assert_eq!(json["commands"][0]["mcp"], json!({ "covered": "sync_set" }));
}

#[test]
fn the_transport_scripts() {
    assert_eq!(
        encode_response(&CoreToShell::Ack),
        Ok(r#"{"type":"ack"}"#.into())
    );
    assert_eq!(
        event_script(&CoreToShell::Ack),
        Ok(r#"window.dispatchEvent(new CustomEvent("rb-ipc",{detail:{"type":"ack"}}));"#.into())
    );
}

#[test]
fn the_findings_messages_shape() {
    assert_eq!(
        parse_request(r#"{ "type": "runChecks", "target": "all" }"#).expect("parses"),
        ShellToCore::RunChecks {
            target: rb_engine::Target::All,
            plugins: None,
            url: None,
        }
    );
    assert_eq!(
        to_json(&ShellToCore::RunChecks {
            target: rb_engine::Target::View(ViewId(2)),
            plugins: Some(vec!["seo".into()]),
            url: Some("https://site.test/".into()),
        }),
        json!({ "type": "runChecks", "target": 2, "plugins": ["seo"], "url": "https://site.test/" })
    );
    assert_eq!(
        parse_request(
            r#"{ "type": "highlightFinding", "findingId": "f1", "selector": "img.hero", "views": [1, 3] }"#
        )
        .expect("parses"),
        ShellToCore::HighlightFinding {
            finding_id: "f1".into(),
            selector: "img.hero".into(),
            views: vec![ViewId(1), ViewId(3)],
        }
    );
    let findings = CoreToShell::Findings {
        run: json!({ "version": 1, "runId": "run-0000000000001", "findings": [] }),
        messages: [(
            "plugin.seo.findings.alt".to_owned(),
            "No alt text".to_owned(),
        )]
        .into(),
        project_off: ProjectOff::default(),
        partial: false,
    };
    assert_eq!(
        to_json(&findings),
        json!({
            "type": "findings",
            "run": { "version": 1, "runId": "run-0000000000001", "findings": [] },
            "messages": { "plugin.seo.findings.alt": "No alt text" }
        })
    );
    // What the project turned off comes along (batch-2 security review, H3).
    let off: CoreToShell = serde_json::from_value(json!({
        "type": "findings",
        "run": {},
        "messages": {},
        "projectOff": {
            "rules": ["seo.title"],
            "ignore": [{ "rule": "a11y.*", "url": "http://localhost:5173/legacy/*" }]
        }
    }))
    .expect("parses");
    let CoreToShell::Findings { project_off, .. } = off else {
        panic!("findings");
    };
    assert_eq!(project_off.rules, ["seo.title"]);
    assert_eq!(project_off.ignore[0].rule.as_deref(), Some("a11y.*"));
    assert_eq!(project_off.count(), 2);
}

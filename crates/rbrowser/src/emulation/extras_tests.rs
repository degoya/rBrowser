use std::sync::Arc;

use rb_commands::lint::lint_defaults;
use rb_commands::{McpCoverage, Platform, builtin, builtin_reserved};
use rb_config::{EmulationUserSettings, Geolocation, MediaType, Paper};
use rb_engine::{ViewHost, ViewId};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn core() -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(host, ui, Handle::current(), CoreSetup::default())
}

fn emulation(core: &Core, view: u32) -> rb_engine::Emulation {
    core.host()
        .emulation(ViewId(view))
        .expect("the view exists")
}

#[test]
fn emulation_extras_have_free_leader_chords_and_their_mcp_tool() {
    let defs = definitions().expect("definitions");
    let found: Vec<(String, Vec<String>, McpCoverage)> = defs
        .iter()
        .map(|def| {
            (
                def.id.as_str().to_owned(),
                def.default_bindings
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                def.mcp.clone(),
            )
        })
        .collect();
    let covered = |tool: &str| McpCoverage::Covered(tool.to_owned());
    assert_eq!(
        found,
        vec![
            (
                GEOLOCATION.to_owned(),
                vec!["Ctrl+Shift+Space V G".to_owned()],
                covered("emulation_set")
            ),
            (
                TIMEZONE.to_owned(),
                vec!["Ctrl+Shift+Space V T".to_owned()],
                covered("emulation_set")
            ),
            (
                CPU.to_owned(),
                vec!["Ctrl+Shift+Space V H".to_owned()],
                covered("emulation_set")
            ),
            (
                OFFLINE.to_owned(),
                vec!["Ctrl+Shift+Space V L".to_owned()],
                covered("emulation_set")
            ),
            (
                ADD_PRINT.to_owned(),
                vec!["Ctrl+Shift+Space V Y".to_owned()],
                covered("breakpoint_add")
            ),
        ]
    );
    assert!(
        defs.iter()
            .all(|def| def.title_key == format!("commands.{}", def.id.as_str()))
    );
}

#[test]
fn emulation_extras_defaults_pass_the_lint_with_every_other_core_command() {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint shortcuts"));
    defs.extend(crate::sidecar::definitions().expect("the sidecar's"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[tokio::test]
async fn emulation_extras_geolocation_toggles_the_default_preset_on_the_active_view() {
    let core = core();
    core.host().set_active_view(ViewId(2)).expect("active");
    run(&core, GEOLOCATION, &Value::Null).expect("on");
    let berlin = emulation(&core, 2).geolocation.expect("a position");
    assert_eq!((berlin.latitude, berlin.longitude), (52.52, 13.405));
    assert_eq!(
        emulation(&core, 1).geolocation,
        None,
        "only the active view"
    );
    run(&core, GEOLOCATION, &Value::Null).expect("off");
    assert_eq!(emulation(&core, 2).geolocation, None);
}

#[tokio::test]
async fn emulation_extras_geolocation_takes_a_preset_a_position_or_null() {
    let core = core();
    run(
        &core,
        GEOLOCATION,
        &json!({ "target": "all", "preset": "tokyo" }),
    )
    .expect("tokyo");
    for view in 1..=3 {
        assert_eq!(
            emulation(&core, view).geolocation.map(|geo| geo.latitude),
            Some(35.6762)
        );
    }
    run(
        &core,
        GEOLOCATION,
        &json!({ "target": 1, "geolocation": { "latitude": -33.0, "longitude": 151.0 } }),
    )
    .expect("free input");
    assert_eq!(
        emulation(&core, 1).geolocation,
        Some(Geolocation {
            latitude: -33.0,
            longitude: 151.0,
            accuracy: 50.0
        })
    );
    run(
        &core,
        GEOLOCATION,
        &json!({ "target": 1, "geolocation": null }),
    )
    .expect("off");
    assert_eq!(emulation(&core, 1).geolocation, None);
    for bad in [
        json!({ "preset": "atlantis" }),
        json!({ "geolocation": { "latitude": 95, "longitude": 0 } }),
        json!({ "preset": "tokyo", "geolocation": null }),
    ] {
        assert!(
            matches!(
                run(&core, GEOLOCATION, &bad),
                Err(CommandError::InvalidArgs(_))
            ),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn emulation_extras_timezone_cpu_and_offline_toggle_and_take_values() {
    let core = core();
    run(&core, TIMEZONE, &Value::Null).expect("on");
    assert_eq!(
        emulation(&core, 1).timezone.as_deref(),
        Some("America/New_York")
    );
    run(&core, TIMEZONE, &Value::Null).expect("off");
    assert_eq!(emulation(&core, 1).timezone, None);
    run(&core, TIMEZONE, &json!({ "timezone": "Asia/Tokyo" })).expect("set");
    assert_eq!(emulation(&core, 1).timezone.as_deref(), Some("Asia/Tokyo"));
    assert!(matches!(
        run(&core, TIMEZONE, &json!({ "timezone": "Mars/Base" })),
        Err(CommandError::InvalidArgs(_))
    ));

    run(&core, CPU, &Value::Null).expect("on");
    assert_eq!(emulation(&core, 1).cpu_throttling, Some(4.0));
    run(&core, CPU, &Value::Null).expect("off");
    assert_eq!(emulation(&core, 1).cpu_throttling, None);
    run(&core, CPU, &json!({ "rate": 6 })).expect("set");
    assert_eq!(emulation(&core, 1).cpu_throttling, Some(6.0));
    assert!(matches!(
        run(&core, CPU, &json!({ "rate": 0.5 })),
        Err(CommandError::InvalidArgs(_))
    ));

    run(&core, OFFLINE, &json!({ "target": "all" })).expect("on");
    for view in 1..=3 {
        assert!(
            emulation(&core, view)
                .network
                .is_some_and(|network| network.offline)
        );
    }
    run(&core, OFFLINE, &json!({ "target": "all" })).expect("off");
    assert_eq!(emulation(&core, 1).network, None, "back without a new view");
    run(&core, OFFLINE, &json!({ "offline": true })).expect("set");
    assert!(
        emulation(&core, 1)
            .network
            .is_some_and(|network| network.offline)
    );
}

#[test]
fn emulation_extras_toggles_read_the_users_defaults() {
    let settings = EmulationUserSettings {
        default_geolocation: "sydney".into(),
        default_timezone: "Europe/Paris".into(),
        default_cpu_throttling: 6.0,
        ..EmulationUserSettings::default()
    };
    let current = rb_engine::Emulation::default();
    let fields = |id: &str| fields_for(id, &Value::Null, &current, &settings).expect("fields");
    assert_eq!(
        fields(GEOLOCATION)
            .geolocation
            .flatten()
            .map(|geo| geo.latitude),
        Some(-33.8688)
    );
    assert_eq!(fields(TIMEZONE).timezone.as_deref(), Some("Europe/Paris"));
    assert_eq!(fields(CPU).cpu_throttling, Some(6.0));
    assert_eq!(fields(OFFLINE).offline, Some(true));
}

#[test]
fn emulation_extras_a_print_breakpoint_is_print_media_on_paper() {
    let breakpoint = print_breakpoint(Paper::Letter, Some("staging"));
    assert_eq!(breakpoint.id, "print-letter");
    assert_eq!(breakpoint.session.as_deref(), Some("staging"));
    assert_eq!(breakpoint.emulation.paper, Some(Paper::Letter));
    assert_eq!(
        breakpoint
            .emulation
            .media
            .and_then(|media| media.media_type),
        Some(MediaType::Print)
    );
    assert_eq!(breakpoint.width, None, "the paper sizes it");
}

#[tokio::test]
async fn emulation_extras_add_print_opens_a_print_view_on_a4() {
    let core = core();
    let before = core.host().views().len();
    let answer = run(&core, ADD_PRINT, &Value::Null).expect("opens");
    let view = ViewId(
        answer["view"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .expect("a view id"),
    );
    assert_eq!(core.host().views().len(), before + 1);
    let print = core.host().emulation(view).expect("the new view");
    assert_eq!(print.media.media_type, Some(MediaType::Print));
    assert_eq!((print.width, print.height), (794, 1123));
    let letter = run(&core, ADD_PRINT, &json!({ "paper": "Letter" })).expect("opens");
    let letter = ViewId(u32::try_from(letter["view"].as_u64().expect("id")).expect("u32"));
    assert_eq!(core.host().emulation(letter).map(|e| e.width), Some(816));
}

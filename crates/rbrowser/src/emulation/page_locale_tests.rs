use std::sync::Arc;

use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};
use rb_config::{PseudoLocaleSettings, TextDirection};
use rb_engine::{ViewHost, ViewId};
use rb_sync::PseudoLocale;
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

fn dir(core: &Core, view: u32) -> Option<TextDirection> {
    core.host()
        .emulation(ViewId(view))
        .expect("the view exists")
        .dir
}

const PSEUDO: PseudoLocale = PseudoLocale {
    factor: 1.4,
    accents: true,
    rtl: false,
};

#[test]
fn both_commands_have_free_leader_chords_and_their_mcp_tool() {
    let defs = definitions().expect("definitions");
    let found: Vec<(String, String, McpCoverage)> = defs
        .iter()
        .map(|def| {
            (
                def.id.as_str().to_owned(),
                def.default_bindings[0].to_string(),
                def.mcp.clone(),
            )
        })
        .collect();
    let covered = |tool: &str| McpCoverage::Covered(tool.to_owned());
    assert_eq!(
        found,
        vec![
            (
                TOGGLE_DIR.to_owned(),
                "Ctrl+Shift+Space V .".to_owned(),
                covered("emulation_set")
            ),
            (
                PSEUDO_LOCALE.to_owned(),
                "Ctrl+Shift+Space O Y".to_owned(),
                covered("pseudo_locale")
            ),
        ]
    );
    let mut all = builtin::definitions();
    all.extend(crate::commands::definitions().expect("the grid's"));
    all.extend(crate::emulation::extras::definitions().expect("the extras"));
    all.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&all, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[tokio::test]
async fn emulation_set_forces_dir_and_lang_live_in_every_view() {
    let core = core();
    let answer = crate::emulation::run_command(
        &core,
        "emulation.set",
        &json!({ "target": "all", "fields": { "dir": "rtl", "lang": "ar" } }),
    )
    .expect("an emulation command")
    .expect("sets");
    assert_eq!(answer["reloadRequired"], false, "no reload");
    for view in 1..=3 {
        let emulation = core.host().emulation(ViewId(view)).expect("a view");
        assert_eq!(emulation.dir, Some(TextDirection::Rtl));
        assert_eq!(emulation.lang.as_deref(), Some("ar"));
    }
    // `null` drops the live override: the page's own again.
    crate::emulation::run_command(
        &core,
        "emulation.set",
        &json!({ "target": 2, "fields": { "dir": null, "lang": null } }),
    )
    .expect("an emulation command")
    .expect("clears");
    assert_eq!(dir(&core, 2), None);
    assert_eq!(core.host().emulation(ViewId(2)).expect("a view").lang, None);
    assert_eq!(dir(&core, 1), Some(TextDirection::Rtl));
}

#[tokio::test]
async fn toggle_dir_flips_the_reference_view_and_aligns_the_targets() {
    let core = core();
    core.host().set_active_view(ViewId(2)).expect("active");
    run(&core, TOGGLE_DIR, &Value::Null).expect("toggles");
    assert_eq!(dir(&core, 2), Some(TextDirection::Rtl));
    assert_eq!(dir(&core, 1), None, "only the active view");
    run(&core, TOGGLE_DIR, &json!({ "target": "all" })).expect("toggles");
    for view in 1..=3 {
        assert_eq!(dir(&core, view), Some(TextDirection::Ltr), "view {view}");
    }
    run(&core, TOGGLE_DIR, &json!({ "target": 3, "dir": "auto" })).expect("sets");
    assert_eq!(dir(&core, 3), Some(TextDirection::Auto));
    assert!(run(&core, TOGGLE_DIR, &json!({ "dir": "up" })).is_err());
}

#[test]
fn pseudo_for_toggles_and_takes_the_user_s_values() {
    let settings = PseudoLocaleSettings {
        factor: 2.0,
        accents: false,
        rtl: true,
    };
    let on = pseudo_for(&PseudoArgs::default(), None, &settings).expect("valid");
    assert_eq!(
        on,
        Some(PseudoLocale {
            factor: 2.0,
            accents: false,
            rtl: true
        })
    );
    assert_eq!(
        pseudo_for(&PseudoArgs::default(), Some(PSEUDO), &settings).expect("valid"),
        None,
        "a toggle of a pseudo-localised view turns it off"
    );
    let args = PseudoArgs {
        on: Some(true),
        factor: Some(1.5),
        accents: Some(true),
        ..PseudoArgs::default()
    };
    let given = pseudo_for(&args, Some(PSEUDO), &settings)
        .expect("valid")
        .expect("on");
    assert!((given.factor - 1.5).abs() < f64::EPSILON);
    assert!(given.accents && given.rtl);
    for factor in [0.5, 3.5, f64::NAN] {
        let args = PseudoArgs {
            factor: Some(factor),
            ..PseudoArgs::default()
        };
        assert!(pseudo_for(&args, None, &settings).is_err(), "{factor}");
    }
}

#[tokio::test]
async fn pseudo_locale_answers_the_views_and_keeps_the_state_per_view() {
    let core = core();
    core.host().set_active_view(ViewId(1)).expect("active");
    let answer = run(&core, PSEUDO_LOCALE, &json!({ "target": "all" })).expect("on");
    assert_eq!(
        answer,
        json!({ "on": true, "views": [1, 2, 3], "unsupported": [] })
    );
    let agents = core.host().sync_agents();
    for view in 1..=3 {
        let pseudo = agents.page_locale(ViewId(view)).pseudo.expect("on");
        assert!((pseudo.factor - 1.4).abs() < f64::EPSILON, "the default");
    }
    let off = run(&core, PSEUDO_LOCALE, &json!({ "target": 2, "on": false })).expect("off");
    assert_eq!(off["on"], false);
    assert_eq!(agents.page_locale(ViewId(2)).pseudo, None);
    assert!(agents.page_locale(ViewId(3)).pseudo.is_some());
    assert!(run(&core, PSEUDO_LOCALE, &json!({ "colour": "red" })).is_err());
}

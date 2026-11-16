use std::sync::Arc;

use rb_config::Orientation;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn core() -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(host, ui, Handle::current(), CoreSetup::default())
}

fn run(core: &Core, id: &str, args: Value) -> Result<Value, CommandError> {
    run_command(core, id, &args).expect("an emulation command")
}

fn emulation(core: &Core, view: u32) -> rb_engine::Emulation {
    core.host()
        .emulation(ViewId(view))
        .expect("the view exists")
}

#[tokio::test]
async fn set_with_target_all_changes_every_view_live() {
    let core = core();
    let answer = run(
        &core,
        "emulation.set",
        json!({ "target": "all", "fields": { "colorScheme": "dark" } }),
    )
    .expect("sets");
    assert_eq!(
        answer,
        json!({ "views": [1, 2, 3], "reloadRequired": false })
    );
    for view in 1..=3 {
        assert_eq!(emulation(&core, view).color_scheme, Some(ColorScheme::Dark));
    }
}

#[tokio::test]
async fn without_a_target_only_the_active_view_changes() {
    let core = core();
    core.host().set_active_view(ViewId(2)).expect("active");
    run(
        &core,
        "emulation.set",
        json!({ "fields": { "reducedMotion": true } }),
    )
    .expect("sets");
    assert_eq!(emulation(&core, 2).reduced_motion, Some(true));
    assert_eq!(emulation(&core, 1).reduced_motion, None);
}

#[tokio::test]
async fn a_server_visible_field_says_a_reload_is_needed() {
    let core = core();
    let answer = run(
        &core,
        "emulation.set",
        json!({ "target": 3, "fields": { "locale": "de-DE" } }),
    )
    .expect("sets");
    assert_eq!(answer["reloadRequired"], true);
    assert_eq!(emulation(&core, 3).locale.as_deref(), Some("de-DE"));
}

#[tokio::test]
async fn the_toggles_flip_and_align_their_targets() {
    let core = core();
    run(&core, "emulation.toggle_color_scheme", Value::Null).expect("toggles");
    assert_eq!(emulation(&core, 1).color_scheme, Some(ColorScheme::Dark));
    run(
        &core,
        "emulation.toggle_color_scheme",
        json!({ "target": "all" }),
    )
    .expect("toggles");
    for view in 1..=3 {
        assert_eq!(
            emulation(&core, view).color_scheme,
            Some(ColorScheme::Light),
            "the first view's dark flips every view to light"
        );
    }
    run(
        &core,
        "emulation.toggle_reduced_motion",
        json!({ "target": 2 }),
    )
    .expect("toggles");
    assert_eq!(emulation(&core, 2).reduced_motion, Some(true));
    run(
        &core,
        "emulation.toggle_reduced_motion",
        json!({ "target": 2 }),
    )
    .expect("toggles");
    assert_eq!(emulation(&core, 2).reduced_motion, Some(false));
}

#[tokio::test]
async fn reset_goes_back_to_the_breakpoint() {
    let core = core();
    run(
        &core,
        "emulation.set",
        json!({ "target": 3, "fields": { "colorScheme": "dark", "orientation": "landscape" } }),
    )
    .expect("sets");
    let turned = emulation(&core, 3);
    assert_eq!(turned.orientation, Some(Orientation::Landscape));
    assert_eq!((turned.width, turned.height), (852, 393));
    run(&core, "emulation.reset_view", json!({ "target": 3 })).expect("resets");
    let reset = emulation(&core, 3);
    assert_eq!(reset.color_scheme, None);
    assert_eq!((reset.width, reset.height), (393, 852));
    assert!(reset.touch, "the breakpoint's own touch stays");
}

#[tokio::test]
async fn wrong_arguments_and_unknown_views_are_refused() {
    let core = core();
    assert!(matches!(
        run(
            &core,
            "emulation.set",
            json!({ "target": 99, "fields": {} })
        ),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run(
            &core,
            "emulation.set",
            json!({ "fields": { "colorScheme": "blue" } })
        ),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run(&core, "emulation.set", json!({ "fields": { "dpr": 50.0 } })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run(&core, "emulation.reset_view", json!({ "view": 1 })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert_eq!(
        emulation(&core, 1).dpr,
        0.0,
        "a refused value changes nothing"
    );
}

#[tokio::test]
async fn set_from_a_shortcut_opens_the_emulation_settings() {
    let core = core();
    assert_eq!(
        run(&core, "emulation.set", Value::Null).expect("opens"),
        Value::Null
    );
}

#[tokio::test]
async fn other_commands_and_messages_are_not_this_modules() {
    let core = core();
    assert!(run_command(&core, "view.back", &Value::Null).is_none());
    assert!(ipc(&core, &ShellToCore::GetState).is_none());
}

#[tokio::test]
async fn get_emulation_answers_with_masked_headers() {
    let core = core();
    run(
        &core,
        "emulation.set",
        json!({ "target": 1, "fields": { "headers": { "Authorization": "Bearer t0ken" } } }),
    )
    .expect("sets");
    let answer = ipc(&core, &ShellToCore::GetEmulation { view: ViewId(1) })
        .expect("ours")
        .expect("answers");
    let CoreToShell::Emulation { view, emulation } = answer else {
        panic!("not an emulation answer");
    };
    assert_eq!(view, ViewId(1));
    assert_eq!(emulation.headers["Authorization"], fields::MASKED_VALUE);
    let missing = ipc(&core, &ShellToCore::GetEmulation { view: ViewId(42) }).expect("ours");
    assert_eq!(
        missing.map(|_| ()).expect_err("unknown").0,
        IpcErrorCode::NotFound
    );
}

#[tokio::test]
async fn the_hooks_resolve_the_views_again() {
    let core = core();
    on_start(&core);
    on_config_changed(&core);
    assert!(emulation(&core, 3).touch);
}

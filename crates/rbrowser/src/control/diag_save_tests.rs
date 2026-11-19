use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn writes(text: &'static str) -> WriteZip {
    Arc::new(move |path: &Path| std::fs::write(path, text).map_err(|error| error.to_string()))
}

fn core(ui: &Arc<RecordingUi>) -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(host, ui.clone(), Handle::current(), CoreSetup::default())
}

fn shell_events(ui: &RecordingUi) -> Vec<String> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) if script.contains("diagnosticsSaved") => Some(script),
            _ => None,
        })
        .collect()
}

#[test]
fn the_dialog_proposes_a_dated_zip_in_the_folder() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_657_600);
    let path = proposed(Path::new("/home/me/Desktop"), now);
    assert_eq!(
        path,
        Path::new("/home/me/Desktop").join(bundle_file_name(now))
    );
    let name = path
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    assert!(name.starts_with("rbrowser-diagnostics-2026"), "{name}");
    assert!(name.ends_with(".zip"), "{name}");
}

/// The dialog asked before replacing: the old ZIP is replaced whole, or not at all.
#[test]
fn a_zip_the_dialog_may_replace_is_replaced_whole_or_kept() {
    let dir = tempfile::tempdir().expect("temp dir");
    let target = dir.path().join("d.zip");
    std::fs::write(&target, "old").expect("write");
    write_replacing(&target, &writes("new")).expect("saved");
    assert_eq!(std::fs::read_to_string(&target).expect("read"), "new");
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("list").count(),
        1,
        "no partial file left"
    );

    let failing: WriteZip = Arc::new(|_: &Path| Err("disk full".to_owned()));
    assert_eq!(
        write_replacing(&target, &failing),
        Err("disk full".to_owned())
    );
    assert_eq!(std::fs::read_to_string(&target).expect("read"), "new");
}

/// RB-080-73: the shell's "Save diagnostics..." writes where the person chose and tells the
/// windows; cancelling writes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn saving_asks_where_writes_there_and_says_so() {
    let dir = tempfile::tempdir().expect("temp dir");
    let target = dir.path().join("chosen.zip");
    let ui = Arc::new(RecordingUi::default());
    let core = core(&ui);

    ask(&core, dir.path(), writes("zip"));
    let asked = ui.calls().into_iter().find_map(|call| match call {
        UiCall::PickPath(pick) => Some(pick),
        _ => None,
    });
    let asked = asked.expect("the dialog was opened");
    assert_eq!(asked.kind, PathKind::File);
    assert_eq!(
        asked.save_as.as_deref().and_then(Path::parent),
        Some(dir.path())
    );
    assert!(!target.exists(), "cancelled: nothing written");
    assert_eq!(shell_events(&ui).len(), 1);
    assert!(!shell_events(&ui)[0].contains("chosen.zip"));

    *lock(&ui.picked) = Some(target.clone());
    ask(&core, dir.path(), writes("zip"));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while shell_events(&ui).len() < 2 && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(std::fs::read_to_string(&target).expect("written"), "zip");
    assert!(shell_events(&ui)[1].contains("chosen.zip"));
    assert_eq!(lock(&LAST).as_deref(), Some(target.as_path()));
    // RB-090-85: the write named its start, then its end.
    let keys: Vec<String> = core
        .status_log()
        .items()
        .into_iter()
        .map(|message| message.message_key)
        .collect();
    let saving = keys.iter().position(|key| key == SAVING_KEY);
    let saved = keys.iter().position(|key| key == SAVED_KEY);
    assert!(saving.is_some() && saving < saved, "{keys:?}");
}

/// Review 6: the button always hears back - a dialog callback dropped unasked is a cancel, a save
/// that never finished a failure.
#[tokio::test]
async fn a_request_dropped_unanswered_still_answers() {
    let ui = Arc::new(RecordingUi::default());
    let core = core(&ui);
    drop(Answer::new(core.arc()));
    let mut chosen = Answer::new(core.arc());
    chosen.failed_on_drop = true;
    drop(chosen);
    Answer::new(core.arc()).send(Some("C:/d.zip".to_owned()), false);
    let events = shell_events(&ui);
    assert_eq!(events.len(), 3, "one answer each, never two");
    assert!(events[0].contains(r#""failed":false"#) && !events[0].contains("path"));
    assert!(events[1].contains(r#""failed":true"#));
    assert!(events[2].contains("C:/d.zip"));
}

//! Builders for the recorder's tests.

use rb_engine::ViewId;

use super::model::RecordedBreakpoint;
use super::record::{Recorder, RecorderOptions, RecordingView, StartInfo};
use crate::event::{
    ChangeData, ElementSelector, ElementTarget, InputData, KeyData, MouseButton, MouseData, Offset,
    ScrollData, SyncEvent, TargetData,
};
use crate::message::EventMessage;

pub const DESKTOP: ViewId = ViewId(1);
pub const MOBILE: ViewId = ViewId(2);
pub const START: u64 = 1_700_000_000_000;
pub const PASSWORD: &str = "Tr0ub4dor&3@xyz";

pub fn css(path: &str) -> ElementSelector {
    ElementSelector::Css { path: path.into() }
}

pub fn role(role: &str, name: &str, path: &str) -> ElementSelector {
    ElementSelector::Role {
        role: role.into(),
        name: name.into(),
        nth: 0,
        path: path.into(),
    }
}

pub fn testid(testid: &str, path: &str) -> ElementSelector {
    ElementSelector::Testid {
        testid: testid.into(),
        nth: 0,
        path: path.into(),
    }
}

pub fn at(element: ElementSelector) -> ElementTarget {
    ElementTarget {
        hosts: Vec::new(),
        element,
        offset: Offset { x: 0.5, y: 0.5 },
    }
}

pub fn message(event: SyncEvent) -> EventMessage {
    EventMessage {
        v: 1,
        top: true,
        frame: None,
        seq: 1,
        time: 0,
        event,
    }
}

pub fn click(element: ElementSelector) -> SyncEvent {
    SyncEvent::Click(MouseData {
        button: MouseButton::Left,
        detail: 1,
        modifiers: 0,
        target: at(element),
    })
}

pub fn hover(element: ElementSelector) -> SyncEvent {
    SyncEvent::Hover(TargetData {
        target: at(element),
    })
}

pub fn input(element: ElementSelector, value: &str, sensitive: bool) -> SyncEvent {
    SyncEvent::Input(InputData {
        input_type: "insertText".into(),
        data: value.chars().last().map(String::from),
        value: Some(value.into()),
        sensitive,
        target: at(element),
    })
}

pub fn change(element: ElementSelector, value: Option<&str>, sensitive: bool) -> SyncEvent {
    SyncEvent::Change(ChangeData {
        value: value.map(str::to_owned),
        checked: None,
        selected: None,
        sensitive,
        target: at(element),
    })
}

pub fn checked(element: ElementSelector, on: bool) -> SyncEvent {
    SyncEvent::Change(ChangeData {
        value: Some("on".into()),
        checked: Some(on),
        selected: None,
        sensitive: false,
        target: at(element),
    })
}

pub fn key(name: &str, modifiers: u8, target: Option<ElementSelector>) -> SyncEvent {
    SyncEvent::Keydown(KeyData {
        key: name.into(),
        code: String::new(),
        key_code: 0,
        location: 0,
        repeat: false,
        modifiers,
        sensitive: false,
        target: target.map(at),
    })
}

/// A key going down in a password field: the agent marks it `sensitive`.
pub fn secret_key(name: &str, target: ElementSelector) -> SyncEvent {
    SyncEvent::Keydown(KeyData {
        key: name.into(),
        code: String::new(),
        key_code: 0,
        location: 0,
        repeat: false,
        modifiers: 0,
        sensitive: true,
        target: Some(at(target)),
    })
}

pub fn scroll(y: f64) -> SyncEvent {
    SyncEvent::Scroll(ScrollData {
        container: None,
        x: 0.0,
        y,
        max_x: 0.0,
        max_y: 2000.0,
        anchor: None,
    })
}

pub fn views() -> Vec<RecordingView> {
    [
        (DESKTOP, "desktop", 1280, 800),
        (MOBILE, "mobile", 390, 844),
    ]
    .into_iter()
    .map(|(view, id, width, height)| RecordingView {
        view,
        breakpoint: RecordedBreakpoint {
            id: id.into(),
            width,
            height,
            device_scale_factor: 1.0,
        },
    })
    .collect()
}

/// A recorder recording from `START` with the desktop view leading, at `url` when given.
pub fn started(options: RecorderOptions, url: Option<&str>) -> Recorder {
    let mut recorder = Recorder::new(options);
    recorder
        .start(StartInfo {
            name: "login".into(),
            start_url: url.map(str::to_owned),
            environment: Some("local".into()),
            app_version: "0.1.0".into(),
            views: views(),
            leader: Some(DESKTOP),
            now_ms: START,
        })
        .expect("starts");
    recorder
}

/// `event` captured in `source` at `ms` after the start and replayed into `targets`.
pub fn feed(
    recorder: &mut Recorder,
    source: ViewId,
    targets: &[ViewId],
    ms: u64,
    event: SyncEvent,
) {
    recorder.on_event(source, &message(event), targets, START + ms);
}

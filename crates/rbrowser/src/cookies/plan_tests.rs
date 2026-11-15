use rb_engine::Target;

use super::*;

pub(crate) fn info(id: u32, engine: EngineKind, session: &str, url: &str) -> ViewInfo {
    ViewInfo {
        id: ViewId(id),
        label: format!("view {id}"),
        breakpoint: None,
        cell_breakpoint: None,
        engine,
        width: 390,
        height: 844,
        dpr: 1.0,
        scale: 1.0,
        url: url.to_owned(),
        title: String::new(),
        loading: false,
        session: session.to_owned(),
        cell: None,
    }
}

const SITE: &str = "http://127.0.0.1:18766/";

/// A grid: Chromium in `default` and `admin`, Firefox and WebKit in both.
fn grid() -> Vec<ViewInfo> {
    vec![
        info(1, EngineKind::Chromium, "default", SITE),
        info(2, EngineKind::Firefox, "default", SITE),
        info(
            3,
            EngineKind::Webkit,
            "default",
            "https://shop.example.com/cart",
        ),
        info(
            4,
            EngineKind::Chromium,
            "admin",
            "http://127.0.0.1:18766/admin",
        ),
        info(5, EngineKind::Firefox, "admin", "about:blank"),
        info(6, EngineKind::Webkit, "admin", SITE),
    ]
}

#[test]
fn every_external_view_gets_its_own_sessions_cookies() {
    let plan = plan(&grid(), &CopyArgs::default()).expect("a plan");
    assert!(plan.other_session.is_empty());
    assert_eq!(
        plan.groups,
        vec![
            Group {
                session: "admin".to_owned(),
                readers: vec![ViewId(4), ViewId(5), ViewId(6)],
                firefox: vec![ViewId(5)],
                webkit: vec![ViewId(6)],
                hosts: vec!["127.0.0.1".to_owned()],
            },
            Group {
                session: "default".to_owned(),
                readers: vec![ViewId(1), ViewId(2), ViewId(3)],
                firefox: vec![ViewId(2)],
                webkit: vec![ViewId(3)],
                hosts: vec!["127.0.0.1".to_owned(), "shop.example.com".to_owned()],
            },
        ]
    );
}

#[test]
fn a_source_session_fills_only_its_own_views_and_reports_the_others() {
    let args = CopyArgs {
        source: Some(Source::Session("admin".to_owned())),
        ..CopyArgs::default()
    };
    let plan = plan(&grid(), &args).expect("a plan");
    assert_eq!(plan.groups.len(), 1);
    assert_eq!(plan.groups[0].session, "admin");
    assert_eq!(plan.groups[0].firefox, vec![ViewId(5)]);
    let left: Vec<ViewId> = plan.other_session.iter().map(|view| view.view).collect();
    assert_eq!(left, vec![ViewId(2), ViewId(3)]);
    assert!(plan.other_session.iter().all(|view| view.error.is_some()));
}

#[test]
fn a_source_view_is_read_first_and_names_its_session() {
    let args = CopyArgs {
        source: Some(Source::View(ViewId(4))),
        target: Some(Targets::One(Target::View(ViewId(6)))),
        domains: None,
    };
    let plan = plan(&grid(), &args).expect("a plan");
    assert_eq!(plan.groups.len(), 1);
    assert_eq!(plan.groups[0].readers[0], ViewId(4));
    assert_eq!(plan.groups[0].webkit, vec![ViewId(6)]);
    assert!(plan.groups[0].firefox.is_empty());
}

#[test]
fn a_list_of_targets_takes_just_those_views() {
    let args = CopyArgs {
        target: Some(Targets::Many(vec![ViewId(2), ViewId(6)])),
        ..CopyArgs::default()
    };
    let plan = plan(&grid(), &args).expect("a plan");
    let firefox: Vec<ViewId> = plan.groups.iter().flat_map(|g| g.firefox.clone()).collect();
    let webkit: Vec<ViewId> = plan.groups.iter().flat_map(|g| g.webkit.clone()).collect();
    assert_eq!((firefox, webkit), (vec![ViewId(2)], vec![ViewId(6)]));
}

#[test]
fn without_an_external_view_there_is_nothing_to_do_and_bad_targets_are_named() {
    let chromium = vec![info(1, EngineKind::Chromium, "default", SITE)];
    assert_eq!(
        plan(&chromium, &CopyArgs::default()),
        Err(PlanError::NoExternalViews)
    );
    let to = |id: u32| CopyArgs {
        target: Some(Targets::One(Target::View(ViewId(id)))),
        ..CopyArgs::default()
    };
    assert_eq!(
        plan(&grid(), &to(1)),
        Err(PlanError::NotExternal(ViewId(1)))
    );
    assert_eq!(
        plan(&grid(), &to(9)),
        Err(PlanError::UnknownView(ViewId(9)))
    );
    let nobody = CopyArgs {
        source: Some(Source::Session("staging".to_owned())),
        ..CopyArgs::default()
    };
    assert_eq!(
        plan(&grid(), &nobody),
        Err(PlanError::NoReader("staging".to_owned()))
    );
}

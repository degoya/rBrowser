use super::*;

fn fresh() -> StartInput<'static> {
    StartInput {
        enabled: true,
        settings_file_exists: false,
        project: None,
    }
}

fn shown(hinted: &[&str]) -> FirstRunState {
    FirstRunState {
        wizard_shown: true,
        hinted_projects: hinted.iter().map(|key| (*key).to_owned()).collect(),
    }
}

#[test]
fn the_very_first_start_opens_the_wizard_once_and_remembers_it() {
    let (action, next) = decide(&fresh(), Loaded::Missing);
    assert_eq!(action, StartAction::OpenWizard);
    assert_eq!(next, Some(shown(&[])));
    // The second start: the state says shown.
    let (action, next) = decide(&fresh(), Loaded::Found(shown(&[])));
    assert_eq!(action, StartAction::Nothing);
    assert_eq!(next, None);
}

#[test]
fn the_wizard_covers_the_project_it_opens_on() {
    let input = StartInput {
        project: Some(ProjectAtStart {
            key: "abc",
            has_config: false,
        }),
        ..fresh()
    };
    let (action, next) = decide(&input, Loaded::Missing);
    assert_eq!(action, StartAction::OpenWizard);
    assert_eq!(next, Some(shown(&["abc"])));
}

#[test]
fn switched_off_nothing_shows_and_nothing_is_written() {
    let input = StartInput {
        enabled: false,
        project: Some(ProjectAtStart {
            key: "abc",
            has_config: false,
        }),
        ..fresh()
    };
    assert_eq!(
        decide(&input, Loaded::Missing),
        (StartAction::Nothing, None)
    );
    assert_eq!(
        decide(&input, Loaded::Found(shown(&[]))),
        (StartAction::Nothing, None)
    );
}

#[test]
fn a_person_who_used_rbrowser_before_is_no_first_start() {
    let input = StartInput {
        settings_file_exists: true,
        ..fresh()
    };
    let (action, next) = decide(&input, Loaded::Missing);
    assert_eq!(action, StartAction::Nothing);
    assert_eq!(next, Some(shown(&[])));
}

#[test]
fn a_broken_state_file_never_nags_and_is_not_written_over() {
    let input = StartInput {
        project: Some(ProjectAtStart {
            key: "abc",
            has_config: false,
        }),
        ..fresh()
    };
    assert_eq!(decide(&input, Loaded::Broken), (StartAction::Nothing, None));
}

#[test]
fn a_project_without_config_is_hinted_once() {
    let input = StartInput {
        settings_file_exists: true,
        project: Some(ProjectAtStart {
            key: "abc",
            has_config: false,
        }),
        ..fresh()
    };
    let (action, next) = decide(&input, Loaded::Found(shown(&[])));
    assert_eq!(action, StartAction::ShowHint);
    assert_eq!(next, Some(shown(&["abc"])));
    assert_eq!(
        decide(&input, Loaded::Found(shown(&["abc"]))),
        (StartAction::Nothing, None)
    );
}

#[test]
fn a_project_with_config_is_never_hinted() {
    let input = StartInput {
        settings_file_exists: true,
        project: Some(ProjectAtStart {
            key: "abc",
            has_config: true,
        }),
        ..fresh()
    };
    assert_eq!(
        decide(&input, Loaded::Found(shown(&[]))),
        (StartAction::Nothing, None)
    );
}

#[test]
fn the_hinted_list_keeps_the_newest_projects() {
    let many: Vec<String> = (0..MAX_HINTED).map(|n| format!("p{n}")).collect();
    let state = FirstRunState {
        wizard_shown: true,
        hinted_projects: many,
    };
    let input = StartInput {
        project: Some(ProjectAtStart {
            key: "new",
            has_config: false,
        }),
        ..fresh()
    };
    let (_, next) = decide(&input, Loaded::Found(state));
    let next = next.expect("written");
    assert_eq!(next.hinted_projects.len(), MAX_HINTED);
    assert_eq!(next.hinted_projects.last().map(String::as_str), Some("new"));
    assert!(!next.hinted_projects.contains(&"p0".to_owned()));
}

#[test]
fn the_file_lies_beside_the_user_settings_and_reads_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    // A first start has no settings folder yet: saving makes it.
    let settings = dir.path().join("rBrowser").join("settings.json");
    let path = path_for(&settings);
    assert_eq!(path, dir.path().join("rBrowser").join(FILE_NAME));
    assert_eq!(load(&path), Loaded::Missing);
    save(&path, &shown(&["abc"])).expect("writes");
    assert_eq!(load(&path), Loaded::Found(shown(&["abc"])));
    std::fs::write(&path, "{ not json").expect("writes");
    assert_eq!(load(&path), Loaded::Broken);
}

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;

fn prefs(entries: &[(&str, Value)]) -> BTreeMap<String, Value> {
    entries
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect()
}

#[test]
fn user_js_turns_off_first_run_updates_and_telemetry() {
    let text = user_js(&BTreeMap::new(), false, None).expect("user.js");
    for line in [
        r#"user_pref("browser.shell.checkDefaultBrowser", false);"#,
        r#"user_pref("app.update.auto", false);"#,
        r#"user_pref("datareporting.policy.dataSubmissionEnabled", false);"#,
        r#"user_pref("toolkit.telemetry.enabled", false);"#,
        r#"user_pref("browser.aboutwelcome.enabled", false);"#,
    ] {
        assert!(text.contains(line), "{line} in\n{text}");
    }
}

#[test]
fn extra_prefs_come_first_so_rbrowsers_own_win() {
    let text = user_js(
        &prefs(&[
            ("ui.prefersReducedMotion", json!(1)),
            ("app.update.auto", json!(true)),
            ("general.useragent.override", json!("Agent \"x\"")),
        ]),
        false,
        None,
    )
    .expect("user.js");
    let extra = text.find("ui.prefersReducedMotion").expect("extra pref");
    let theirs = text
        .find(r#"user_pref("app.update.auto", true);"#)
        .expect("theirs");
    let ours = text
        .rfind(r#"user_pref("app.update.auto", false);"#)
        .expect("ours");
    assert!(extra < ours && theirs < ours, "{text}");
    assert!(
        text.contains(r#"user_pref("general.useragent.override", "Agent \"x\"");"#),
        "strings are escaped: {text}"
    );
}

#[test]
fn bad_names_values_and_the_remote_agents_prefs_are_refused() {
    for (name, value) in [
        ("a\");evil(\"", json!(true)),
        ("", json!(true)),
        ("remote.allowHosts", json!("0.0.0.0")),
        ("x.float", json!(1.5)),
        ("x.big", json!(9_000_000_000_i64)),
        ("x.list", json!([1])),
    ] {
        let result = user_js(&prefs(&[(name, value.clone())]), false, None);
        assert!(
            matches!(result, Err(ProfileError::InvalidPref(_))),
            "{name} = {value}: {result:?}"
        );
    }
}

#[test]
fn a_new_or_empty_folder_becomes_an_rbrowser_profile() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("ff-profile");
    prepare(&dir, &BTreeMap::new(), false, None).expect("new folder");
    assert!(dir.join(MARKER).is_file());
    assert!(dir.join("user.js").is_file());
    // Again: it is ours now.
    prepare(&dir, &prefs(&[("a.b", json!(true))]), false, None).expect("our folder");
    let text = std::fs::read_to_string(dir.join("user.js")).expect("read");
    assert!(text.contains("a.b"), "user.js is written anew");
}

#[test]
fn a_folder_with_someone_elses_data_is_never_used() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::write(root.path().join("places.sqlite"), b"history").expect("write");
    let result = prepare(root.path(), &BTreeMap::new(), false, None);
    assert!(
        matches!(result, Err(ProfileError::NotOurs(_))),
        "{result:?}"
    );
    assert!(!root.path().join("user.js").exists(), "nothing written");
}

/// A link is never followed: a folder or `user.js` that points elsewhere (a symlink, on Windows
/// also a junction) could make rBrowser write prefs into someone else's files.
#[cfg(unix)]
#[test]
fn a_linked_profile_folder_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let elsewhere = root.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).expect("mkdir");
    let link = root.path().join("ff-profile");
    std::os::unix::fs::symlink(&elsewhere, &link).expect("symlink");
    let result = prepare(&link, &BTreeMap::new(), false, None);
    assert!(matches!(result, Err(ProfileError::Link(_))), "{result:?}");
    assert!(
        std::fs::read_dir(&elsewhere)
            .expect("read")
            .next()
            .is_none(),
        "nothing written through the link"
    );
}

#[cfg(unix)]
#[test]
fn a_linked_user_js_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("ff-profile");
    prepare(&dir, &BTreeMap::new(), false, None).expect("our folder");
    let victim = root.path().join("victim.txt");
    std::fs::write(&victim, b"keep").expect("write");
    std::fs::remove_file(dir.join("user.js")).expect("rm");
    std::os::unix::fs::symlink(&victim, dir.join("user.js")).expect("symlink");
    let result = prepare(&dir, &BTreeMap::new(), false, None);
    assert!(matches!(result, Err(ProfileError::Link(_))), "{result:?}");
    assert_eq!(std::fs::read(&victim).expect("read"), b"keep");
}

#[test]
fn the_browser_chrome_is_hidden_only_for_a_view_in_its_cell() {
    let pref = "toolkit.legacyUserProfileCustomizations.stylesheets";
    let hidden = user_js(&BTreeMap::new(), true, None).expect("user.js");
    assert!(
        hidden.contains(&format!("user_pref(\"{pref}\", true);")),
        "{hidden}"
    );
    let shown = user_js(&BTreeMap::new(), false, None).expect("user.js");
    assert!(
        shown.contains(&format!("user_pref(\"{pref}\", false);")),
        "{shown}"
    );
    // The person's own pref never wins over the setting.
    let theirs = user_js(&prefs(&[(pref, json!(true))]), false, None).expect("user.js");
    assert!(
        theirs.rfind("stylesheets\", false") > theirs.find("stylesheets\", true"),
        "{theirs}"
    );
}

#[test]
fn user_chrome_css_collapses_the_tabs_and_toolbars_in_the_profile() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("ff-profile");
    prepare(&dir, &BTreeMap::new(), true, None).expect("new folder");
    let css = std::fs::read_to_string(dir.join("chrome").join("userChrome.css")).expect("css");
    assert!(css.contains("#TabsToolbar"), "{css}");
    assert!(css.contains("#nav-bar"), "{css}");
    assert!(css.contains("#PersonalToolbar"), "{css}");
    assert!(css.contains("visibility: collapse"), "{css}");
    assert_eq!(css, USER_CHROME_CSS);
    // Written whether the chrome is hidden or not: only the pref decides, so a flip of the
    // setting needs no new file.
    prepare(&dir, &BTreeMap::new(), false, None).expect("our folder");
    assert!(dir.join("chrome").join("userChrome.css").is_file());
}

#[cfg(unix)]
#[test]
fn a_linked_user_chrome_css_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("ff-profile");
    prepare(&dir, &BTreeMap::new(), true, None).expect("our folder");
    let victim = root.path().join("victim.css");
    std::fs::write(&victim, b"keep").expect("write");
    let css = dir.join("chrome").join("userChrome.css");
    std::fs::remove_file(&css).expect("rm");
    std::os::unix::fs::symlink(&victim, &css).expect("symlink");
    let result = prepare(&dir, &BTreeMap::new(), true, None);
    assert!(matches!(result, Err(ProfileError::Link(_))), "{result:?}");
    assert_eq!(std::fs::read(&victim).expect("read"), b"keep");
}

/// The views in their cells render at one factor per profile (RB-080-40): the pref is written
/// on every start, the display's own ratio without a factor; the person's own pref loses.
#[test]
fn the_render_scale_is_the_profiles_device_pixels_per_px() {
    let scaled = user_js(&BTreeMap::new(), true, Some(0.82)).expect("user.js");
    assert!(
        scaled.contains("user_pref(\"layout.css.devPixelsPerPx\", \"0.82\");"),
        "{scaled}"
    );
    let plain = user_js(&BTreeMap::new(), false, None).expect("user.js");
    assert!(
        plain.contains("user_pref(\"layout.css.devPixelsPerPx\", \"-1.0\");"),
        "{plain}"
    );
    let theirs = user_js(
        &prefs(&[("layout.css.devPixelsPerPx", json!("2.0"))]),
        true,
        Some(0.5),
    )
    .expect("user.js");
    assert!(
        theirs.rfind("devPixelsPerPx\", \"0.50\"") > theirs.find("devPixelsPerPx\", \"2.0\""),
        "{theirs}"
    );
}

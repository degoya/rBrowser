use super::*;

#[test]
fn the_script_writes_the_toggles_setting_under_both_names() {
    let off = preference_script(false);
    assert_eq!(
        off,
        "try { for (const key of ['screencast-enabled', 'screencastEnabled']) { localStorage.setItem(key, 'false'); } } catch (error) {}"
    );
    let on = preference_script(true);
    assert!(on.contains("localStorage.setItem(key, 'true')"), "{on}");
    assert!(!on.contains("'false'"));
    for key in SETTING_KEYS {
        assert!(on.contains(&format!("'{key}'")), "{key}");
    }
}

#[test]
fn the_script_cannot_throw_into_the_frontend() {
    let script = preference_script(false);
    assert!(script.starts_with("try {"));
    assert!(script.ends_with("catch (error) {}"));
}

use std::path::Path;

use super::*;

fn probe(userns: Option<&str>, max: Option<&str>, apparmor: Option<&str>, helper: Helper) -> Probe {
    Probe {
        unprivileged_userns_clone: userns.map(str::to_owned),
        max_user_namespaces: max.map(str::to_owned),
        apparmor_restrict_unprivileged_userns: apparmor.map(str::to_owned),
        helper,
    }
}

#[test]
fn user_namespaces_without_a_restriction_are_the_sandbox() {
    // A plain kernel (Debian 13, Fedora, WSL 2): the files are missing or allow it.
    for (userns, max, apparmor) in [
        (None, None, None),
        (Some("1\n"), Some("63211\n"), Some("0\n")),
        (None, Some("15000"), None),
    ] {
        assert_eq!(
            verdict(&probe(userns, max, apparmor, Helper::Missing)),
            Verdict::Namespaces,
            "{userns:?} {max:?} {apparmor:?}"
        );
    }
}

#[test]
fn the_setuid_helper_stands_in_when_namespaces_are_off() {
    let off = [
        (Some("0"), None, None),
        (None, Some("0\n"), None),
        (Some("1"), Some("100"), Some("1\n")),
    ];
    for (userns, max, apparmor) in off {
        assert_eq!(
            verdict(&probe(userns, max, apparmor, Helper::Setuid)),
            Verdict::SetuidHelper,
            "{userns:?} {max:?} {apparmor:?}"
        );
    }
}

#[test]
fn an_apparmor_restriction_alone_may_still_be_lifted_by_a_profile() {
    // Ubuntu 23.10 and later: an AppArmor profile can grant the namespaces to one program, which
    // nothing short of trying finds out - Chromium tries and says so itself.
    for helper in [Helper::Missing, Helper::NotSetuid] {
        assert_eq!(
            verdict(&probe(Some("1"), None, Some("1"), helper)),
            Verdict::AppArmorRestricted
        );
    }
}

#[test]
fn without_namespaces_and_helper_there_is_no_sandbox() {
    for helper in [Helper::Missing, Helper::NotSetuid] {
        assert_eq!(
            verdict(&probe(Some("0"), None, None, helper)),
            Verdict::Unavailable
        );
        assert_eq!(
            verdict(&probe(None, Some("0"), Some("1"), helper)),
            Verdict::Unavailable
        );
    }
}

#[test]
fn a_value_that_does_not_read_counts_as_allowed() {
    // Only a clear 0 (or 1 for the AppArmor switch) changes the verdict; Chromium decides the rest.
    assert_eq!(
        verdict(&probe(
            Some("yes"),
            Some(""),
            Some("maybe"),
            Helper::Missing
        )),
        Verdict::Namespaces
    );
}

#[test]
fn the_helper_is_setuid_only_when_root_owns_it_with_the_setuid_bit() {
    assert_eq!(helper_state(None), Helper::Missing);
    assert_eq!(helper_state(Some((0, 0o104_755))), Helper::Setuid);
    assert_eq!(helper_state(Some((0, 0o4755))), Helper::Setuid);
    // Not root's, or root's without the bit (as an unpacked tarball leaves it).
    assert_eq!(helper_state(Some((1000, 0o104_755))), Helper::NotSetuid);
    assert_eq!(helper_state(Some((0, 0o100_755))), Helper::NotSetuid);
    // Writable by the group or others: Chromium refuses it, so do we.
    assert_eq!(helper_state(Some((0, 0o104_775))), Helper::NotSetuid);
}

#[test]
fn only_an_unavailable_sandbox_stops_the_start() {
    assert!(!Verdict::Namespaces.stops_start());
    assert!(!Verdict::SetuidHelper.stops_start());
    assert!(!Verdict::AppArmorRestricted.stops_start());
    assert!(Verdict::Unavailable.stops_start());
}

#[test]
fn the_message_names_the_helper_and_both_ways_out() {
    let text = message(Path::new("/opt/rbrowser"));
    assert!(text.contains("/opt/rbrowser/chrome-sandbox"), "{text}");
    assert!(text.contains("chmod 4755"), "{text}");
    assert!(text.contains("--no-sandbox"), "{text}");
    assert!(text.contains("engines.chromium.sandbox"), "{text}");
}

#[test]
fn a_subprocess_is_told_by_its_type_switch() {
    let args = |list: &[&str]| {
        list.iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>()
    };
    assert!(is_subprocess(args(&[
        "rbrowser",
        "--type=renderer",
        "--lang=en"
    ])));
    assert!(is_subprocess(args(&[
        "/opt/rbrowser/rbrowser",
        "--type=zygote"
    ])));
    assert!(!is_subprocess(args(&["rbrowser", "open", "."])));
    assert!(!is_subprocess(args(&["rbrowser", "--no-sandbox"])));
    // A URL or a path that merely contains the text is no switch.
    assert!(!is_subprocess(args(&[
        "rbrowser",
        "https://example.test/?x=--type=renderer"
    ])));
    // The program name itself is never read as a switch.
    assert!(!is_subprocess(args(&["--type=renderer"])));
}

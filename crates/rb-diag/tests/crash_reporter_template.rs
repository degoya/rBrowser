//! `packaging/windows/crash_reporter.cfg.in` is [`rb_diag::crash_reporter_cfg`] with the version
//! left as `@VERSION@`: `scripts/package-windows.sh` fills it in and puts the file beside
//! `rbrowser.exe` (RB-010-15), so the package needs no host build and the function stays the one
//! source of the file.

/// The placeholder the package script replaces with the workspace version.
const PLACEHOLDER: &str = "@VERSION@";

#[test]
fn the_package_template_is_the_crash_reporter_config() {
    let template = include_str!("../../../packaging/windows/crash_reporter.cfg.in");
    assert_eq!(
        template,
        rb_diag::crash_reporter_cfg(PLACEHOLDER),
        "regenerate packaging/windows/crash_reporter.cfg.in from rb_diag::crash_reporter_cfg(\"@VERSION@\")"
    );
}

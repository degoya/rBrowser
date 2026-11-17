use std::path::Path;

use super::*;

const PLUGIN: &str = "lighthouse";
const RUN: &str = "20261112-100000";

/// A project with one Lighthouse run: `<run>/mobile.html`, `<run>/mobile.json`, `<run>/run.json`.
fn project() -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("tempdir");
    let run = project.path().join(".rbrowser/out/lighthouse").join(RUN);
    std::fs::create_dir_all(&run).expect("folders");
    std::fs::write(run.join("mobile.html"), "<html>report</html>").expect("report");
    std::fs::write(run.join("mobile.json"), "{}").expect("lhr");
    std::fs::write(run.join("run.json"), "{}").expect("summary");
    project
}

fn report(root: &Path, rel: &str) -> Result<std::path::PathBuf, Refusal> {
    report_file(root, PLUGIN, rel)
}

#[test]
fn a_report_of_the_plugins_out_folder_resolves() {
    let project = project();
    let path = report(project.path(), &format!("{RUN}/mobile.html")).expect("a report");
    assert_eq!(
        path,
        project
            .path()
            .join(".rbrowser/out/lighthouse")
            .join(RUN)
            .join("mobile.html")
    );
}

#[test]
fn a_parent_step_is_refused_before_the_disk_is_read() {
    let project = project();
    for rel in [
        "../config.json",
        &format!("{RUN}/../{RUN}/mobile.html"),
        "../../../.ssh/id_rsa.html",
        &format!("{RUN}\\..\\{RUN}\\mobile.html"),
    ] {
        assert_eq!(report(project.path(), rel), Err(Refusal::Path), "{rel}");
    }
}

#[test]
fn an_absolute_path_is_refused_even_inside_the_folder() {
    let project = project();
    let inside = project
        .path()
        .join(".rbrowser/out/lighthouse")
        .join(RUN)
        .join("mobile.html");
    let outside = std::env::temp_dir().join("elsewhere.html");
    for rel in [
        inside.to_string_lossy().into_owned(),
        outside.to_string_lossy().into_owned(),
        "/etc/passwd.html".to_owned(),
        "C:\\Windows\\win.html".to_owned(),
        "\\\\server\\share\\x.html".to_owned(),
        format!("{RUN}/mobile.html:stream"),
    ] {
        assert_eq!(report(project.path(), &rel), Err(Refusal::Path), "{rel}");
    }
}

#[test]
fn only_an_html_file_opens() {
    let project = project();
    for rel in [
        format!("{RUN}/mobile.json"),
        format!("{RUN}/run.json"),
        RUN.to_owned(),
    ] {
        assert_eq!(report(project.path(), &rel), Err(Refusal::NotHtml), "{rel}");
    }
    assert_eq!(
        report(project.path(), &format!("{RUN}/desktop.html")),
        Err(Refusal::NotFound)
    );
}

#[test]
fn a_folder_named_like_a_report_does_not_open() {
    let project = project();
    let folder = project
        .path()
        .join(".rbrowser/out/lighthouse")
        .join(RUN)
        .join("folder.html");
    std::fs::create_dir(&folder).expect("folder");
    assert_eq!(
        report(project.path(), &format!("{RUN}/folder.html")),
        Err(Refusal::NotFound)
    );
}

#[test]
fn another_plugins_folder_is_out_of_reach() {
    let project = project();
    let other = project.path().join(".rbrowser/out/coverage");
    std::fs::create_dir_all(&other).expect("folders");
    std::fs::write(other.join("report.html"), "x").expect("file");
    // Through the plugin's own folder: a parent step.
    assert_eq!(
        report(project.path(), "../coverage/report.html"),
        Err(Refusal::Path)
    );
    // An id that is no plugin id never names a folder.
    assert_eq!(
        report_file(project.path(), "../coverage", "report.html"),
        Err(Refusal::Path)
    );
    assert_eq!(
        report_file(project.path(), "", "x.html"),
        Err(Refusal::Path)
    );
}

#[cfg(unix)]
#[test]
fn a_link_anywhere_on_the_way_is_refused() {
    let project = project();
    let elsewhere = tempfile::tempdir().expect("tempdir");
    std::fs::write(elsewhere.path().join("secret.html"), "secret").expect("file");
    let out = project.path().join(".rbrowser/out/lighthouse");
    // A linked file inside a run.
    std::os::unix::fs::symlink(
        elsewhere.path().join("secret.html"),
        out.join(RUN).join("linked.html"),
    )
    .expect("link");
    assert_eq!(
        report(project.path(), &format!("{RUN}/linked.html")),
        Err(Refusal::Link)
    );
    // A linked run folder.
    std::os::unix::fs::symlink(elsewhere.path(), out.join("20261112-110000")).expect("link");
    assert_eq!(
        report(project.path(), "20261112-110000/secret.html"),
        Err(Refusal::Link)
    );
    assert_eq!(
        run_folder(project.path(), PLUGIN, "20261112-110000"),
        Err(Refusal::Link)
    );
}

#[cfg(unix)]
#[test]
fn a_linked_out_folder_is_refused() {
    let project = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let run = elsewhere.path().join("lighthouse").join(RUN);
    std::fs::create_dir_all(&run).expect("folders");
    std::fs::write(run.join("mobile.html"), "x").expect("file");
    std::fs::create_dir(project.path().join(".rbrowser")).expect("folder");
    std::os::unix::fs::symlink(elsewhere.path(), project.path().join(".rbrowser/out"))
        .expect("link");
    assert_eq!(
        report(project.path(), &format!("{RUN}/mobile.html")),
        Err(Refusal::Link)
    );
}

#[test]
fn a_report_over_the_limit_is_refused() {
    let project = project();
    let big = project
        .path()
        .join(".rbrowser/out/lighthouse")
        .join(RUN)
        .join("big.html");
    let file = std::fs::File::create(&big).expect("file");
    file.set_len(MAX_REPORT_BYTES + 1).expect("length");
    assert_eq!(
        report(project.path(), &format!("{RUN}/big.html")),
        Err(Refusal::TooLarge)
    );
}

#[test]
fn only_an_open_trusted_project_admits() {
    let project = project();
    assert_eq!(admit(None, true), Err(Refusal::NoProject));
    assert_eq!(
        admit(Some(project.path().to_path_buf()), false),
        Err(Refusal::Untrusted)
    );
    assert_eq!(
        admit(Some(project.path().to_path_buf()), true),
        Ok(project.path().to_path_buf())
    );
}

#[test]
fn a_run_folder_is_one_direct_subfolder() {
    let project = project();
    let folder = run_folder(project.path(), PLUGIN, RUN).expect("the run");
    assert!(folder.ends_with(RUN));
    assert_eq!(
        run_folder(project.path(), PLUGIN, &format!("{RUN}/mobile.html")),
        Err(Refusal::Path)
    );
    assert_eq!(
        run_folder(project.path(), PLUGIN, "latest.json"),
        Err(Refusal::NotFound)
    );
    assert_eq!(run_folder(project.path(), PLUGIN, ".."), Err(Refusal::Path));
    assert_eq!(run_folder(project.path(), PLUGIN, "."), Err(Refusal::Path));
    assert_eq!(
        run_folder(project.path(), PLUGIN, "20991231-235959"),
        Err(Refusal::NotFound)
    );
}

#[test]
fn every_refusal_has_a_code_and_a_text_key() {
    for refusal in Refusal::ALL {
        assert!(refusal.code().starts_with("report_"), "{refusal:?}");
        assert!(refusal.key().starts_with("reports.error."), "{refusal:?}");
    }
}

#[test]
fn a_report_is_read_no_further_than_the_limit() {
    // Security review L2: a file that grew after the check is refused, not read whole.
    let project = project();
    let path = report(project.path(), &format!("{RUN}/mobile.html")).expect("a report");
    assert_eq!(
        read_bounded(&path, 1024).expect("read"),
        b"<html>report</html>".to_vec()
    );
    assert_eq!(read_bounded(&path, 5), Err(Refusal::TooLarge));
    assert_eq!(
        read_bounded(&path, b"<html>report</html>".len() as u64).map(|bytes| bytes.len()),
        Ok(19)
    );
    assert_eq!(
        read_bounded(&project.path().join("missing.html"), 1024),
        Err(Refusal::Io)
    );
}

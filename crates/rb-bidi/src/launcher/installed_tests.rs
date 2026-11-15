use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::*;

fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let vars: HashMap<String, OsString> = vars
        .iter()
        .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
        .collect();
    move |name| vars.get(name).cloned()
}

fn candidate(path: &str, kind: Kind) -> Candidate {
    Candidate {
        path: PathBuf::from(path),
        kind,
    }
}

#[test]
fn windows_looks_in_app_paths_the_release_folders_the_uninstall_entries_then_the_rest() {
    let found = candidates(
        true,
        Some(candidate(r"D:\Apps\Firefox\firefox.exe", Kind::Release)),
        vec![candidate(r"E:\Dev\firefox.exe", Kind::Developer)],
        env(&[
            ("ProgramFiles", r"C:\Program Files"),
            ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            ("LOCALAPPDATA", r"C:\Users\me\AppData\Local"),
        ]),
    );
    let paths: Vec<PathBuf> = found
        .iter()
        .map(|candidate| candidate.path.clone())
        .collect();
    let under = |base: &str, folder: &str| Path::new(base).join(folder).join("firefox.exe");
    assert_eq!(paths[0], PathBuf::from(r"D:\Apps\Firefox\firefox.exe"));
    // The release folders next: an empty setting starts what it started before RB-080-32.
    assert_eq!(
        paths[1..4],
        [
            under(r"C:\Program Files", "Mozilla Firefox"),
            under(r"C:\Program Files (x86)", "Mozilla Firefox"),
            under(r"C:\Users\me\AppData\Local", "Mozilla Firefox"),
        ]
    );
    assert_eq!(paths[4], PathBuf::from(r"E:\Dev\firefox.exe"));
    assert_eq!(
        paths[5..8],
        [
            under(r"C:\Program Files", "Mozilla Firefox ESR"),
            under(r"C:\Program Files", "Firefox Developer Edition"),
            under(r"C:\Program Files", "Firefox Nightly"),
        ]
    );
    assert_eq!(found.len(), 14, "{paths:?}");
    assert_eq!(
        paths[13],
        under(r"C:\Users\me\AppData\Local", "Firefox Nightly")
    );
    let kinds: Vec<Kind> = found[4..8].iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [Kind::Developer, Kind::Esr, Kind::Developer, Kind::Nightly]
    );
}

#[test]
fn a_folder_variable_that_is_empty_or_missing_adds_nothing() {
    let found = candidates(true, None, Vec::new(), env(&[("ProgramFiles", "")]));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn elsewhere_the_usual_paths_stand_in_as_release() {
    let found = candidates(false, None, Vec::new(), env(&[]));
    assert!(
        found.contains(&candidate("/usr/bin/firefox", Kind::Release)),
        "{found:?}"
    );
    assert!(found.iter().all(|c| c.kind == Kind::Release));
}

#[test]
fn the_registry_entries_name_their_edition() {
    // Forward slashes, so `Path::parent` splits the folder on Linux as on Windows.
    let (app_path, found) = from_registry(
        Some(PathBuf::from(
            "C:/Program Files/Firefox Nightly/firefox.exe",
        )),
        vec![
            (
                "Mozilla Firefox 134.0 (x64 en-US)".to_owned(),
                PathBuf::from(r"C:\Program Files\Mozilla Firefox"),
            ),
            (
                "Firefox Developer Edition 135.0b3 (x64 en-US)".to_owned(),
                PathBuf::from(r"C:\Program Files\Firefox Developer Edition"),
            ),
            (
                "Nightly 136.0a1 (x64 en-US)".to_owned(),
                PathBuf::from(r"C:\Users\me\AppData\Local\Firefox Nightly"),
            ),
            (
                "Mozilla Firefox ESR 128.5.0esr (x64 de)".to_owned(),
                PathBuf::from(r"C:\Program Files\Mozilla Firefox ESR"),
            ),
            (
                "Mozilla Maintenance Service".to_owned(),
                PathBuf::from(r"C:\Program Files (x86)\Mozilla Maintenance Service"),
            ),
            ("Mozilla Firefox 1.0".to_owned(), PathBuf::new()),
        ],
    );
    let in_folder = |folder: &str, kind: Kind| Candidate {
        path: Path::new(folder).join("firefox.exe"),
        kind,
    };
    assert_eq!(
        app_path,
        Some(candidate(
            "C:/Program Files/Firefox Nightly/firefox.exe",
            Kind::Nightly
        ))
    );
    assert_eq!(
        found,
        vec![
            in_folder(r"C:\Program Files\Mozilla Firefox", Kind::Release),
            in_folder(
                r"C:\Program Files\Firefox Developer Edition",
                Kind::Developer
            ),
            in_folder(r"C:\Users\me\AppData\Local\Firefox Nightly", Kind::Nightly),
            in_folder(r"C:\Program Files\Mozilla Firefox ESR", Kind::Esr),
        ],
        "the maintenance service and an empty location are no Firefox"
    );
}

#[test]
fn an_app_path_in_an_unknown_folder_is_a_release() {
    let (app_path, found) = from_registry(Some(PathBuf::from(r"D:\ff\firefox.exe")), Vec::new());
    assert_eq!(
        app_path,
        Some(candidate(r"D:\ff\firefox.exe", Kind::Release))
    );
    assert!(found.is_empty());
}

#[test]
fn the_version_comes_from_the_app_section_of_application_ini() {
    let ini = "\u{feff}[General]\r\nVersion=9\r\n\r\n[App]\r\nVendor=Mozilla\r\nName=Firefox\r\nVersion=134.0.2\r\nBuildID=20241201\r\n";
    assert_eq!(version_in(ini), Some("134.0.2".to_owned()));
    assert_eq!(version_in("[App]\nName=Firefox\n"), None);
    assert_eq!(version_in("Version=1.0\n"), None, "outside every section");
    assert_eq!(
        version_in("[App]\n  Version = 128.5.0esr  \n"),
        Some("128.5.0esr".to_owned())
    );
}

#[test]
fn the_version_refines_the_edition() {
    assert_eq!(kind_of(Kind::Release, Some("128.5.0esr")), Kind::Esr);
    assert_eq!(kind_of(Kind::Release, Some("136.0a1")), Kind::Nightly);
    assert_eq!(kind_of(Kind::Release, Some("135.0b3")), Kind::Release);
    assert_eq!(kind_of(Kind::Developer, Some("135.0b3")), Kind::Developer);
    assert_eq!(kind_of(Kind::Developer, None), Kind::Developer);
    assert_eq!(kind_of(Kind::Release, Some("134.0")), Kind::Release);
}

#[test]
fn installed_keeps_every_existing_file_once_in_search_order_with_its_version() {
    // Forward slashes, so `Path::parent` finds the folder on Linux as on Windows.
    let release = "C:/Program Files/Mozilla Firefox/firefox.exe";
    let developer = "C:/Program Files/Firefox Developer Edition/firefox.exe";
    let candidates = vec![
        candidate(
            "C:/PROGRAM FILES/Mozilla Firefox/firefox.exe",
            Kind::Release,
        ),
        candidate(release, Kind::Release),
        candidate(
            "C:/Program Files/Mozilla Firefox ESR/firefox.exe",
            Kind::Esr,
        ),
        candidate(developer, Kind::Developer),
    ];
    let exists = |path: &Path| {
        path.to_string_lossy().eq_ignore_ascii_case(release) || path == Path::new(developer)
    };
    let read = |path: &Path| {
        path.to_string_lossy()
            .eq_ignore_ascii_case("C:/Program Files/Mozilla Firefox/application.ini")
            .then(|| "[App]\nVersion=134.0\n".to_owned())
    };
    let found = installed(candidates, true, exists, read);
    assert_eq!(
        found,
        vec![
            Installed {
                path: PathBuf::from("C:/PROGRAM FILES/Mozilla Firefox/firefox.exe"),
                kind: Kind::Release,
                version: Some("134.0".to_owned()),
            },
            Installed {
                path: PathBuf::from(developer),
                kind: Kind::Developer,
                version: None,
            },
        ],
        "the same file twice (differing in case) is one installation; the first spelling stays"
    );
}

#[test]
fn elsewhere_paths_differing_in_case_are_different_files() {
    let candidates = vec![
        candidate("/opt/Firefox/firefox", Kind::Release),
        candidate("/opt/firefox/firefox", Kind::Release),
    ];
    let found = installed(candidates, false, |_| true, |_| None);
    assert_eq!(found.len(), 2);
}

#[test]
fn the_kinds_have_stable_ids() {
    assert_eq!(Kind::Release.id(), "release");
    assert_eq!(Kind::Esr.id(), "esr");
    assert_eq!(Kind::Developer.id(), "developer");
    assert_eq!(Kind::Nightly.id(), "nightly");
    assert_eq!(
        serde_json::to_value(Kind::Developer).expect("json"),
        serde_json::json!("developer")
    );
}

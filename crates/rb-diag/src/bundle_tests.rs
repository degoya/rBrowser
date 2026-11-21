use std::collections::BTreeMap;
use std::io::Read as _;
use std::time::{Duration, UNIX_EPOCH};

use serde_json::json;

use super::*;

/// Every entry of the ZIP at `path`: name -> bytes.
fn entries(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(path).expect("zip")).expect("archive");
    let mut out = BTreeMap::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).expect("entry");
        let name = file.name().expect("name").into_owned();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("read");
        out.insert(name, bytes);
    }
    out
}

fn input(root: &Path, config: Option<Value>) -> BundleInput {
    BundleInput {
        paths: DiagPaths::under(root),
        versions: Versions {
            app: "0.1.0".to_owned(),
            cef: Some("156.4.0".to_owned()),
            ..Versions::default()
        },
        config_effective: config,
        plugins: json!([{ "id": "seo", "version": "1.0.0" }]),
        perf: json!({ "enabled": true, "counters": [{ "name": "ipc.query", "p95Ms": 1.5 }] }),
        max_crash_dumps: 2,
    }
}

#[test]
fn the_zip_holds_logs_versions_config_and_plugins() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = DiagPaths::under(dir.path());
    std::fs::create_dir_all(&paths.logs).expect("logs");
    std::fs::write(paths.log_file(), "line one\n").expect("log");
    std::fs::write(paths.cef_log_file(), "cef line\n").expect("cef log");
    let out = dir.path().join("out").join("d.zip");
    let report = write_bundle(
        &input(dir.path(), Some(json!({ "startUrl": "https://a.test" }))),
        &out,
    )
    .expect("bundle");
    assert_eq!(report.path, out);
    let zip = entries(&out);
    let names: Vec<&str> = zip.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "config.effective.json",
            "logs/",
            "logs/cef.log",
            "logs/rbrowser.log",
            "perf.json",
            "plugins.json",
            "versions.json"
        ]
    );
    assert_eq!(zip["logs/rbrowser.log"], b"line one\n");
    let versions: Value = serde_json::from_slice(&zip["versions.json"]).expect("json");
    assert_eq!(versions["app"], json!("0.1.0"));
    assert_eq!(versions["cef"], json!("156.4.0"));
    assert_eq!(versions["os"]["family"], json!(std::env::consts::OS));
    let plugins: Value = serde_json::from_slice(&zip["plugins.json"]).expect("json");
    assert_eq!(plugins[0]["id"], json!("seo"));
    // The timing counters (RB-090-32).
    let perf: Value = serde_json::from_slice(&zip["perf.json"]).expect("json");
    assert_eq!(perf["counters"][0]["name"], json!("ipc.query"));
}

#[test]
fn a_secret_of_config_local_json_is_nowhere_in_the_zip() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = DiagPaths::under(dir.path());
    std::fs::create_dir_all(&paths.logs).expect("logs");
    // An old line written before the value was known as a secret.
    std::fs::write(
        paths.log_file(),
        "sent X-Team hdr-hidden-0815 to the view\nproxy auth pw-hidden-1234 ok\n",
    )
    .expect("log");
    // The effective config after merging config.local.json (headers and proxy are local).
    let config = json!({
        "startUrl": "https://shop.test",
        "breakpoints": [{ "id": "phone", "headers": { "Authorization": "Bearer abc-hidden-4711", "X-Team": "hdr-hidden-0815" } }],
        "network": { "proxy": "http://bob:pw-hidden-1234@proxy.local:3128" }
    });
    let out = dir.path().join("d.zip");
    write_bundle(&input(dir.path(), Some(config)), &out).expect("bundle");
    let raw = std::fs::read(&out).expect("zip bytes");
    let zip = entries(&out);
    for secret in ["abc-hidden-4711", "hdr-hidden-0815", "pw-hidden-1234"] {
        for (name, bytes) in &zip {
            assert!(
                !String::from_utf8_lossy(bytes).contains(secret),
                "{secret} found in {name}"
            );
        }
        assert!(
            !String::from_utf8_lossy(&raw).contains(secret),
            "{secret} in the raw zip"
        );
    }
    let config: Value = serde_json::from_slice(&zip["config.effective.json"]).expect("json");
    assert_eq!(config["startUrl"], json!("https://shop.test"));
    assert_eq!(
        config["network"]["proxy"],
        json!("http://***@proxy.local:3128")
    );
}

#[test]
fn only_the_newest_crash_dumps_are_taken() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = DiagPaths::under(dir.path());
    let reports = paths.crash_dumps.join("reports");
    std::fs::create_dir_all(&reports).expect("reports");
    for (index, name) in ["old.dmp", "mid.dmp", "new.dmp"].iter().enumerate() {
        let path = reports.join(name);
        std::fs::write(&path, name.as_bytes()).expect("dump");
        let time = UNIX_EPOCH + Duration::from_secs(1_000_000 + index as u64 * 60);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(time))
            .expect("mtime");
    }
    std::fs::write(reports.join("settings.dat"), "x").expect("other file");
    let out = dir.path().join("d.zip");
    let report = write_bundle(&input(dir.path(), None), &out).expect("bundle");
    let zip = entries(&out);
    assert!(zip.contains_key("crashdumps/new.dmp"));
    assert!(zip.contains_key("crashdumps/mid.dmp"));
    assert!(!zip.contains_key("crashdumps/old.dmp"));
    assert!(!zip.keys().any(|name| name.ends_with("settings.dat")));
    assert_eq!(zip["config.effective.json"], b"null");
    assert!(report.entries.contains(&"crashdumps/new.dmp".to_owned()));
}

#[test]
fn no_crash_dumps_when_the_setting_is_zero() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = DiagPaths::under(dir.path());
    std::fs::create_dir_all(paths.crash_dumps.join("reports")).expect("reports");
    std::fs::write(paths.crash_dumps.join("reports/a.dmp"), "a").expect("dump");
    let mut input = input(dir.path(), None);
    input.max_crash_dumps = 0;
    let out = dir.path().join("d.zip");
    write_bundle(&input, &out).expect("bundle");
    assert!(
        !entries(&out)
            .keys()
            .any(|name| name.starts_with("crashdumps/"))
    );
}

#[test]
fn the_default_name_carries_the_time() {
    let time = UNIX_EPOCH + Duration::from_secs(20_767 * 86_400 + 13 * 3_600 + 5 * 60 + 9);
    assert_eq!(
        bundle_file_name(time),
        "rbrowser-diagnostics-20261110-130509.zip"
    );
    let paths = DiagPaths::under(Path::new("/r"));
    assert_eq!(
        default_bundle_path(&paths, time),
        Path::new("/r/diagnostics/rbrowser-diagnostics-20261110-130509.zip")
    );
}

#[test]
fn an_unwritable_target_is_a_bundle_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocker = dir.path().join("file");
    std::fs::write(&blocker, "x").expect("file");
    let error = write_bundle(&input(dir.path(), None), &blocker.join("d.zip")).expect_err("error");
    assert_eq!(error.i18n_key(), "diagnostics.error.bundleNotWritable");
}

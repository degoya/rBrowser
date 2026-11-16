use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

use super::*;
use crate::supervisor::{Launched, Launcher, PING, Sidecar};

fn absolute(name: &str) -> String {
    let root = if cfg!(windows) { r"D:\" } else { "/srv" };
    PathBuf::from(root).join(name).display().to_string()
}

#[test]
fn the_default_folder_is_under_local_app_data() {
    let lad = absolute("lad");
    let path = browsers_path(None, Some(OsString::from(&lad))).expect("path");
    assert_eq!(
        path,
        PathBuf::from(lad).join("rBrowser").join("ms-playwright")
    );
}

#[test]
fn the_setting_wins_and_must_be_absolute() {
    let own = absolute("pw");
    assert_eq!(
        browsers_path(Some(&own), None).expect("path"),
        PathBuf::from(&own)
    );
    assert!(matches!(
        browsers_path(Some("pw"), Some(OsString::from(absolute("lad")))),
        Err(SidecarError::BrowsersPath(_))
    ));
}

#[test]
fn without_setting_and_local_app_data_there_is_no_folder() {
    let error = browsers_path(Some(" "), None).expect_err("none");
    assert_eq!(error.settings_key(), Some("engines.playwrightBrowsersPath"));
}

/// A sidecar whose `playwright.install` reports two steps, or the build as present.
struct InstallFake {
    present: bool,
}

impl Launcher for InstallFake {
    async fn launch(&self) -> Result<Launched, SidecarError> {
        let (client_out, fake_in) = tokio::io::duplex(4096);
        let (fake_out, client_in) = tokio::io::duplex(4096);
        tokio::spawn(serve(self.present, fake_in, fake_out));
        Ok(Launched {
            reader: Box::new(client_in),
            writer: Box::new(client_out),
            child: None,
            job: None,
        })
    }
}

async fn send(out: &mut DuplexStream, message: Value) {
    let mut line = message.to_string();
    line.push('\n');
    out.write_all(line.as_bytes()).await.expect("writes");
}

async fn serve(present: bool, input: DuplexStream, mut out: DuplexStream) {
    let mut lines = BufReader::new(input).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let request: Value = serde_json::from_str(&line).expect("JSON");
        let id = request["id"].clone();
        if request["method"] == PING {
            send(
                &mut out,
                json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
            )
            .await;
            continue;
        }
        assert_eq!(request["method"], INSTALL);
        assert_eq!(request["params"], json!({ "browser": "webkit" }));
        if !present {
            for (stage, percent) in [("download", 0), ("download", 50), ("done", 100)] {
                let params =
                    json!({ "id": id, "browser": "webkit", "stage": stage, "percent": percent });
                send(
                    &mut out,
                    json!({ "jsonrpc": "2.0", "method": "$/progress", "params": params }),
                )
                .await;
            }
        }
        let result = json!({
            "browser": "webkit", "revision": "2388", "version": "27.2",
            "path": "/pw/webkit-2388", "alreadyInstalled": present
        });
        send(
            &mut out,
            json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        )
        .await;
    }
}

#[tokio::test]
async fn an_install_reports_its_progress_and_the_result() {
    let sidecar = Sidecar::new(InstallFake { present: false }, Duration::from_secs(5));
    let mut seen = Vec::new();
    let report = install(&sidecar, "webkit", |p| seen.push((p.stage, p.percent)))
        .await
        .expect("installed");
    assert_eq!(
        seen,
        vec![
            ("download".to_owned(), 0),
            ("download".to_owned(), 50),
            ("done".to_owned(), 100)
        ]
    );
    assert!(!report.already_installed);
    assert_eq!(report.revision, "2388");
    assert_eq!(report.i18n_key(), DONE_KEY);
}

#[tokio::test]
async fn a_present_build_is_recognised() {
    let sidecar = Sidecar::new(InstallFake { present: true }, Duration::from_secs(5));
    let mut seen = 0;
    let report = install(&sidecar, "webkit", |_| seen += 1)
        .await
        .expect("present");
    assert!(report.already_installed);
    assert_eq!(report.i18n_key(), ALREADY_KEY);
    assert_eq!(seen, 0);
}

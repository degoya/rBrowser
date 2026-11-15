use super::*;
use crate::extensions::webstore::{DownloadError, MemoryHops, host_allowed};

const ID: &str = "nhdogjmejiglipccpnnnanhbledajbpd";
const OTHER: &str = "fmkadmapgofadopljbjfkapdkoienihi";

fn answer(apps: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gupdate xmlns="http://www.google.com/update2/response" protocol="2.0" server="prod">
  <daystart elapsed_days="20770" elapsed_seconds="43200"/>
{apps}</gupdate>
"#
    )
}

fn app(id: &str, version: &str) -> String {
    format!(
        r#"  <app appid="{id}" cohort="1:xyz:" cohortname="Stable" status="ok">
    <updatecheck codebase="https://clients2.googleusercontent.com/crx/blobs/x/extension_{v}.crx" fp="1.abc" hash_sha256="00" prodversionmin="102.0" protected="0" size="1234" status="ok"
      version="{version}"/>
  </app>
"#,
        v = version.replace('.', "_")
    )
}

#[test]
fn the_update_check_url_asks_for_the_id_without_a_download() {
    let url = update_check_url(ID, "156.0.7000.1");
    assert!(url.starts_with("https://clients2.google.com/service/update2/crx?"));
    assert!(url.contains("response=updatecheck"));
    assert!(!url.contains("response=redirect"));
    assert!(url.contains("prodversion=156.0.7000.1"));
    assert!(url.contains(&format!("x=id%3D{ID}%26uc")));
    assert!(host_allowed(&url));
}

#[test]
fn the_version_is_read_from_the_apps_updatecheck_element() {
    let xml = answer(&format!("{}{}", app(OTHER, "6.1.1"), app(ID, "7.6.4")));
    assert_eq!(latest_version(&xml, ID).as_deref(), Some("7.6.4"));
    assert_eq!(latest_version(&xml, OTHER).as_deref(), Some("6.1.1"));
    assert_eq!(latest_version(&answer(&app(OTHER, "6.1.1")), ID), None);
}

#[test]
fn an_answer_without_a_version_is_none() {
    let no_update = answer(&format!(
        r#"  <app appid="{ID}" status="ok"><updatecheck status="noupdate"/></app>
"#
    ));
    assert_eq!(latest_version(&no_update, ID), None);
    let unknown = answer(&format!(
        r#"  <app appid="{ID}" status="error-invalidAppId"/>
"#
    ));
    assert_eq!(latest_version(&unknown, ID), None);
    let prodversion_only = answer(&format!(
        r#"  <app appid="{ID}" status="ok"><updatecheck prodversionmin="1.0" status="ok"/></app>
"#
    ));
    assert_eq!(latest_version(&prodversion_only, ID), None);
    let not_a_version = answer(&format!(
        r#"  <app appid="{ID}" status="ok"><updatecheck status="ok" version="x;y"/></app>
"#
    ));
    assert_eq!(latest_version(&not_a_version, ID), None);
    assert_eq!(latest_version("not xml", ID), None);
    assert_eq!(latest_version("", ID), None);
}

#[test]
fn check_version_fetches_the_answer_within_the_limit() {
    let hops = MemoryHops::default();
    let url = update_check_url(ID, "156.0.0.0");
    hops.body(&url, answer(&app(ID, "7.6.4")).as_bytes(), None);
    assert_eq!(
        check_version(&hops, ID, "156.0.0.0")
            .expect("checks")
            .as_deref(),
        Some("7.6.4")
    );
    assert_eq!(hops.asked(), std::slice::from_ref(&url));
    let hops = MemoryHops::default();
    hops.body(&url, b"<gupdate/>", Some(MAX_UPDATE_CHECK_BYTES + 1));
    assert_eq!(
        check_version(&hops, ID, "156.0.0.0").expect_err("refused"),
        DownloadError::TooLarge
    );
    let hops = MemoryHops::default();
    assert!(matches!(
        check_version(&hops, "nope", "156.0.0.0").expect_err("refused"),
        DownloadError::BadId
    ));
    assert!(matches!(
        check_version(&hops, ID, "156.0.0.0").expect_err("refused"),
        DownloadError::Network(_)
    ));
}

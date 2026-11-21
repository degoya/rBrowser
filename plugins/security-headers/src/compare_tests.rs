use rb_plugin_api::{FailureKind, codes};

use super::*;
use crate::cookies::parse_set_cookie;

fn audited(headers: &[(&str, &str)], cookies: &[&str]) -> Audited {
    let pairs: Vec<(String, String)> = headers
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    Audited {
        final_url: "https://example.test/".to_owned(),
        status: 200,
        headers: Headers::from_pairs(&pairs),
        cookies: cookies
            .iter()
            .filter_map(|line| parse_set_cookie(line))
            .collect(),
        findings: Vec::new(),
    }
}

fn column(id: &str, outcome: Result<Audited, Failure>) -> Column {
    Column {
        id: id.to_owned(),
        url: format!("https://{id}.example.test/"),
        outcome,
    }
}

fn differing(table: &Value) -> Vec<String> {
    table["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["differs"] == json!(true))
        .map(|row| match row["cookie"].as_str() {
            Some(cookie) => format!("cookie {cookie}"),
            None => row["header"].as_str().unwrap_or_default().to_owned(),
        })
        .collect()
}

const SAME: [(&str, &str); 3] = [
    ("strict-transport-security", "max-age=31536000"),
    ("x-frame-options", "DENY"),
    ("referrer-policy", "no-referrer"),
];

#[test]
fn two_environments_with_another_csp_differ_in_exactly_that_row() {
    let mut staging = SAME.to_vec();
    staging.push(("content-security-policy", "script-src 'self'"));
    let mut prod = SAME.to_vec();
    prod.push((
        "content-security-policy",
        "script-src 'self' 'unsafe-inline'",
    ));
    let table = table(
        "/",
        &[
            column("staging", Ok(audited(&staging, &[]))),
            column("prod", Ok(audited(&prod, &[]))),
        ],
    );
    assert_eq!(differing(&table), ["content-security-policy"]);
    assert_eq!(table["differences"], 1);
    assert_eq!(
        table["rows"][0]["values"]["prod"],
        "script-src 'self' 'unsafe-inline'"
    );
    assert_eq!(table["rows"].as_array().map(Vec::len), Some(COMPARED.len()));
}

#[test]
fn a_missing_header_is_null_and_differs_from_a_present_one() {
    let mut prod = SAME.to_vec();
    prod.retain(|(name, _)| *name != "strict-transport-security");
    let table = table(
        "/",
        &[
            column("prod", Ok(audited(&SAME, &[]))),
            column("staging", Ok(audited(&prod, &[]))),
        ],
    );
    assert_eq!(differing(&table), ["strict-transport-security"]);
    assert_eq!(table["rows"][1]["values"]["staging"], Value::Null);
}

#[test]
fn nonces_and_whitespace_do_not_make_a_difference() {
    let one = [(
        "content-security-policy",
        "script-src  'nonce-AAA'   'self';",
    )];
    let two = [("content-security-policy", "script-src 'nonce-BBB' 'self';")];
    let table = table(
        "/",
        &[
            column("a", Ok(audited(&one, &[]))),
            column("b", Ok(audited(&two, &[]))),
        ],
    );
    assert!(differing(&table).is_empty());
    assert_eq!(
        table["rows"][0]["values"]["a"],
        "script-src 'nonce-*' 'self';"
    );
    assert_eq!(normalize("'NONCE-x';"), "'nonce-*';");
    assert_eq!(normalize("'nonce-unterminated"), "'nonce-*'");
}

#[test]
fn cookies_are_compared_by_their_attributes_never_their_values() {
    let table = table(
        "/",
        &[
            column(
                "staging",
                Ok(audited(&SAME, &["sid=STAGING-SECRET; HttpOnly"])),
            ),
            column(
                "prod",
                Ok(audited(
                    &SAME,
                    &["sid=PROD-SECRET; HttpOnly; Secure; SameSite=Lax"],
                )),
            ),
        ],
    );
    assert_eq!(differing(&table), ["cookie sid"]);
    let text = table.to_string();
    assert!(!text.contains("SECRET"));
    let row = table["rows"]
        .as_array()
        .and_then(|rows| rows.last())
        .expect("cookie row");
    assert_eq!(row["values"]["prod"], "Secure; HttpOnly; SameSite=Lax");
}

#[test]
fn a_failed_environment_shows_its_failure_and_takes_no_part() {
    let failure =
        Failure::coded(FailureKind::Io, codes::HTTP_FAILED).with_param("reason", "refused");
    let table = table(
        "/shop",
        &[
            column("local", Err(failure)),
            column("staging", Ok(audited(&SAME, &[]))),
        ],
    );
    assert!(differing(&table).is_empty());
    assert_eq!(table["path"], "/shop");
    assert_eq!(
        table["environments"][0]["error"]["code"],
        codes::HTTP_FAILED
    );
    assert_eq!(table["environments"][1]["status"], 200);
}

#[test]
fn a_long_value_is_cut() {
    let long = format!("script-src {}", "a".repeat(2000));
    let headers = [("content-security-policy", long.as_str())];
    let table = table("/", &[column("a", Ok(audited(&headers, &[])))]);
    assert_eq!(
        table["rows"][0]["values"]["a"]
            .as_str()
            .map(|cell| cell.chars().count()),
        Some(MAX_CELL_CHARS)
    );
}

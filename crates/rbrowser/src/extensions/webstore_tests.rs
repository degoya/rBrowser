use super::*;

const ID: &str = "nhdogjmejiglipccpnnnanhbledajbpd";

#[test]
fn an_id_or_a_store_url_names_the_extension() {
    assert_eq!(parse_ref(ID).as_deref(), Some(ID));
    assert_eq!(parse_ref(&format!("  {ID}\n")).as_deref(), Some(ID));
    assert_eq!(
        parse_ref(&format!(
            "https://chromewebstore.google.com/detail/vuejs-devtools/{ID}"
        ))
        .as_deref(),
        Some(ID)
    );
    assert_eq!(
        parse_ref(&format!(
            "https://chromewebstore.google.com/detail/vuejs-devtools/{ID}?hl=de&utm_source=x"
        ))
        .as_deref(),
        Some(ID)
    );
    assert_eq!(
        parse_ref(&format!("https://chrome.google.com/webstore/detail/{ID}")).as_deref(),
        Some(ID)
    );
    assert_eq!(
        parse_ref(&format!("https://chromewebstore.google.com/detail/{ID}/")).as_deref(),
        Some(ID)
    );
}

#[test]
fn anything_else_is_no_reference() {
    assert_eq!(parse_ref(""), None);
    assert_eq!(parse_ref("abc"), None);
    assert_eq!(parse_ref(&ID.replace('n', "q")), None);
    assert_eq!(parse_ref(&ID.to_uppercase()), None);
    assert_eq!(parse_ref(&format!("https://evil.test/detail/x/{ID}")), None);
    assert_eq!(
        parse_ref(&format!("http://chromewebstore.google.com/detail/x/{ID}")),
        None
    );
    assert_eq!(
        parse_ref(&format!("https://chromewebstore.google.com/other/x/{ID}")),
        None
    );
}

#[test]
fn the_update_url_asks_for_the_id_as_crx3() {
    let url = update_url(ID, "156.0.7000.1");
    assert!(url.starts_with("https://clients2.google.com/service/update2/crx?"));
    assert!(url.contains("response=redirect"));
    assert!(url.contains("prodversion=156.0.7000.1"));
    assert!(url.contains("acceptformat=crx3"));
    assert!(url.contains(&format!("x=id%3D{ID}%26uc")));
    assert!(host_allowed(&url));
}

#[test]
fn the_chromium_version_comes_from_the_cef_version() {
    assert_eq!(
        chromium_version(Some("156.4.0+g0f8d7a1+chromium-156.0.7000.1")),
        "156.0.7000.1"
    );
    assert_eq!(chromium_version(Some("garbage")), FALLBACK_CHROMIUM);
    assert_eq!(chromium_version(None), FALLBACK_CHROMIUM);
}

#[test]
fn only_https_to_googles_hosts_is_allowed() {
    assert!(host_allowed(
        "https://clients2.google.com/service/update2/crx?x=1"
    ));
    assert!(host_allowed(
        "https://clients2.googleusercontent.com/crx/blobs/abc/extension_1_0_0.crx"
    ));
    assert!(host_allowed("https://CLIENTS2.google.com/x"));
    assert!(host_allowed("HTTPS://Clients2.GoogleUserContent.com/x"));
    assert!(host_allowed("https://clients2.google.com"), "a bare host");
    assert!(host_allowed("https://clients2.google.com?x=1"));
    assert!(host_allowed("https://clients2.google.com#x"));
    assert!(!host_allowed("http://clients2.google.com/x"));
    assert!(!host_allowed("https://clients2.google.com.evil.test/x"));
    assert!(!host_allowed("https://evil.test/clients2.google.com/x"));
    assert!(!host_allowed("https://user@clients2.google.com/x"));
    assert!(!host_allowed("https://clients2.google.com:8443/x"));
    assert!(!host_allowed("clients2.google.com/x"));
    assert!(!host_allowed("/crx/blobs/x"));
}

/// RFC 7231: a `Location` may be a relative reference, resolved against the request's URL.
#[test]
fn a_relative_location_is_resolved_against_the_previous_url() {
    let start = update_url(ID, "156.0.0.0");
    let hops = MemoryHops::default();
    let at = "https://clients2.google.com/crx/blobs/x.crx";
    hops.redirect(&start, "/crx/blobs/x.crx")
        .body(at, b"Cr24", Some(4));
    assert_eq!(
        download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect("downloads"),
        b"Cr24"
    );
    assert_eq!(hops.asked(), [start.clone(), at.to_owned()]);
    let hops = MemoryHops::default();
    hops.redirect(&start, "//clients2.googleusercontent.com/crx/blobs/x.crx")
        .body(
            "https://clients2.googleusercontent.com/crx/blobs/x.crx",
            b"Cr24",
            Some(4),
        );
    assert_eq!(
        download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect("downloads"),
        b"Cr24"
    );
    let hops = MemoryHops::default();
    hops.redirect(
        &start,
        "https://clients2.googleusercontent.com/crx/blobs/a/one.crx",
    )
    .redirect(
        "https://clients2.googleusercontent.com/crx/blobs/a/one.crx",
        "two.crx",
    )
    .body(
        "https://clients2.googleusercontent.com/crx/blobs/a/two.crx",
        b"Cr24",
        Some(4),
    );
    assert_eq!(
        download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect("downloads"),
        b"Cr24"
    );
    assert_eq!(
        resolve("https://clients2.google.com", "x.crx"),
        "https://clients2.google.com/x.crx"
    );
    assert_eq!(
        resolve("https://clients2.google.com/a/b?q=1#f", "/c?d"),
        "https://clients2.google.com/c?d"
    );
    assert_eq!(
        resolve("https://clients2.google.com/a/b?q=1", "c"),
        "https://clients2.google.com/a/c"
    );
    assert_eq!(
        resolve("https://a.test/x", "HTTPS://B.test/y"),
        "HTTPS://B.test/y"
    );
}

/// The error names the host (never the path), the scheme too when it is not https, and
/// "relative" for a reference that names no host.
#[test]
fn the_refusal_names_the_host() {
    assert_eq!(host_of("https://evil.test/a/b/c.crx"), "evil.test");
    assert_eq!(host_of("https://evil.test"), "evil.test");
    assert_eq!(host_of("https://Evil.test?x"), "evil.test");
    assert_eq!(
        host_of("http://clients2.google.com/x"),
        "http://clients2.google.com"
    );
    assert_eq!(host_of("ftp://x.test"), "ftp://x.test");
    assert_eq!(host_of("javascript:alert(1)"), "javascript:");
    assert_eq!(host_of("/crx/blobs/x"), "relative");
    assert_eq!(host_of("crx/blobs/x"), "relative");
    let start = update_url(ID, "156.0.0.0");
    let hops = MemoryHops::default();
    hops.redirect(&start, "http://clients2.google.com/x");
    assert_eq!(
        download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect_err("refused"),
        DownloadError::HostNotAllowed("http://clients2.google.com".to_owned())
    );
}

const BLOB: &str = "https://clients2.googleusercontent.com/crx/blobs/x/extension_1_0_0.crx";

#[test]
fn the_download_follows_the_redirect_to_googles_blob_host() {
    let hops = MemoryHops::default();
    let start = update_url(ID, "156.0.0.0");
    hops.redirect(&start, BLOB)
        .body(BLOB, b"Cr24 bytes", Some(10));
    let bytes = download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect("downloads");
    assert_eq!(bytes, b"Cr24 bytes");
    assert_eq!(hops.asked(), [start, BLOB.to_owned()]);
}

#[test]
fn a_redirect_to_another_host_is_refused_and_not_followed() {
    let hops = MemoryHops::default();
    let start = update_url(ID, "156.0.0.0");
    let elsewhere = "https://evil.test/x.crx";
    hops.redirect(&start, elsewhere)
        .body(elsewhere, b"Cr24", Some(4));
    let error = download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect_err("refused");
    assert_eq!(error, DownloadError::HostNotAllowed("evil.test".to_owned()));
    assert_eq!(hops.asked(), std::slice::from_ref(&start));
    let hops = MemoryHops::default();
    hops.redirect(&start, "http://clients2.googleusercontent.com/x.crx");
    let error = download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect_err("refused");
    assert!(
        matches!(error, DownloadError::HostNotAllowed(_)),
        "{error:?}"
    );
}

#[test]
fn a_redirect_loop_ends_after_the_limit() {
    let hops = MemoryHops::default();
    let start = update_url(ID, "156.0.0.0");
    hops.redirect(&start, BLOB).redirect(BLOB, BLOB);
    let error = download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect_err("refused");
    assert_eq!(error, DownloadError::TooManyRedirects);
    assert_eq!(
        hops.asked().len(),
        usize::try_from(MAX_HOPS).expect("fits") + 1
    );
}

#[test]
fn a_body_over_the_limit_is_refused_announced_or_not() {
    let hops = MemoryHops::default();
    let start = update_url(ID, "156.0.0.0");
    hops.body(&start, b"Cr24", Some(MAX_CRX_BYTES + 1));
    assert_eq!(
        download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect_err("refused"),
        DownloadError::TooLarge
    );
    let hops = MemoryHops::default();
    hops.body(&start, &[0_u8; 32], None);
    assert_eq!(
        download(&hops, ID, "156.0.0.0", 16).expect_err("refused"),
        DownloadError::TooLarge
    );
    let hops = MemoryHops::default();
    hops.body(&start, &[0_u8; 16], None);
    assert_eq!(
        download(&hops, ID, "156.0.0.0", 16).expect("fits"),
        vec![0_u8; 16]
    );
}

#[test]
fn a_bad_id_or_a_failed_request_is_an_error() {
    let hops = MemoryHops::default();
    assert_eq!(
        download(&hops, "nope", "156.0.0.0", MAX_CRX_BYTES).expect_err("refused"),
        DownloadError::BadId
    );
    assert!(hops.asked().is_empty());
    let error = download(&hops, ID, "156.0.0.0", MAX_CRX_BYTES).expect_err("refused");
    assert!(matches!(error, DownloadError::Network(_)), "{error:?}");
}

#[test]
fn every_download_error_has_a_code() {
    for error in [
        DownloadError::BadId,
        DownloadError::HostNotAllowed(String::new()),
        DownloadError::TooManyRedirects,
        DownloadError::TooLarge,
        DownloadError::Network(String::new()),
    ] {
        assert!(
            error.code().starts_with("extensions.download_"),
            "{error:?}"
        );
    }
}

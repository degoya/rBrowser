use super::{cdp_domain, check_cdp_method, check_http_url};
use crate::codes;

#[test]
fn a_cdp_method_names_its_domain() {
    assert_eq!(cdp_domain("CSS.getComputedStyleForNode"), Some("CSS"));
    assert_eq!(cdp_domain("Runtime"), None);
    assert_eq!(cdp_domain("Runtime.evaluate.x"), None);
    assert_eq!(cdp_domain(".evaluate"), None);
}

#[test]
fn only_listed_domains_pass() {
    let domains = vec!["CSS".to_owned(), "DOM".to_owned()];
    assert!(check_cdp_method("DOM.getDocument", &domains).is_ok());
    let refused = check_cdp_method("Runtime.evaluate", &domains).expect_err("not listed");
    assert_eq!(refused.code(), Some(codes::CDP_DOMAIN_DENIED));
    assert_eq!(refused.param("domain"), Some("Runtime"));
    assert!(check_cdp_method("css.getStyleSheetText", &domains).is_err());
    assert!(check_cdp_method("garbage", &domains).is_err());
}

#[test]
fn http_reaches_http_and_https_hosts_only() {
    assert!(check_http_url("https://example.org/a?b").is_ok());
    assert!(check_http_url("http://localhost:3000/").is_ok());
    for url in [
        "file:///etc/passwd",
        "ftp://example.org",
        "https://",
        "https://user:pw@example.org/",
        "https://exa mple.org",
        "javascript:alert(1)",
    ] {
        let refused = check_http_url(url).expect_err(url);
        assert_eq!(refused.code(), Some(codes::HTTP_URL_DENIED), "{url}");
    }
}

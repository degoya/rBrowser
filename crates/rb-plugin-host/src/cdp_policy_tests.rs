use rb_plugin_types::codes;
use serde_json::{Value, json};

use super::{
    MAX_PLUGIN_WIDTH, MIN_PLUGIN_WIDTH, PLUGIN_CDP_METHODS, Pending, allowed, check,
    domain_allowed, domain_matches, guard, origin, view_allows,
};

/// What the first-party plugins send today (grep of `plugins/*/src`), plus storage-panel's
/// cookies, network-panel's response bodies, simulators' vision deficiencies, color-picker's
/// pixels, coverage's rule usage, precise coverage and sheet texts, pwa-check's
/// installability verdict, and width-sweep's width lease (RB-093-01).
const FIRST_PARTY: &[&str] = &[
    "Accessibility.getFullAXTree",
    "Accessibility.getPartialAXTree",
    "CSS.enable",
    "CSS.forcePseudoState",
    "CSS.getPlatformFontsForNode",
    "CSS.getStyleSheetText",
    "CSS.startRuleUsageTracking",
    "CSS.stopRuleUsageTracking",
    "CSS.takeCoverageDelta",
    "DOM.enable",
    "DOM.getDocument",
    "DOM.getNodeForLocation",
    "DOM.pushNodesByBackendIdsToFrontend",
    "DOM.querySelector",
    "DOM.scrollIntoViewIfNeeded",
    "Emulation.clearDeviceMetricsOverride",
    "Emulation.setDeviceMetricsOverride",
    "Emulation.setEmulatedVisionDeficiency",
    "Emulation.setScriptExecutionDisabled",
    "Network.deleteCookies",
    "Network.getCookies",
    "Network.getResponseBody",
    "Network.setCookie",
    "Overlay.enable",
    "Overlay.hideHighlight",
    "Overlay.highlightNode",
    "Page.captureScreenshot",
    "Page.createIsolatedWorld",
    "Page.getFrameTree",
    "Page.getInstallabilityErrors",
    "Page.getResourceContent",
    "Profiler.enable",
    "Profiler.startPreciseCoverage",
    "Profiler.stopPreciseCoverage",
    "Profiler.takePreciseCoverage",
    "Runtime.evaluate",
];

fn refused_reason(answer: Result<Pending, rb_plugin_types::Failure>) -> String {
    let failure = answer.expect_err("refused");
    assert_eq!(failure.code(), Some(codes::CDP_DOMAIN_DENIED));
    failure.param("reason").unwrap_or_default().to_owned()
}

#[test]
fn the_allowlist_is_exactly_what_the_first_party_plugins_send() {
    let mut listed: Vec<String> = PLUGIN_CDP_METHODS
        .iter()
        .flat_map(|(domain, methods)| methods.iter().map(move |m| format!("{domain}.{m}")))
        .collect();
    listed.sort();
    assert_eq!(listed, FIRST_PARTY);
    for method in FIRST_PARTY {
        assert!(allowed(method), "{method}");
    }
}

#[test]
fn what_reaches_beyond_the_page_or_switches_the_host_off_is_never_listed() {
    for method in [
        "Browser.close",
        "Browser.setDownloadBehavior",
        "CSS.disable",
        "DOM.disable",
        "DOM.resolveNode",
        "DOM.setFileInputFiles",
        "Fetch.enable",
        "Fetch.continueRequest",
        "Input.dispatchKeyEvent",
        "Input.dispatchMouseEvent",
        "Log.disable",
        "Network.clearBrowserCookies",
        "Network.disable",
        "Network.getAllCookies",
        "Network.setCookies",
        "Overlay.disable",
        "Page.addScriptToEvaluateOnNewDocument",
        "Page.close",
        "Page.crash",
        "Page.disable",
        "Page.navigate",
        "Page.removeScriptToEvaluateOnNewDocument",
        "Page.setDownloadBehavior",
        "Profiler.disable",
        "Profiler.start",
        "Profiler.setSamplingInterval",
        "Debugger.enable",
        "Debugger.getScriptSource",
        "Debugger.pause",
        "Runtime.addBinding",
        "Runtime.callFunctionOn",
        "Runtime.disable",
        "Runtime.getProperties",
        "Runtime.terminateExecution",
        "Security.setIgnoreCertificateErrors",
        "Storage.getCookies",
        "Target.attachToTarget",
        "Target.createTarget",
    ] {
        assert!(!allowed(method), "{method}");
        let answer = check("sample", method, &json!({}));
        assert_eq!(refused_reason(answer), "method", "{method}");
    }
    for (_, methods) in PLUGIN_CDP_METHODS {
        assert!(methods.iter().all(|method| *method != "disable"));
    }
}

#[test]
fn a_manifest_may_list_only_the_domains_of_the_allowlist() {
    for domain in ["Accessibility", "CSS", "DOM", "Network", "Page", "Runtime"] {
        assert!(domain_allowed(domain), "{domain}");
    }
    for domain in [
        "Browser", "Fetch", "Input", "Security", "Storage", "Target", "dom",
    ] {
        assert!(!domain_allowed(domain), "{domain}");
    }
}

#[test]
fn an_isolated_world_is_only_the_plugins_own() {
    let own = json!({ "frameId": "F", "worldName": "rbplugin-sample" });
    assert_eq!(
        check("sample", "Page.createIsolatedWorld", &own).expect("own"),
        Pending::None
    );
    for params in [
        json!({ "frameId": "F", "worldName": "rbrowser" }),
        json!({ "frameId": "F", "worldName": "rbplugin-other" }),
        json!({ "frameId": "F", "worldName": "rbplugin-sample2" }),
        json!({ "frameId": "F" }),
        json!({ "frameId": "F", "worldName": "rbplugin-sample", "grantUniveralAccess": true }),
    ] {
        let answer = check("sample", "Page.createIsolatedWorld", &params);
        assert_eq!(refused_reason(answer), "world", "{params}");
    }
    let quiet =
        json!({ "frameId": "F", "worldName": "rbplugin-sample", "grantUniveralAccess": false });
    assert!(check("sample", "Page.createIsolatedWorld", &quiet).is_ok());
}

#[test]
fn a_script_for_new_documents_would_go_only_into_the_plugins_own_world() {
    let own = json!({ "source": "1", "worldName": "rbplugin-sample" });
    assert!(guard("sample", "Page.addScriptToEvaluateOnNewDocument", &own).is_ok());
    for params in [
        json!({ "source": "1" }),
        json!({ "source": "1", "worldName": "rbrowser" }),
        json!({ "source": "1", "worldName": "" }),
    ] {
        let answer = guard("sample", "Page.addScriptToEvaluateOnNewDocument", &params);
        assert_eq!(refused_reason(answer), "world", "{params}");
    }
}

#[test]
fn evaluate_runs_in_the_main_world_or_names_a_context_the_host_checks() {
    let main = json!({ "expression": "1", "includeCommandLineAPI": true });
    assert_eq!(
        check("sample", "Runtime.evaluate", &main).expect("main world"),
        Pending::None
    );
    let isolated = json!({ "expression": "1", "contextId": 7 });
    assert_eq!(
        check("sample", "Runtime.evaluate", &isolated).expect("checked later"),
        Pending::OwnContext(7)
    );
    for params in [
        json!({ "expression": "1", "uniqueContextId": "123.456" }),
        json!({ "expression": "1", "contextId": "7" }),
        json!({ "expression": "1", "contextId": 1.5 }),
    ] {
        let answer = check("sample", "Runtime.evaluate", &params);
        assert_eq!(refused_reason(answer), "context", "{params}");
    }
}

#[test]
fn navigation_would_reach_web_pages_and_the_blank_page_only() {
    for url in [
        "https://example.com/",
        "http://localhost:3000/a",
        "about:blank",
        "ABOUT:BLANK",
    ] {
        assert!(
            guard("sample", "Page.navigate", &json!({ "url": url })).is_ok(),
            "{url}"
        );
    }
    for params in [
        json!({ "url": "file:///C:/Users/a/.ssh/id_rsa" }),
        json!({ "url": "FILE:///etc/passwd" }),
        json!({ "url": "javascript:alert(1)" }),
        json!({ "url": "data:text/html,x" }),
        json!({ "url": "chrome://settings" }),
        json!({ "url": "devtools://devtools/bundled/inspector.html" }),
        json!({ "url": "about:srcdoc" }),
        json!({ "url": "https://user:pass@example.com/" }),
        json!({}),
    ] {
        let answer = guard("sample", "Page.navigate", &params);
        assert_eq!(refused_reason(answer), "url", "{params}");
    }
}

#[test]
fn cookies_name_their_urls_or_domains_for_the_view_check() {
    let urls = json!({ "urls": ["https://example.com/a"] });
    assert_eq!(
        check("sample", "Network.getCookies", &urls).expect("listed"),
        Pending::ViewOrigin(vec!["https://example.com/a".to_owned()])
    );
    for params in [json!({}), json!({ "urls": [] }), json!({ "urls": [1] })] {
        let answer = check("sample", "Network.getCookies", &params);
        assert_eq!(refused_reason(answer), "urls", "{params}");
    }
    for method in ["Network.setCookie", "Network.deleteCookies"] {
        let params = json!({ "name": "a", "value": "b", "domain": ".example.com" });
        assert_eq!(
            check("sample", method, &params).expect("domain"),
            Pending::ViewHost {
                urls: Vec::new(),
                domains: vec![".example.com".to_owned()],
            }
        );
        let params = json!({
            "name": "a",
            "url": "https://example.com/",
            "partitionKey": { "topLevelSite": "https://example.com", "hasCrossSiteAncestor": false },
        });
        assert_eq!(
            check("sample", method, &params).expect("url"),
            Pending::ViewHost {
                urls: vec!["https://example.com/".to_owned()],
                domains: vec!["example.com".to_owned()],
            }
        );
        // Without either, `deleteCookies` would delete the name on every site.
        let answer = check("sample", method, &json!({ "name": "a" }));
        assert_eq!(refused_reason(answer), "cookie", "{method}");
        let answer = check("sample", method, &json!({ "name": "a", "domain": 3 }));
        assert_eq!(refused_reason(answer), "cookie", "{method}");
        let odd_site = json!({ "name": "a", "domain": "example.com", "partitionKey": { "topLevelSite": "file:///x" } });
        let answer = check("sample", method, &odd_site);
        assert_eq!(refused_reason(answer), "cookie", "{method}");
    }
}

#[test]
fn a_response_body_is_asked_for_by_request_id() {
    let params = json!({ "requestId": "1234.56" });
    assert_eq!(
        check("sample", "Network.getResponseBody", &params).expect("id"),
        Pending::None
    );
    for params in [
        json!({}),
        json!({ "requestId": 5 }),
        json!({ "requestId": "x".repeat(200) }),
    ] {
        let answer = check("sample", "Network.getResponseBody", &params);
        assert_eq!(refused_reason(answer), "request", "{params}");
    }
}

#[test]
fn origins_are_scheme_host_and_port() {
    assert_eq!(
        origin("https://Example.com/a?b"),
        Some(("https".to_owned(), "example.com".to_owned(), 443))
    );
    assert_eq!(
        origin("http://localhost:3000"),
        Some(("http".to_owned(), "localhost".to_owned(), 3000))
    );
    assert_eq!(
        origin("http://[::1]:8080/"),
        Some(("http".to_owned(), "[::1]".to_owned(), 8080))
    );
    for url in [
        "about:blank",
        "file:///x",
        "https://",
        "https://a:b@c/",
        "https://h:99999/",
    ] {
        assert_eq!(origin(url), None, "{url}");
    }
}

#[test]
fn a_cookie_domain_matches_the_host_or_a_parent_of_it() {
    assert!(domain_matches("www.example.com", "www.example.com"));
    assert!(domain_matches("www.example.com", ".example.com"));
    assert!(domain_matches("www.example.com", "EXAMPLE.com"));
    assert!(!domain_matches("www.example.com", "com"));
    assert!(!domain_matches("www.example.com", ".com"));
    assert!(!domain_matches("www.example.com", "other.com"));
    assert!(!domain_matches("www.example.com", "ample.com"));
    assert!(!domain_matches("www.example.com", "api.example.com"));
    assert!(!domain_matches("www.example.com", ""));
    assert!(!domain_matches("www.example.com", "."));
    assert!(domain_matches("127.0.0.1", "127.0.0.1"));
    assert!(!domain_matches("127.0.0.1", "0.0.1"));
}

#[test]
fn the_view_decides_which_cookies_a_plugin_reaches() {
    let view = "https://www.example.com/shop";
    let same = Pending::ViewOrigin(vec!["https://www.example.com/".to_owned()]);
    assert!(view_allows(&same, view));
    for other in [
        "https://example.com/",
        "http://www.example.com/",
        "https://www.example.com:8443/",
        "https://evil.com/",
        "not a url",
    ] {
        let pending = Pending::ViewOrigin(vec![
            "https://www.example.com/".to_owned(),
            other.to_owned(),
        ]);
        assert!(!view_allows(&pending, view), "{other}");
    }
    let host = |urls: &[&str], domains: &[&str]| Pending::ViewHost {
        urls: urls.iter().map(|url| (*url).to_owned()).collect(),
        domains: domains.iter().map(|domain| (*domain).to_owned()).collect(),
    };
    assert!(view_allows(&host(&[], &[".example.com"]), view));
    assert!(view_allows(
        &host(&["https://www.example.com/x"], &[]),
        view
    ));
    assert!(!view_allows(&host(&[], &["evil.com"]), view));
    assert!(!view_allows(
        &host(&["https://api.example.com/"], &[]),
        view
    ));
    assert!(!view_allows(&host(&["file:///x"], &[]), view));
    // A view that shows no web page reaches no cookie at all.
    for blank in ["about:blank", "", "file:///C:/x.html"] {
        assert!(
            !view_allows(&host(&[], &[".example.com"]), blank),
            "{blank}"
        );
        assert!(!view_allows(&same, blank), "{blank}");
    }
    assert!(view_allows(&Pending::None, "about:blank"));
}

#[test]
fn params_must_be_an_object() {
    let answer = check("sample", "DOM.getDocument", &Value::Array(Vec::new()));
    assert_eq!(refused_reason(answer), "params");
}

#[test]
fn a_width_override_names_one_width_in_range_and_nothing_else() {
    let set = "Emulation.setDeviceMetricsOverride";
    for width in [MIN_PLUGIN_WIDTH, 720, MAX_PLUGIN_WIDTH] {
        let answer = check("width-sweep", set, &json!({ "width": width }));
        assert_eq!(answer, Ok(Pending::None), "{width}");
    }
    for params in [
        json!({}),
        json!({ "width": 0 }),
        json!({ "width": MIN_PLUGIN_WIDTH - 1 }),
        json!({ "width": MAX_PLUGIN_WIDTH + 1 }),
        json!({ "width": -720 }),
        json!({ "width": 720.5 }),
        json!({ "width": "720" }),
        json!({ "width": 720, "height": 800 }),
        json!({ "width": 720, "deviceScaleFactor": 1 }),
        json!({ "width": 720, "mobile": true }),
        json!({ "width": 720, "scale": 2 }),
        json!({ "width": 720, "screenWidth": 5000 }),
        json!({ "width": 720, "positionX": 0 }),
        json!({ "width": 720, "dontSetVisibleSize": false }),
    ] {
        let answer = check("width-sweep", set, &params);
        assert_eq!(refused_reason(answer), "width", "{params}");
    }
}

#[test]
fn the_width_lease_ends_without_parameters() {
    let clear = "Emulation.clearDeviceMetricsOverride";
    assert_eq!(check("width-sweep", clear, &json!({})), Ok(Pending::None));
    let answer = check("width-sweep", clear, &json!({ "width": 720 }));
    assert_eq!(refused_reason(answer), "params");
}

/// The page-domain rules of the newer plugins (screenshots, installability, vision deficiency,
/// coverage, resources), apart so neither file outgrows 500 lines.
#[path = "cdp_policy_page_tests.rs"]
mod page;

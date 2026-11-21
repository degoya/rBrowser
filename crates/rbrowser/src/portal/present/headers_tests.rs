use super::*;

#[test]
fn no_cookie_and_no_cloudflare_header_reaches_the_dev_server() {
    for name in [
        "cookie",
        "cf-connecting-ip",
        "cf-ray",
        "cf-ipcountry",
        "cdn-loop",
        "x-real-ip",
        // The viewer's address, as cloudflared or a proxy before it passes it on.
        "x-forwarded-for",
        "forwarded",
        "true-client-ip",
        "x-client-ip",
        "x-cluster-client-ip",
    ] {
        assert!(request_dropped(name), "{name}");
    }
    for name in ["accept", "user-agent", "content-type", "sec-fetch-dest"] {
        assert!(!request_dropped(name), "{name}");
    }
}

#[test]
fn no_cookie_and_no_server_detail_reaches_the_viewer() {
    for name in [
        "set-cookie",
        "server",
        "x-powered-by",
        "x-aspnet-version",
        "x-aspnetmvc-version",
        "x-debug-token",
        "x-debug-token-link",
        "x-sourcemap",
        "sourcemap",
        "x-runtime",
        "via",
    ] {
        assert!(response_dropped(name), "{name}");
    }
    for name in [
        "content-type",
        "location",
        "cache-control",
        "content-security-policy",
    ] {
        assert!(!response_dropped(name), "{name}");
    }
}

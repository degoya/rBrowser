use super::*;

const TOKEN: &str = "abc123";

fn facts(token: &ProxyToken) -> Facts<'_> {
    Facts {
        port: 9222,
        token,
        browser_id: "B1",
    }
}

#[test]
fn the_answer_gives_the_url_its_header_and_the_url_with_the_token() {
    let token = ProxyToken::from_text(TOKEN.to_owned());
    let answer = answer(
        &facts(&token),
        None,
        &WslRoute::Unavailable("no WSL".to_owned()),
    );
    assert_eq!(answer["enabled"], json!(true));
    assert_eq!(answer["url"], json!("http://127.0.0.1:9222"));
    assert_eq!(answer["headers"]["Authorization"], json!("Bearer abc123"));
    assert_eq!(
        answer["urlWithToken"],
        json!("http://127.0.0.1:9222/t/abc123")
    );
    assert_eq!(
        answer["webSocketDebuggerUrl"],
        json!("ws://127.0.0.1:9222/devtools/browser/B1")
    );
    assert_eq!(
        answer["wsl"],
        json!({ "available": false, "reason": "no WSL" })
    );
    assert!(answer.get("page").is_none());
}

#[test]
fn a_target_adds_its_page_socket() {
    let token = ProxyToken::from_text(TOKEN.to_owned());
    let answer = answer(
        &facts(&token),
        Some((ViewId(2), "T2")),
        &WslRoute::Unavailable(String::new()),
    );
    assert_eq!(
        answer["page"],
        json!({ "view": 2, "targetId": "T2", "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/T2" })
    );
}

#[test]
fn mirrored_wsl_uses_windows_url_and_nat_the_tunnels_port() {
    let token = ProxyToken::from_text(TOKEN.to_owned());
    let direct = WslRoute::Direct {
        distro: "Debian".to_owned(),
        mode: NetworkingMode::Mirrored,
    };
    let mirrored = answer(&facts(&token), None, &direct);
    assert_eq!(mirrored["wsl"]["url"], json!("http://127.0.0.1:9222"));
    assert_eq!(mirrored["wsl"]["tunnel"], json!(false));
    assert_eq!(mirrored["wsl"]["networkingMode"], json!("mirrored"));
    let tunnel = WslRoute::Tunnel {
        distro: "Debian".to_owned(),
        mode: NetworkingMode::Nat,
        port: 40001,
    };
    let nat = answer(&facts(&token), None, &tunnel);
    assert_eq!(nat["wsl"]["url"], json!("http://127.0.0.1:40001"));
    assert_eq!(
        nat["wsl"]["urlWithToken"],
        json!("http://127.0.0.1:40001/t/abc123")
    );
    assert_eq!(nat["wsl"]["tunnel"], json!(true));
    assert_eq!(nat["wsl"]["networkingMode"], json!("nat"));
}

#[test]
fn a_failed_tunnel_names_why_and_the_manual_way() {
    let token = ProxyToken::from_text(TOKEN.to_owned());
    let failed = WslRoute::Failed {
        distro: "Debian".to_owned(),
        mode: NetworkingMode::Nat,
        error: "no helper".to_owned(),
    };
    let answer = answer(&facts(&token), None, &failed);
    assert_eq!(answer["wsl"]["error"], json!("no helper"));
    assert!(
        answer["wsl"]["hint"]
            .as_str()
            .is_some_and(|hint| hint.contains("cdp-tunnel"))
    );
    assert!(answer["wsl"].get("url").is_none());
}

#[test]
fn off_the_tool_fails_with_the_setting_as_deep_link() {
    let failure = disabled();
    assert_eq!(failure.code, "cdp.disabled");
    assert_eq!(failure.settings_key.as_deref(), Some("cdp.enabled"));
}

#[test]
fn the_answer_carries_a_ready_snippet_for_windows_and_for_wsl() {
    let token = ProxyToken::from_text(TOKEN.to_owned());
    let tunnel = WslRoute::Tunnel {
        distro: "Debian".to_owned(),
        mode: NetworkingMode::Nat,
        port: 40001,
    };
    let answer = answer(&facts(&token), None, &tunnel);
    let windows = answer["snippet"].as_str().expect("a snippet");
    assert!(windows.contains("import { chromium } from 'playwright-core';"));
    assert!(windows.contains(
        "chromium.connectOverCDP('http://127.0.0.1:9222', {\n  headers: { Authorization: 'Bearer abc123' },\n  noDefaults: true,\n});"
    ));
    let wsl = answer["wsl"]["snippet"].as_str().expect("a WSL snippet");
    assert!(wsl.contains("connectOverCDP('http://127.0.0.1:40001'"));
    assert_eq!(snippet(9222, &token), windows);
}

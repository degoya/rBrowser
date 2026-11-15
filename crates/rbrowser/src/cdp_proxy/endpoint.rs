//! The answer of the MCP tool `cdp_endpoint` (RB-030-06, threat model T5): the URL for
//! Playwright's `connectOverCDP(url, { headers })` with its `Authorization` header, the same URL
//! with the token in the path for clients without headers, a ready `connectOverCDP` script
//! (`snippet`, RB-040-06), the browser socket and - with a
//! `target` - the page socket of that view; and the URL from WSL: Windows' own in `mirrored`
//! mode, the tunnel's port inside the distribution in NAT mode. The token is in the answer
//! because the MCP server answers authenticated clients only (every request carries its bearer
//! token; `rbrowser mcp` forwards with it). Off, the tool fails with the settings key instead.

use rb_cdp::ProxyToken;
use rb_cdp::token::PATH_SEGMENT;
use rb_engine::ViewId;
use rb_mcp::ToolFailure;
use rb_wsl::netmode::NetworkingMode;
use serde_json::{Value, json};

/// What the proxy tells about itself.
#[derive(Debug)]
pub struct Facts<'a> {
    /// The port on Windows' `127.0.0.1`.
    pub port: u16,
    /// The token now.
    pub token: &'a ProxyToken,
    /// The id of `/devtools/browser/<id>`.
    pub browser_id: &'a str,
}

/// How a client in WSL reaches the proxy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WslRoute {
    /// No WSL, or it could not be asked; the reason.
    Unavailable(String),
    /// `127.0.0.1` in the distribution is Windows' own: the same URL.
    Direct {
        /// The distribution.
        distro: String,
        /// Its mode.
        mode: NetworkingMode,
    },
    /// NAT: the tunnel listens on this port inside the distribution.
    Tunnel {
        /// The distribution.
        distro: String,
        /// Its mode.
        mode: NetworkingMode,
        /// The tunnel's port in the distribution.
        port: u16,
    },
    /// NAT, but the tunnel did not start.
    Failed {
        /// The distribution.
        distro: String,
        /// Its mode.
        mode: NetworkingMode,
        /// Why.
        error: String,
    },
}

/// `http://127.0.0.1:<port>`.
#[must_use]
pub fn http_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// `http://127.0.0.1:<port>/t/<token>`: one URL that carries its proof.
#[must_use]
pub fn url_with_token(port: u16, token: &ProxyToken) -> String {
    format!("{}{PATH_SEGMENT}{}", http_url(port), token.expose())
}

/// A ready script that connects Playwright to the proxy on `port` with the token's header and
/// `noDefaults` (RB-040-06). It holds the token: for authenticated MCP clients and the person
/// only.
#[must_use]
pub fn snippet(port: u16, token: &ProxyToken) -> String {
    format!(
        "import {{ chromium }} from 'playwright-core';\n\n\
         const browser = await chromium.connectOverCDP('{}', {{\n  headers: {{ Authorization: '{}' }},\n  noDefaults: true,\n}});\n\
         // One page per Chromium view; rBrowser owns each view's size and emulation. noDefaults\n\
         // leaves the views' download behaviour, fonts and focus as rBrowser set them.\n\
         const pages = browser.contexts()[0].pages();\n",
        http_url(port),
        token.bearer()
    )
}

/// The URLs of the endpoint on `port` and the snippet for it.
fn urls(port: u16, facts: &Facts<'_>) -> Value {
    json!({
        "url": http_url(port),
        "urlWithToken": url_with_token(port, facts.token),
        "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/browser/{}", facts.browser_id),
        "snippet": snippet(port, facts.token),
    })
}

/// The tool's answer: the Windows URLs, the page of `page` (view and target id) and the route
/// from WSL.
#[must_use]
pub fn answer(facts: &Facts<'_>, page: Option<(ViewId, &str)>, wsl: &WslRoute) -> Value {
    let mut answer = urls(facts.port, facts);
    answer["enabled"] = json!(true);
    answer["headers"] = json!({ "Authorization": facts.token.bearer() });
    if let Some((view, target)) = page {
        answer["page"] = json!({
            "view": view,
            "targetId": target,
            "webSocketDebuggerUrl": format!("ws://127.0.0.1:{}/devtools/page/{target}", facts.port),
        });
    }
    answer["wsl"] = match wsl {
        WslRoute::Unavailable(reason) => json!({ "available": false, "reason": reason }),
        WslRoute::Direct { distro, mode } => {
            let mut route = urls(facts.port, facts);
            route["distro"] = json!(distro);
            route["networkingMode"] = json!(mode);
            route["tunnel"] = json!(false);
            route
        }
        WslRoute::Tunnel { distro, mode, port } => {
            let mut route = urls(*port, facts);
            route["distro"] = json!(distro);
            route["networkingMode"] = json!(mode);
            route["tunnel"] = json!(true);
            route
        }
        WslRoute::Failed {
            distro,
            mode,
            error,
        } => json!({
            "distro": distro,
            "networkingMode": mode,
            "tunnel": false,
            "error": error,
            "hint": "start the tunnel by hand: rbrowser.exe cdp-tunnel --port <port>",
        }),
    };
    answer["note"] = json!(
        "rBrowser owns each view's size (its breakpoint) and emulation: setViewportSize, setDeviceMetricsOverride and setEmulatedMedia (colour scheme, reduced motion) have no effect - use the view of the size you need and set emulation in rBrowser or with emulation_set. The shell, settings and DevTools are not targets."
    );
    answer
}

/// The failure while the proxy is off: the setting is the deep link.
#[must_use]
pub fn disabled() -> ToolFailure {
    ToolFailure::new(
        "cdp.disabled",
        "the CDP proxy is off; a person switches it on in rBrowser's Settings > Automation > CDP proxy",
    )
    .with_setting(super::ENABLED_KEY)
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;

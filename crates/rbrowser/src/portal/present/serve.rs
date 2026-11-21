//! The presentation's listener (RB-096-05): what cloudflared forwards from the internet, every
//! request from 127.0.0.1. Before the tunnel named its address every request answers 503; after,
//! a `Host` other than the tunnel's answers 403. Served are:
//!
//! - the join page ([`JOIN_PATH`]) the link opens: its script takes the token from the link's
//!   fragment, drops the fragment from the address bar and posts the token to the pairing path;
//! - the pairing ([`PAIR_PATH`], `POST` only, the token in the body - never in a URL a log
//!   keeps; a foreign `Origin` is refused): the link is checked ([`super::link`]), the viewer
//!   admitted up to `portal.presentation.maxViewers` and given an `HttpOnly`, `Secure`,
//!   `SameSite=Strict` cookie;
//! - with that cookie: the portal agent, the viewer's socket (the viewer reports only, it drives
//!   nothing), a waiting page with the agent ([`FOLLOW_PATH`]), and the active view's page
//!   through the proxy - only a path [`super::paths`] allows; any other answers 404 from here and
//!   never reaches the dev server. The traffic keeps cookies, Cloudflare's headers and the dev
//!   server's details at the portal ([`super::headers`]).
//!
//! A page of the portal's own (the waiting page, a refused path) carries the agent for a viewer,
//! so the viewer follows the presenter on from it. Nothing logged here carries a path, a header
//! or a body: the token is in the body, the session in the cookie.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use rb_cdp::request::{RequestHead, websocket_key};
use rb_plugin_types::html::escape;
use tokio::io::AsyncReadExt as _;
use tokio::net::TcpStream;

use super::link::{JOIN_PATH, PairError};
use super::public::Public;
use crate::cdp::lock;
use crate::portal::inject::{AGENT_PATH, AGENT_TAG};
use crate::portal::origins::{host_matches, upstream_for};
use crate::portal::pairing::{COOKIE_NAME, Secret, session_of};
use crate::portal::server::{
    OWN_PREFIX, PAIR_PATH, PortalServer, SOCKET_PATH, respond, respond_text,
};

/// The page a viewer waits on after joining; the agent on it follows the presenter.
pub const FOLLOW_PATH: &str = "/__rbrowser/follow";
/// The longest pairing body taken (the token is 32 hex digits).
const MAX_TOKEN_BODY: usize = 64;
/// How long the pairing body may take.
const BODY_TIMEOUT: Duration = Duration::from_secs(10);

/// Serves one request on the presentation's listener.
pub(in crate::portal) async fn serve(
    mut stream: TcpStream,
    request: RequestHead,
    rest: Vec<u8>,
    peer: IpAddr,
    server: &Arc<PortalServer>,
    public: &Public,
) {
    let Some(host) = public.host() else {
        let _ = page(&mut stream, 503, "portal.present.page.starting", false).await;
        return;
    };
    if !host_matches(request.header("host"), &host) {
        tracing::debug!("a presentation request named another host");
        let _ = page(&mut stream, 403, "portal.page.not_allowed", false).await;
        return;
    }
    let origin = format!("https://{host}");
    let path = request.path().to_owned();
    if path == JOIN_PATH {
        let body = join_html();
        let _ = respond(&mut stream, 200, "OK", "text/html", body.as_bytes(), &[]).await;
        return;
    }
    if path == PAIR_PATH {
        pair(&mut stream, &request, rest, peer, server, &origin).await;
        return;
    }
    let device = {
        let offered = session_of(request.header("cookie")).unwrap_or("");
        let mut shared = lock(&server.shared);
        shared
            .pairings
            .authenticate(offered, SystemTime::now())
            .map(|device| (device, shared.watch_device(device)))
    };
    let Some((device, revoked)) = device else {
        let _ = page(&mut stream, 403, "portal.present.page.not_viewer", false).await;
        return;
    };
    if path == AGENT_PATH {
        let agent = server.agent.as_bytes();
        let _ = respond(&mut stream, 200, "OK", "application/javascript", agent, &[]).await;
        return;
    }
    if path == SOCKET_PATH {
        if let Some(key) = websocket_key(&request) {
            let key = key.to_owned();
            crate::portal::socket::run(stream, &key, device, server).await;
        } else {
            let _ = respond_text(&mut stream, 400, "Bad Request").await;
        }
        return;
    }
    if path == FOLLOW_PATH {
        let _ = page(&mut stream, 200, "portal.present.page.follow", true).await;
        return;
    }
    if path.starts_with(OWN_PREFIX) {
        let _ = respond_text(&mut stream, 404, "Not Found").await;
        return;
    }
    if !public.rules().allows(&path) {
        tracing::debug!("a viewer asked for a path the presentation does not show");
        let _ = page(&mut stream, 404, "portal.present.page.not_shown", true).await;
        return;
    }
    let route = {
        let shared = lock(&server.shared);
        upstream_for(shared.current_url.as_deref(), &shared.allowlist)
            .map(|upstream| (shared.origin_map(&upstream, &origin).public(), upstream))
    };
    match route {
        Ok((map, upstream)) => {
            let ended = server.ended(revoked);
            if let Err(error) =
                crate::portal::proxy::forward(stream, request, rest, &upstream, &map, ended).await
            {
                tracing::debug!(%error, "a presentation request to the upstream failed");
            }
        }
        Err(refusal) => {
            let _ = page(&mut stream, 403, refusal.message_key(), true).await;
        }
    }
}

/// `POST /__rbrowser/pair` with the link's token as the body: the viewer's cookie, or why not.
async fn pair(
    stream: &mut TcpStream,
    request: &RequestHead,
    rest: Vec<u8>,
    peer: IpAddr,
    server: &Arc<PortalServer>,
    origin: &str,
) {
    if request.method != "POST" {
        let _ = respond_text(stream, 405, "Method Not Allowed").await;
        return;
    }
    if request
        .header("origin")
        .is_some_and(|sent| !sent.trim().eq_ignore_ascii_case(origin))
    {
        let _ = refused(stream, PairError::Invalid).await;
        return;
    }
    let Some(offered) = read_token(stream, request, rest).await else {
        let _ = respond_text(stream, 400, "Bad Request").await;
        return;
    };
    let user_agent = request.header("user-agent").unwrap_or("");
    let redeemed = {
        let mut shared = lock(&server.shared);
        let now = Instant::now();
        match shared.link.check(&offered, now) {
            Ok(()) => {
                let admitted = shared
                    .pairings
                    .admit(peer, user_agent, now, SystemTime::now());
                if admitted.is_ok() {
                    shared.link.spend();
                }
                admitted
            }
            Err(error) => Err(error),
        }
    };
    match redeemed {
        Ok((device, session)) => {
            rb_diag::register_secret(session.expose());
            tracing::info!(device, "a viewer joined the presentation");
            server.changed();
            let cookie = viewer_cookie(&session);
            let body = format!("{{\"next\":\"{FOLLOW_PATH}\"}}");
            let _ = respond(
                stream,
                200,
                "OK",
                "application/json",
                body.as_bytes(),
                &[("Set-Cookie", &cookie)],
            )
            .await;
        }
        Err(error) => {
            tracing::info!(?error, "a presentation link was refused");
            let _ = refused(stream, error).await;
        }
    }
}

/// The 403 of a refused link: its text for the join page, nothing of the project.
async fn refused(stream: &mut TcpStream, error: PairError) -> std::io::Result<()> {
    let body = serde_json::json!({ "message": rb_i18n::t(link_message_key(error)) }).to_string();
    respond(
        stream,
        403,
        "Forbidden",
        "application/json",
        body.as_bytes(),
        &[],
    )
    .await
}

/// The text a viewer reads for a refused link.
#[must_use]
pub fn link_message_key(error: PairError) -> &'static str {
    match error {
        PairError::Invalid => "portal.present.page.invalid",
        PairError::Expired => "portal.present.page.expired",
        PairError::RateLimited => "portal.present.page.rate_limited",
        PairError::Full => "portal.present.page.full",
    }
}

/// The pairing body: at most [`MAX_TOKEN_BODY`] bytes of `Content-Length`, read within
/// [`BODY_TIMEOUT`], trimmed.
async fn read_token(
    stream: &mut TcpStream,
    request: &RequestHead,
    mut body: Vec<u8>,
) -> Option<String> {
    let length: usize = request.header("content-length")?.trim().parse().ok()?;
    if length > MAX_TOKEN_BODY || body.len() > length {
        return None;
    }
    let mut chunk = [0u8; MAX_TOKEN_BODY];
    let rest = async {
        while body.len() < length {
            let read = stream.read(&mut chunk).await.ok()?;
            if read == 0 {
                return None;
            }
            body.extend_from_slice(chunk.get(..read)?);
        }
        Some(())
    };
    tokio::time::timeout(BODY_TIMEOUT, rest).await.ok()??;
    if body.len() != length {
        return None;
    }
    String::from_utf8(body)
        .ok()
        .map(|text| text.trim().to_owned())
}

/// The viewer's cookie: the tunnel speaks https to the viewer, so it is `Secure` too.
#[must_use]
pub fn viewer_cookie(session: &Secret) -> String {
    format!(
        "{COOKIE_NAME}={}; Path=/; HttpOnly; Secure; SameSite=Strict",
        session.expose()
    )
}

/// A page of the presentation's own with the text of `key`; with the agent for a viewer.
async fn page(
    stream: &mut TcpStream,
    status: u16,
    key: &str,
    with_agent: bool,
) -> std::io::Result<()> {
    let body = page_html(key, with_agent);
    let phrase = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        503 => "Service Unavailable",
        _ => "Error",
    };
    respond(stream, status, phrase, "text/html", body.as_bytes(), &[]).await
}

/// The HTML of a page of the presentation's own.
#[must_use]
pub fn page_html(key: &str, with_agent: bool) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
         content=\"width=device-width\"><meta name=\"robots\" content=\"noindex\">{agent}\
         <title>{title}</title></head><body><h1>{title}</h1><p>{text}</p></body></html>",
        agent = if with_agent { AGENT_TAG } else { "" },
        title = escape(&rb_i18n::t("portal.present.page.title")),
        text = escape(&rb_i18n::t(key)),
    )
}

/// The join page's script: the token from the fragment to the pairing path, the fragment out of
/// the address bar first; the viewer then goes on to where the answer says (a path of this
/// origin only). Its texts are JSON strings with `<` escaped, so none can end the script.
const JOIN_SCRIPT: &str = r##"(function () {
  var message = document.getElementById("m");
  var hash = location.hash;
  var token = hash.indexOf("#t=") === 0 ? hash.slice(3) : "";
  history.replaceState(null, "", location.pathname);
  if (!token) { message.textContent = NO_TOKEN; return; }
  fetch("PAIR_PATH", { method: "POST", credentials: "same-origin", headers: { "Content-Type": "text/plain" }, body: token })
    .then(function (answer) {
      return answer.json().then(function (body) {
        var next = body && body.next;
        if (answer.ok && typeof next === "string" && next.charAt(0) === "/" && next.charAt(1) !== "/") {
          location.replace(next);
        } else {
          message.textContent = body && typeof body.message === "string" ? body.message : FAILED;
        }
      });
    })
    .catch(function () { message.textContent = FAILED; });
})();"##;

/// The join page the link opens.
#[must_use]
pub fn join_html() -> String {
    let script = JOIN_SCRIPT
        .replace("PAIR_PATH", PAIR_PATH)
        .replace("NO_TOKEN", &script_string("portal.present.page.invalid"))
        .replace("FAILED", &script_string("portal.present.page.failed"));
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
         content=\"width=device-width\"><meta name=\"robots\" content=\"noindex\">\
         <meta name=\"referrer\" content=\"no-referrer\"><title>{title}</title></head><body>\
         <h1>{title}</h1><p id=\"m\">{joining}</p><noscript><p>{noscript}</p></noscript>\
         <script>{script}</script></body></html>",
        title = escape(&rb_i18n::t("portal.present.page.title")),
        joining = escape(&rb_i18n::t("portal.present.page.joining")),
        noscript = escape(&rb_i18n::t("portal.present.page.noscript")),
    )
}

/// The text of `key` as a JSON string that cannot end a `<script>`.
fn script_string(key: &str) -> String {
    serde_json::Value::String(rb_i18n::t(key))
        .to_string()
        .replace('<', "\\u003c")
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;

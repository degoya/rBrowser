//! The Streamable HTTP transport: `http://127.0.0.1:<port>/mcp`, stateless, JSON answers.
//!
//! Bound to the loopback address only. Every request to `/mcp` passes [`guard`] first: the
//! `Host` must be a loopback name and an `Origin`, when a browser sends one, a loopback origin -
//! a web page that rebinds its own name to 127.0.0.1 is refused by both (DNS rebinding) - and the
//! bearer token must match (401 otherwise). rmcp's own loopback `Host` check stays on behind it.
//! `/health` passes the same `Host` and `Origin` checks but wants no token: it answers a nonce
//! with the token's proof ([`McpToken::proof`]), so the probe never sends the token.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use rmcp::transport::{
    StreamableHttpServerConfig, StreamableHttpService,
    streamable_http_server::session::never::NeverSessionManager,
};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::backend::Backend;
use crate::instance::{self, FILE_VERSION, Instance};
use crate::{McpToken, RbMcpServer};

/// The header of every `/mcp` answer that names the revision of the tool list
/// ([`tools_revision`]).
pub const TOOLS_REVISION: &str = "x-rbrowser-tools";
/// How long [`HttpHandle::shutdown`] waits for the requests still open.
const SHUTDOWN_GRACE: Duration = Duration::from_millis(1500);

/// How to serve.
#[derive(Debug, Clone)]
pub struct McpHttpConfig {
    /// The port on 127.0.0.1; 0 picks a free one (`mcp.port` absent).
    pub port: u16,
    /// The bearer token every request must carry.
    pub token: McpToken,
    /// Where to write the instance file; `None` writes none.
    pub instance_file: Option<PathBuf>,
    /// The project root the instance file names; empty without a project.
    pub project: String,
}

/// A running server. Stops on [`HttpHandle::shutdown`] or when dropped; either way the
/// instance file goes.
pub struct HttpHandle {
    addr: SocketAddr,
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<std::io::Result<()>>>,
    instance_file: Option<PathBuf>,
}

impl std::fmt::Debug for HttpHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpHandle")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl HttpHandle {
    /// The address bound.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The MCP endpoint.
    #[must_use]
    pub fn mcp_url(&self) -> String {
        format!("http://{}/mcp", self.addr)
    }

    /// Stops accepting, waits [`SHUTDOWN_GRACE`] for open requests and removes the instance
    /// file.
    pub async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(SHUTDOWN_GRACE, &mut task).await {
                Ok(Ok(Err(error))) => tracing::warn!(%error, "the MCP server failed"),
                Ok(_) => {}
                Err(_) => task.abort(),
            }
        }
        self.remove_instance_file();
    }

    fn remove_instance_file(&mut self) {
        if let Some(path) = self.instance_file.take() {
            instance::remove_own(&path, self.addr.port());
        }
    }
}

impl Drop for HttpHandle {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        self.remove_instance_file();
    }
}

/// Starts the server on `127.0.0.1:<port>` and writes the instance file. Runs on the current
/// tokio runtime.
///
/// # Errors
///
/// The port cannot be bound or the instance file cannot be written.
pub async fn serve_http(
    backend: Arc<dyn Backend>,
    config: McpHttpConfig,
) -> std::io::Result<HttpHandle> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port)).await?;
    let addr = listener.local_addr()?;
    let tools = Arc::clone(&backend);
    let service = StreamableHttpService::new(
        move || Ok(RbMcpServer::new(Arc::clone(&backend))),
        Arc::new(NeverSessionManager::default()),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true),
    );
    let token = Arc::new(config.token.clone());
    let prover = Arc::clone(&token);
    let port = addr.port();
    // The token guard wraps the routes before it only: `/health` checks host and origin itself.
    let app = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(tools, revision))
        .layer(middleware::from_fn_with_state(token, guard))
        .route(
            "/health",
            get(move |headers: HeaderMap, uri: Uri| {
                std::future::ready(health(&prover, port, &headers, &uri))
            }),
        );
    let (stop, mut stopped) = watch::channel(false);
    let mut handle = HttpHandle {
        addr,
        stop,
        task: None,
        instance_file: None,
    };
    if let Some(path) = config.instance_file {
        instance::write(
            &path,
            &Instance {
                version: FILE_VERSION,
                pid: std::process::id(),
                port,
                token: config.token.expose().to_owned(),
                project: config.project.clone(),
            },
        )?;
        handle.instance_file = Some(path);
    }
    handle.task = Some(tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = stopped.wait_for(|stop| *stop).await;
            })
            .await
    }));
    tracing::info!(port, "the MCP server listens on 127.0.0.1");
    Ok(handle)
}

/// The probe: a loopback request with a nonce is answered with the token's proof for this port
/// and that nonce - the token is neither wanted nor shown.
fn health(token: &McpToken, port: u16, headers: &HeaderMap, uri: &Uri) -> Response {
    if let Err(refusal) = check_loopback(headers) {
        return refusal.into_response();
    }
    let Some(nonce) = uri.query().and_then(nonce_of) else {
        return Refusal {
            status: StatusCode::BAD_REQUEST,
            code: "nonce_required",
            message: "the probe sends a nonce of 32 to 128 hex digits",
        }
        .into_response();
    };
    Json(serde_json::json!({
        "app": instance::APP,
        "proof": token.proof(port, nonce),
    }))
    .into_response()
}

/// The `nonce` of a query string: 32 to 128 hex digits, or `None`.
fn nonce_of(query: &str) -> Option<&str> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("nonce="))
        .filter(|nonce| {
            (32..=128).contains(&nonce.len()) && nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

/// Every `/mcp` answer names the revision of the tool list in [`TOOLS_REVISION`]: the stateless
/// server cannot send `notifications/tools/list_changed` itself, so `rbrowser mcp` compares it
/// and sends the notification when plugin tools come or go (RB-060-12).
async fn revision(
    State(backend): State<Arc<dyn Backend>>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&tools_revision(backend.as_ref())) {
        response.headers_mut().insert(TOOLS_REVISION, value);
    }
    response
}

/// The revision of the tools `backend` serves: a hash over the plugin tools' names (the core
/// tools are fixed for a build).
#[must_use]
pub fn tools_revision(backend: &dyn Backend) -> String {
    let mut names: Vec<String> = backend
        .plugin_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    names.sort();
    let mut hasher = std::hash::DefaultHasher::new();
    std::hash::Hash::hash(&names, &mut hasher);
    format!("{:016x}", std::hash::Hasher::finish(&hasher))
}

/// Host, Origin and token, before anything else runs.
async fn guard(State(token): State<Arc<McpToken>>, request: Request, next: Next) -> Response {
    match check(&token, request.headers()) {
        Ok(()) => next.run(request).await,
        Err(refusal) => {
            tracing::debug!(code = refusal.code, "an MCP request was refused");
            refusal.into_response()
        }
    }
}

/// Why a request is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Refusal {
    pub(crate) status: StatusCode,
    pub(crate) code: &'static str,
    message: &'static str,
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(serde_json::json!({ "error": self.message, "code": self.code })),
        )
            .into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

/// The checks every request to `/mcp` passes: [`check_loopback`] and the bearer token.
pub(crate) fn check(token: &McpToken, headers: &HeaderMap) -> Result<(), Refusal> {
    check_loopback(headers)?;
    let offered = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .split_once(' ')
                .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        })
        .map(|(_, offered)| offered.trim());
    if !offered.is_some_and(|offered| token.matches(offered)) {
        return Err(Refusal {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "a valid bearer token is required",
        });
    }
    Ok(())
}

/// The checks every request passes, `/health` too: a loopback `Host`, and a loopback `Origin`
/// when there is one.
fn check_loopback(headers: &HeaderMap) -> Result<(), Refusal> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !is_loopback_authority(host) {
        return Err(Refusal {
            status: StatusCode::FORBIDDEN,
            code: "host_rejected",
            message: "only 127.0.0.1 and localhost are served",
        });
    }
    if let Some(origin) = headers.get(header::ORIGIN)
        && !origin.to_str().is_ok_and(is_loopback_origin)
    {
        return Err(Refusal {
            status: StatusCode::FORBIDDEN,
            code: "origin_rejected",
            message: "requests from web pages are refused",
        });
    }
    Ok(())
}

/// Whether a host name (without port) is the loopback.
fn is_loopback_host(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "127.0.0.1" | "localhost" | "[::1]"
    )
}

/// `Host`: a loopback name with an optional port.
pub(crate) fn is_loopback_authority(authority: &str) -> bool {
    let host = match authority.rsplit_once(':') {
        Some((host, port))
            if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            host
        }
        _ => authority,
    };
    is_loopback_host(host)
}

/// `Origin`: `http(s)://` and a loopback authority, nothing else (no user info, no path);
/// `null` and everything else is refused.
pub(crate) fn is_loopback_origin(origin: &str) -> bool {
    let lower = origin.to_ascii_lowercase();
    let Some(authority) = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = authority.strip_suffix('/').unwrap_or(authority);
    !authority.contains(['@', '/', '?', '#']) && is_loopback_authority(authority)
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;

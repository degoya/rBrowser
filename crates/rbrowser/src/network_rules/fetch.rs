//! The rules in CDP's `Fetch` vocabulary: the patterns `Fetch.enable` pauses requests for (at
//! the request stage, and at the response stage for the rules and breakpoints that act there),
//! and the one outcome each paused request gets - `fulfillRequest` (mock), `failRequest` (block),
//! `continueRequest` (rewrite, map remote, or unchanged when no rule matches), a local file
//! ([`Outcome::Local`]) or a rewritten response ([`Outcome::Response`]). Every paused request
//! gets an answer, or it would hang.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

use super::action::{Action, BodyReplace, HeaderOp, Rewrite};
use super::document::{Breakpoint, Stage};
use super::glob;
use super::rewrite;
use super::rule::{Rule, UrlPattern};

/// A request `Fetch.requestPaused` reported.
#[derive(Clone, Debug)]
pub struct Paused {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    pub headers: Vec<(String, String)>,
    /// The request's post data, when CDP delivered it.
    pub post_data: Option<Vec<u8>>,
    /// Response: the event carries the response's status (or its error).
    pub stage: Stage,
    pub response_status: Option<u16>,
    pub response_headers: Vec<(String, String)>,
}

fn header_pairs(raw: &Value) -> Vec<(String, String)> {
    match raw {
        Value::Object(map) => map
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_owned),
                )
            })
            .collect(),
        Value::Array(list) => list
            .iter()
            .filter_map(|entry| {
                Some((
                    entry["name"].as_str()?.to_owned(),
                    entry["value"].as_str()?.to_owned(),
                ))
            })
            .collect(),
        _ => Vec::new(),
    }
}

impl Paused {
    /// The request of a `Fetch.requestPaused` event's `params`; none without a request id.
    #[must_use]
    pub fn of(params: &Value) -> Option<Self> {
        let request_id = params["requestId"].as_str()?.to_owned();
        let request = &params["request"];
        let stage = if params["responseStatusCode"].is_number()
            || params["responseErrorReason"].is_string()
        {
            Stage::Response
        } else {
            Stage::Request
        };
        Some(Self {
            request_id,
            url: request["url"].as_str().unwrap_or_default().to_owned(),
            method: request["method"].as_str().unwrap_or("GET").to_owned(),
            resource_type: params["resourceType"]
                .as_str()
                .unwrap_or("Other")
                .to_owned(),
            headers: header_pairs(&request["headers"]),
            post_data: request["postData"]
                .as_str()
                .map(|data| data.as_bytes().to_vec()),
            stage,
            response_status: params["responseStatusCode"]
                .as_u64()
                .and_then(|status| u16::try_from(status).ok()),
            response_headers: header_pairs(&params["responseHeaders"]),
        })
    }

    /// The call that lets the request go on unchanged at its stage.
    #[must_use]
    pub fn unchanged(&self) -> Decision {
        Decision {
            rule: None,
            delay: Duration::ZERO,
            method: match self.stage {
                Stage::Request => "Fetch.continueRequest",
                Stage::Response => "Fetch.continueResponse",
            },
            params: json!({ "requestId": self.request_id }),
        }
    }
}

/// One CDP call answering a paused request.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    /// The rule that matched, if any.
    pub rule: Option<String>,
    /// How long to wait before answering (a mock's `delayMs`).
    pub delay: Duration,
    pub method: &'static str,
    pub params: Value,
}

/// What a paused request gets.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// One call, after its delay.
    Call(Decision),
    /// A file below `root` (the URL's path), or `file` itself, with `mime` or the extension's.
    Local {
        rule: String,
        request_id: String,
        root: PathBuf,
        file: Option<PathBuf>,
        mime: Option<String>,
    },
    /// The response with `headers` applied and `body` replaced (read first, when there is one).
    Response {
        rule: String,
        request_id: String,
        status: u16,
        response_headers: Vec<(String, String)>,
        headers: Vec<HeaderOp>,
        body: Option<BodyReplace>,
    },
}

impl Outcome {
    /// The rule that decided it, if any.
    #[must_use]
    pub fn rule(&self) -> Option<&str> {
        match self {
            Self::Call(decision) => decision.rule.as_deref(),
            Self::Local { rule, .. } | Self::Response { rule, .. } => Some(rule),
        }
    }
}

/// Whether `matcher` matches `request`.
#[must_use]
pub fn matcher_matches(matcher: &super::rule::Matcher, request: &Paused) -> bool {
    if matcher
        .method
        .as_deref()
        .is_some_and(|method| !method.eq_ignore_ascii_case(&request.method))
    {
        return false;
    }
    if matcher
        .resource_type
        .as_deref()
        .is_some_and(|kind| kind != request.resource_type)
    {
        return false;
    }
    match &matcher.url {
        UrlPattern::Glob(pattern) => glob::matches(pattern, &request.url),
        UrlPattern::Regex(regex) => regex.is_match(&request.url),
    }
}

/// Whether `rule` matches `request`.
#[must_use]
pub fn rule_matches(rule: &Rule, request: &Paused) -> bool {
    matcher_matches(&rule.matcher, request)
}

/// Whether `rule`'s action has anything to do at `stage`.
fn acts_at(rule: &Rule, stage: Stage) -> bool {
    match (&rule.action, stage) {
        (Action::Rewrite(rewrite), Stage::Request) => rewrite.acts_on_request(),
        (Action::Rewrite(rewrite), Stage::Response) => rewrite.acts_on_response(),
        (Action::MapLocal(local), Stage::Request) => local.resolved.is_some(),
        (_, Stage::Request) => true,
        (_, Stage::Response) => false,
    }
}

/// The first rule of `rules` matching `request` with something to do at its stage.
#[must_use]
pub fn first_match<'a>(rules: &'a [Rule], request: &Paused) -> Option<&'a Rule> {
    rules
        .iter()
        .find(|rule| acts_at(rule, request.stage) && rule_matches(rule, request))
}

fn header_entries(headers: &[(String, String)]) -> Value {
    Value::Array(
        headers
            .iter()
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect(),
    )
}

/// The outcome `rules` give `request`.
#[must_use]
pub fn decide(rules: &[Rule], request: &Paused) -> Outcome {
    let id = json!(request.request_id);
    let Some(rule) = first_match(rules, request) else {
        return Outcome::Call(request.unchanged());
    };
    let call = |delay, method, params| {
        Outcome::Call(Decision {
            rule: Some(rule.id.clone()),
            delay,
            method,
            params,
        })
    };
    match &rule.action {
        Action::Mock {
            status,
            headers,
            body_base64,
            delay_ms,
        } => call(
            Duration::from_millis(*delay_ms),
            "Fetch.fulfillRequest",
            json!({ "requestId": id, "responseCode": status,
                    "responseHeaders": header_entries(headers), "body": body_base64 }),
        ),
        Action::Block => call(
            Duration::ZERO,
            "Fetch.failRequest",
            json!({ "requestId": id, "errorReason": "BlockedByClient" }),
        ),
        Action::Rewrite(rewrite) if request.stage == Stage::Response => Outcome::Response {
            rule: rule.id.clone(),
            request_id: request.request_id.clone(),
            status: request.response_status.unwrap_or(200),
            response_headers: request.response_headers.clone(),
            headers: rewrite.response_headers.clone(),
            body: rewrite.body.clone(),
        },
        Action::Rewrite(rewrite) => call(
            Duration::ZERO,
            "Fetch.continueRequest",
            rewritten_request(request, rewrite),
        ),
        Action::MapLocal(local) => Outcome::Local {
            rule: rule.id.clone(),
            request_id: request.request_id.clone(),
            root: local.resolved.clone().unwrap_or_default(),
            file: local.file.as_ref().and_then(|_| local.resolved.clone()),
            mime: local.mime.clone(),
        },
        Action::MapRemote { from, to } => {
            let Some(url) = rewrite::map_remote(&request.url, from, to) else {
                return Outcome::Call(request.unchanged());
            };
            let mut ops = vec![HeaderOp::Set("Host".to_owned(), rewrite::host_header(to))];
            if request
                .headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("origin"))
                && let Some(origin) = rewrite::origin_header(&url)
            {
                ops.push(HeaderOp::Set("Origin".to_owned(), origin));
            }
            call(
                Duration::ZERO,
                "Fetch.continueRequest",
                json!({ "requestId": id, "url": url,
                        "headers": header_entries(&rewrite::apply_headers(&request.headers, &ops)) }),
            )
        }
    }
}

/// `Fetch.continueRequest`'s parameters for `rewrite` at the request stage.
fn rewritten_request(request: &Paused, rewrite: &Rewrite) -> Value {
    let mut params = json!({ "requestId": request.request_id });
    let base = rewrite.url.as_deref().unwrap_or(&request.url);
    let url = if rewrite.query.is_empty() {
        base.to_owned()
    } else {
        rewrite::apply_query(base, &rewrite.query)
    };
    if url != request.url {
        params["url"] = json!(url);
    }
    if !rewrite.headers.is_empty() {
        params["headers"] =
            header_entries(&rewrite::apply_headers(&request.headers, &rewrite.headers));
    }
    params
}

/// What to send, in order, when `decision`'s answer failed, so the request does not stay paused
/// (review finding M1 of Charge 7): a mock or a rewrite continues the request unchanged and, if
/// that fails too, fails it; an unchanged continue fails it; a block is never let through.
#[must_use]
pub fn fallbacks(decision: &Decision) -> Vec<(&'static str, Value)> {
    let id = decision.params["requestId"].clone();
    let plain = json!({ "requestId": id });
    let fail = (
        "Fetch.failRequest",
        json!({ "requestId": id, "errorReason": "Failed" }),
    );
    match decision.method {
        "Fetch.failRequest" => Vec::new(),
        "Fetch.continueRequest" | "Fetch.continueResponse" if decision.params == plain => {
            vec![fail]
        }
        "Fetch.continueResponse" => vec![("Fetch.continueResponse", plain), fail],
        _ => vec![("Fetch.continueRequest", plain), fail],
    }
}

fn pattern(matcher: &super::rule::Matcher, stage: Stage) -> Value {
    let mut pattern = json!({
        "urlPattern": match &matcher.url {
            UrlPattern::Glob(glob) => glob.as_str(),
            UrlPattern::Regex(_) => "*",
        },
        "requestStage": stage.cdp(),
    });
    if let Some(kind) = &matcher.resource_type {
        pattern["resourceType"] = json!(kind);
    }
    pattern
}

fn push_once(patterns: &mut Vec<Value>, pattern: Value) {
    if !patterns.contains(&pattern) {
        patterns.push(pattern);
    }
}

/// The `Fetch.enable` patterns that pause every request a rule of `rules` could match: a glob
/// rule's pattern and resource type as they are, `*` for a regex rule; the request stage for
/// what acts there, the response stage for the rewrites of a response; each once, in order.
#[must_use]
pub fn patterns(rules: &[Rule]) -> Vec<Value> {
    let mut patterns: Vec<Value> = Vec::new();
    for rule in rules {
        for stage in [Stage::Request, Stage::Response] {
            if acts_at(rule, stage) {
                push_once(&mut patterns, pattern(&rule.matcher, stage));
            }
        }
    }
    patterns
}

/// The `Fetch.enable` patterns of the enabled `breakpoints`, each at its stage.
#[must_use]
pub fn breakpoint_patterns(breakpoints: &[Breakpoint]) -> Vec<Value> {
    let mut patterns: Vec<Value> = Vec::new();
    for breakpoint in breakpoints.iter().filter(|breakpoint| breakpoint.enabled) {
        push_once(
            &mut patterns,
            pattern(&breakpoint.matcher, breakpoint.stage),
        );
    }
    patterns
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;

//! An operation's security (its own, else the document's) as an auth with variables in place of
//! the credentials: HTTP basic and digest `{{username}}`/`{{password}}`, bearer `{{token}}`, an
//! API key `{{<scheme name>}}` in its header or query, OAuth 2 client credentials or
//! authorization code with `{{clientId}}`/`{{clientSecret}}` and the requirement's scopes. The
//! first scheme of the first requirement is taken; other kinds are reported.

use rb_api_model::{Auth, KeyPlace, OAuthGrant};
use serde_json::Value;

use crate::collect::Collector;
use crate::json::{as_text, list, text};

/// An API key's place.
fn place_of(place: &str) -> Option<KeyPlace> {
    match place {
        "header" => Some(KeyPlace::Header),
        "query" => Some(KeyPlace::Query),
        _ => None,
    }
}

/// The grant of an OAuth 2 flow by its name in OpenAPI 3 or Swagger 2.
fn grant_of(flow: &str) -> Option<OAuthGrant> {
    match flow {
        "clientCredentials" | "application" => Some(OAuthGrant::ClientCredentials),
        "authorizationCode" | "accessCode" => Some(OAuthGrant::AuthorizationCode),
        _ => None,
    }
}

/// A variable's name from a scheme's name: other characters as `_`, a letter or `_` first.
#[must_use]
pub fn variable_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_-.".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !out
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        out.insert(0, '_');
    }
    out
}

fn basic() -> Auth {
    Auth::Basic {
        username: "{{username}}".to_owned(),
        password: "{{password}}".to_owned(),
    }
}

/// The auth of `operation`.
pub fn auth_of(
    root: &Value,
    operation: &Value,
    swagger: bool,
    at: &str,
    out: &mut Collector,
) -> Auth {
    let requirements = if operation.get("security").is_some() {
        list(operation, "security")
    } else {
        list(root, "security")
    };
    let Some((name, scopes)) = requirements
        .first()
        .and_then(Value::as_object)
        .and_then(|map| map.iter().next())
    else {
        return Auth::None;
    };
    let schemes = if swagger {
        &root["securityDefinitions"]
    } else {
        &root["components"]["securitySchemes"]
    };
    let scheme = &schemes[name.as_str()];
    let scope = scopes.as_array().map_or_else(String::new, |items| {
        items
            .iter()
            .map(|item| as_text(item).into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    });
    let kind = text(scheme, "type").into_owned();
    match kind.as_str() {
        "basic" => basic(),
        "http" => match text(scheme, "scheme").to_ascii_lowercase().as_str() {
            "basic" => basic(),
            "bearer" => Auth::Bearer {
                token: "{{token}}".to_owned(),
            },
            "digest" => Auth::Digest {
                username: "{{username}}".to_owned(),
                password: "{{password}}".to_owned(),
            },
            other => {
                out.reported(at, "auth", &format!("http {other}"));
                Auth::None
            }
        },
        "apiKey" => match place_of(&text(scheme, "in")) {
            Some(place) => Auth::ApiKey {
                name: text(scheme, "name").into_owned(),
                value: format!("{{{{{}}}}}", variable_name(name)),
                place,
            },
            None => {
                out.reported(at, "auth", &format!("apiKey in {}", text(scheme, "in")));
                Auth::None
            }
        },
        "oauth2" => {
            let (flow_name, flow) = if swagger {
                (text(scheme, "flow").into_owned(), scheme)
            } else {
                let flows = &scheme["flows"];
                let chosen = ["clientCredentials", "authorizationCode"]
                    .into_iter()
                    .find(|key| flows[*key].is_object())
                    .map(str::to_owned)
                    .or_else(|| flows.as_object().and_then(|m| m.keys().next().cloned()))
                    .unwrap_or_default();
                let flow = &flows[chosen.as_str()];
                (chosen, flow)
            };
            match grant_of(&flow_name) {
                Some(grant) => Auth::OAuth2 {
                    grant,
                    token_url: text(flow, "tokenUrl").into_owned(),
                    auth_url: text(flow, "authorizationUrl").into_owned(),
                    client_id: "{{clientId}}".to_owned(),
                    client_secret: "{{clientSecret}}".to_owned(),
                    scope,
                    redirect_uri: String::new(),
                },
                None => {
                    out.reported(at, "auth", &format!("oauth2 {flow_name}"));
                    Auth::None
                }
            }
        }
        "" => {
            out.reported(at, "auth", &format!("unknown scheme {name}"));
            Auth::None
        }
        other => {
            out.reported(at, "auth", other);
            Auth::None
        }
    }
}

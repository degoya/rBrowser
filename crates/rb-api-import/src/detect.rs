//! Which tool a file comes from, by its name and its first marks: Postman's `info.schema` or an
//! environment's `values`, Insomnia's `__export_format`, Thunder Client's `clientName`, HTTPie's
//! `meta.format`, `openapi`/`swagger`, HAR's `log.entries`, a `.bru` file, a request line, a
//! command that starts with `curl`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Source;
use crate::json::json_or_yaml;

/// A format an import reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Postman,
    Insomnia,
    ThunderClient,
    Bruno,
    Httpie,
    /// `.http` files and `http-client.env.json`.
    Http,
    /// OpenAPI 3.x and Swagger 2.0.
    OpenApi,
    Har,
    Curl,
}

/// Every format, in the order the panel offers them.
pub const FORMATS: &[Format] = &[
    Format::Postman,
    Format::Insomnia,
    Format::ThunderClient,
    Format::Bruno,
    Format::Httpie,
    Format::Http,
    Format::OpenApi,
    Format::Har,
    Format::Curl,
];

impl Format {
    /// The format's id, as the panel and the op name it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Postman => "postman",
            Self::Insomnia => "insomnia",
            Self::ThunderClient => "thunderClient",
            Self::Bruno => "bruno",
            Self::Httpie => "httpie",
            Self::Http => "http",
            Self::OpenApi => "openApi",
            Self::Har => "har",
            Self::Curl => "curl",
        }
    }

    /// The format of `id`.
    #[must_use]
    pub fn of(id: &str) -> Option<Self> {
        FORMATS.iter().copied().find(|format| format.id() == id)
    }
}

const METHODS: &[&str] = &[
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "CONNECT",
];

/// Whether `text` is a command that runs curl.
#[must_use]
pub fn is_curl(text: &str) -> bool {
    let first = text.split_whitespace().next().unwrap_or("");
    let first = first.trim_matches(['^', '"', '\'']);
    first.eq_ignore_ascii_case("curl") || first.eq_ignore_ascii_case("curl.exe")
}

/// The format of `source`, by its name and its text.
#[must_use]
pub fn detect(source: &Source) -> Option<Format> {
    let name = source.name.to_ascii_lowercase();
    let file = name.rsplit(['/', '\\']).next().unwrap_or(&name);
    if file.ends_with(".bru") || file == "bruno.json" {
        return Some(Format::Bruno);
    }
    if file.ends_with(".http")
        || file.ends_with(".rest")
        || file == "http-client.env.json"
        || file == "http-client.private.env.json"
    {
        return Some(Format::Http);
    }
    let text = source.text.trim_start_matches('\u{feff}').trim();
    if is_curl(text) {
        return Some(Format::Curl);
    }
    if text.starts_with("meta {") {
        return Some(Format::Bruno);
    }
    if let Ok(value) = json_or_yaml(text)
        && let Some(format) = of_document(&value)
    {
        return Some(format);
    }
    let request_line = text.lines().any(|line| {
        let mut words = line.split_whitespace();
        let method = words.next().unwrap_or("");
        METHODS.contains(&method)
            && words.next().is_some_and(|url| {
                url.contains("://") || url.starts_with("{{") || url.starts_with('/')
            })
    });
    request_line.then_some(Format::Http)
}

fn thunder(value: &Value) -> bool {
    ["clientName", "client"]
        .iter()
        .any(|key| value[*key].as_str() == Some("Thunder Client"))
}

/// The format of a JSON or YAML document.
#[must_use]
pub fn of_document(value: &Value) -> Option<Format> {
    let schema = value["info"]["schema"].as_str().unwrap_or("");
    let postman_scope = value["_postman_variable_scope"].is_string();
    let postman_env = value["values"].is_array() && value["name"].is_string();
    if schema.contains("getpostman.com") || postman_scope || postman_env {
        return Some(Format::Postman);
    }
    if value["info"].is_object() && value["item"].is_array() {
        return Some(Format::Postman);
    }
    if value["_type"].as_str() == Some("export") || value["__export_format"].is_number() {
        return Some(Format::Insomnia);
    }
    if thunder(value)
        || value
            .as_array()
            .is_some_and(|items| items.iter().any(thunder))
    {
        return Some(Format::ThunderClient);
    }
    if value["meta"]["format"].as_str() == Some("httpie") {
        return Some(Format::Httpie);
    }
    if value["openapi"].is_string() || value["swagger"].is_string() {
        return Some(Format::OpenApi);
    }
    if value["log"]["entries"].is_array() {
        return Some(Format::Har);
    }
    None
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;

//! The API client's request model (RB-096-01), pure and `std` + serde, so the app's sender, the
//! importers of RB-096-02 and the exporters of RB-096-03 build on one model:
//!
//! | Part | Where |
//! | --- | --- |
//! | A request: method, URL, headers, body (JSON, form, multipart, raw, binary, GraphQL), auth (basic, bearer, API key, digest, OAuth 2), options (timeout, redirects, TLS) | [`request`] |
//! | Its `.http` file - one request per file, read by VS Code REST Client and the JetBrains HTTP Client; what they do not know as `# @rb-*` metadata | [`http_file`] (write), [`http_parse`] (read) |
//! | `{{name}}` and `{{$processEnv RB_SECRET_X}}` templates | [`vars`] |
//! | The environments: `.rbrowser/api/http-client.env.json` (JetBrains form), the private values beside it, the layers | [`env`] |
//! | A request's id - its path below `.rbrowser/api/` - and a file name from a title | [`paths`] |
//! | Secret values masked in any text that leaves the core | [`mask`] |
//! | A request's checks (`# @rb-check`), the JSONPath subset, an answer judged against them (RB-096-04) | [`checks`], [`jsonpath`], [`verdict`] |
//! | Order statistics of timings: percentiles by nearest rank, min/median/p95/max (the benchmark, the load test of RB-096-07) | [`stats`] |

pub mod checks;
pub mod env;
pub mod http_file;
pub mod http_parse;
pub mod jsonpath;
pub mod mask;
pub mod paths;
pub mod request;
pub mod stats;
pub mod vars;
pub mod verdict;

pub use checks::{Check, Compare};
pub use env::{EnvFile, Layers};
pub use http_file::to_http;
pub use http_parse::{ParseError, parse_http};
pub use request::{ApiRequest, Auth, Body, Entry, Invalid, KeyPlace, OAuthGrant, Options, Part};
pub use vars::{Lookup, Resolved, Unresolved, Value, substitute};

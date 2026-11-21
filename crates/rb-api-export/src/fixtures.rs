//! Requests the tests of every format share.

use rb_api_model::{ApiRequest, Auth, Body, Entry};

/// The panel's usual request, its plain variables already filled in: a JSON POST with a
/// disabled header and the bearer token a secret variable.
pub(crate) fn create_user() -> ApiRequest {
    let mut request = ApiRequest::new("POST", "https://api.example.com/users");
    request.name = "Create user".into();
    request.headers = vec![
        Entry::new("Accept", "application/json"),
        Entry {
            enabled: false,
            ..Entry::new("X-Debug", "1")
        },
    ];
    request.body = Body::Json {
        text: "{\"name\": \"Ada\"}".into(),
    };
    request.auth = Auth::Bearer {
        token: "{{token}}".into(),
    };
    request
}

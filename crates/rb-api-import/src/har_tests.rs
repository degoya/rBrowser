use rb_api_model::Body;

use crate::{Import, Source, import};

fn capture() -> Import {
    import(
        &[Source::new(
            "shop.har",
            include_str!("../fixtures/har/capture.har"),
        )],
        None,
    )
    .expect("imports")
}

#[test]
fn every_call_but_static_files_becomes_a_request_by_host() {
    let import = capture();
    assert_eq!(import.collection.as_deref(), Some("shop"));
    let names: Vec<(&str, &str)> = import
        .requests
        .iter()
        .map(|r| (r.folder.as_str(), r.request.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("shop-example", "GET /account"),
            ("api-shop-example", "POST /cart"),
            ("api-shop-example", "POST /login"),
        ]
    );
    let account = &import.requests[0].request;
    assert_eq!(account.url, "https://shop.example/account?tab=orders");
    let headers: Vec<&str> = account.headers.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(headers, ["accept", "cookie", "user-agent"]);
    assert_eq!(account.header("cookie"), Some("{{$secret cookie}}"));
}

#[test]
fn bodies_are_mapped_and_secrets_replaced() {
    let import = capture();
    let cart = &import.requests[1].request;
    assert_eq!(
        cart.body,
        Body::Json {
            text: "{\"sku\":\"A1\"}".into()
        }
    );
    assert_eq!(
        cart.header("authorization"),
        Some("Bearer {{$secret token}}")
    );
    assert_eq!(cart.header("content-length"), None);
    let Body::Form { fields } = &import.requests[2].request.body else {
        panic!("form")
    };
    assert_eq!(fields[1].value, "{{$secret password}}");
    let text = serde_json::to_string(&import).expect("serializes");
    for secret in ["abc123", "eyJ.secret.sig", "hunter2"] {
        assert!(!text.contains(secret), "{secret} leaked");
    }
}

#[test]
fn what_was_left_out_is_named() {
    let report = capture().report;
    assert_eq!(report.of("skipped")[0].detail, "1");
    assert_eq!(
        report.of("transport")[0].detail,
        ":authority, :method, content-length"
    );
}

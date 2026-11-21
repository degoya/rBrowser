use rb_api_model::{Auth, Body, KeyPlace, OAuthGrant};
use serde_json::json;

use crate::{Import, Imported, Source, import};

fn petstore() -> Import {
    import(
        &[Source::new(
            "petstore.yaml",
            include_str!("../fixtures/openapi/petstore.yaml"),
        )],
        None,
    )
    .expect("imports")
}

fn swagger() -> Import {
    import(
        &[Source::new(
            "swagger.json",
            include_str!("../fixtures/openapi/swagger.json"),
        )],
        None,
    )
    .expect("imports")
}

fn find<'a>(import: &'a Import, name: &str) -> &'a Imported {
    import
        .requests
        .iter()
        .find(|r| r.request.name == name)
        .unwrap_or_else(|| panic!("no request {name}"))
}

const BASE: &str = "https://{{region}}.petstore.example/v1";

#[test]
fn one_request_per_operation_with_its_parameters() {
    let import = petstore();
    assert_eq!(import.collection.as_deref(), Some("Petstore"));
    assert_eq!(import.requests.len(), 6);
    let list = find(&import, "List pets");
    assert_eq!(list.folder, "pets");
    assert_eq!(list.request.url, format!("{BASE}/pets?limit=20"));
    assert_eq!(
        list.request.header("X-Request-Id"),
        Some("{{X-Request-Id}}")
    );
    assert_eq!(
        list.request.auth,
        Auth::Bearer {
            token: "{{token}}".into()
        }
    );
    let show = &find(&import, "showPet").request;
    assert_eq!(show.url, format!("{BASE}/pets/{{{{petId}}}}"));
    assert_eq!(show.comments, ["Deprecated.", "One pet by its id."]);
    assert_eq!(
        show.auth,
        Auth::ApiKey {
            name: "X-Api-Key".into(),
            value: "{{apiKey}}".into(),
            place: KeyPlace::Header
        }
    );
    assert_eq!(find(&import, "Delete a pet").request.auth, Auth::None);
    assert_eq!(import.environments[0].name, "$shared");
    assert_eq!(import.environments[0].variables[0].name, "region");
    assert_eq!(import.environments[0].variables[0].value, "eu");
}

#[test]
fn bodies_get_an_example_from_the_schema() {
    let import = petstore();
    let Body::Json { text } = &find(&import, "Create a pet").request.body else {
        panic!("json")
    };
    let value: serde_json::Value = serde_json::from_str(text).expect("json");
    assert_eq!(
        value,
        json!({
            "name": "Rex",
            "tag": "dog",
            "born": "2024-01-01",
            "owner": { "email": "user@example.com", "verified": true },
            "friends": []
        })
    );
    assert_eq!(find(&import, "Upload a photo").folder, "");
    let photo = &find(&import, "Upload a photo").request;
    let Body::Multipart { parts } = &photo.body else {
        panic!("multipart")
    };
    assert_eq!(parts[0].value, "Rex");
    assert_eq!(parts[1].file.as_deref(), Some("./image"));
    assert!(matches!(
        &photo.auth,
        Auth::OAuth2 { grant: OAuthGrant::ClientCredentials, token_url, scope, .. }
            if token_url == "https://auth.petstore.example/token" && scope == "pets:write"
    ));
    let login = &find(&import, "Log in").request;
    let Body::Form { fields } = &login.body else {
        panic!("form")
    };
    assert_eq!(fields[1].value, "true");
    assert_eq!(
        login.auth,
        Auth::Basic {
            username: "{{username}}".into(),
            password: "{{password}}".into()
        }
    );
}

#[test]
fn what_a_description_has_beyond_a_request_is_reported() {
    let report = petstore().report;
    let of = |code: &str| -> Vec<(String, String)> {
        report
            .of(code)
            .iter()
            .map(|n| (n.at.clone(), n.detail.clone()))
            .collect()
    };
    assert_eq!(
        of("server"),
        [(
            "Petstore".into(),
            "https://staging.petstore.example/v1".into()
        )]
    );
    assert_eq!(of("item"), [("Create a pet".into(), "callbacks".into())]);
    assert_eq!(
        of("setting"),
        [("List pets".into(), "cookie session".into())]
    );
}

#[test]
fn swagger_2_reads_its_host_body_and_form_parameters() {
    let import = swagger();
    let order = find(&import, "Place an order");
    assert_eq!(order.folder, "orders");
    assert_eq!(order.request.url, "https://store.example/api/orders");
    let Body::Json { text } = &order.request.body else {
        panic!("json")
    };
    let value: serde_json::Value = serde_json::from_str(text).expect("json");
    assert_eq!(value, json!({ "item": "book", "quantity": 1 }));
    assert_eq!(
        order.request.auth,
        Auth::ApiKey {
            name: "api_key".into(),
            value: "{{key}}".into(),
            place: KeyPlace::Query
        }
    );
    let receipt = &find(&import, "uploadReceipt").request;
    assert_eq!(
        receipt.url,
        "https://store.example/api/orders/{{orderId}}/receipt"
    );
    let Body::Multipart { parts } = &receipt.body else {
        panic!("multipart")
    };
    assert_eq!(parts[0].value, "paid");
    assert_eq!(parts[1].file.as_deref(), Some("./scan"));
    assert_eq!(receipt.auth, Auth::None);
    let auth = import.report.of("auth");
    assert_eq!(auth[0].detail, "oauth2 implicit");
    assert!(import.report.of("body")[0].detail.contains("other.json"));
}

#[test]
fn another_version_says_which() {
    let error = import(
        &[Source::new(
            "x.json",
            r#"{"openapi": "4.0.0", "paths": {}}"#,
        )],
        None,
    )
    .expect_err("refused");
    assert!(
        matches!(error, crate::ImportError::Unreadable { reason, .. } if reason.contains("4.0.0"))
    );
}

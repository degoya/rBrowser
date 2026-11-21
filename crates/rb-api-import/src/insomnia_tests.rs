use rb_api_model::{Auth, Body, KeyPlace};

use crate::{Import, Imported, Source, import};

fn read(name: &str, text: &str) -> Import {
    import(&[Source::new(name, text)], None).expect("imports")
}

fn json() -> Import {
    read(
        "insomnia.json",
        include_str!("../fixtures/insomnia/export.json"),
    )
}

fn find<'a>(import: &'a Import, name: &str) -> &'a Imported {
    import
        .requests
        .iter()
        .find(|r| r.request.name == name)
        .unwrap_or_else(|| panic!("no request {name}"))
}

#[test]
fn requests_keep_their_folders_variables_and_settings() {
    let import = json();
    assert_eq!(import.collection.as_deref(), Some("Billing"));
    assert_eq!(import.requests.len(), 4);
    let list = find(&import, "List invoices");
    assert_eq!(list.folder, "invoices");
    assert_eq!(list.request.url, "{{baseUrl}}/invoices?status=open");
    assert_eq!(
        list.request.auth,
        Auth::Bearer {
            token: "{{token}}".into()
        }
    );
    assert!(!list.request.options.follow_redirects);
    assert_eq!(list.request.comments, ["Open invoices."]);
    assert_eq!(find(&import, "Upload").folder, "");
}

#[test]
fn bodies_and_auth_are_mapped_and_secrets_replaced() {
    let import = json();
    let create = &find(&import, "Create invoice").request;
    assert_eq!(
        create.auth,
        Auth::Basic {
            username: "billing".into(),
            password: "{{$secret password}}".into()
        }
    );
    assert!(matches!(&create.body, Body::Json { text } if text.contains("amount")));
    let Body::Multipart { parts } = &find(&import, "Upload").request.body else {
        panic!("multipart")
    };
    assert_eq!(parts[1].file.as_deref(), Some("./invoice.pdf"));
    let query = &find(&import, "Query").request;
    assert_eq!(
        query.body,
        Body::GraphQl {
            query: "{ invoices { id } }".into(),
            variables: "{\"first\":1}".into()
        }
    );
    assert_eq!(
        query.auth,
        Auth::ApiKey {
            name: "X-Key".into(),
            value: "{{$secret x-key}}".into(),
            place: KeyPlace::Query
        }
    );
}

#[test]
fn environments_take_the_base_as_shared_and_private_ones_as_secrets() {
    let import = json();
    let names: Vec<&str> = import
        .environments
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, ["$shared", "Production", "Mine"]);
    assert_eq!(import.environments[0].variables[1].name, "nested.region");
    assert_eq!(
        import.environments[1].variables[1].value,
        "{{$secret token}}"
    );
    assert_eq!(import.environments[2].variables[0].value, "{{$secret pin}}");
    let text = serde_json::to_string(&import).expect("serializes");
    for secret in ["pa55", "abc123", "live-token", "1234"] {
        assert!(!text.contains(secret), "{secret} leaked");
    }
}

#[test]
fn what_rbrowser_lacks_is_reported() {
    let report = json().report;
    let of = |code: &str| -> Vec<(String, String)> {
        report
            .of(code)
            .iter()
            .map(|n| (n.at.clone(), n.detail.clone()))
            .collect()
    };
    assert_eq!(of("template"), [("Create invoice".into(), "now".into())]);
    assert_eq!(
        of("script"),
        [("Create invoice".into(), "pre-request".into())]
    );
    assert_eq!(of("auth"), [("Upload".into(), "ntlm".into())]);
    assert_eq!(of("item"), [("Live".into(), "websocket_request".into())]);
    assert_eq!(
        of("disabled"),
        [("List invoices".into(), "query debug".into())]
    );
}

#[test]
fn the_yaml_export_reads_as_the_json_one() {
    let yaml = read(
        "insomnia.yaml",
        include_str!("../fixtures/insomnia/export.yaml"),
    );
    assert_eq!(yaml.requests, json().requests);
    assert_eq!(yaml.environments, json().environments);
}

#[test]
fn nunjucks_tags_become_variables_where_rbrowser_has_them() {
    let mut out = crate::Collector::default();
    assert_eq!(
        super::nunjucks(
            "{{ _.a }}/{{b}}/{% uuid %}/{% now 'unix' %}/{{ _.a + 1 }}",
            "R",
            &mut out
        ),
        "{{a}}/{{b}}/{{$guid}}/{{$timestamp}}/{{ _.a + 1 }}"
    );
    assert!(out.finish().report.notes.is_empty());
}

#[test]
fn another_export_format_says_which() {
    let error = import(
        &[Source::new(
            "x.json",
            r#"{"_type":"export","__export_format":3,"resources":[]}"#,
        )],
        None,
    )
    .expect_err("refused");
    assert!(matches!(error, crate::ImportError::Unreadable { reason, .. } if reason.contains('3')));
}

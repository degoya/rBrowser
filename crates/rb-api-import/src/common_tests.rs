use super::*;

#[test]
fn path_parameters_become_values_or_variables() {
    let values = vec![
        ("id".to_owned(), "42".to_owned()),
        ("empty".to_owned(), String::new()),
    ];
    assert_eq!(
        path_params("https://a.example:8080/u/:id/:empty/:other?x=:id", &values),
        "https://a.example:8080/u/42/{{empty}}/{{other}}?x=:id"
    );
    assert_eq!(
        path_params("{{baseUrl}}/u/:id", &values),
        "{{baseUrl}}/u/42"
    );
    assert_eq!(
        path_params("https://a.example", &values),
        "https://a.example"
    );
}

#[test]
fn openapi_braces_become_variables() {
    assert_eq!(
        brace_params("https://{region}.example/{{v}}/pets/{petId}/{bad name}"),
        "https://{{region}}.example/{{v}}/pets/{{petId}}/{bad name}"
    );
}

#[test]
fn a_text_body_follows_its_language() {
    let mut headers = Vec::new();
    assert!(matches!(
        text_body("{}".into(), "json", &mut headers),
        Body::Json { .. }
    ));
    assert!(matches!(
        text_body("<a/>".into(), "xml", &mut headers),
        Body::Raw { .. }
    ));
    assert_eq!(headers[0].value, "application/xml");
    let mut json = vec![Entry::new("content-type", "application/vnd.api+json")];
    assert!(matches!(
        text_body("{}".into(), "", &mut json),
        Body::Json { .. }
    ));
    assert_eq!(text_body("  ".into(), "json", &mut headers), Body::None);
}

#[test]
fn a_file_lands_beside_the_request_and_is_reported() {
    let mut out = Collector::default();
    assert_eq!(
        local_file("C:\\Users\\ada\\a.png", "R", &mut out),
        "./a.png"
    );
    assert_eq!(local_file("", "R", &mut out), "./file");
    assert_eq!(out.finish().report.of("file").len(), 2);
}

#[test]
fn query_pairs_are_appended() {
    let pairs = vec![("a".to_owned(), "{{a}}".to_owned())];
    assert_eq!(with_query("https://x/y", &pairs), "https://x/y?a={{a}}");
    assert_eq!(
        with_query("https://x/y?b=1", &pairs),
        "https://x/y?b=1&a={{a}}"
    );
}

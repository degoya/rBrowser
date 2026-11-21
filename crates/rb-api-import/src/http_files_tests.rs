use rb_api_model::{Auth, Body};

use crate::{Import, Source, import};

fn requests() -> Import {
    import(
        &[Source::new(
            "todo/requests.http",
            include_str!("../fixtures/http/requests.http"),
        )],
        None,
    )
    .expect("imports")
}

#[test]
fn a_file_of_several_requests_is_cut_into_one_each() {
    let import = requests();
    assert_eq!(import.collection.as_deref(), Some("requests"));
    let names: Vec<&str> = import
        .requests
        .iter()
        .map(|r| r.request.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "List todos",
            "Create todo",
            "DELETE {{host}}/todos/1",
            "Login form"
        ]
    );
    assert!(import.requests.iter().all(|r| r.folder == "requests"));
    let create = &import.requests[1].request;
    assert_eq!(create.method, "POST");
    assert!(matches!(&create.body, Body::Json { text } if text.trim_end().ends_with('}')));
    assert_eq!(create.header("Authorization"), Some("Bearer {{apiToken}}"));
    let delete = &import.requests[2].request;
    assert_eq!(delete.comments, ["A note of the person"]);
    assert_eq!(
        delete.header("Authorization"),
        Some("Basic {{$secret basic}}")
    );
    assert_eq!(delete.auth, Auth::None);
}

#[test]
fn file_variables_become_shared_variables_and_secrets_placeholders() {
    let import = requests();
    let shared = &import.environments[0];
    assert_eq!(shared.name, "$shared");
    assert_eq!(shared.variables[0].value, "https://todo.example");
    assert_eq!(shared.variables[1].value, "{{$secret apitoken}}");
    let Body::Form { fields } = &import.requests[3].request.body else {
        panic!("form")
    };
    assert_eq!(fields[1].value, "{{$secret password}}");
    let text = serde_json::to_string(&import).expect("serializes");
    for secret in ["t0d0-secret", "dXNlcjpwYXNz", "letmein"] {
        assert!(!text.contains(secret), "{secret} leaked");
    }
}

#[test]
fn handlers_and_redirects_are_reported_not_sent() {
    let import = requests();
    let of = |code: &str| -> Vec<(String, String)> {
        import
            .report
            .of(code)
            .iter()
            .map(|n| (n.at.clone(), n.detail.clone()))
            .collect()
    };
    assert_eq!(
        of("script"),
        [
            ("Create todo".into(), "response handler".into()),
            ("DELETE {{host}}/todos/1".into(), "pre-request".into())
        ]
    );
    assert_eq!(
        of("setting"),
        [("DELETE {{host}}/todos/1".into(), "output to a file".into())]
    );
    assert_eq!(of("variable")[0].1, "$randomInt");
    let create = &import.requests[1].request;
    assert!(matches!(&create.body, Body::Json { text } if !text.contains("client.global")));
}

#[test]
fn the_jetbrains_environment_files_are_read_public_and_private() {
    let import = import(
        &[
            Source::new(
                "http-client.env.json",
                include_str!("../fixtures/http/http-client.env.json"),
            ),
            Source::new(
                "http-client.private.env.json",
                include_str!("../fixtures/http/http-client.private.env.json"),
            ),
        ],
        None,
    )
    .expect("imports");
    let names: Vec<&str> = import
        .environments
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, ["dev", "prod"]);
    let dev = &import.environments[0];
    assert_eq!(dev.variables.len(), 3);
    assert_eq!(dev.variables[2].value, "{{$secret apitoken}}");
}

#[test]
fn a_pasted_request_reads_without_a_folder() {
    let import = import(
        &[Source::new("", "GET https://a.example/ping\nAccept: */*\n")],
        None,
    )
    .expect("imports");
    assert_eq!(import.requests[0].folder, "");
    assert_eq!(import.requests[0].request.url, "https://a.example/ping");
}

#[test]
fn a_request_that_does_not_read_is_dropped_with_its_line() {
    let import = import(
        &[Source::new(
            "x.http",
            "### ok\nGET https://a.example/\n\n### broken\nGET https://a.example/\nnot a header\n",
        )],
        None,
    )
    .expect("imports");
    assert_eq!(import.requests.len(), 1);
    let invalid = import.report.of("invalid");
    assert_eq!(invalid[0].at, "request 2");
    assert!(
        invalid[0].detail.contains("header"),
        "{}",
        invalid[0].detail
    );
}

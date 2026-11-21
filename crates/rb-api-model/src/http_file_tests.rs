use super::*;
use crate::checks::{Check, Compare};
use crate::http_parse::parse_http;
use crate::request::{KeyPlace, Options};

fn json_request() -> ApiRequest {
    ApiRequest {
        name: "Create user".into(),
        method: "POST".into(),
        url: "{{baseUrl}}/users".into(),
        headers: vec![
            Entry::new("Content-Type", "application/json"),
            Entry {
                name: "X-Debug".into(),
                value: "1".into(),
                enabled: false,
            },
        ],
        body: Body::Json {
            text: "{\"name\": \"Ada\"}".into(),
        },
        auth: Auth::Bearer {
            token: "{{token}}".into(),
        },
        options: Options {
            timeout_ms: 5000,
            follow_redirects: false,
            verify_tls: false,
        },
        comments: vec!["made by hand".into()],
        checks: vec![
            Check::Status {
                expected: "201".into(),
            },
            Check::Json {
                path: "$.name".into(),
                compare: Compare::Equals,
                value: "\"Ada\"".into(),
            },
        ],
    }
}

#[test]
fn a_request_is_written_in_the_form_other_tools_run() {
    let text = to_http(&json_request());
    assert_eq!(
        text,
        "### Create user\n\
         # made by hand\n\
         # @rb-auth bearer\n\
         # @rb-body json\n\
         # @no-redirect\n\
         # @rb-timeout-ms 5000\n\
         # @rb-insecure\n\
         # @rb-off X-Debug: 1\n\
         # @rb-check status 201\n\
         # @rb-check json $.name == \"Ada\"\n\
         POST {{baseUrl}}/users\n\
         Content-Type: application/json\n\
         Authorization: Bearer {{token}}\n\
         \n\
         {\"name\": \"Ada\"}\n"
    );
}

#[test]
fn every_kind_reads_back_as_it_was_written() {
    let mut requests = vec![
        json_request(),
        ApiRequest::new("GET", "https://a.example/x?y=1"),
    ];
    let mut form = ApiRequest::new("POST", "https://a.example/login");
    form.body = Body::Form {
        fields: vec![
            Entry::new("user", "{{user}}"),
            Entry::new("note", "a&b=c 100%+"),
            Entry {
                name: "debug".into(),
                value: "1".into(),
                enabled: false,
            },
        ],
    };
    form.auth = Auth::Basic {
        username: "{{user}}".into(),
        password: "{{password}}".into(),
    };
    requests.push(form);
    let mut multipart = ApiRequest::new("POST", "https://a.example/upload");
    multipart.headers.push(Entry::new(
        "Content-Type",
        "multipart/form-data; boundary=rbrowser-part",
    ));
    multipart.body = Body::Multipart {
        parts: vec![
            Part {
                name: "title".into(),
                value: "Logo".into(),
                file: None,
                content_type: None,
            },
            Part {
                name: "image".into(),
                value: String::new(),
                file: Some("./logo.png".into()),
                content_type: Some("image/png".into()),
            },
        ],
    };
    requests.push(multipart);
    let mut binary = ApiRequest::new("PUT", "https://a.example/blob");
    binary.body = Body::Binary {
        path: "./data/blob.bin".into(),
    };
    requests.push(binary);
    let mut graphql = ApiRequest::new("POST", "https://a.example/graphql");
    graphql.body = Body::GraphQl {
        query: "query Me {\n  me { id }\n}".into(),
        variables: "{\"x\": 1}".into(),
    };
    graphql.auth = Auth::ApiKey {
        name: "X-Api-Key".into(),
        value: "{{key}}".into(),
        place: KeyPlace::Header,
    };
    requests.push(graphql);
    let mut raw = ApiRequest::new("POST", "https://a.example/xml");
    raw.headers
        .push(Entry::new("Content-Type", "application/xml"));
    raw.body = Body::Raw {
        text: "<a>\n  <b/>\n</a>".into(),
    };
    raw.auth = Auth::Digest {
        username: "u".into(),
        password: "{{p}}".into(),
    };
    requests.push(raw);
    for request in requests {
        let text = to_http(&request);
        assert_eq!(parse_http(&text), Ok(request), "{text}");
    }
}

#[test]
fn a_multipart_body_without_a_content_type_gets_the_boundary_header() {
    let mut request = ApiRequest::new("POST", "https://a.example/upload");
    request.body = Body::Multipart {
        parts: vec![Part {
            name: "a".into(),
            value: "1".into(),
            file: None,
            content_type: None,
        }],
    };
    let text = to_http(&request);
    assert!(text.contains("Content-Type: multipart/form-data; boundary=rbrowser-part\n"));
    assert!(text.ends_with(
        "--rbrowser-part\nContent-Disposition: form-data; name=\"a\"\n\n1\n--rbrowser-part--\n"
    ));
}

#[test]
fn form_values_escape_what_would_end_them_and_keep_variables() {
    assert_eq!(
        form_escape("a&b=c d+{{x}}#%"),
        "a%26b%3Dc%20d%2B{{x}}%23%25"
    );
    assert_eq!(
        boundary_of("multipart/form-data; boundary=\"abc\""),
        Some("abc".into())
    );
    assert_eq!(boundary_of("multipart/form-data"), None);
}

#[test]
fn a_check_line_that_does_not_read_names_its_line() {
    let text = "### x\n# @rb-check time 500\nGET https://a.example/\n";
    let error = parse_http(text).expect_err("refused");
    assert_eq!(error.line, 2);
    assert_eq!(error.reason, "check time");
}

#[test]
fn checks_are_held_to_their_form_and_count() {
    let mut request = ApiRequest::new("GET", "https://a.example/");
    request.checks = vec![Check::Time { below_ms: 0 }];
    assert_eq!(request.check(), Err(crate::request::Invalid::Check(1)));
    request.checks = vec![Check::Time { below_ms: 5 }; crate::checks::MAX_CHECKS + 1];
    assert!(request.check().is_err());
    request.checks.truncate(crate::checks::MAX_CHECKS);
    assert_eq!(request.check(), Ok(()));
}

/// The w31 security review: a text part whose value reads as a file include (`< ../.env`) is
/// written so that neither this parser nor REST Client reads it back as a file.
#[test]
fn a_text_part_never_reads_back_as_a_file_include() {
    for value in [
        "< ../../../.env",
        "  < ./secret.txt",
        "<@ ./secret.txt",
        "<@latin1 ./secret.txt",
        "<./secret.txt",
        "first line\n< ../.env",
    ] {
        let mut request = ApiRequest::new("POST", "https://api.example.test/upload");
        request.body = Body::Multipart {
            parts: vec![Part {
                name: "note".into(),
                value: value.into(),
                file: None,
                content_type: None,
            }],
        };
        let text = to_http(&request);
        assert!(!text.lines().any(is_include_line), "{value:?}: {text}");
        let back = parse_http(&text).expect("reads back");
        let Body::Multipart { parts } = back.body else {
            panic!("a multipart body: {text}");
        };
        assert_eq!(parts[0].file, None, "{value:?}: {text}");
    }
}

#[test]
fn include_lines_are_told_from_markup_and_text() {
    for line in [
        "< ./a",
        "<\t./a",
        "  <@ ./a",
        "<@utf8 ./a",
        "<./a",
        "<../a",
        "<~/a",
        "<\\a",
    ] {
        assert!(is_include_line(line), "{line:?}");
    }
    for line in [
        "<div>",
        "</div>",
        "<?xml version=\"1.0\"?>",
        "<!-- c -->",
        "a < b",
        "<",
        "<@",
    ] {
        assert!(!is_include_line(line), "{line:?}");
    }
    assert!(holds_include("a\n< ./b"));
    assert!(!holds_include("<p>a</p>\nb"));
}

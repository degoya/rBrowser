use rb_api_model::{Auth, Body};

use crate::{Import, Source, import};

fn read(text: &str) -> Import {
    import(&[Source::new("", text)], None).expect("imports")
}

#[test]
fn words_follow_the_shells_quoting() {
    assert_eq!(
        super::words("curl 'a b' \"c \\\"d\\\"\" $'e\\'f\\n' g\\ h \\\n i").expect("reads"),
        ["curl", "a b", "c \"d\"", "e'f\n", "g h", "i"]
    );
    assert_eq!(
        super::words("curl ^\"a^\" ^\n -H ^\"b: ^\\^\"c^\\^\"^\"").expect("reads"),
        ["curl", "a", "-H", "b: \"c\""]
    );
    assert_eq!(
        super::words("curl.exe \"a\" `\n -k").expect("reads"),
        ["curl.exe", "a", "-k"]
    );
    assert!(super::words("curl 'open").is_err());
}

#[test]
fn a_copy_as_curl_from_bash_reads_with_its_headers_cookie_and_body() {
    let import = read(include_str!("../fixtures/curl/chrome-bash.txt"));
    let request = &import.requests[0].request;
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "https://api.example.com/v1/items?sort=new");
    assert_eq!(request.name, "POST /v1/items");
    assert_eq!(
        request.header("authorization"),
        Some("Bearer {{$secret token}}")
    );
    assert_eq!(request.header("x-note"), Some("it's fine"));
    assert_eq!(request.header("Cookie"), Some("{{$secret cookie}}"));
    assert!(matches!(&request.body, Body::Json { text } if text.contains("Lamp")));
    let text = serde_json::to_string(&import).expect("serializes");
    assert!(!text.contains("tok.en.value") && !text.contains("s3ss10n"));
}

#[test]
fn a_copy_as_curl_from_cmd_reads_through_its_carets() {
    let import = read(include_str!("../fixtures/curl/chrome-cmd.txt"));
    let request = &import.requests[0].request;
    assert_eq!(request.url, "https://api.example.com/v1/items");
    assert_eq!(
        request.body,
        Body::Json {
            text: "{\"name\":\"Lamp\"}".into()
        }
    );
}

#[test]
fn powershell_user_insecure_and_timeout_are_mapped() {
    let import = read(include_str!("../fixtures/curl/powershell.txt"));
    let request = &import.requests[0].request;
    assert_eq!(request.method, "PUT");
    assert_eq!(
        request.auth,
        Auth::Basic {
            username: "admin".into(),
            password: "{{$secret password}}".into()
        }
    );
    assert_eq!(
        request.body,
        Body::Raw {
            text: "hello".into()
        }
    );
    assert!(!request.options.verify_tls);
    assert_eq!(request.options.timeout_ms, 5000);
}

#[test]
fn form_parts_and_unknown_options_are_reported() {
    let import = read(include_str!("../fixtures/curl/form.txt"));
    let request = &import.requests[0].request;
    assert_eq!(request.method, "POST");
    let Body::Multipart { parts } = &request.body else {
        panic!("multipart")
    };
    assert_eq!(parts[0].value, "Report");
    assert_eq!(parts[1].file.as_deref(), Some("./report.pdf"));
    assert_eq!(parts[1].content_type.as_deref(), Some("application/pdf"));
    assert_eq!(import.report.of("option")[0].detail, "--proxy");
    assert_eq!(import.report.of("file")[0].detail, "report.pdf");
}

#[test]
fn get_data_goes_into_the_query_and_form_data_into_fields() {
    let import = read("curl -G https://a.example/s -d q=lamp -d page=2");
    assert_eq!(
        import.requests[0].request.url,
        "https://a.example/s?q=lamp&page=2"
    );
    assert_eq!(import.requests[0].request.method, "GET");
    let import = read("curl https://a.example/login -d 'user=ada&password=x1'");
    let Body::Form { fields } = &import.requests[0].request.body else {
        panic!("form")
    };
    assert_eq!(fields[1].value, "{{$secret password}}");
}

#[test]
fn no_url_is_no_request() {
    let error = import(&[Source::new("", "curl -X POST")], None).expect_err("refused");
    assert!(matches!(error, crate::ImportError::Unreadable { .. }));
}

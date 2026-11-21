//! The w31 security review of the capture (RB-096-08): no file include, no `{{...}}` of the
//! page, no secret in the path, the header names, the JSON values, the folder per host.

use rb_api_model::{Body, parse_http, to_http};

use super::*;

fn captured(method: &str, url: &str, headers: &[(&str, &str)], body: Option<&str>) -> Captured {
    Captured {
        method: method.into(),
        url: url.into(),
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: body.map(str::to_owned),
        body_unread: false,
    }
}

fn post(content_type: &str, body: &str) -> Result<Converted, Refused> {
    convert(&captured(
        "POST",
        "https://api.example.test/v1/upload",
        &[("Content-Type", content_type)],
        Some(body),
    ))
}

fn value_of<'a>(converted: &'a Converted, name: &str) -> Option<Option<&'a str>> {
    converted
        .variables
        .iter()
        .find(|variable| variable.name == name)
        .map(|variable| variable.value.as_deref())
}

/// HIGH 1: a text part `< ../../../.env` would be sent as the project's file.
#[test]
fn a_part_or_body_line_that_reads_as_a_file_include_is_left_out() {
    let multipart =
        "--b\r\nContent-Disposition: form-data; name=\"note\"\r\n\r\n< ../../../.env\r\n--b--\r\n";
    let converted = post("multipart/form-data; boundary=b", multipart).expect("converts");
    assert_eq!(converted.request.body, Body::None);
    assert_eq!(converted.body_missing, Some(BodyMissing::Unchecked));
    for body in ["< ./.env", "text\n<@ ../secrets.json", "<./id_rsa"] {
        let converted = post("text/plain", body).expect("converts");
        assert_eq!(converted.request.body, Body::None, "{body:?}");
        assert_eq!(converted.body_missing, Some(BodyMissing::Unchecked));
    }
    let form = post("application/x-www-form-urlencoded", "%3C+.%2F.env=1").expect("converts");
    assert_eq!(form.body_missing, Some(BodyMissing::Unchecked));
    let markup = post("text/xml", "<a>\n</a>").expect("converts");
    assert_eq!(markup.body_missing, None);
}

#[test]
fn a_converted_request_never_reads_back_with_a_file() {
    for value in ["< ../../../.env", "<@ ./x", "ok", "two\n< ./lines"] {
        let multipart = format!(
            "--b\r\nContent-Disposition: form-data; name=\"note\"\r\n\r\n{value}\r\n--b--\r\n"
        );
        let converted = post("multipart/form-data; boundary=b", &multipart).expect("converts");
        let back = parse_http(&to_http(&converted.request)).expect("reads back");
        if let Body::Multipart { parts } = back.body {
            assert!(parts.iter().all(|part| part.file.is_none()), "{value:?}");
        }
    }
}

/// HIGH 2: a `{{...}}` of the page would resolve a secret or another host's value on send.
#[test]
fn a_request_with_a_template_of_the_page_is_refused() {
    let refused = |one: Captured| convert(&one).err();
    let url = "https://api.example.test/v1?q={{$secret other-token}}";
    assert_eq!(
        refused(captured("GET", url, &[], None)),
        Some(Refused::Template)
    );
    let header = [("X-Note", "{{$processEnv RB_SECRET_DEPLOY}}")];
    assert_eq!(
        refused(captured("GET", "https://api.example.test/", &header, None)),
        Some(Refused::Template)
    );
    let cookie = [("Cookie", "sid={{$secret other-token}}")];
    assert_eq!(
        refused(captured("GET", "https://api.example.test/", &cookie, None)),
        Some(Refused::Template)
    );
    assert_eq!(
        post("application/json", r#"{"q":"{{shop-example-test-token}}"}"#).err(),
        Some(Refused::Template)
    );
    assert_eq!(
        post(
            "application/x-www-form-urlencoded",
            "q=%7B%7B%24secret%20other%7D%7D"
        )
        .err(),
        Some(Refused::Template)
    );
    let multipart =
        "--b\r\nContent-Disposition: form-data; name=\"q\"\r\n\r\n{{baseUrl}}\r\n--b--\r\n";
    assert_eq!(
        post("multipart/form-data; boundary=b", multipart).err(),
        Some(Refused::Template)
    );
    assert_eq!(
        post(
            "application/json",
            r#"{"password":"pw-123456","q":"{{$secret x}}"}"#
        )
        .err(),
        Some(Refused::Template)
    );
    assert_eq!(Refused::Template.id(), "template");
    assert_eq!(Refused::NotHttp.id(), "notHttp");
}

#[test]
fn an_encoded_template_stays_encoded_and_never_resolves() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1?q=%7B%7B%24secret%20x%7D%7D",
        &[],
        None,
    ))
    .expect("an encoded template is plain text");
    assert!(!converted.request.url.contains("{{"));
    assert!(rb_api_model::vars::references(&converted.request.url).is_empty());
    let json = post(
        "application/json",
        r#"{"q":"\u007b\u007b$secret x\u007d\u007d"}"#,
    )
    .expect("a JSON escape is kept as written");
    let Body::Json { text } = &json.request.body else {
        panic!("a JSON body");
    };
    assert!(!text.contains("{{"), "{text}");
}

#[test]
fn the_variables_of_the_capture_itself_pass() {
    let converted = convert(&captured(
        "GET",
        "https://api.example.test/v1?token=tok-123456",
        &[("Authorization", "Bearer tok-123456")],
        None,
    ))
    .expect("its own variables are no template of the page");
    assert!(converted.request.url.contains("{{api-example-test-token}}"));
}

/// MEDIUM 3: a token in the path lands in the URL, the title and the file name.
#[test]
fn a_secret_path_segment_becomes_a_variable_in_the_url_title_and_path() {
    let link = "aB3dE5fG7hI9jK1lM3nO";
    let converted = convert(&captured(
        "POST",
        &format!("https://api.example.test/reset/{link}"),
        &[],
        None,
    ))
    .expect("converts");
    assert_eq!(
        converted.request.url,
        "https://api.example.test/reset/{{api-example-test-reset}}"
    );
    assert_eq!(
        converted.request.name,
        "POST /reset/{{api-example-test-reset}}"
    );
    assert_eq!(converted.path, "/reset/{{api-example-test-reset}}");
    assert_eq!(
        value_of(&converted, "api-example-test-reset"),
        Some(Some(link))
    );
    let invite =
        convert(&captured("GET", "https://a.test/invite/x7k2m9", &[], None)).expect("converts");
    assert_eq!(
        invite.request.url,
        "https://a.test/invite/{{a-test-invite}}"
    );
    let bot = convert(&captured(
        "POST",
        "https://api.telegram.test/bot123456789:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw/sendMessage",
        &[],
        None,
    ))
    .expect("converts");
    assert_eq!(
        bot.request.url,
        "https://api.telegram.test/{{api-telegram-test-path}}/sendMessage"
    );
    let hook = convert(&captured(
        "POST",
        "https://chat.test/api/webhooks/1234567890/k3j9x0q2m8z7w1v5b6n4",
        &[],
        None,
    ))
    .expect("converts");
    assert_eq!(
        hook.request.url,
        "https://chat.test/api/webhooks/{{chat-test-webhooks}}/{{chat-test-path}}"
    );
    let hash = convert(&captured(
        "GET",
        "https://a.test/files/9f86d081884c7d659a2feaa0c55ad015a3bf4f1b",
        &[],
        None,
    ))
    .expect("converts");
    assert_eq!(hash.request.url, "https://a.test/files/{{a-test-files}}");
}

#[test]
fn ordinary_path_segments_stay() {
    for url in [
        "https://a.test/v1/users/42",
        "https://a.test/blog/this-is-a-long-article-slug-2024",
        "https://a.test/oauth/token/refresh",
        "https://a.test/2024-11-21-release-notes-v1",
        "https://a.test/auth/login",
    ] {
        let converted = convert(&captured("GET", url, &[], None)).expect("converts");
        assert_eq!(converted.request.url, url);
        assert!(converted.variables.is_empty(), "{url}");
    }
}

#[test]
fn the_referer_path_keeps_no_token_either() {
    let converted = convert(&captured(
        "GET",
        "https://a.test/v1/me",
        &[("Referer", "https://a.test/reset/aB3dE5fG7hI9jK1lM3nO")],
        None,
    ))
    .expect("converts");
    let referer = converted
        .request
        .headers
        .iter()
        .find(|header| header.name == "Referer")
        .map(|header| header.value.as_str());
    assert_eq!(referer, Some("https://a.test/reset/{{a-test-reset}}"));
}

/// MEDIUM 4: session and auth headers by other names.
#[test]
fn session_and_auth_headers_are_secret_by_their_bare_name() {
    for name in [
        "X-Session",
        "X-Auth",
        "X-Sid",
        "X-Session-Id",
        "x-session-id",
        "X-Token-Id",
        "X-Auth-Id",
        "Session",
    ] {
        assert!(is_secret_header(name), "{name}");
    }
    for name in [
        "X-Request-Id",
        "X-Correlation-Id",
        "X-Client-Type",
        "Content-Type",
        "Accept",
        "X-Debug",
    ] {
        assert!(!is_secret_header(name), "{name}");
    }
}

/// MEDIUM 5: a number or a list under a secret's name, a secret inside another string.
#[test]
fn every_value_under_a_secret_name_becomes_a_variable_with_its_type() {
    let converted = post(
        "application/json",
        r#"{"user":"ada","password":12345678,"auth":["u","pw-1"],"pin":null}"#,
    )
    .expect("converts");
    let Body::Json { text } = &converted.request.body else {
        panic!("a JSON body");
    };
    assert!(
        text.contains("\"password\": {{api-example-test-password}}"),
        "{text}"
    );
    assert!(
        text.contains("\"auth\": {{api-example-test-auth}}"),
        "{text}"
    );
    assert!(
        !text.contains("12345678") && !text.contains("pw-1"),
        "{text}"
    );
    assert_eq!(
        value_of(&converted, "api-example-test-password"),
        Some(Some("12345678"))
    );
    assert_eq!(
        value_of(&converted, "api-example-test-auth"),
        Some(Some(r#"["u","pw-1"]"#))
    );
}

#[test]
fn a_string_that_names_a_secret_inside_leaves_the_body_out() {
    let converted = post(
        "application/json",
        r#"{"query":"mutation { login(user: \"ada\", password: \"pw-123456\") { id } }"}"#,
    )
    .expect("converts");
    assert_eq!(converted.request.body, Body::None);
    assert_eq!(converted.body_missing, Some(BodyMissing::Unchecked));
}

/// LOW 6: two hosts never share a folder or a variable prefix.
#[test]
fn hosts_that_differ_only_in_dots_and_dashes_get_their_own_folder() {
    assert_eq!(folder_of("api.example.test"), "api-example-test");
    assert_eq!(folder_of("API.example.test:8443"), "api-example-test-8443");
    let dashed = folder_of("api-example.test");
    assert_ne!(dashed, "api-example-test");
    assert!(dashed.starts_with("api-example-test-"), "{dashed}");
    assert_eq!(dashed, folder_of("api-example.test"));
    assert_ne!(folder_of("[::1]:3000"), folder_of("[::1:3000]"));
    let long = |end: &str| format!("{}{end}.test", "a".repeat(90));
    assert_ne!(folder_of(&long("x")), folder_of(&long("y")));
    assert!(folder_of(&long("x")).len() <= 80);
}

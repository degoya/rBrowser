use serde_json::json;

use super::*;

fn request(view: u32, stage: Stage) -> HeldRequest {
    HeldRequest {
        id: String::new(),
        key: String::new(),
        view: ViewId(view),
        session: format!("S{view}"),
        request_id: format!("interception-{view}"),
        stage,
        method: "POST".to_owned(),
        url: "https://a.test/api/user?token=abc".to_owned(),
        headers: vec![
            ("Cookie".to_owned(), "sid=1".to_owned()),
            ("Accept".to_owned(), "*/*".to_owned()),
        ],
        body: Some(b"{\"a\":1}".to_vec()),
        body_truncated: false,
        resource_type: "XHR".to_owned(),
        breakpoint: "bp-1".to_owned(),
        since: 1_700_000_000_000,
        response: (stage == Stage::Response).then(|| HeldResponse {
            status: 200,
            headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        }),
    }
}

#[test]
fn holding_numbers_the_requests_and_keys_them_by_stage_method_and_url() {
    let held = Held::default();
    let (first, _rx1) = held.hold(request(1, Stage::Request));
    let (second, _rx2) = held.hold(request(2, Stage::Request));
    let (third, _rx3) = held.hold(request(1, Stage::Response));
    assert_eq!(
        (first.as_str(), second.as_str(), third.as_str()),
        ("h1", "h2", "h3")
    );
    let list = held.list();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].key, list[1].key, "the same request in two views");
    assert_ne!(list[0].key, list[2].key, "another stage");
    assert_eq!(
        list[0].key,
        "request POST https://a.test/api/user?token=abc"
    );
}

#[tokio::test]
async fn resolving_one_or_all_views_reaches_the_waiting_tasks() {
    let held = Held::default();
    let (first, rx1) = held.hold(request(1, Stage::Request));
    let (_second, rx2) = held.hold(request(2, Stage::Request));
    let (_third, rx3) = held.hold(request(3, Stage::Response));
    assert_eq!(
        held.resolve("nope", false, Resolution::Abort),
        Err(HeldError::Unknown)
    );
    let edits = Edits {
        status: Some(500),
        ..Edits::default()
    };
    assert_eq!(
        held.resolve(&first, true, Resolution::Continue(edits.clone())),
        Ok(2)
    );
    assert!(matches!(rx1.await, Ok(Resolution::Continue(e)) if e == edits));
    assert!(matches!(rx2.await, Ok(Resolution::Continue(_))));
    assert_eq!(held.len(), 1, "the response-stage one is another key");
    held.clear();
    assert!(rx3.await.is_err(), "clearing drops the deciders");
    assert_eq!(held.len(), 0);
}

#[test]
fn the_state_entry_shows_the_body_as_text_and_masks_nothing_for_the_person() {
    let held = Held::default();
    let (_, _rx) = held.hold(request(1, Stage::Response));
    let entry = held.list()[0].to_json(1024);
    assert_eq!(entry["id"], "h1");
    assert_eq!(entry["view"], 1);
    assert_eq!(entry["stage"], "response");
    assert_eq!(entry["body"], "{\"a\":1}");
    assert_eq!(entry["bodyBase64"], false);
    assert_eq!(entry["headers"][0], json!(["Cookie", "sid=1"]));
    assert_eq!(entry["response"]["status"], 200);
    let mut binary = request(1, Stage::Request);
    binary.body = Some(vec![0xff, 0xfe, 0x00]);
    let (_, _rx) = held.hold(binary);
    let entry = held.list()[1].to_json(1024);
    assert_eq!(entry["bodyBase64"], true);
    assert_eq!(entry["body"], "//4A");
    let entry = held.list()[0].to_json(3);
    assert_eq!(entry["bodyTruncated"], true);
    assert_eq!(entry["body"], "{\"a");
}

#[test]
fn the_cdp_answers_of_a_continue_follow_the_stage_and_the_edits() {
    let plain = request(1, Stage::Request);
    assert_eq!(
        continue_params(&plain, &Edits::default()),
        (
            "Fetch.continueRequest",
            json!({ "requestId": "interception-1" })
        )
    );
    let edits = Edits {
        method: Some("PUT".to_owned()),
        url: Some("https://a.test/other".to_owned()),
        headers: Some(vec![("x-a".to_owned(), "1".to_owned())]),
        body: Some(b"hi".to_vec()),
        status: None,
    };
    assert_eq!(
        continue_params(&plain, &edits),
        (
            "Fetch.continueRequest",
            json!({ "requestId": "interception-1", "method": "PUT", "url": "https://a.test/other",
                    "headers": [{ "name": "x-a", "value": "1" }], "postData": "aGk=" })
        )
    );
    let response = request(1, Stage::Response);
    assert_eq!(
        continue_params(&response, &Edits::default()),
        (
            "Fetch.continueResponse",
            json!({ "requestId": "interception-1" })
        )
    );
    let status_only = Edits {
        status: Some(500),
        ..Edits::default()
    };
    assert_eq!(
        continue_params(&response, &status_only),
        (
            "Fetch.continueResponse",
            json!({ "requestId": "interception-1", "responseCode": 500 })
        )
    );
    let with_body = Edits {
        body: Some(b"{}".to_vec()),
        ..Edits::default()
    };
    assert_eq!(
        continue_params(&response, &with_body),
        (
            "Fetch.fulfillRequest",
            json!({ "requestId": "interception-1", "responseCode": 200,
                    "responseHeaders": [{ "name": "content-type", "value": "application/json" }],
                    "body": "e30=" })
        )
    );
    assert_eq!(
        abort_params(&plain),
        (
            "Fetch.failRequest",
            json!({ "requestId": "interception-1", "errorReason": "Aborted" })
        )
    );
}

#[test]
fn masked_values_sent_back_mean_as_they_were() {
    let held = request(1, Stage::Request);
    let edits = Edits {
        url: Some("https://a.test/api/user?token=***&x=1".to_owned()),
        headers: Some(vec![
            ("cookie".to_owned(), "***".to_owned()),
            ("Accept".to_owned(), "text/html".to_owned()),
            ("X-New".to_owned(), "***".to_owned()),
        ]),
        ..Edits::default()
    };
    let unmasked = unmask_edits(&held, &edits);
    assert_eq!(
        unmasked.url.as_deref(),
        Some("https://a.test/api/user?token=abc&x=1")
    );
    assert_eq!(
        unmasked.headers,
        Some(vec![
            ("cookie".to_owned(), "sid=1".to_owned()),
            ("Accept".to_owned(), "text/html".to_owned()),
            ("X-New".to_owned(), "***".to_owned()),
        ])
    );
    let response = request(1, Stage::Response);
    let edits = Edits {
        headers: Some(vec![("content-type".to_owned(), "***".to_owned())]),
        ..Edits::default()
    };
    assert_eq!(
        unmask_edits(&response, &edits).headers,
        Some(vec![(
            "content-type".to_owned(),
            "application/json".to_owned()
        )]),
        "at the response stage the response's headers are the originals"
    );
}

#[test]
fn an_edited_response_body_drops_the_encoding_and_length_of_the_original() {
    let mut held = request(1, Stage::Response);
    if let Some(response) = &mut held.response {
        response.headers.extend([
            ("Content-Encoding".to_owned(), "gzip".to_owned()),
            ("content-length".to_owned(), "9".to_owned()),
            ("Transfer-Encoding".to_owned(), "chunked".to_owned()),
        ]);
    }
    let edits = Edits {
        body: Some(b"{}".to_vec()),
        ..Edits::default()
    };
    let (method, params) = continue_params(&held, &edits);
    assert_eq!(method, "Fetch.fulfillRequest");
    assert_eq!(
        params["responseHeaders"],
        json!([{ "name": "content-type", "value": "application/json" }])
    );
    let headers_only = Edits {
        headers: Some(vec![("x-a".to_owned(), "1".to_owned())]),
        ..Edits::default()
    };
    let (method, _) = continue_params(&held, &headers_only);
    assert_eq!(
        method, "Fetch.continueResponse",
        "no body: the headers go on as sent"
    );
}

#[test]
fn repeated_header_names_unmask_by_position() {
    let mut held = request(1, Stage::Response);
    if let Some(response) = &mut held.response {
        response.headers = vec![
            ("set-cookie".to_owned(), "a=1".to_owned()),
            ("Set-Cookie".to_owned(), "b=2".to_owned()),
        ];
    }
    let edits = Edits {
        headers: Some(vec![
            ("Set-Cookie".to_owned(), "***".to_owned()),
            ("set-cookie".to_owned(), "***".to_owned()),
            ("set-cookie".to_owned(), "***".to_owned()),
        ]),
        ..Edits::default()
    };
    assert_eq!(
        unmask_edits(&held, &edits).headers,
        Some(vec![
            ("Set-Cookie".to_owned(), "a=1".to_owned()),
            ("set-cookie".to_owned(), "b=2".to_owned()),
            ("set-cookie".to_owned(), "***".to_owned()),
        ]),
        "the third has no original and stays as sent"
    );
}

#[test]
fn edits_to_another_origin_unmask_nothing() {
    let held = request(1, Stage::Request);
    let edits = Edits {
        url: Some("https://evil.test/c?token=***".to_owned()),
        headers: Some(vec![("Cookie".to_owned(), "***".to_owned())]),
        ..Edits::default()
    };
    assert_eq!(unmask_edits(&held, &edits), edits);
}

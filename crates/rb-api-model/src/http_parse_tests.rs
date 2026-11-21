use super::*;

#[test]
fn a_file_another_tool_wrote_reads() {
    let text = "# @name getUser\n\
                // a note\n\
                https://api.example.com/users\n\
                    ?page=2\n\
                    &size=10\n\
                Accept: application/json\n\
                # a header comment\n\
                \n";
    let request = parse_http(text).expect("reads");
    assert_eq!(request.name, "getUser");
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, "https://api.example.com/users?page=2&size=10");
    assert_eq!(request.headers, [Entry::new("Accept", "application/json")]);
    assert_eq!(request.comments, ["a note", "a header comment"]);
    assert_eq!(request.body, Body::None);
}

#[test]
fn the_body_kind_is_inferred_without_metadata() {
    let json = parse_http("POST https://a.example\nContent-Type: application/vnd.api+json\n\n{}\n")
        .expect("reads");
    assert_eq!(json.body, Body::Json { text: "{}".into() });
    let form = parse_http(
        "POST https://a.example\nContent-Type: application/x-www-form-urlencoded\n\na=1\n&b=x+y\n",
    )
    .expect("reads");
    assert_eq!(
        form.body,
        Body::Form {
            fields: vec![Entry::new("a", "1"), Entry::new("b", "x y")]
        }
    );
    let binary = parse_http("PUT https://a.example\n\n< ./file.bin\n").expect("reads");
    assert_eq!(
        binary.body,
        Body::Binary {
            path: "./file.bin".into()
        }
    );
    let graphql =
        parse_http("POST https://a.example/gql\nX-REQUEST-TYPE: GraphQL\n\n{ me { id } }\n")
            .expect("reads");
    assert_eq!(
        graphql.body,
        Body::GraphQl {
            query: "{ me { id } }".into(),
            variables: String::new()
        }
    );
    assert!(graphql.headers.is_empty());
    let raw = parse_http("POST https://a.example\n\nhello\n").expect("reads");
    assert_eq!(
        raw.body,
        Body::Raw {
            text: "hello".into()
        }
    );
}

#[test]
fn an_authorization_header_is_auth_only_under_its_metadata() {
    let plain = parse_http("GET https://a.example\nAuthorization: Bearer abc\n").expect("reads");
    assert_eq!(plain.auth, Auth::None);
    assert_eq!(plain.headers.len(), 1);
    let basic =
        parse_http("# @rb-auth basic\nGET https://a.example\nAuthorization: Basic ada lovelace\n")
            .expect("reads");
    assert_eq!(
        basic.auth,
        Auth::Basic {
            username: "ada".into(),
            password: "lovelace".into()
        }
    );
    assert!(basic.headers.is_empty());
}

#[test]
fn what_does_not_read_names_its_line() {
    assert_eq!(
        parse_http("# only a comment\n"),
        Err(ParseError {
            line: 0,
            reason: "no request line"
        })
    );
    assert_eq!(
        parse_http("GET https://a.example\nnot a header\n"),
        Err(ParseError {
            line: 2,
            reason: "header"
        })
    );
    assert_eq!(
        parse_http("# @rb-timeout-ms soon\nGET https://a.example\n"),
        Err(ParseError {
            line: 1,
            reason: "timeout"
        })
    );
    assert_eq!(
        parse_http("# @rb-body yaml\nPOST https://a.example\n\nx\n"),
        Err(ParseError {
            line: 0,
            reason: "body kind"
        })
    );
    assert!(parse_http("# @rb-auth {not json}\nGET https://a.example\n").is_err());
    assert!(parse_http("G(T https://a.example\n").is_err());
}

#[test]
fn a_file_of_several_requests_is_cut_for_the_importers_and_refused_as_one() {
    let text = "### One\nGET https://a.example/1\n\n### Two\nGET https://a.example/2\n###\n";
    let requests = split_requests(text);
    assert_eq!(requests.len(), 2);
    assert_eq!(parse_http(&requests[1]).expect("reads").name, "Two");
    assert_eq!(
        parse_http(text),
        Err(ParseError {
            line: 0,
            reason: "more than one request"
        })
    );
}

#[test]
fn a_broken_escape_stays_as_written() {
    assert_eq!(form_unescape("100%"), "100%");
    assert_eq!(form_unescape("%zz%41"), "%zzA");
}

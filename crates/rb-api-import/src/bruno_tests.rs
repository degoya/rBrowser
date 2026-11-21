use rb_api_model::{Auth, Body, KeyPlace};

use crate::{Import, Imported, Source, import};

fn library() -> Import {
    let files = [
        (
            "library/bruno.json",
            include_str!("../fixtures/bruno/bruno.json"),
        ),
        (
            "library/collection.bru",
            include_str!("../fixtures/bruno/collection.bru"),
        ),
        (
            "library/users/folder.bru",
            include_str!("../fixtures/bruno/users/folder.bru"),
        ),
        (
            "library/users/list-users.bru",
            include_str!("../fixtures/bruno/users/list-users.bru"),
        ),
        (
            "library/users/create-user.bru",
            include_str!("../fixtures/bruno/users/create-user.bru"),
        ),
        (
            "library/search.bru",
            include_str!("../fixtures/bruno/search.bru"),
        ),
        (
            "library/upload.bru",
            include_str!("../fixtures/bruno/upload.bru"),
        ),
        (
            "library/environments/Local.bru",
            include_str!("../fixtures/bruno/environments/Local.bru"),
        ),
        ("library/.env", "TOKEN=abc"),
    ];
    let sources: Vec<Source> = files
        .iter()
        .map(|(name, text)| Source::new(*name, *text))
        .collect();
    import(&sources, None).expect("imports")
}

fn find<'a>(import: &'a Import, name: &str) -> &'a Imported {
    import
        .requests
        .iter()
        .find(|r| r.request.name == name)
        .unwrap_or_else(|| panic!("no request {name}"))
}

#[test]
fn blocks_read_as_dictionaries_texts_and_lists() {
    let blocks = super::blocks(
        "meta {\n  name: A: b\n  ~off: 1\n}\n\nbody:json {\n  {\n    \"a\": 1\n  }\n}\n\nvars:secret [\n  x,\n  y\n]\n",
    );
    assert_eq!(blocks.len(), 3);
    assert_eq!(
        blocks[0].dict,
        [
            ("name".into(), "A: b".into(), true),
            ("off".into(), "1".into(), false)
        ]
    );
    assert_eq!(blocks[1].text, "{\n  \"a\": 1\n}");
    assert_eq!(blocks[2].list, ["x", "y"]);
}

#[test]
fn a_collection_folder_reads_with_its_folders_and_inherited_auth() {
    let import = library();
    assert_eq!(import.collection.as_deref(), Some("Library"));
    assert_eq!(import.requests.len(), 4);
    let list = find(&import, "List users");
    assert_eq!(list.folder, "users");
    assert_eq!(list.request.url, "{{baseUrl}}/users?page=1");
    assert_eq!(
        list.request.auth,
        Auth::Bearer {
            token: "{{token}}".into()
        }
    );
    let headers: Vec<(&str, bool)> = list
        .request
        .headers
        .iter()
        .map(|h| (h.name.as_str(), h.enabled))
        .collect();
    assert_eq!(
        headers,
        [("X-Client", true), ("Accept", true), ("X-Trace", false)]
    );
    assert_eq!(list.request.comments, ["Every user of the library."]);
    assert_eq!(find(&import, "Search").folder, "");
}

#[test]
fn bodies_and_auth_kinds_are_mapped() {
    let import = library();
    let create = &find(&import, "Create user").request;
    assert_eq!(create.method, "POST");
    assert_eq!(
        create.body,
        Body::Json {
            text: "{\n  \"name\": \"Ada\",\n  \"roles\": [\"reader\"]\n}".into()
        }
    );
    assert_eq!(
        create.auth,
        Auth::Basic {
            username: "librarian".into(),
            password: "{{$secret password}}".into()
        }
    );
    let Body::GraphQl { query, variables } = &find(&import, "Search").request.body else {
        panic!("graphql")
    };
    assert!(query.starts_with("query {"));
    assert!(variables.contains("\"first\": 2"));
    let upload = &find(&import, "Upload cover").request;
    assert_eq!(upload.method, "PUT");
    let Body::Multipart { parts } = &upload.body else {
        panic!("multipart")
    };
    assert_eq!(parts[0].value, "Cover");
    assert_eq!(parts[1].file.as_deref(), Some("./front.png"));
    assert_eq!(
        upload.auth,
        Auth::ApiKey {
            name: "X-Api-Key".into(),
            value: "{{$secret x-api-key}}".into(),
            place: KeyPlace::Header
        }
    );
}

#[test]
fn the_environment_keeps_secret_names_as_placeholders() {
    let import = library();
    let local = &import.environments[0];
    assert_eq!(local.name, "Local");
    let pairs: Vec<(&str, &str)> = local
        .variables
        .iter()
        .map(|v| (v.name.as_str(), v.value.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("baseUrl", "http://localhost:3000"),
            ("token", "{{$secret token}}"),
            ("apiSecret", "{{$secret apisecret}}"),
        ]
    );
}

#[test]
fn scripts_tests_and_the_rest_are_reported() {
    let report = library().report;
    let of = |code: &str| -> Vec<(String, String)> {
        report
            .of(code)
            .iter()
            .map(|n| (n.at.clone(), n.detail.clone()))
            .collect()
    };
    assert!(of("script").contains(&("Library".into(), "pre-request".into())));
    assert!(of("script").contains(&("Create user".into(), "post-response".into())));
    assert_eq!(
        of("tests"),
        [
            ("List users".into(), "assert".into()),
            ("List users".into(), "tests".into())
        ]
    );
    assert_eq!(
        of("variable"),
        [("Search".into(), "vars:pre-request".into())]
    );
    assert!(of("disabled").contains(&("List users".into(), "query debug".into())));
    assert_eq!(of("item"), [(".env".into(), "not read".into())]);
    let text = serde_json::to_string(&library()).expect("serializes");
    assert!(!text.contains("b00ks") && !text.contains("lib-key-1") && !text.contains("abc"));
}

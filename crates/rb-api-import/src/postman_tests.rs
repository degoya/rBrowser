use rb_api_model::{Auth, Body, KeyPlace};

use crate::{Format, Source, import};

fn find<'a>(import: &'a crate::Import, name: &str) -> &'a crate::Imported {
    import
        .requests
        .iter()
        .find(|r| r.request.name == name)
        .unwrap_or_else(|| panic!("no request {name}"))
}

fn shop() -> crate::Import {
    import(
        &[Source::new(
            "shop.postman_collection.json",
            include_str!("../fixtures/postman/collection.json"),
        )],
        None,
    )
    .expect("imports")
}

#[test]
fn a_collection_keeps_its_folders_urls_headers_and_inherited_auth() {
    let import = shop();
    assert_eq!(import.formats, [Format::Postman]);
    assert_eq!(import.collection.as_deref(), Some("Shop API"));
    assert_eq!(import.requests.len(), 6);
    let list = find(&import, "List users");
    assert_eq!(list.folder, "users");
    assert_eq!(list.request.method, "GET");
    assert_eq!(
        list.request.url,
        "{{baseUrl}}/{{apiVersion}}/users?page=1&limit=20"
    );
    assert_eq!(list.request.headers.len(), 2);
    assert!(list.request.headers[0].enabled);
    assert!(!list.request.headers[1].enabled);
    assert_eq!(
        list.request.auth,
        Auth::Bearer {
            token: "{{token}}".into()
        }
    );
    assert_eq!(list.request.comments, ["Every user, a page at a time."]);
    assert_eq!(
        find(&import, "Get user").request.url,
        "{{baseUrl}}/users/42"
    );
}

#[test]
fn bodies_of_every_mode_are_mapped() {
    let import = shop();
    let create = &find(&import, "Create user").request;
    assert!(matches!(&create.body, Body::Json { text } if text.contains("Ada")));
    let login = &find(&import, "Login").request;
    assert_eq!(login.auth, Auth::None);
    let Body::Form { fields } = &login.body else {
        panic!("form")
    };
    assert_eq!(fields.len(), 3);
    assert!(!fields[2].enabled);
    let Body::Multipart { parts } = &find(&import, "Upload avatar").request.body else {
        panic!("multipart")
    };
    assert_eq!(parts[0].value, "Me");
    assert_eq!(parts[1].file.as_deref(), Some("./avatar.png"));
    let search = &find(&import, "Search").request;
    assert!(
        matches!(&search.body, Body::GraphQl { query, variables } if query.contains("users") && variables.contains("first"))
    );
    assert!(!search.options.follow_redirects);
    assert!(!search.options.verify_tls);
}

#[test]
fn secrets_become_placeholders_and_the_report_says_so() {
    let import = shop();
    let create = &find(&import, "Create user").request;
    assert_eq!(
        create.auth,
        Auth::Basic {
            username: "admin".into(),
            password: "{{$secret password}}".into()
        }
    );
    assert!(
        find(&import, "Search")
            .request
            .url
            .ends_with("api_key={{api-key}}")
    );
    let shared = &import.environments[0];
    assert_eq!(shared.name, "$shared");
    assert_eq!(shared.variables[1].value, "{{$secret adminpassword}}");
    // A URL takes no space: its secret is a variable set to the placeholder.
    assert_eq!(shared.variables[2].name, "api-key");
    assert_eq!(shared.variables[2].value, "{{$secret api-key}}");
    let text = serde_json::to_string(&import).expect("serializes");
    for secret in ["hunter2", "s3cret", "\"pw\"", "k3y"] {
        assert!(!text.contains(secret), "{secret} leaked");
    }
}

#[test]
fn what_rbrowser_lacks_is_reported() {
    let report = shop().report;
    let codes = |code: &str| -> Vec<(String, String)> {
        report
            .of(code)
            .iter()
            .map(|n| (n.at.clone(), n.detail.clone()))
            .collect()
    };
    assert_eq!(codes("script"), [("Shop API".into(), "pre-request".into())]);
    assert_eq!(
        codes("tests"),
        [("List users".into(), "post-response script".into())]
    );
    assert_eq!(codes("examples"), [("List users".into(), "1".into())]);
    assert_eq!(codes("auth"), [("Search".into(), "awsv4".into())]);
    assert_eq!(
        codes("setting"),
        [("Search".into(), "disableCookies".into())]
    );
    assert_eq!(
        codes("file"),
        [("Upload avatar".into(), "avatar.png".into())]
    );
    assert!(codes("disabled").contains(&("List users".into(), "query debug".into())));
}

#[test]
fn a_v20_collection_reads_its_object_auth_and_string_headers() {
    let import = import(
        &[Source::new(
            "old.json",
            include_str!("../fixtures/postman/collection-v20.json"),
        )],
        None,
    )
    .expect("imports");
    let ping = find(&import, "Ping");
    assert_eq!(ping.request.headers.len(), 2);
    assert_eq!(ping.request.headers[1].name, "X-Trace");
    assert!(
        matches!(&ping.request.auth, Auth::Basic { username, password } if username == "u" && password == "{{$secret password}}")
    );
    assert_eq!(
        find(&import, "Key").request.auth,
        Auth::ApiKey {
            name: "X-Key".into(),
            value: "{{key}}".into(),
            place: KeyPlace::Header
        }
    );
}

#[test]
fn environments_and_globals_are_read_beside_a_collection() {
    let import = import(
        &[
            Source::new(
                "staging.postman_environment.json",
                include_str!("../fixtures/postman/environment.json"),
            ),
            Source::new(
                "globals.json",
                include_str!("../fixtures/postman/globals.json"),
            ),
        ],
        None,
    )
    .expect("imports");
    assert!(import.requests.is_empty());
    let staging = &import.environments[0];
    assert_eq!(staging.name, "Staging");
    assert_eq!(staging.variables.len(), 2);
    assert_eq!(staging.variables[1].value, "{{$secret token}}");
    assert_eq!(import.environments[1].name, "$shared");
    assert_eq!(import.report.of("disabled")[0].detail, "region");
    assert_eq!(import.report.secrets[0].variable, "RB_SECRET_TOKEN");
}

use rb_api_model::{Auth, Body, KeyPlace};

use crate::{Import, Imported, Source, import};

fn notes() -> Import {
    import(
        &[Source::new(
            "notes.httpie.json",
            include_str!("../fixtures/httpie/collection.json"),
        )],
        None,
    )
    .expect("imports")
}

fn find<'a>(import: &'a Import, name: &str) -> &'a Imported {
    import
        .requests
        .iter()
        .find(|r| r.request.name == name)
        .unwrap_or_else(|| panic!("no request {name}"))
}

#[test]
fn requests_take_their_query_path_and_inherited_auth() {
    let import = notes();
    assert_eq!(import.collection.as_deref(), Some("Notes"));
    assert_eq!(import.requests.len(), 5);
    let list = &find(&import, "List notes").request;
    assert_eq!(list.url, "{{baseUrl}}/notes?tag=work");
    assert!(list.headers[0].enabled && !list.headers[1].enabled);
    assert_eq!(
        list.auth,
        Auth::Bearer {
            token: "{{token}}".into()
        }
    );
    assert_eq!(find(&import, "Get note").request.url, "{{baseUrl}}/notes/7");
    assert_eq!(find(&import, "Get note").request.auth, Auth::None);
}

#[test]
fn bodies_and_auth_kinds_are_mapped() {
    let import = notes();
    let create = &find(&import, "Create note").request;
    assert!(matches!(&create.body, Body::Json { text } if text.contains("Hi")));
    assert_eq!(
        create.auth,
        Auth::Basic {
            username: "writer".into(),
            password: "{{$secret password}}".into()
        }
    );
    let attach = &find(&import, "Attach").request;
    let Body::Multipart { parts } = &attach.body else {
        panic!("multipart")
    };
    assert_eq!(parts[1].file.as_deref(), Some("./pic.jpg"));
    assert_eq!(
        attach.auth,
        Auth::ApiKey {
            name: "X-Notes-Key".into(),
            value: "{{$secret x-notes-key}}".into(),
            place: KeyPlace::Header
        }
    );
    let query = &find(&import, "Query").request;
    assert!(matches!(&query.body, Body::GraphQl { variables, .. } if variables.contains("\"n\"")));
    assert!(matches!(&query.auth, Auth::Digest { password, .. } if password == "{{pw}}"));
    assert_eq!(import.report.of("disabled")[0].detail, "query debug");
    let text = serde_json::to_string(&import).expect("serializes");
    assert!(!text.contains("n0tes") && !text.contains("nk-9"));
}

#[test]
fn a_workspace_reads_each_collection_as_a_folder() {
    let collection: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/httpie/collection.json")).expect("json");
    let workspace = serde_json::json!({
        "meta": { "format": "httpie", "version": "1.0.0", "contentType": "workspace" },
        "entry": { "name": "Mine", "collections": [collection["entry"]] }
    });
    let import = import(&[Source::new("w.json", workspace.to_string())], None).expect("imports");
    assert_eq!(import.collection.as_deref(), Some("Mine"));
    assert_eq!(find(&import, "List notes").folder, "notes");
}

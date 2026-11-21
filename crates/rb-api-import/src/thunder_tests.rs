use rb_api_model::{Auth, Body};

use crate::{Import, Imported, Source, import};

fn weather() -> Import {
    import(
        &[
            Source::new(
                "thunder-collection_Weather.json",
                include_str!("../fixtures/thunder/collection.json"),
            ),
            Source::new(
                "thunder-environment_Local.json",
                include_str!("../fixtures/thunder/environment.json"),
            ),
        ],
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
fn requests_land_in_their_folders_with_the_inherited_auth_and_headers() {
    let import = weather();
    assert_eq!(import.collection.as_deref(), Some("Weather"));
    assert_eq!(import.requests.len(), 4);
    let today = find(&import, "Today");
    assert_eq!(today.folder, "forecast/daily");
    assert_eq!(today.request.url, "{{baseUrl}}/forecast/today?city=Berlin");
    assert_eq!(
        today.request.auth,
        Auth::Bearer {
            token: "{{token}}".into()
        }
    );
    let headers: Vec<(&str, bool)> = today
        .request
        .headers
        .iter()
        .map(|h| (h.name.as_str(), h.enabled))
        .collect();
    assert_eq!(
        headers,
        [("X-App", true), ("Accept", true), ("X-Trace", false)]
    );
}

#[test]
fn bodies_and_system_variables_are_mapped() {
    let import = weather();
    let report = &find(&import, "Report").request;
    assert!(matches!(&report.body, Body::Json { text } if text.contains("{{$guid}}")));
    assert_eq!(
        report.auth,
        Auth::Basic {
            username: "met".into(),
            password: "{{$secret password}}".into()
        }
    );
    assert!(report.url.ends_with("api_key={{api-key}}"));
    let Body::Multipart { parts } = &find(&import, "Upload").request.body else {
        panic!("multipart")
    };
    assert_eq!(parts[0].value, "B1");
    assert_eq!(parts[1].file.as_deref(), Some("./b1.csv"));
    let Body::Form { fields } = &find(&import, "Login").request.body else {
        panic!("form")
    };
    assert!(fields[0].enabled && !fields[1].enabled);
}

#[test]
fn the_environment_loses_its_token_and_the_report_names_the_rest() {
    let import = weather();
    let local = import
        .environments
        .iter()
        .find(|e| e.name == "Local")
        .expect("the Local environment");
    assert_eq!(local.name, "Local");
    assert_eq!(local.variables[1].value, "{{$secret token}}");
    let report = &import.report;
    assert_eq!(report.of("tests")[0].at, "Today");
    assert_eq!(report.of("auth")[0].detail, "aws");
    assert_eq!(report.of("file")[0].detail, "b1.csv");
    let text = serde_json::to_string(&import).expect("serializes");
    for secret in ["rain", "wk-123", "local-token"] {
        assert!(!text.contains(secret), "{secret} leaked");
    }
}

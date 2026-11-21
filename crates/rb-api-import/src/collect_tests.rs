use rb_api_model::{ApiRequest, Entry};

use super::*;

fn names(folders: &[&str]) -> Vec<String> {
    folders.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_request_gets_a_title_a_clean_url_and_safe_folders() {
    let mut collector = Collector::default();
    let mut request = ApiRequest::new("get", "https://api.example.com/a b?q=1");
    request.headers.push(Entry::new(" Accept ", "a\nb"));
    collector.request(&names(&["Users & Admins", "  "]), request);
    let import = collector.finish();
    let taken = &import.requests[0];
    assert_eq!(taken.folder, "users-admins");
    assert_eq!(taken.request.name, "GET /a%20b");
    assert_eq!(taken.request.method, "GET");
    assert_eq!(taken.request.url, "https://api.example.com/a%20b?q=1");
    assert_eq!(taken.request.headers[0].name, "Accept");
    assert_eq!(taken.request.headers[0].value, "a b");
    assert_eq!(import.report.imported, 1);
}

#[test]
fn deep_folders_are_flattened_and_reported() {
    let mut collector = Collector::default();
    collector.request(
        &names(&["a", "b", "c", "d", "e"]),
        ApiRequest::new("GET", "https://x.example/"),
    );
    let import = collector.finish();
    assert_eq!(import.requests[0].folder, "a/b/c-d-e");
    assert_eq!(import.report.of("folder").len(), 1);
    assert!(rb_api_model::paths::is_request_id(&format!(
        "import/{}/x.http",
        import.requests[0].folder
    )));
}

#[test]
fn an_invalid_request_is_dropped_with_its_reason() {
    let mut collector = Collector::default();
    collector.request(&[], ApiRequest::new("GET", ""));
    collector.request(&[], ApiRequest::new("BAD METHOD", "https://x.example/"));
    let import = collector.finish();
    assert!(import.requests.is_empty());
    let invalid = import.report.of("invalid");
    assert_eq!(invalid.len(), 2);
    assert_eq!(invalid[0].kind, NoteKind::Dropped);
    assert_eq!(invalid[0].detail, "url");
}

#[test]
fn unknown_dynamic_variables_are_reported() {
    let mut collector = Collector::default();
    collector.request(
        &[],
        ApiRequest::new(
            "GET",
            "https://x.example/{{$randomInt}}?t={{$timestamp}}&g={{ $guid }}&d={{$dotenv KEY}}",
        ),
    );
    let import = collector.finish();
    let reported: Vec<&str> = import
        .report
        .of("variable")
        .iter()
        .map(|note| note.detail.as_str())
        .collect();
    assert_eq!(reported, ["$randomInt", "$dotenv"]);
}

#[test]
fn environments_merge_and_lose_their_secrets() {
    let mut collector = Collector::default();
    collector.environment(
        "Staging",
        &[
            ("baseUrl".into(), "https://staging.example".into(), false),
            ("token".into(), "abc".into(), false),
            ("pin".into(), "1234".into(), true),
            ("bad name".into(), "x".into(), false),
        ],
    );
    collector.environment(
        "Staging",
        &[
            ("baseUrl".into(), "https://other.example".into(), false),
            ("extra".into(), "1".into(), false),
        ],
    );
    let import = collector.finish();
    assert_eq!(import.environments.len(), 1);
    let variables = &import.environments[0].variables;
    let pairs: Vec<(&str, &str)> = variables
        .iter()
        .map(|v| (v.name.as_str(), v.value.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("baseUrl", "https://staging.example"),
            ("token", "{{$secret token}}"),
            ("pin", "{{$secret pin}}"),
            ("extra", "1"),
        ]
    );
    assert_eq!(import.report.of("environment")[0].detail, "bad name");
    assert_eq!(import.report.secrets.len(), 2);
    assert_eq!(import.report.secrets[0].at, "environment Staging");
}

#[test]
fn the_first_collection_name_stays() {
    let mut collector = Collector::default();
    collector.collection("  ");
    collector.collection("Pets");
    collector.collection("Other");
    assert_eq!(collector.finish().collection.as_deref(), Some("Pets"));
}

use super::*;

fn of(name: &str, text: &str) -> Option<Format> {
    detect(&Source::new(name, text))
}

#[test]
fn every_fixture_is_told_by_its_marks() {
    let cases = [
        (
            "c.json",
            include_str!("../fixtures/postman/collection.json"),
            Format::Postman,
        ),
        (
            "e.json",
            include_str!("../fixtures/postman/environment.json"),
            Format::Postman,
        ),
        (
            "i.json",
            include_str!("../fixtures/insomnia/export.json"),
            Format::Insomnia,
        ),
        (
            "i.yaml",
            include_str!("../fixtures/insomnia/export.yaml"),
            Format::Insomnia,
        ),
        (
            "t.json",
            include_str!("../fixtures/thunder/collection.json"),
            Format::ThunderClient,
        ),
        (
            "h.json",
            include_str!("../fixtures/httpie/collection.json"),
            Format::Httpie,
        ),
        (
            "o.yaml",
            include_str!("../fixtures/openapi/petstore.yaml"),
            Format::OpenApi,
        ),
        (
            "s.json",
            include_str!("../fixtures/openapi/swagger.json"),
            Format::OpenApi,
        ),
        (
            "x.har",
            include_str!("../fixtures/har/capture.har"),
            Format::Har,
        ),
    ];
    for (name, text, format) in cases {
        assert_eq!(of(name, text), Some(format), "{name}");
        assert_eq!(of("", text), Some(format), "{name} pasted");
    }
}

#[test]
fn names_and_text_shapes_decide_the_rest() {
    assert_eq!(of("users/get.bru", "meta {}"), Some(Format::Bruno));
    assert_eq!(of("bruno.json", "{}"), Some(Format::Bruno));
    assert_eq!(of("", "meta {\n  name: a\n}\n"), Some(Format::Bruno));
    assert_eq!(of("api.http", "GET /"), Some(Format::Http));
    assert_eq!(of("http-client.env.json", "{}"), Some(Format::Http));
    assert_eq!(
        of("", "### a\nGET https://a.example/\n"),
        Some(Format::Http)
    );
    assert_eq!(
        of("", "curl 'https://a.example' -H 'a: b'"),
        Some(Format::Curl)
    );
    assert_eq!(of("", "curl.exe https://a.example"), Some(Format::Curl));
    assert_eq!(of("", "^\"curl^\" x"), Some(Format::Curl));
    assert_eq!(of("", "just some words"), None);
    assert_eq!(of("", "{\"a\": 1}"), None);
}

#[test]
fn ids_round_trip() {
    for format in FORMATS {
        assert_eq!(Format::of(format.id()), Some(*format));
        assert_eq!(
            serde_json::to_value(format).expect("serializes"),
            serde_json::json!(format.id())
        );
    }
    assert_eq!(Format::of("word"), None);
}

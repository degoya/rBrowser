use super::{Template, placeholder, split, templated, templates};

#[test]
fn numbers_uuids_and_hashes_become_placeholders() {
    assert_eq!(placeholder("42"), Some("{id}"));
    assert_eq!(
        placeholder("3f2b8c1e-9d4a-4c5b-8e7f-0a1b2c3d4e5f"),
        Some("{uuid}")
    );
    assert_eq!(placeholder("5f8d0d55b54764421b7156c9"), Some("{hash}"));
    assert_eq!(
        placeholder("d41d8cd98f00b204e9800998ecf8427e"),
        Some("{hash}")
    );
    assert_eq!(placeholder("eyJhbGciOiJIUzI1NiJ9xYz9"), Some("{hash}"));
}

#[test]
fn words_slugs_and_versions_stay() {
    for segment in [
        "users",
        "v2",
        "my-long-article-title-2024",
        "deadbeefdeadbeef",
        "index.html",
        "",
    ] {
        assert_eq!(placeholder(segment), None, "{segment}");
    }
}

#[test]
fn a_url_splits_into_origin_and_path_without_query_or_fragment() {
    assert_eq!(
        split("https://API.example.com:443/api/users/7?token=secret#top"),
        Some((
            "https://api.example.com".to_owned(),
            "/api/users/7".to_owned()
        ))
    );
    assert_eq!(
        split("http://localhost:3000?x=1"),
        Some(("http://localhost:3000".to_owned(), "/".to_owned()))
    );
    assert_eq!(split("data:text/plain,hi"), None);
    assert_eq!(split("chrome-extension://abc/x.js"), None);
}

#[test]
fn a_path_is_templated_segment_by_segment() {
    assert_eq!(
        templated(
            "/api/users/42/posts/3f2b8c1e-9d4a-4c5b-8e7f-0a1b2c3d4e5f/",
            &[]
        ),
        "/api/users/{id}/posts/{uuid}"
    );
    assert_eq!(templated("/", &[]), "/");
    assert_eq!(templated("//api//items", &[]), "/api/items");
}

#[test]
fn a_pattern_of_the_settings_wins_over_the_automatic_placeholders() {
    let own = templates(&[
        "/api/products/{slug}".to_owned(),
        "/blog/{post}/comments/{id}".to_owned(),
    ]);
    assert_eq!(own.len(), 2);
    assert_eq!(
        templated("/api/products/red-shoes", &own),
        "/api/products/{slug}"
    );
    assert_eq!(
        templated("/blog/hello/comments/9", &own),
        "/blog/{post}/comments/{id}"
    );
    // Another length or literal: the automatic placeholders.
    assert_eq!(
        templated("/api/products/red-shoes/12", &own),
        "/api/products/red-shoes/{id}"
    );
}

#[test]
fn a_bad_pattern_is_left_out() {
    for bad in ["api/x", "/a/{bad name}", "/a/{}", "", "/{a-b}"] {
        assert!(Template::parse(bad).is_none(), "{bad}");
    }
    let long = format!("/{}", "a".repeat(600));
    assert!(Template::parse(&long).is_none());
    assert_eq!(
        Template::parse("/api//users/{id}/").map(|template| template.text().to_owned()),
        Some("/api/users/{id}".to_owned())
    );
}

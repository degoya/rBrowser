use super::{SheetRef, imports, resolve, stylesheets};

#[test]
fn links_and_style_blocks_in_document_order() {
    let html = r#"<!doctype html><html><head>
        <!-- <link rel="stylesheet" href="commented.css"> -->
        <link href="css/a.css" rel="stylesheet">
        <LINK REL='preload stylesheet' HREF='/b.css?v=1&amp;x=2' media="(min-width: 768px)">
        <link rel="icon" href="favicon.ico">
        <style media="screen">@media (min-width: 600px) { a { color: red } }</style>
        <script>var s = '<link rel="stylesheet" href="script.css">';</script>
        <link rel=stylesheet href=//cdn.example.org/c.css>
        </head></html>"#;
    assert_eq!(
        stylesheets(html, "http://127.0.0.1:8080/site/index.html"),
        vec![
            SheetRef::Link {
                url: "http://127.0.0.1:8080/site/css/a.css".to_owned(),
                media: None
            },
            SheetRef::Link {
                url: "http://127.0.0.1:8080/b.css?v=1&x=2".to_owned(),
                media: Some("(min-width: 768px)".to_owned())
            },
            SheetRef::Inline {
                text: "@media (min-width: 600px) { a { color: red } }".to_owned(),
                media: Some("screen".to_owned())
            },
            SheetRef::Link {
                url: "http://cdn.example.org/c.css".to_owned(),
                media: None
            },
        ]
    );
}

#[test]
fn references_resolve_like_a_browser() {
    let base = "https://example.org/a/b/page.html?q=1#top";
    assert_eq!(
        resolve(base, "../c.css").as_deref(),
        Some("https://example.org/a/c.css")
    );
    assert_eq!(
        resolve(base, "./assets/../app.css").as_deref(),
        Some("https://example.org/a/b/app.css")
    );
    assert_eq!(
        resolve(base, "/root.css").as_deref(),
        Some("https://example.org/root.css")
    );
    assert_eq!(
        resolve(base, "http://other.test/x.css").as_deref(),
        Some("http://other.test/x.css")
    );
    assert_eq!(
        resolve("http://host", "x.css").as_deref(),
        Some("http://host/x.css")
    );
    assert_eq!(
        resolve("file:///home/me/site/index.html", "style.css").as_deref(),
        Some("file:///home/me/site/style.css")
    );
    for refused in [
        "",
        "#x",
        "data:text/css,a{}",
        "javascript:alert(1)",
        "ftp://x/y.css",
    ] {
        assert_eq!(resolve(base, refused), None, "{refused}");
    }
}

#[test]
fn imports_in_every_spelling() {
    let css = "@import url(\"wide.css\") screen;\n@import 'theme/dark.css' (prefers-color-scheme: dark);\n\
               @import url(/abs.css);\n@import \"https://cdn.test/x.css\";";
    assert_eq!(
        imports(css, "http://h/css/main.css"),
        vec![
            "http://h/css/wide.css",
            "http://h/css/theme/dark.css",
            "http://h/abs.css",
            "https://cdn.test/x.css"
        ]
    );
}

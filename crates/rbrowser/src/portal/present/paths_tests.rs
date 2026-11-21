use super::*;

fn rules(base: &str, allow: &[&str], deny: &[&str]) -> PathRules {
    let owned = |list: &[&str]| {
        list.iter()
            .map(|rule| (*rule).to_owned())
            .collect::<Vec<_>>()
    };
    PathRules::new(base, &owned(allow), &owned(deny))
}

fn defaults() -> PathRules {
    let settings = rb_config::PresentationSettings::default();
    PathRules::new("/", &settings.allow_paths, &settings.deny_paths)
}

#[test]
fn without_allowed_paths_the_start_page_and_below_are_allowed() {
    let rules = rules("/shop/", &[], &[]);
    for path in ["/shop", "/shop/", "/shop/cart", "/shop/a/b.css"] {
        assert!(rules.allows(path), "{path}");
    }
    for path in ["/", "/shopping", "/admin", "/assets/app.js"] {
        assert!(!rules.allows(path), "{path}");
    }
}

#[test]
fn an_allowed_path_covers_its_segments_only() {
    let rules = rules("/", &["/shop", "/assets/"], &[]);
    assert!(rules.allows("/shop/item"));
    assert!(rules.allows("/assets/app.js"));
    assert!(!rules.allows("/shopping"));
    assert!(!rules.allows("/"));
}

#[test]
fn the_defaults_deny_the_usual_secrets_and_debug_pages() {
    let rules = defaults();
    assert!(rules.allows("/"));
    assert!(rules.allows("/products/1"));
    assert!(
        rules.allows("/node_modules/.vite/deps/vue.js"),
        "Vite's deps"
    );
    for path in [
        "/admin",
        "/admin/users",
        "/administrator",
        "/.env",
        "/.env.local",
        "/.git/config",
        "/_debug/",
        "/@fs/etc/passwd",
        // The dev servers' editor launches and source readers: a viewer opens no file.
        "/__open-in-editor",
        "/__open-stack-frame-in-editor",
        "/__nextjs_launch-editor",
        "/__nextjs_original-stack-frame",
        "/__nextjs_original-stack-frames",
    ] {
        assert!(!rules.allows(path), "{path}");
    }
}

/// A denied path is denied in every spelling a server may read as the same file.
#[test]
fn a_denied_path_is_denied_in_every_spelling() {
    let rules = defaults();
    for path in [
        "/ADMIN",
        "/Admin/users",
        "//admin",
        "/%61dmin",
        "/%2Eenv",
        "/.%65nv",
        "/shop/../admin",
        "/./admin",
        "/admin%2fusers",
        "/x%5c..%5cadmin",
        "/x\\..\\admin",
        "/a%00",
        "/%ff",
        "admin",
        "",
        // 8.3 short names a Windows dev server reads as `.git`, `.env` or `admin`.
        "/GIT~1/config",
        "/ENV~1",
        "/ADMIN~1/users",
        "/%47IT~1/config",
    ] {
        assert!(!rules.allows(path), "{path:?}");
    }
}

#[test]
fn a_short_name_segment_is_refused_even_where_it_is_allowed() {
    let rules = rules("/", &[], &[]);
    assert!(rules.allows("/shop/cart"));
    assert!(rules.allows("/a~b/c"), "a tilde alone is no short name");
    assert!(!rules.allows("/shop/CART~1.HTM"));
    assert_eq!(normalize("/x/GIT~1"), None);
}

#[test]
fn a_rule_is_read_like_a_path() {
    let rules = rules("/", &[], &["/Secret/", "  ", "relative", "/private"]);
    assert!(!rules.allows("/secret/key"));
    assert!(!rules.allows("/private"));
    assert!(
        rules.allows("/relative"),
        "a rule without a leading slash is none"
    );
    assert!(rules.allows("/open"));
}

#[test]
fn a_deny_rule_wins_below_an_allowed_one() {
    let rules = rules("/", &["/shop"], &["/shop/admin"]);
    assert!(rules.allows("/shop/cart"));
    assert!(!rules.allows("/shop/admin"));
    assert!(!rules.allows("/shop/admin/x"));
}

#[test]
fn the_normal_form_decodes_and_folds() {
    assert_eq!(normalize("/A%20b//c/").as_deref(), Some("/a b/c"));
    assert_eq!(normalize("/").as_deref(), Some("/"));
    assert_eq!(normalize("/a/../b"), None);
    assert_eq!(normalize("/a%2Fb"), None);
}

#[test]
fn the_base_is_the_path_of_the_environment_the_page_is_on() {
    let environments = ["http://localhost:5173/shop/", "http://localhost:8080/"];
    assert_eq!(
        base_path(environments, Some("http://localhost:5173/shop/cart?x=1")),
        "/shop/"
    );
    assert_eq!(
        base_path(environments, Some("http://localhost:8080/a")),
        "/"
    );
    assert_eq!(base_path(environments, Some("http://other.test/")), "/");
    assert_eq!(base_path(environments, None), "/");
    assert_eq!(
        base_path(
            ["http://localhost:5173/app?mode=1#top"],
            Some("http://localhost:5173/")
        ),
        "/app"
    );
}

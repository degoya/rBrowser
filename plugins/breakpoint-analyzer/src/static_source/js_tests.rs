use super::{Config, Js};

fn object(entries: &[(&str, Js)]) -> Js {
    Js::Object(
        entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

fn text(value: &str) -> Js {
    Js::Str(value.to_owned())
}

#[test]
fn static_js_reads_object_literals_with_any_key_and_quote() {
    let config = Config::parse(
        r#"
        // a comment with theme: { screens: 'no' }
        module.exports = {
          /* theme: 1 */
          theme: {
            screens: { sm: '640px', "md": "768px", 'tablet-lg': `900px`, 2: '2px', },
          },
        }
        "#,
    );
    let theme = config.property("theme").expect("theme");
    assert_eq!(
        theme.get("screens"),
        Some(&object(&[
            ("sm", text("640px")),
            ("md", text("768px")),
            ("tablet-lg", text("900px")),
            ("2", text("2px")),
        ]))
    );
    assert!(config.exports_object());
}

#[test]
fn static_js_keeps_names_calls_and_functions_as_dynamic() {
    let config = Config::parse(
        "export default defineConfig({ theme: { screens: { \
           sm: base.sm, md: px(768), lg: `${lg}px`, xl: '1280' + 'px', \
           ...defaultTheme.screens, [key]: '1px', short, \
           fn() { return 1 }, \
         }, extend: ({ theme }) => ({ screens: {} }) } })",
    );
    assert!(config.exports_object());
    let theme = config.property("theme").expect("theme");
    assert_eq!(
        theme.get("screens"),
        Some(&object(&[
            ("sm", Js::Dynamic("base.sm".to_owned())),
            ("md", Js::Dynamic("px()".to_owned())),
            ("lg", Js::Dynamic("`...`".to_owned())),
            ("xl", Js::Dynamic("'1280'".to_owned())),
            ("...", Js::Spread("defaultTheme.screens".to_owned())),
            ("[[]", Js::Dynamic("[".to_owned())),
            ("short", Js::Dynamic("short".to_owned())),
            ("fn", Js::Dynamic("fn".to_owned())),
        ]))
    );
    assert_eq!(
        theme.get("extend"),
        Some(&Js::Dynamic("(...) =>".to_owned()))
    );
}

#[test]
fn static_js_typescript_satisfies_and_arrays_and_spread_objects() {
    let config = Config::parse(
        "const config = { theme: { screens: { ...{ xs: '475px' }, \
           range: { min: '640px', max: '767px' }, list: [{ min: '1px' }, 2] } as const } } \
         satisfies Config; export default config;",
    );
    let screens = config
        .property("theme")
        .and_then(|theme| theme.get("screens").cloned())
        .expect("screens");
    assert_eq!(
        screens,
        object(&[
            ("xs", text("475px")),
            (
                "range",
                object(&[("min", text("640px")), ("max", text("767px"))])
            ),
            (
                "list",
                Js::Array(vec![object(&[("min", text("1px"))]), Js::Num(2.0)])
            ),
        ])
    );
    assert!(!config.exports_object(), "the export is a name");
}

#[test]
fn static_js_survives_broken_and_hostile_input() {
    for broken in [
        "",
        "theme: {",
        "{ theme: { screens: { sm: '640px'",
        "{ theme: ",
        "{ theme: )))]]]}}}",
        "{ theme: '\\",
        "{ theme: `unterminated",
        "/* never closed",
        "{ theme: { a: { b: { c: { d: { e: { f: { g: { h: { i: { j: { k: { l: { m: { n: { o: { p: \
         { q: { r: { s: { t: { u: { v: { w: { x: { y: { z: { aa: { ab: { ac: { ad: { ae: { af: \
         { ag: { ah: 1 } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }",
    ] {
        let config = Config::parse(broken);
        let _ = config.property("theme");
        let _ = config.exports_object();
    }
    let deep = format!("{{ theme: {}1{} }}", "[".repeat(5_000), "]".repeat(5_000));
    assert!(matches!(
        Config::parse(&deep).property("theme"),
        Some(Js::Array(_) | Js::Dynamic(_))
    ));
}

#[test]
fn static_js_a_property_is_only_a_key_of_an_object() {
    let config = Config::parse("const x = flag ? theme : other; module.exports = { a: 1 }");
    assert_eq!(config.property("theme"), None);
    assert!(config.exports_object());
}

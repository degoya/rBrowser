use super::scan;
use crate::suggest::Framework;

fn widths(text: &str) -> Vec<(String, u32)> {
    scan("tailwind.config.js", text, 16.0)
        .declared
        .iter()
        .map(|found| (found.label.clone(), found.width))
        .collect()
}

fn pairs(list: &[(&str, u32)]) -> Vec<(String, u32)> {
    list.iter()
        .map(|(label, width)| ((*label).to_owned(), *width))
        .collect()
}

fn hint_names(text: &str) -> Vec<(String, String)> {
    scan("tailwind.config.ts", text, 16.0)
        .hints
        .iter()
        .map(|hint| {
            (
                hint.key.rsplit('.').next().unwrap_or_default().to_owned(),
                hint.params["name"].clone(),
            )
        })
        .collect()
}

const DEFAULTS: [(&str, u32); 5] = [
    ("sm", 640),
    ("md", 768),
    ("lg", 1024),
    ("xl", 1280),
    ("2xl", 1536),
];

#[test]
fn static_tailwind3_without_screens_gives_the_defaults() {
    let config = "module.exports = { content: ['./src/**/*.{html,js}'], theme: { extend: {} } }";
    assert_eq!(widths(config), pairs(&DEFAULTS));
    assert_eq!(widths("export default { plugins: [] }"), pairs(&DEFAULTS));
    let found = scan("tailwind.config.js", config, 16.0);
    assert!(found.declared.iter().all(|declared| {
        declared.framework == Some(Framework::Tailwind)
            && !declared.heuristic
            && declared.path == "tailwind.config.js"
    }));
    assert!(found.hints.is_empty());
}

#[test]
fn static_tailwind3_extend_screens_3xl_adds_1920() {
    let config = "/** @type {import('tailwindcss').Config} */\n\
        module.exports = { theme: { extend: { screens: { '3xl': '1920px' } } } }";
    let mut expected = DEFAULTS.to_vec();
    expected.push(("3xl", 1920));
    assert_eq!(widths(config), pairs(&expected));
}

#[test]
fn static_tailwind3_theme_screens_replace_the_defaults() {
    let config = "export default { theme: { screens: { tablet: '40rem', laptop: { min: '1024px' }, \
                  desktop: '1280.5px' }, extend: { screens: { tablet: '700px' } } } } satisfies Config";
    assert_eq!(
        widths(config),
        pairs(&[("tablet", 700), ("laptop", 1024), ("desktop", 1281)])
    );
}

#[test]
fn static_tailwind3_default_theme_spread_keeps_the_defaults() {
    let config = "const defaultTheme = require('tailwindcss/defaultTheme');\n\
        module.exports = { theme: { screens: { xs: '475px', ...defaultTheme.screens } } }";
    let mut expected = vec![("xs", 475)];
    expected.extend(DEFAULTS);
    assert_eq!(widths(config), pairs(&expected));
    let whole = "module.exports = { theme: { screens: defaultTheme.screens } }";
    assert_eq!(widths(whole), pairs(&DEFAULTS));
}

#[test]
fn static_tailwind3_a_function_call_is_a_hint_not_an_error() {
    let config =
        "module.exports = { theme: { screens: { sm: '640px', md: rem(48), lg: base.lg } } }";
    assert_eq!(widths(config), pairs(&[("sm", 640)]));
    assert_eq!(
        hint_names(config),
        vec![
            ("dynamic".to_owned(), "md".to_owned()),
            ("dynamic".to_owned(), "lg".to_owned())
        ]
    );
    let whole = "module.exports = { theme: { screens: makeScreens() } }";
    assert!(widths(whole).is_empty());
    assert_eq!(
        hint_names(whole),
        vec![("dynamic".to_owned(), "screens: makeScreens()".to_owned())]
    );
    let theme_fn = "module.exports = { theme: (helpers) => ({ screens: { sm: '1px' } }) }";
    assert!(widths(theme_fn).is_empty());
    assert_eq!(hint_names(theme_fn)[0].0, "dynamic");
    let opaque = "module.exports = require('./base.config')";
    assert!(widths(opaque).is_empty());
    assert_eq!(
        hint_names(opaque),
        vec![("dynamic".to_owned(), "module.exports".to_owned())]
    );
}

#[test]
fn static_tailwind3_ranges_and_raw_queries_are_left_out_with_a_hint() {
    let config = "module.exports = { theme: { screens: { sm: { min: '640px', max: '767px' }, \
                  print: { raw: 'print' }, md: [{ min: '668px', max: '767px' }], lg: '1024px', \
                  bad: 'wide' } } }";
    assert_eq!(widths(config), pairs(&[("lg", 1024)]));
    assert_eq!(
        hint_names(config),
        vec![
            ("range".to_owned(), "sm".to_owned()),
            ("range".to_owned(), "print".to_owned()),
            ("range".to_owned(), "md".to_owned()),
            ("dynamic".to_owned(), "bad".to_owned()),
        ]
    );
}

#[test]
fn static_tailwind3_presets_are_named_in_a_hint() {
    let config = "module.exports = { presets: [require('./preset')], theme: {} }";
    assert_eq!(widths(config), pairs(&DEFAULTS));
    assert_eq!(
        hint_names(config),
        vec![("presets".to_owned(), "presets".to_owned())]
    );
}

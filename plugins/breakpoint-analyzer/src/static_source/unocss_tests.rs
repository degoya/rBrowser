use super::scan;

fn widths(text: &str) -> Vec<(String, u32)> {
    scan("uno.config.ts", text, 16.0)
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

fn hint_names(text: &str) -> Vec<String> {
    scan("uno.config.ts", text, 16.0)
        .hints
        .iter()
        .map(|hint| hint.params["name"].clone())
        .collect()
}

/// The shape of the project in which the footer named no class from `2xl` on (RB-090-50):
/// `presetWind4`, `theme.breakpoint` with names that begin with a digit, beside a
/// `getCSS: ({ theme }) => ...` that names the theme too.
const WIND4: &str = "import { defineConfig, presetWind4 } from 'unocss'\n\
    const helper = (breakpoint: Record<string, string> = {}): string => ''\n\
    export default defineConfig<Theme>({\n\
      presets: [presetWind4({ preflights: { reset: true } })],\n\
      preflights: [{ getCSS: ({ theme }) => helper(theme.breakpoint) }],\n\
      theme: {\n\
        aria: { invalid: 'invalid=\"true\"' },\n\
        breakpoint: {\n\
          sm: '768px',\n\
          md: '1024px',\n\
          lg: '1280px',\n\
          xl: '1600px',\n\
          '2xl': '1920px',\n\
          '3xl': '2000px'\n\
        },\n\
        colors: { blue: { DEFAULT: '#1ea2bd', 80: '#6fc4d5' } }\n\
      },\n\
      transformers: [transformerDirectives()]\n\
    })";

#[test]
fn static_unocss_wind4_names_every_breakpoint_2xl_and_3xl_too() {
    assert_eq!(
        widths(WIND4),
        pairs(&[
            ("sm", 768),
            ("md", 1024),
            ("lg", 1280),
            ("xl", 1600),
            ("2xl", 1920),
            ("3xl", 2000),
        ])
    );
    let found = scan("uno.config.ts", WIND4, 16.0);
    assert!(found.hints.is_empty());
    assert!(found.declared.iter().all(|declared| {
        !declared.heuristic && declared.framework.is_none() && declared.path == "uno.config.ts"
    }));
}

#[test]
fn static_unocss_breakpoints_of_wind3_and_own_names_in_rem() {
    let config = "export default defineConfig({ theme: { breakpoints: {\
        tablet: '48rem', 'wide-1': '90rem', '4k': '160rem' } } })";
    assert_eq!(
        widths(config),
        pairs(&[("tablet", 768), ("wide-1", 1440), ("4k", 2560)])
    );
}

#[test]
fn static_unocss_without_breakpoints_declares_nothing() {
    // The preset's defaults hold: Tailwind's widths, named once the page's queries are known.
    assert!(widths("export default defineConfig({ theme: { colors: { a: '#000' } } })").is_empty());
    assert!(widths("export default defineConfig({ presets: [presetUno()] })").is_empty());
    assert!(hint_names("export default defineConfig({ presets: [presetUno()] })").is_empty());
}

#[test]
fn static_unocss_what_is_not_a_literal_is_a_hint() {
    let config = "export default defineConfig({ theme: { breakpoint: {\
        ...base.breakpoint, md: size, lg: '1024px' } } })";
    assert_eq!(widths(config), pairs(&[("lg", 1024)]));
    assert_eq!(hint_names(config), vec!["...base.breakpoint", "md"]);
    assert_eq!(
        hint_names("export default defineConfig({ theme: { breakpoints: own() } })"),
        vec!["breakpoints"]
    );
}

use serde_json::json;

use super::*;

fn read(media: &[&str]) -> PageRead {
    PageRead {
        url: "https://a.test/start#top".to_owned(),
        media: media
            .iter()
            .map(|media| vec![(*media).to_owned()])
            .collect(),
        ..PageRead::default()
    }
}

fn nested(chains: &[&[&str]]) -> PageRead {
    PageRead {
        media: chains
            .iter()
            .map(|chain| chain.iter().map(|media| (*media).to_owned()).collect())
            .collect(),
        ..read(&[])
    }
}

#[test]
fn the_script_s_answer_is_read() {
    let read: PageRead = serde_json::from_value(json!({
        "url": "https://a.test/",
        "media": [["(min-width: 768px)"], ["screen", "(max-width: 1023px)"]],
        "skipped": 2,
        "capped": true,
        "sheets": 5,
        "rootFontSizePx": 16
    }))
    .expect("read");
    assert_eq!(read.media.len(), 2);
    assert_eq!((read.skipped, read.sheets, read.capped), (2, 5, true));
    assert_eq!(read.root_font_size_px, Some(16.0));
    // A page that answers less is an empty read, not a failure.
    let empty: PageRead = serde_json::from_value(json!({})).expect("read");
    assert_eq!(empty, PageRead::default());
}

#[test]
fn the_queries_become_one_sheet_of_empty_media_rules() {
    let collected = read(&["(min-width: 768px)", " screen and (max-width: 1023px) "])
        .collected()
        .expect("sheet");
    assert_eq!(collected.sheets.len(), 1);
    let sheet = &collected.sheets[0];
    assert_eq!(sheet.url, "https://a.test/start#media");
    assert_eq!(
        sheet.text,
        "@media (min-width: 768px){a{b:c}}\n@media screen and (max-width: 1023px){a{b:c}}\n"
    );
    assert!(read(&[]).collected().is_none());
    assert!(read(&["", "  "]).collected().is_none());
}

#[test]
fn a_media_list_that_could_leave_its_rule_is_not_taken() {
    let long = "x".repeat(MAX_MEDIA_LEN + 1);
    let page = read(&[
        "(min-width: 1px){} body{color:red} @media all",
        "all; @import url(x)",
        "(min-width:\n2px)",
        "@import 'x'",
        "a\\7b",
        long.as_str(),
        "(min-width: 3px)",
    ]);
    assert_eq!(page.conditions().collect::<Vec<_>>(), ["(min-width: 3px)"]);
    let many = vec!["(min-width: 4px)"; MAX_MEDIA + 10];
    assert_eq!(read(&many).conditions().count(), MAX_MEDIA);
}

#[test]
fn a_media_list_holds_on_a_screen_or_does_not() {
    for list in [
        "(min-width: 768px)",
        "screen",
        "only screen and (max-width: 600px)",
        "all and (min-width: 1px)",
        "print, screen",
        "not print",
        "NOT PRINT",
        "not (hover: hover)",
        "Screen and (min-width: 1px)",
    ] {
        assert!(for_screen(list), "{list}");
    }
    for list in [
        "print",
        "only print and (min-width: 1px)",
        "speech",
        "not screen",
        "not all",
        "print, speech",
    ] {
        assert!(!for_screen(list), "{list}");
    }
}

#[test]
fn a_query_inside_a_list_that_never_holds_on_a_screen_is_no_boundary() {
    // `<link media="print">` with `@media (min-width: 900px)` inside; a query under
    // `@media screen`; and the same query once more at the top.
    let page = nested(&[
        &["print"],
        &["print", "(min-width: 900px)"],
        &["screen"],
        &["screen", "(min-width: 768px)"],
        &["screen", "print", "(min-width: 500px)"],
        &["(min-width: 1024px)"],
    ]);
    assert_eq!(
        page.conditions().collect::<Vec<_>>(),
        ["screen", "(min-width: 768px)", "(min-width: 1024px)"]
    );
}

#[test]
fn the_signature_follows_what_was_read_and_the_config_not_where() {
    let first = read(&["(min-width: 768px)"]);
    let mut same = first.clone();
    same.url = "https://a.test/other".to_owned();
    same.sheets = 9;
    assert_eq!(first.signature(1), same.signature(1));
    assert_ne!(
        first.signature(1),
        read(&["(min-width: 800px)"]).signature(1)
    );
    let mut skipped = first.clone();
    skipped.skipped = 1;
    assert_ne!(first.signature(1), skipped.signature(1));
    let mut capped = first.clone();
    capped.capped = true;
    assert_ne!(first.signature(1), capped.signature(1));
    // Another analyser setting, another breakpoint: analysed again.
    assert_ne!(first.signature(1), first.signature(2));
}

#[test]
fn the_config_mark_follows_the_analyser_settings_and_the_breakpoints() {
    let config =
        |value| -> rb_config::ProjectConfig { serde_json::from_value(value).expect("config") };
    let base =
        json!({ "version": 1, "breakpoints": [{ "id": "md", "width": 768, "height": 1024 }] });
    let mark = config_mark(&config(base.clone()));
    assert_eq!(mark, config_mark(&config(base.clone())));
    let mut tolerance = base.clone();
    tolerance["analyzer"] = json!({ "mergeTolerancePx": 40 });
    assert_ne!(mark, config_mark(&config(tolerance)));
    let mut wider = base.clone();
    wider["breakpoints"][0]["width"] = json!(800);
    assert_ne!(mark, config_mark(&config(wider)));
    // What no analysis depends on changes nothing.
    let mut url = base;
    url["startUrl"] = json!("https://a.test/");
    url["breakpoints"][0]["color"] = json!("#22c55e");
    assert_eq!(mark, config_mark(&config(url)));
}

#[test]
fn the_script_reads_the_loaded_document_and_requests_nothing() {
    let script = expression();
    assert!(script.starts_with("(function ()") && script.ends_with(")()"));
    for read in [
        "styleSheets",
        "adoptedStyleSheets",
        "shadowRoot",
        "disabled",
    ] {
        assert!(script.contains(read), "{read}");
    }
    for request in [
        "fetch(",
        "XMLHttpRequest",
        "import(",
        ".src",
        "createElement",
        "open(",
    ] {
        assert!(!script.contains(request), "{request}");
    }
}

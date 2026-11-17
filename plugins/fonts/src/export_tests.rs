use super::*;
use crate::inventory::{RenderedEntry, UsageEntry};

#[test]
fn cells_are_quoted_and_formulas_defused() {
    assert_eq!(cell("Inter"), "Inter");
    assert_eq!(cell("a,b"), "\"a,b\"");
    assert_eq!(cell("say \"hi\""), "\"say \"\"hi\"\"\"");
    assert_eq!(cell("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
    assert_eq!(cell("@SUM"), "'@SUM");
    assert_eq!(cell("-1"), "'-1");
}

#[test]
fn a_row_per_family_and_view() {
    let family = Family {
        family: "Inter".to_owned(),
        provider: "own".to_owned(),
        host: "localhost".to_owned(),
        web_font: true,
        formats: vec!["woff2".to_owned()],
        variable: false,
        faces: Vec::new(),
        file_kb: Some(34.4),
        usage: vec![
            UsageEntry {
                view: 2,
                elements: 7,
                weights: vec![400, 600],
                styles: vec!["normal".to_owned()],
                sizes: vec![16.0, 32.0],
            },
            UsageEntry {
                view: 1,
                elements: 8,
                weights: vec![400],
                styles: vec!["normal".to_owned()],
                sizes: vec![16.5],
            },
        ],
        rendered: vec![RenderedEntry {
            view: 2,
            state: "fallback".to_owned(),
            instead: vec!["Arial".to_owned()],
        }],
    };
    let text = csv(&[family]);
    let lines: Vec<&str> = text.split("\r\n").collect();
    assert_eq!(lines[0], COLUMNS.join(","));
    assert_eq!(
        lines[1],
        "1,Inter,own,localhost,true,woff2,false,0,34.4,8,400,16.5,,"
    );
    assert_eq!(
        lines[2],
        "2,Inter,own,localhost,true,woff2,false,0,34.4,7,400 600,16 32,fallback,Arial"
    );
    assert_eq!(lines[3], "");
}

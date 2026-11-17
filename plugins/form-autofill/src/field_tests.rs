use super::*;
use serde_json::json;

#[test]
fn a_collected_page_parses_with_its_hints() {
    let page = Collected::parse(
        &json!({
            "locale": "de-DE",
            "url": "http://127.0.0.1:18767/forms/register.html",
            "fields": [{
                "index": 2, "key": "input:email", "sig": "input|email|email|email",
                "tag": "input", "type": "email", "name": "email", "id": "email",
                "autocomplete": "email", "label": "E-Mail", "placeholder": "",
                "pattern": "", "min": "", "max": "", "step": "",
                "minLength": 3, "maxLength": 80, "required": true, "multiple": false,
                "options": [], "skip": ""
            }]
        })
        .to_string(),
    )
    .expect("parses");
    assert_eq!(page.locale, "de-DE");
    let field = &page.fields[0];
    assert_eq!(field.index, 2);
    assert_eq!(field.control, "email");
    assert_eq!(field.min_length, Some(3));
    assert_eq!(field.max_length, Some(80));
    assert!(field.required);
}

#[test]
fn the_page_shapes_nothing_beyond_the_caps() {
    let long = "x".repeat(5000);
    let mut options: Vec<_> = (0..250)
        .map(|n| json!({ "value": n.to_string(), "label": "o" }))
        .collect();
    options[0] = json!({ "value": "0", "label": long });
    let mut fields: Vec<_> = (0..400).map(|n| json!({ "index": n })).collect();
    fields[0] = json!({ "index": 0, "label": long, "pattern": long, "options": options });
    let page =
        Collected::parse(&json!({ "locale": long, "fields": fields }).to_string()).expect("parses");
    assert_eq!(page.fields.len(), MAX_FIELDS);
    assert_eq!(page.locale.chars().count(), 35);
    let field = &page.fields[0];
    assert_eq!(field.label.chars().count(), MAX_TEXT);
    assert_eq!(field.pattern.chars().count(), MAX_PATTERN);
    assert_eq!(field.options.len(), MAX_OPTIONS);
    assert_eq!(field.options[0].label.chars().count(), MAX_TEXT);
}

#[test]
fn wrong_shapes_read_as_defaults_and_no_object_is_bad_output() {
    let page = Collected::parse(
        &json!({ "fields": [{ "index": -4, "required": "yes", "maxLength": -1, "label": 7 }] })
            .to_string(),
    )
    .expect("parses");
    let field = &page.fields[0];
    assert_eq!(field.index, 0);
    assert!(!field.required);
    assert_eq!(field.max_length, None);
    assert_eq!(field.label, "");
    assert!(Collected::parse("[]").is_err());
    assert!(Collected::parse("not json").is_err());
}

#[test]
fn an_error_of_the_page_is_kept() {
    let page = Collected::parse(r#"{"error":"selector"}"#).expect("parses");
    assert_eq!(page.error, "selector");
    assert!(page.fields.is_empty());
}

#[test]
fn the_forms_of_the_page_are_capped_and_a_field_names_one_of_them() {
    let long = "f".repeat(5000);
    let mut forms: Vec<_> = (0..60)
        .map(|n| json!({ "selector": format!("#f{n}"), "name": "F" }))
        .collect();
    forms[0] = json!({ "selector": long, "name": long });
    let page = Collected::parse(
        &json!({ "forms": forms, "fields": [
            { "index": 0, "form": 1 }, { "index": 1, "form": 55 }, { "index": 2, "form": -1 },
            { "index": 3 }
        ] })
        .to_string(),
    )
    .expect("parses");
    assert_eq!(page.forms.len(), MAX_FORMS);
    assert_eq!(page.forms[0].selector.chars().count(), MAX_PATTERN);
    assert_eq!(page.forms[0].name.chars().count(), MAX_TEXT);
    assert_eq!(page.forms[1].selector, "#f1");
    // A form past the cap or no form at all reads as none.
    let named: Vec<_> = page.fields.iter().map(|field| field.form).collect();
    assert_eq!(named, [Some(1), None, None, None]);
}

use super::*;

#[test]
fn nothing_to_read_too_much_and_an_unknown_text_are_refused() {
    assert_eq!(import(&[], None), Err(ImportError::Empty));
    assert_eq!(
        import(&[Source::new("a.json", "  ")], None),
        Err(ImportError::Empty)
    );
    let big = "x".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(
        import(&[Source::new("a.http", big)], None),
        Err(ImportError::TooLarge)
    );
    assert_eq!(
        import(&[Source::new("notes.txt", "some words")], None),
        Err(ImportError::Unknown {
            source: "notes.txt".into()
        })
    );
    assert_eq!(ImportError::Empty.code(), "empty");
}

#[test]
fn a_named_format_is_read_as_that_format() {
    let error = import(
        &[Source::new("", "{\"info\": {}, \"item\": []}")],
        Some(Format::Har),
    )
    .expect_err("no HAR");
    assert_eq!(
        error,
        ImportError::Unreadable {
            source: "pasted text".into(),
            reason: "no HAR log".into()
        }
    );
}

#[test]
fn files_of_several_formats_add_up_in_one_import() {
    let import = import(
        &[
            Source::new("a.http", "GET https://a.example/one\n"),
            Source::new("", "curl https://b.example/two"),
        ],
        None,
    )
    .expect("imports");
    assert_eq!(import.formats, [Format::Http, Format::Curl]);
    assert_eq!(import.report.imported, 2);
}

#[test]
fn every_note_code_has_its_text() {
    let catalogue: serde_json::Value =
        serde_json::from_str(include_str!("../../../plugins/api-client/locales/en.json"))
            .expect("the plugin's catalogue reads");
    let notes = &catalogue["plugin"]["api-client"]["panel"]["import"]["notes"];
    for code in report::CODES {
        assert!(notes[*code].is_string(), "no text for the note {code}");
    }
    let formats = &catalogue["plugin"]["api-client"]["panel"]["import"]["formats"];
    for format in detect::FORMATS {
        assert!(
            formats[format.id()].is_string(),
            "no text for {}",
            format.id()
        );
    }
}

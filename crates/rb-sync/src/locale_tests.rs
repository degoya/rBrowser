use serde_json::json;

use super::*;
use crate::AgentCommand;

#[test]
fn the_locale_command_has_the_agent_s_shape() {
    let command = AgentCommand::Locale {
        locale: PageLocale {
            dir: Some(PageDir::Rtl),
            lang: Some("ar".to_owned()),
            pseudo: Some(PseudoLocale {
                factor: 1.4,
                accents: true,
                rtl: false,
            }),
        },
    };
    assert_eq!(
        serde_json::to_value(&command).expect("serializes"),
        json!({
            "cmd": "locale",
            "locale": {
                "dir": "rtl", "lang": "ar",
                "pseudo": { "factor": 1.4, "accents": true, "rtl": false }
            }
        })
    );
}

#[test]
fn a_plain_locale_is_an_empty_object() {
    let plain = PageLocale::default();
    assert!(plain.is_plain());
    assert_eq!(serde_json::to_value(&plain).expect("serializes"), json!({}));
    let read: PageLocale = serde_json::from_value(json!({ "dir": "auto" })).expect("reads");
    assert_eq!(read.dir, Some(PageDir::Auto));
    assert!(!read.is_plain());
}

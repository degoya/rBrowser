use super::*;

#[test]
fn restart_arguments_read_engine_and_view() {
    let args: RestartArgs =
        serde_json::from_value(json!({ "engine": "webkit", "view": 4 })).expect("parses");
    assert_eq!(args.engine, Some(EngineKind::Webkit));
    assert_eq!(args.view, Some(ViewId(4)));
    let none = parse(&Value::Null).expect("no arguments");
    assert_eq!((none.engine, none.view), (None, None));
    assert!(parse(&json!({ "engine": "opera" })).is_err());
}

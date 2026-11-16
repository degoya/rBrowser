use super::*;

#[test]
fn target_is_all_or_a_view_id() {
    assert_eq!(
        serde_json::to_value(Target::All).expect("serializes"),
        "all"
    );
    assert_eq!(
        serde_json::to_value(Target::View(ViewId(3))).expect("serializes"),
        3
    );
    let all: Target = serde_json::from_str(r#""all""#).expect("all");
    assert_eq!(all, Target::All);
    let view: Target = serde_json::from_str("2").expect("id");
    assert_eq!(view, Target::View(ViewId(2)));
    assert!(serde_json::from_str::<Target>(r#""some""#).is_err());
}

#[test]
fn view_info_serializes_camel_case() {
    let info = ViewInfo {
        id: ViewId(1),
        label: "iPhone 15".into(),
        breakpoint: Some("mobile".into()),
        cell_breakpoint: Some("mobile".into()),
        engine: EngineKind::Chromium,
        width: 393,
        height: 852,
        dpr: 3.0,
        scale: 0.5,
        url: "http://localhost:5173/".into(),
        title: String::new(),
        loading: false,
        session: "default".into(),
        cell: None,
    };
    let json = serde_json::to_value(&info).expect("serializes");
    assert_eq!(json["id"], 1);
    assert_eq!(json["cellBreakpoint"], "mobile");
    assert_eq!(json["engine"], "chromium");
}

/// The trait stays object-safe: other jobs hold an `Arc<dyn ViewHost>`.
#[test]
fn view_host_is_object_safe() {
    fn takes(_host: Option<std::sync::Arc<dyn ViewHost>>) {}
    takes(None);
}

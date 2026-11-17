use rb_engine::test_support::conformance::sample_spec;
use rb_engine::{Engine, EngineKind, ViewId};
use serde_json::json;

use super::CookieOutcome;
use crate::error::BidiError;
use crate::firefox::test_util::engine;
use crate::storage::Cookie;
use crate::testing::FakeFirefox;

fn cookie(name: &str) -> Cookie {
    Cookie {
        name: name.to_owned(),
        value: "hunter2".to_owned(),
        domain: "127.0.0.1".to_owned(),
        path: "/".to_owned(),
        http_only: true,
        secure: false,
        same_site: "lax".to_owned(),
        expiry: None,
    }
}

#[tokio::test]
async fn a_named_session_opens_its_windows_in_a_user_context_of_its_own() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    let mut admin = sample_spec(EngineKind::Firefox, 1);
    admin.breakpoint.session = Some("admin".to_owned());
    let mut also_admin = sample_spec(EngineKind::Firefox, 2);
    also_admin.breakpoint.session = Some("admin".to_owned());
    let plain = sample_spec(EngineKind::Firefox, 3);
    for spec in [&admin, &also_admin, &plain] {
        engine.create_view(spec).await.expect("a view");
    }
    assert_eq!(
        fake.calls("browser.createUserContext").len(),
        1,
        "one user context per session"
    );
    let creates = fake.calls("browsingContext.create");
    assert_eq!(creates[0]["userContext"], json!("uc-1"));
    assert_eq!(creates[1]["userContext"], json!("uc-1"));
    assert!(
        creates[2].get("userContext").is_none(),
        "the default session"
    );
    assert!(engine.view_state(ViewId(1)).is_some());
}

#[tokio::test]
async fn cookies_go_into_the_jar_of_their_session_and_refusals_are_counted() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    let mut admin = sample_spec(EngineKind::Firefox, 1);
    admin.breakpoint.session = Some("admin".to_owned());
    engine.create_view(&admin).await.expect("a view");
    fake.refuse("storage.setCookie", "unable to set cookie");
    let refused = engine
        .set_cookies("admin", &[cookie("sid")])
        .await
        .expect("an outcome");
    assert_eq!(refused, CookieOutcome { set: 0, refused: 1 });
    let fake = FakeFirefox::start().await;
    let engine = crate::firefox::test_util::engine(&fake, "");
    engine.create_view(&admin).await.expect("a view");
    let outcome = engine
        .set_cookies("admin", &[cookie("sid"), cookie("theme")])
        .await
        .expect("an outcome");
    assert_eq!(outcome, CookieOutcome { set: 2, refused: 0 });
    let calls = fake.calls("storage.setCookie");
    assert_eq!(
        calls[0]["partition"],
        json!({ "type": "storageKey", "userContext": "uc-1" })
    );
    engine
        .set_cookies("default", &[cookie("sid")])
        .await
        .expect("an outcome");
    assert!(
        fake.calls("storage.setCookie")[2]
            .get("partition")
            .is_none()
    );
}

#[tokio::test]
async fn cookies_never_start_firefox() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    assert_eq!(
        engine.set_cookies("default", &[cookie("sid")]).await,
        Err(BidiError::Closed)
    );
    assert!(fake.commands().is_empty(), "{:?}", fake.commands().len());
}

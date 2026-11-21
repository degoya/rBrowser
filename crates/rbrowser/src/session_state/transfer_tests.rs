use std::sync::Mutex;

use serde_json::json;

use super::*;
use crate::cookies::model::cookies_of;
use crate::session_state::model::{NameValue, OriginState};

#[derive(Default)]
struct FakeJar {
    calls: Mutex<Vec<String>>,
}

impl FakeJar {
    fn calls(&self) -> Vec<String> {
        rb_util::lock(&self.calls).clone()
    }
}

#[async_trait]
impl StateJar for FakeJar {
    async fn cookies(&self, view: ViewId) -> Result<Vec<ChromeCookie>, String> {
        rb_util::lock(&self.calls).push(format!("cookies {view}"));
        Ok(cookies_of(&json!({ "cookies": [
            { "name": "sid", "value": "v1", "domain": "staging.example.test", "path": "/",
              "session": true }
        ]})))
    }

    async fn local_storage(&self, view: ViewId, origin: &str) -> Result<Vec<NameValue>, String> {
        rb_util::lock(&self.calls).push(format!("storage {view} {origin}"));
        if origin.contains("broken") {
            return Err("no frame".to_owned());
        }
        Ok(vec![NameValue {
            name: "token".to_owned(),
            value: "t1".to_owned(),
        }])
    }

    async fn set_cookies(&self, view: ViewId, params: Value) -> Result<(), String> {
        let count = params["cookies"].as_array().map_or(0, Vec::len);
        rb_util::lock(&self.calls).push(format!("setCookies {view} {count}"));
        Ok(())
    }

    async fn set_item(&self, view: ViewId, origin: &str, item: &NameValue) -> Result<(), String> {
        rb_util::lock(&self.calls).push(format!("setItem {view} {origin} {}", item.name));
        Ok(())
    }
}

fn shown(view: u32, origin: Option<&str>) -> Shown {
    Shown {
        view: ViewId(view),
        origin: origin.map(str::to_owned),
    }
}

#[tokio::test]
async fn export_reads_the_cookies_once_and_each_shown_origins_storage_once() {
    let jar = FakeJar::default();
    let state = export(
        &jar,
        &[
            shown(1, Some("https://staging.example.test")),
            shown(2, Some("https://staging.example.test")),
            shown(3, None),
            shown(4, Some("https://broken.example.test")),
        ],
    )
    .await
    .expect("exported");
    assert_eq!(state.cookies.len(), 1);
    assert_eq!(state.origins.len(), 1, "the broken origin is left out");
    assert_eq!(state.origins[0].origin, "https://staging.example.test");
    assert_eq!(
        jar.calls(),
        [
            "cookies 1",
            "storage 1 https://staging.example.test",
            "storage 4 https://broken.example.test"
        ]
    );
}

#[tokio::test]
async fn a_session_without_a_chromium_view_has_nothing_to_read_or_write() {
    let jar = FakeJar::default();
    assert!(matches!(
        export(&jar, &[]).await,
        Err(TransferError::NoView)
    ));
    assert!(matches!(
        import(&jar, &[], &StorageState::default()).await,
        Err(TransferError::NoView)
    ));
    assert!(jar.calls().is_empty());
}

#[tokio::test]
async fn import_sets_the_cookies_and_the_storage_of_the_origins_shown() {
    let jar = FakeJar::default();
    let state = StorageState {
        cookies: StorageState::from_chrome(
            &cookies_of(&json!({ "cookies": [
                { "name": "sid", "value": "v1", "domain": "staging.example.test", "path": "/" }
            ]})),
            Vec::new(),
        )
        .cookies,
        origins: vec![
            OriginState {
                origin: "https://staging.example.test".to_owned(),
                local_storage: vec![NameValue {
                    name: "token".to_owned(),
                    value: "t1".to_owned(),
                }],
            },
            OriginState {
                origin: "https://other.example.test".to_owned(),
                local_storage: Vec::new(),
            },
        ],
    };
    let report = import(
        &jar,
        &[
            shown(5, None),
            shown(6, Some("https://staging.example.test")),
        ],
        &state,
    )
    .await
    .expect("imported");
    assert_eq!(
        report,
        ImportReport {
            cookies: 1,
            origins: 1,
            origins_skipped: 1
        }
    );
    assert_eq!(
        jar.calls(),
        [
            "setCookies 5 1",
            "setItem 6 https://staging.example.test token"
        ]
    );
}

#[tokio::test]
async fn a_hostile_state_is_refused_before_cdp_sees_it() {
    let jar = FakeJar::default();
    let state = StorageState {
        cookies: Vec::new(),
        origins: vec![OriginState {
            origin: "javascript:alert(1)".to_owned(),
            local_storage: Vec::new(),
        }],
    };
    assert!(matches!(
        import(&jar, &[shown(1, None)], &state).await,
        Err(TransferError::State(_))
    ));
    assert!(jar.calls().is_empty());
}

use std::sync::Mutex;

use serde_json::json;

use super::*;
use crate::cdp::lock;
use crate::cookies::model::{SkipReason, cookies_of};
use crate::cookies::plan::Group;

const NOW: u64 = 1_800_000_000;

/// Jars that answer from a script and record what each call got (names only).
#[derive(Default)]
pub(crate) struct FakeJars {
    /// What `read` answers per view; an absent view fails.
    pub cookies: Mutex<BTreeMap<ViewId, Vec<ChromeCookie>>>,
    /// Firefox is not running.
    pub firefox_down: bool,
    /// The calls: `read 1`, `firefox admin sid,theme`, `webkit 3 sid`, `reload 3`.
    pub calls: Mutex<Vec<String>>,
}

#[async_trait]
impl Jars for FakeJars {
    async fn read(&self, view: ViewId) -> Result<Vec<ChromeCookie>, String> {
        lock(&self.calls).push(format!("read {view}"));
        lock(&self.cookies)
            .get(&view)
            .cloned()
            .ok_or_else(|| format!("view {view} has no CDP session yet"))
    }

    async fn set_firefox(&self, session: &str, cookies: &[BidiCookie]) -> Result<Taken, String> {
        if self.firefox_down {
            return Err("Firefox is not running".to_owned());
        }
        let names: Vec<&str> = cookies.iter().map(|cookie| cookie.name.as_str()).collect();
        lock(&self.calls).push(format!("firefox {session} {}", names.join(",")));
        Ok(Taken {
            set: cookies.len(),
            refused: 0,
        })
    }

    async fn set_webkit(
        &self,
        view: ViewId,
        cookies: &[PlaywrightCookie],
    ) -> Result<Taken, String> {
        let names: Vec<&str> = cookies.iter().map(|cookie| cookie.name.as_str()).collect();
        lock(&self.calls).push(format!("webkit {view} {}", names.join(",")));
        Ok(Taken {
            set: cookies.len().saturating_sub(1),
            refused: usize::from(!cookies.is_empty()),
        })
    }

    async fn reload(&self, view: ViewId) -> Result<(), String> {
        lock(&self.calls).push(format!("reload {view}"));
        Ok(())
    }
}

pub(crate) fn jar() -> Vec<ChromeCookie> {
    cookies_of(&json!({ "cookies": [
        { "name": "sid", "value": "hunter2", "domain": "127.0.0.1", "path": "/",
          "httpOnly": true, "session": true },
        { "name": "theme", "value": "dark", "domain": ".example.com", "path": "/",
          "expires": 1_900_000_000.0 },
        { "name": "_ga", "value": "GA1", "domain": "127.0.0.1", "path": "/",
          "expires": 1_900_000_000.0 },
        { "name": "chip", "value": "c", "domain": "127.0.0.1", "path": "/",
          "partitionKey": { "topLevelSite": "http://127.0.0.1" } },
        { "name": "ad", "value": "x", "domain": "ads.example.org", "path": "/" },
    ]}))
}

fn options() -> Options {
    Options {
        domains: None,
        all_domains: false,
        environment_host: None,
        exclude: vec!["_ga*".to_owned()],
        reload: true,
        now: NOW,
    }
}

fn group(session: &str, readers: &[u32], firefox: &[u32], webkit: &[u32]) -> Group {
    let ids = |list: &[u32]| list.iter().copied().map(ViewId).collect();
    Group {
        session: session.to_owned(),
        readers: ids(readers),
        firefox: ids(firefox),
        webkit: ids(webkit),
        hosts: vec!["127.0.0.1".to_owned()],
    }
}

#[tokio::test]
async fn a_session_is_read_once_filtered_and_set_into_firefox_once_and_each_webkit_view() {
    let jars = FakeJars::default();
    lock(&jars.cookies).insert(ViewId(1), jar());
    let plan = Plan {
        groups: vec![group("default", &[1], &[2, 5], &[3])],
        other_session: Vec::new(),
    };
    let report = run(&jars, plan, &options()).await;
    assert_eq!(
        *lock(&jars.calls),
        vec![
            "read 1",
            "firefox default sid",
            "webkit 3 sid",
            "reload 2",
            "reload 5"
        ]
    );
    let skipped = BTreeMap::from([
        (SkipReason::OtherDomain, 2),
        (SkipReason::Excluded, 1),
        (SkipReason::Partitioned, 1),
    ]);
    let firefox = &report.views[0];
    assert_eq!((firefox.view, firefox.copied), (ViewId(2), 1));
    assert_eq!(firefox.skipped, skipped);
    assert!(firefox.reloaded);
    let webkit = &report.views[1];
    assert_eq!(
        (webkit.view, webkit.copied, webkit.refused, webkit.reloaded),
        (ViewId(3), 0, 1, false),
        "nothing taken: no reload"
    );
    assert_eq!(report.copied, 2);
    assert!(
        !serde_json::to_string(&report)
            .expect("json")
            .contains("hunter2")
    );
}

#[tokio::test]
async fn the_domains_and_the_environment_widen_or_narrow_the_sites() {
    let jars = FakeJars::default();
    lock(&jars.cookies).insert(ViewId(1), jar());
    let plan = || Plan {
        groups: vec![group("default", &[1], &[2], &[])],
        other_session: Vec::new(),
    };
    let with_env = Options {
        environment_host: Some("www.example.com".to_owned()),
        reload: false,
        ..options()
    };
    run(&jars, plan(), &with_env).await;
    let all = Options {
        all_domains: true,
        exclude: Vec::new(),
        ..with_env.clone()
    };
    run(&jars, plan(), &all).await;
    let only = Options {
        domains: Some(vec!["ads.example.org".to_owned()]),
        ..all.clone()
    };
    run(&jars, plan(), &only).await;
    let firefox: Vec<String> = lock(&jars.calls)
        .iter()
        .filter(|call| call.starts_with("firefox"))
        .cloned()
        .collect();
    assert_eq!(
        firefox,
        vec![
            "firefox default sid,theme",
            "firefox default sid,theme,_ga,ad",
            "firefox default ad",
        ]
    );
}

#[tokio::test]
async fn a_view_that_cannot_be_read_falls_back_and_an_engine_that_is_down_is_reported() {
    let jars = FakeJars {
        firefox_down: true,
        ..FakeJars::default()
    };
    lock(&jars.cookies).insert(ViewId(4), jar());
    let plan = Plan {
        groups: vec![group("admin", &[9, 4], &[5], &[])],
        other_session: Vec::new(),
    };
    let report = run(&jars, plan, &options()).await;
    assert_eq!(lock(&jars.calls)[..2], ["read 9", "read 4"]);
    assert_eq!(
        report.views[0].error.as_deref(),
        Some("Firefox is not running")
    );
    assert_eq!(report.failed(), 1);
    let unreadable = Plan {
        groups: vec![group("admin", &[9], &[5], &[6])],
        other_session: Vec::new(),
    };
    let report = run(&FakeJars::default(), unreadable, &options()).await;
    assert_eq!(report.failed(), 2);
    assert_eq!(report.copied, 0);
}

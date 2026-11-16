use std::time::Duration;

use super::*;

#[tokio::test]
async fn a_held_answer_keeps_the_events_it_scripted() {
    let (fake, router) = FakeBrowser::connect();
    fake.auto_attach();
    fake.add_target("V1", "https://a.test/");
    let attached = router
        .call("Target.attachToTarget", json!({ "targetId": "V1" }), None)
        .await
        .expect("attaches");
    let session = attached["sessionId"]
        .as_str()
        .expect("a session")
        .to_owned();
    let mut events = router.subscribe();
    fake.hold("Page.navigate", None);
    let navigating = tokio::spawn({
        let router = Arc::clone(&router);
        async move {
            router
                .call(
                    "Page.navigate",
                    json!({ "url": "https://b.test/" }),
                    Some(&session),
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(events.try_recv().is_err(), "nothing before the release");
    fake.release();
    let answer = navigating.await.expect("joins").expect("answers");
    assert_eq!(answer["frameId"], json!("V1"));
    let mut methods = Vec::new();
    while let Ok(event) = events.try_recv() {
        methods.push(event.method);
    }
    assert_eq!(
        methods,
        [
            "Page.frameNavigated",
            "Page.lifecycleEvent",
            "Page.lifecycleEvent"
        ]
    );
}

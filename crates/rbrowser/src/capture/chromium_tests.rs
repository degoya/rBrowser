use std::time::Duration;

use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::ui::recording::RecordingUi;

/// Two captures at once would interleave their `setDeviceMetricsOverride`s: a shot waits for
/// the one before it, before it reads its view.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_shot_waits_for_the_one_before_it() {
    let (_fake, router) = FakeBrowser::connect();
    let host = Host::new(Arc::new(RecordingUi::default()), router, Handle::current());
    let shooter = HostShooter::new(host);
    let held = SHOOTING.lock().await;
    let shot = tokio::spawn(async move {
        shooter
            .shoot(ViewId(99), &crate::capture::Shot::Viewport, 1.0)
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!shot.is_finished(), "the second shot waits");
    drop(held);
    let result = tokio::time::timeout(Duration::from_secs(5), shot)
        .await
        .expect("it ran once the first was done")
        .expect("the task ran");
    assert!(matches!(result, Err(EngineError::UnknownView(ViewId(99)))));
}

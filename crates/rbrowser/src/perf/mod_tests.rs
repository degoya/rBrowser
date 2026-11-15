use std::sync::Arc;

use rb_config::ProjectConfig;
use rb_engine::ViewHost;
use tokio::runtime::Handle;

use crate::cdp::fake::FakeBrowser;
use crate::host::Host;
use crate::host::ui::recording::RecordingUi;

const FIXTURE: &str = include_str!("../../../../tests/fixtures/project-perf/.rbrowser/config.json");

#[tokio::test]
async fn the_k3_fixture_opens_eight_views_two_of_them_at_dpr_3() {
    let config: ProjectConfig = serde_json::from_str(FIXTURE).expect("the fixture parses");
    let (_fake, router) = FakeBrowser::connect();
    let host = Host::new(Arc::new(RecordingUi::default()), router, Handle::current());
    host.open_config(config, None).expect("opens");
    let views = host.views();
    assert_eq!(views.len(), 8);
    let dpr3 = views
        .iter()
        .filter(|view| (view.dpr - 3.0).abs() < f64::EPSILON)
        .count();
    assert_eq!(dpr3, 2);
}

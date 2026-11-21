use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, server, view};
use crate::tools::portal::PortalParams;

fn host(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

#[tokio::test]
async fn both_tools_go_to_the_host() {
    let fake = Arc::new(FakeBackend::new(vec![view(1, "mobile", "http://a.test/")]));
    let server = server(&fake);
    server
        .portal_present_status(Parameters(PortalParams::default()))
        .await
        .expect("answer");
    server
        .portal_present_stop(Parameters(PortalParams::default()))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [
            (host("portal_present_status"), json!({})),
            (host("portal_present_stop"), json!({})),
        ]
    );
}

/// RB-096-05: no tool shares the site on the internet or hands out a link.
#[test]
fn no_tool_starts_a_presentation_or_issues_a_link() {
    let names: Vec<String> = crate::RbMcpServer::portal_present_router()
        .list_all()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    for name in &names {
        assert!(
            name == "portal_present_status" || name == "portal_present_stop",
            "{name}"
        );
    }
}

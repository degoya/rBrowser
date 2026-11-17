use super::*;

#[test]
fn ordinary_page_commands_are_forwarded() {
    for method in [
        "Page.navigate",
        "Runtime.evaluate",
        "Input.dispatchMouseEvent",
        "Target.setAutoAttach",
        "Target.getTargets",
        "Target.attachToTarget",
        "Browser.getVersion",
        "Emulation.setFocusEmulationEnabled",
    ] {
        assert_eq!(verdict(method), Verdict::Forward, "{method}");
    }
}

#[test]
fn the_device_metrics_are_rbrowsers_and_are_answered_without_being_sent() {
    for method in [
        "Emulation.setDeviceMetricsOverride",
        "Emulation.clearDeviceMetricsOverride",
        "Emulation.setVisibleSize",
        "Page.setDeviceMetricsOverride",
        "Page.clearDeviceMetricsOverride",
        "Browser.setWindowBounds",
        "Browser.setContentsSize",
    ] {
        assert_eq!(verdict(method), Verdict::Owned, "{method}");
    }
}

#[test]
fn the_media_emulation_is_rbrowsers_too() {
    // Playwright sets it on every page when it connects; a dark view would turn light.
    assert_eq!(verdict("Emulation.setEmulatedMedia"), Verdict::Owned);
}

#[test]
fn the_download_behaviour_is_rbrowsers() {
    // Playwright's connectOverCDP sets it browser-wide (allowAndName into its temp folder) unless
    // `noDefaults`; it would take rBrowser's own downloads with it.
    assert_eq!(verdict("Browser.setDownloadBehavior"), Verdict::Owned);
}

#[test]
fn what_could_reach_the_shell_or_the_app_is_refused_with_a_reason() {
    for method in [
        "Browser.close",
        "Browser.crash",
        "Browser.crashGpuProcess",
        "Target.createTarget",
        "Target.closeTarget",
        "Target.createBrowserContext",
        "Target.disposeBrowserContext",
        "Target.attachToBrowserTarget",
        "Target.exposeDevToolsProtocol",
        "Target.sendMessageToTarget",
        "Target.setRemoteLocations",
        "Tethering.bind",
    ] {
        match verdict(method) {
            Verdict::Refuse(reason) => assert!(!reason.is_empty(), "{method}"),
            other => panic!("{method}: {other:?}"),
        }
    }
}

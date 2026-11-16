use super::*;

#[test]
fn the_defaults_are_the_configs() {
    let effective = Effective::new(&SitemapConfig::default(), &CaptureArgs::default());
    assert_eq!(effective.batch.parallel, 4);
    assert_eq!(effective.batch.host_interval, Duration::from_millis(250));
    assert_eq!(effective.batch.retries, 1);
    assert!(effective.batch.full_page);
    assert!(effective.batch.wait.network_idle);
    assert!(effective.batch.wait.auto_scroll);
    assert_eq!(effective.batch.wait.timeout, Duration::from_secs(30));
    assert_eq!(effective.batch.wait.selector, None);
    assert_eq!(effective.max_pages, 0);
    assert!(effective.respect_robots);
    assert_eq!(effective.max_bytes, 10 * 1024 * 1024);
}

#[test]
fn values_out_of_bounds_are_held_to_them() {
    let config = SitemapConfig {
        parallel: 99,
        timeout_sec: 1,
        delay_ms: u32::MAX,
        retries: 50,
        max_depth: 50,
        max_sitemap_mib: 0,
        max_urls: u32::MAX,
        wait_selector: "x".repeat(MAX_SELECTOR + 1),
        ..SitemapConfig::default()
    };
    let effective = Effective::new(&config, &CaptureArgs::default());
    assert_eq!(effective.batch.parallel, MAX_PARALLEL);
    assert_eq!(effective.batch.wait.timeout, Duration::from_secs(5));
    assert_eq!(effective.batch.wait.delay, Duration::from_secs(30));
    assert_eq!(effective.batch.retries, MAX_RETRIES);
    assert_eq!(effective.max_depth, 5);
    assert_eq!(effective.max_bytes, 1024 * 1024);
    assert_eq!(effective.max_urls, 200_000);
    assert_eq!(effective.batch.wait.selector, None);
}

#[test]
fn the_calls_arguments_win() {
    let args = CaptureArgs {
        max: Some(20),
        include: Some(vec!["/blog/**".to_owned()]),
        ..CaptureArgs::default()
    };
    let config = SitemapConfig {
        max_pages: 5,
        include: vec!["/shop/**".to_owned()],
        exclude: vec!["/private/**".to_owned()],
        ..SitemapConfig::default()
    };
    let effective = Effective::new(&config, &args);
    assert_eq!(effective.max_pages, 20);
    assert_eq!(effective.include, ["/blog/**"]);
    assert_eq!(effective.exclude, ["/private/**"]);
    let (limits, selection) = effective.discover_args();
    assert_eq!(limits["maxDepth"], 3);
    assert_eq!(selection["max"], 20);
    assert_eq!(selection["respectRobots"], true);
}

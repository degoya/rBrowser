use rb_plugin_api::test_support::MemoryHost;

use super::*;
use crate::testing::serve;

/// A page linking `count` alternates on its own origin, each of them served linking back.
fn chain(count: usize) -> MemoryHost {
    let links: String = (0..count)
        .map(|index| {
            format!(r#"<link rel="alternate" hreflang="x{index}" href="https://a.test/p{index}/">"#)
        })
        .collect();
    let page = format!("<html><head>{links}</head><body></body></html>");
    let mut host = serve("https://a.test/p0/", &page, &[]);
    for index in 1..count {
        let other = serve(&format!("https://a.test/p{index}/"), &page, &[]);
        host.responses.extend(other.responses);
    }
    host
}

#[test]
fn a_set_is_cut_at_its_limit_and_says_so() {
    let mut host = chain(MAX_SET + 5);
    let start = Url::parse("https://a.test/p0/").expect("url");
    let site = Site::load(&mut host, &start);
    let config = Config::default();
    let only = vec!["hreflang.target_unchecked".to_owned()];
    let mut sink = Sink::new(&config, &only);
    let report = run(&mut host, &config, &site, &start, None, &mut sink);
    assert_eq!(report.pages.len(), MAX_SET);
    let pages = host
        .fetched
        .iter()
        .filter(|request| request.url.contains("/p"))
        .count();
    assert_eq!(pages, MAX_SET);
    assert_eq!(sink.findings.len(), 1);
    assert!(
        sink.findings[0]
            .params
            .contains(&("count".to_owned(), "5".to_owned()))
    );
}

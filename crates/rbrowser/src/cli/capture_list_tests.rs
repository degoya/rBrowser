use clap::Parser as _;
use rb_ipc::SitemapDiscovery;

use super::*;
use crate::cli::capture::{CaptureCliArgs, CaptureCommand};
use crate::cli::{Cli, Command};

fn parse(args: &[&str]) -> Result<SitemapCliArgs, clap::Error> {
    let cli = Cli::try_parse_from(args)?;
    match cli.command {
        Some(Command::Capture(CaptureCliArgs {
            command: CaptureCommand::Sitemap(args),
        })) => Ok(args),
        other => panic!("not capture sitemap: {other:?}"),
    }
}

#[test]
fn the_url_file_goes_over_as_its_project_path_not_its_urls() {
    let project = tempfile::tempdir().expect("temp");
    let root = project.path();
    std::fs::create_dir_all(root.join("lists")).expect("folder");
    let urls: String = (0..crate::sitemap::tool::MAX_PAGES)
        .map(|n| format!("https://a.test/{n}\n"))
        .collect();
    std::fs::write(root.join("lists/pages.txt"), &urls).expect("written");
    let capture = with_urls_file(
        CaptureArgs::default(),
        Some(Path::new("pages.txt")),
        &root.join("lists"),
        root,
    )
    .expect("inside the project");
    assert_eq!(capture.urls_file.as_deref(), Some("lists/pages.txt"));
    assert_eq!(capture.urls, None);
    // 100000 URLs in the file, a few bytes on the channel.
    let line = serde_json::to_vec(&Request::SitemapCapture { args: capture }).expect("json");
    assert!(urls.len() > crate::instance::protocol::MAX_LINE);
    assert!(
        line.len() < crate::instance::protocol::MAX_LINE,
        "{}",
        line.len()
    );
    assert_eq!(
        with_urls_file(CaptureArgs::default(), None, root, root),
        Ok(CaptureArgs::default())
    );
}

#[test]
fn a_url_file_outside_the_project_or_of_another_kind_is_refused() {
    let project = tempfile::tempdir().expect("temp");
    let elsewhere = tempfile::tempdir().expect("temp");
    std::fs::write(elsewhere.path().join("pages.txt"), "https://a.test/\n").expect("written");
    std::fs::write(project.path().join("pages.xml"), "<urlset/>").expect("written");
    let refused = |file: &Path| {
        with_urls_file(
            CaptureArgs::default(),
            Some(file),
            project.path(),
            project.path(),
        )
        .is_err()
    };
    assert!(refused(&elsewhere.path().join("pages.txt")));
    assert!(refused(Path::new("pages.xml")));
    assert!(refused(Path::new("missing.txt")));
}

#[test]
fn the_listing_reads_the_discovery_the_instance_wrote() {
    let project = tempfile::tempdir().expect("temp");
    let discovery = SitemapDiscovery {
        discovery_id: "1795000000-3".to_owned(),
        pages: (0..10_000).map(|n| format!("https://a.test/{n}")).collect(),
        ..SitemapDiscovery::default()
    };
    let written =
        crate::sitemap::discovery::write_discovery(project.path(), &discovery).expect("written");
    assert_eq!(read_discovery(&written, project.path()), Ok(discovery));
    let forged = DiscoveryFile {
        discovery_id: "../../secret".to_owned(),
        ..written
    };
    assert!(read_discovery(&forged, project.path()).is_err());
}

#[test]
fn list_and_urls_read_from_the_command_line() {
    let listed = parse(&[
        "rbrowser",
        "capture",
        "sitemap",
        "--list",
        "--include",
        "/de/**",
    ])
    .expect("parses");
    assert!(listed.list);
    let discover = discover_args(&listed);
    assert_eq!(discover.include, Some(vec!["/de/**".to_owned()]));
    let chosen = parse(&["rbrowser", "capture", "sitemap", "--urls", "pages.txt"]).expect("parses");
    assert_eq!(chosen.urls.as_deref(), Some(Path::new("pages.txt")));
    assert!(parse(&["rbrowser", "capture", "sitemap", "--list", "--resume", "r1"]).is_err());
    assert!(parse(&["rbrowser", "capture", "sitemap", "--list", "--visible"]).is_err());
    assert!(
        parse(&[
            "rbrowser", "capture", "sitemap", "--urls", "a.txt", "--resume", "r1"
        ])
        .is_err()
    );
}

#[test]
fn the_listing_is_one_url_a_line_or_the_whole_discovery() {
    let discovery = SitemapDiscovery {
        discovery_id: "1-1".to_owned(),
        pages: vec!["https://a.test/".to_owned(), "https://a.test/de".to_owned()],
        ..SitemapDiscovery::default()
    };
    assert_eq!(
        listing(&discovery, false),
        "https://a.test/\nhttps://a.test/de"
    );
    let json: serde_json::Value = serde_json::from_str(&listing(&discovery, true)).expect("json");
    assert_eq!(json["discoveryId"], "1-1");
}

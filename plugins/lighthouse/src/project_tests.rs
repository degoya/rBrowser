use super::*;
use crate::fake::FakeHost;

const CONFIG: &str = r#"{
  "version": 1,
  "breakpoints": [
    { "id": "mobile", "width": 375, "height": 812, "touch": true, "userAgent": "Phone UA" },
    { "id": "desktop", "width": 1440, "height": 900, "mobile": false },
    { "label": "no id" }
  ],
  "checks": { "lighthouse": { "categories": ["performance", "pwa", 3] } }
}"#;

#[test]
fn reads_the_categories_and_the_breakpoints() {
    let project = Project::parse(CONFIG);
    assert_eq!(project.categories, ["performance"]);
    assert_eq!(
        project.breakpoints["mobile"],
        BreakpointExtras {
            touch: Some(true),
            mobile: None,
            user_agent: Some("Phone UA".to_owned()),
        }
    );
    assert_eq!(project.breakpoints["desktop"].mobile, Some(false));
    assert_eq!(project.breakpoints.len(), 2);
}

#[test]
fn a_config_that_does_not_read_leaves_the_defaults() {
    assert_eq!(Project::parse("{ not json"), Project::default());
    assert_eq!(Project::parse("[]"), Project::default());
}

#[test]
fn reads_the_file_through_the_host_and_survives_its_absence() {
    let mut host = FakeHost::default().with_file(CONFIG_PATH, CONFIG);
    assert_eq!(Project::read(&mut host).categories, ["performance"]);
    let mut empty = FakeHost::default();
    assert_eq!(Project::read(&mut empty), Project::default());
    assert_eq!(empty.logs.len(), 1);
}

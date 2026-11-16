use std::fs;

use rb_commands::McpCoverage;

use super::{PENDING, check, orphans, pending, pending_job_problems, problems};
use crate::RbMcpServer;

fn covered(tool: &str) -> McpCoverage {
    McpCoverage::Covered(tool.to_owned())
}

fn omitted(reason: &str) -> McpCoverage {
    McpCoverage::Omitted(reason.to_owned())
}

#[test]
fn a_registered_a_pending_and_an_omitted_tool_pass() {
    let navigate = covered("navigate");
    let sync = covered("sync_set");
    let omitted = omitted("ui-only");
    let entries = [
        ("view.navigate", &navigate),
        ("sync.toggle", &sync),
        ("settings.open", &omitted),
    ];
    assert_eq!(check(&entries), Vec::<String>::new());
}

#[test]
fn a_tool_nobody_registers_is_a_problem() {
    let teleport = covered("teleport");
    assert_eq!(
        check(&[("view.navigate", &teleport)]),
        vec!["view.navigate: covered(teleport) names no registered or pending tool".to_owned()]
    );
    let blank = omitted(" ");
    assert_eq!(
        check(&[("view.zoom_in", &blank)]),
        vec!["view.zoom_in: omitted without a reason".to_owned()]
    );
}

#[test]
fn a_reason_is_plain_english_not_a_key_a_placeholder_or_a_sentinel() {
    let cases = [
        ("commands.view.zoom_in", "an i18n key"),
        ("ui-only \u{2014} grid zoom", "not ASCII"),
        ("mcp: RB-030-01", "names the job RB-030-01"),
        ("undeclared", "declares neither a tool nor a reason"),
        (
            "no mcp_prefix",
            "names a tool but its plugin has no mcp_prefix",
        ),
    ];
    for (reason, why) in cases {
        let entry = omitted(reason);
        let found = check(&[("plugin.demo.run", &entry)]);
        assert_eq!(found.len(), 1, "{reason}: {found:?}");
        assert!(found[0].starts_with("plugin.demo.run: "), "{found:?}");
        assert!(found[0].contains(why), "{reason}: {found:?}");
    }
    let fine = omitted("ui-only: grid zoom");
    assert_eq!(check(&[("view.zoom_in", &fine)]), Vec::<String>::new());
}

#[test]
fn a_pending_tool_that_got_registered_must_leave_the_list() {
    // Each pending tool, as if it were registered (none is pending since RB-040-05).
    for (tool, _) in PENDING {
        let tools = vec![(*tool).to_owned()];
        assert_eq!(
            problems(&[], &tools),
            vec![format!("{tool}: registered and still listed as pending")]
        );
    }
}

#[test]
fn no_pending_tool_is_registered_and_each_names_one_job() {
    let tools = RbMcpServer::tool_names();
    for (tool, job) in PENDING {
        assert!(!tools.iter().any(|known| known == tool), "{tool}");
        let digits: Vec<&str> = job.strip_prefix("RB-").unwrap_or("").split('-').collect();
        assert!(
            digits.len() == 2 && digits.iter().all(|part| part.len() >= 2),
            "{tool}: {job} is not one job id"
        );
    }
    // RB-040-05 registered `cookies_copy`.
    assert_eq!(pending("cookies_copy"), None);
    assert_eq!(pending("flow_list"), None);
    assert_eq!(pending("click"), None);
    assert_eq!(pending("navigate"), None);
}

#[test]
fn a_tool_no_command_names_is_an_orphan_unless_it_says_why() {
    let tools = vec![
        "navigate".to_owned(),
        "views_list".to_owned(),
        "teleport".to_owned(),
    ];
    let navigate = covered("navigate");
    assert_eq!(
        orphans(&[("view.navigate", &navigate)], &tools),
        vec!["teleport: no command names it and it is not listed in WITHOUT_COMMAND".to_owned()]
    );
}

#[test]
fn a_tool_listed_without_command_that_a_command_names_is_stale() {
    let tools = vec!["views_list".to_owned()];
    let views = covered("views_list");
    assert_eq!(
        orphans(&[("views.list", &views)], &tools),
        vec!["views_list: a command names it, so it leaves WITHOUT_COMMAND".to_owned()]
    );
}

fn job(root: &std::path::Path, dir: &str, name: &str, status: &str) {
    let dir = root.join("docs/roadmap/jobs").join(dir);
    fs::create_dir_all(&dir).expect("job dir");
    fs::write(
        dir.join(name),
        format!("# RB\n\n- **Milestone:** 0.2\n- **Status:** {status}\n"),
    )
    .expect("job file");
}

#[test]
fn a_pending_job_must_exist_and_must_not_be_done() {
    let root = tempfile::tempdir().expect("temp dir");
    let root = root.path();
    job(root, "", "020-02-synchub-leader.md", "Open");
    job(root, "", "020-03-input-replay.md", "In progress");
    job(root, "archive", "050-03-screenshots.md", "Implemented");
    job(
        root,
        "",
        "060-01-findings.md",
        "Partial (Abnahme offen: Windows)",
    );
    let pending = [
        ("sync_set", "RB-020-02"),
        ("click", "RB-020-03"),
        ("screenshot", "RB-050-03"),
        ("checks_run", "RB-060-01"),
        ("flow_run", "RB-040-07"),
    ];
    assert_eq!(
        pending_job_problems(root, &pending),
        vec![
            "screenshot: pending on RB-050-03, which is Implemented - register the tool or \
             name the job that brings it"
                .to_owned(),
            "checks_run: pending on RB-060-01, which is Partial (Abnahme offen: Windows) - \
             register the tool or name the job that brings it"
                .to_owned(),
            "flow_run: pending on RB-040-07, which has no job file".to_owned(),
        ]
    );
}

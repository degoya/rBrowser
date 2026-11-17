use super::{Analyzer, valid_pointer};
use crate::test_support::MemoryHost;
use crate::{Capability, Failure, PluginHost, Proposal, ViewInfo};

/// Proposes a breakpoint at every distinct view width it sees: the shape of the
/// `breakpoint-analyzer` (RB-010-13), without WebAssembly.
struct WidthsAnalyzer;

impl Analyzer for WidthsAnalyzer {
    fn analyze(
        &mut self,
        host: &mut dyn PluginHost,
        _args: &str,
    ) -> Result<Vec<Proposal>, Failure> {
        let mut widths: Vec<u32> = host.views()?.iter().map(|view| view.width).collect();
        widths.sort_unstable();
        widths.dedup();
        host.log("info", &format!("{} widths", widths.len()));
        Ok(widths
            .into_iter()
            .enumerate()
            .map(|(index, width)| Proposal {
                pointer: format!("/breakpoints/{index}/width"),
                value: width.to_string(),
                reason: "plugin.widths.reason.seen".to_owned(),
                params: Vec::new(),
            })
            .collect())
    }
}

#[test]
fn an_analyzer_runs_natively_against_the_memory_host() {
    let mut host = MemoryHost::new().grant(Capability::Views);
    for (id, width) in [(1, 375), (2, 1280), (3, 375)] {
        host.views.push(ViewInfo {
            id,
            width,
            ..ViewInfo::default()
        });
    }
    let proposals = WidthsAnalyzer.analyze(&mut host, "{}").expect("analyzed");
    assert_eq!(proposals.len(), 2);
    assert!(
        proposals
            .iter()
            .all(|proposal| valid_pointer(&proposal.pointer))
    );
    assert_eq!(host.logs, vec![("info".to_owned(), "2 widths".to_owned())]);
}

#[test]
fn an_analyzer_without_views_learns_why() {
    let refused = WidthsAnalyzer
        .analyze(&mut MemoryHost::new(), "{}")
        .expect_err("views not granted");
    assert_eq!(refused.param("capability"), Some("views"));
}

#[test]
fn pointers_follow_rfc_6901() {
    for pointer in ["", "/breakpoints/0/width", "/a~0b/c~1d", "/"] {
        assert!(valid_pointer(pointer), "{pointer}");
    }
    for pointer in ["breakpoints", "/a~2", "/a~", "/a\nb"] {
        assert!(!valid_pointer(pointer), "{pointer}");
    }
}

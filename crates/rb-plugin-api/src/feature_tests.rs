use super::Feature;
use crate::test_support::MemoryHost;
use crate::{Capability, Failure, FailureKind, HttpRequest, PluginHost, codes};

/// Fetches a URL and saves the status into its out folder: two capabilities in one command.
struct StatusSaver;

impl Feature for StatusSaver {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        if command != "save" {
            return Err(
                Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
                    .with_param("name", command),
            );
        }
        let answer = host.fetch(&HttpRequest {
            method: "GET".to_owned(),
            url: args.to_owned(),
            ..HttpRequest::default()
        })?;
        host.out_write("status.txt", answer.status.to_string().as_bytes())?;
        Ok(answer.status.to_string())
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        self.run_command(host, tool, args)
    }
}

#[test]
fn a_feature_runs_its_command_with_the_capabilities_granted() {
    let mut host = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::OutWrite);
    let mut plugin = StatusSaver;
    plugin.activate(&mut host).expect("default activate");
    let answer = plugin
        .run_command(&mut host, "save", "https://example.org/")
        .expect("ran");
    assert_eq!(answer, "404");
    assert_eq!(
        host.out.get("status.txt").map(Vec::as_slice),
        Some(&b"404"[..])
    );
}

#[test]
fn a_feature_without_http_gets_the_capability_failure() {
    let mut host = MemoryHost::new().grant(Capability::OutWrite);
    let refused = StatusSaver
        .call_tool(&mut host, "save", "https://example.org/")
        .expect_err("http not granted");
    assert_eq!(refused.code(), Some(codes::CAPABILITY_DENIED));
    assert_eq!(refused.param("capability"), Some("http"));
    assert!(host.out.is_empty());
}

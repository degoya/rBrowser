//! What a launch decides before CEF starts: the browser's own switches and the window title.

use rb_config::NetworkConfig;

/// The switches the browser process adds to its command line: the CDP pipe (ADR 0001,
/// Decision 4; no port) and the project's network settings, which apply to this instance only.
pub fn browser_switches(
    pipe: Option<&str>,
    network: &NetworkConfig,
) -> Vec<(String, Option<String>)> {
    let mut switches = Vec::new();
    if let Some(pipe) = pipe {
        switches.push(("remote-debugging-pipe".to_owned(), None));
        switches.push((
            "remote-debugging-io-pipes".to_owned(),
            Some(pipe.to_owned()),
        ));
    }
    if let Some(rules) = network
        .host_resolver_rules
        .as_ref()
        .filter(|rules| !rules.is_empty())
    {
        switches.push(("host-resolver-rules".to_owned(), Some(rules.clone())));
    }
    if let Some(proxy) = network.proxy.as_ref().filter(|proxy| !proxy.is_empty()) {
        // A PAC script is given as its URL; anything else is the proxy server itself.
        let path = proxy.split(['?', '#']).next().unwrap_or_default();
        let name = if path.to_ascii_lowercase().ends_with(".pac") {
            "proxy-pac-url"
        } else {
            "proxy-server"
        };
        switches.push((name.to_owned(), Some(proxy.clone())));
    }
    if network.ignore_certificate_errors {
        switches.push(("ignore-certificate-errors".to_owned(), None));
    }
    switches
}

/// The window title: the project's folder name, then the product.
pub fn window_title(project: Option<&std::path::Path>) -> String {
    let name = project.and_then(|root| {
        let written = root.to_string_lossy().replace('/', "\\");
        written
            .trim_end_matches('\\')
            .rsplit('\\')
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    });
    match name {
        Some(name) => format!("{name} - rBrowser"),
        None => "rBrowser".to_owned(),
    }
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;

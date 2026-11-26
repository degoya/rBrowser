//! What a launch decides before CEF starts: the browser's own switches and the window title.
//! The main window's title follows `ui.titleBar` (RB-094-19) at once
//! ([`on_user_settings_changed`]); the other windows keep their own titles.

use rb_config::{NetworkConfig, TitleBarSettings};

use crate::control::Core;

/// The credit line at the end of the main window's title (RB-094-19): the owner's, fixed text,
/// never translated.
pub const CREDIT: &str = "made with Love by Alexander Herling/DEGOYA.digital";

/// Chromium's own sign-in prompt off, so every Basic auth challenge reaches the app's
/// `GetAuthCredentials` (`cef_app/auth.rs`): with the prompt on, CEF hands a browser's challenge
/// to the prompt and never asks the handler, so no stored credential could answer (RB-099-04).
pub const LOGIN_PROMPT_OFF: &str = "disable-chrome-login-prompt";

/// The switches the browser process adds to its command line: the CDP pipe (ADR 0001,
/// Decision 4; no port), the project's network settings, which apply to this instance only, and
/// [`LOGIN_PROMPT_OFF`].
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
    switches.push((LOGIN_PROMPT_OFF.to_owned(), None));
    switches
}

/// The main window's title: the project's folder name, then the product, its version
/// (`ui.titleBar.showVersion`, as About shows it) and the credit line
/// (`ui.titleBar.showCredit`): `shop - rBrowser 0.9.4 - made with Love by ...`.
pub fn window_title(project: Option<&std::path::Path>, bar: &TitleBarSettings) -> String {
    let name = project.and_then(|root| {
        let written = root.to_string_lossy().replace('/', "\\");
        written
            .trim_end_matches('\\')
            .rsplit('\\')
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    });
    let mut title = name.map_or_else(String::new, |name| format!("{name} - "));
    title.push_str("rBrowser");
    if bar.show_version {
        title.push(' ');
        title.push_str(env!("CARGO_PKG_VERSION"));
    }
    if bar.show_credit {
        title.push_str(" - ");
        title.push_str(CREDIT);
    }
    title
}

/// The user settings changed: the main window takes its title again, `ui.titleBar` may have.
pub fn on_user_settings_changed(core: &Core) {
    let project = core.host().project_root();
    let title = window_title(project.as_deref(), &core.user_settings().ui.title_bar);
    core.ui().set_window_title(title);
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;

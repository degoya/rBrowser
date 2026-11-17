//! The WebKit engine against a real sidecar and a real WebKit build (ignored: it needs Node, the
//! built sidecar and an installed WebKit, and opens windows). Run it where all three exist:
//!
//! ```text
//! (cd sidecar && npm run build)
//! RB_WEBKIT_E2E_NODE=/path/to/node RB_WEBKIT_E2E_BROWSERS=/path/to/ms-playwright \
//!   cargo nextest run -p rbrowser --run-ignored only -E 'test(/webkit::e2e/)'
//! ```
//!
//! `RB_WEBKIT_E2E_BROWSERS` holds `webkit-<revision>/` with `RBROWSER_PIN` naming the pinned
//! SHA-256 (`sidecar/src/webkit/pin.ts`) - on Windows what `engines.update` installs; elsewhere
//! Playwright's own build of that revision with that file written by hand. The K11 timing (p95
//! under 300 ms) is measured on Windows by the acceptance run, not here.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rb_config::EngineKind;
use rb_engine::test_support::conformance::sample_spec;
use rb_engine::{Engine, ScreenshotRequest};
use rb_sidecar::{NodeLauncher, Sidecar};
use tokio::runtime::Handle;

use super::{AppSidecar, WebkitEngine};
use crate::sync_agent::AGENT_SOURCE;

fn engine() -> Option<Arc<WebkitEngine>> {
    let node = std::env::var("RB_WEBKIT_E2E_NODE").ok()?;
    let browsers = std::env::var_os("RB_WEBKIT_E2E_BROWSERS")?;
    let launcher = NodeLauncher {
        install_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        node_path: Some(node),
        browsers_path: PathBuf::from(browsers),
        lighthouse_browser: None,
    };
    let sidecar = Arc::new(Sidecar::new(launcher, Duration::from_secs(60)));
    Some(WebkitEngine::new(
        Arc::new(AppSidecar(sidecar)),
        Handle::current(),
        AGENT_SOURCE,
    ))
}

/// The width and height of a PNG, from its IHDR chunk (bytes 16 to 24: the first 32 base64
/// characters).
fn png_size(base64: &str) -> Option<(u32, u32)> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    for chunk in base64.as_bytes().get(..32)?.chunks(4) {
        let mut word = 0u32;
        for &c in chunk {
            let digit = ALPHABET.iter().position(|&a| a == c)?;
            word = (word << 6) | u32::try_from(digit).ok()?;
        }
        bytes.extend_from_slice(&word.to_be_bytes()[1..]);
    }
    let at = |offset: usize| -> Option<u32> {
        Some(u32::from_be_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    Some((at(16)?, at(20)?))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Node, the built sidecar and an installed WebKit (RB_WEBKIT_E2E_*)"]
async fn real_webkit_opens_a_phone_view_at_its_size_and_density() {
    let Some(webkit) = engine() else {
        panic!("set RB_WEBKIT_E2E_NODE and RB_WEBKIT_E2E_BROWSERS");
    };
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/site/sync.html");
    let fixture = fixture.canonicalize().expect("the fixture exists");
    let mut spec = sample_spec(EngineKind::Webkit, 1);
    spec.url = format!("file://{}", fixture.display());
    let phone = webkit.create_view(&spec).await.expect("opens");
    assert_eq!(webkit.health(), rb_engine::EngineHealth::Running);
    phone.reload(false).await.expect("reloads");
    let report = phone
        .apply_emulation(&spec.emulation)
        .await
        .expect("emulates");
    assert!(
        report.unsupported().is_empty(),
        "{:?}",
        report.unsupported()
    );
    // 390 x 844 at DPR 3: the viewport screenshot is 1170 x 2532 device pixels.
    let shot = phone
        .screenshot(ScreenshotRequest::default())
        .await
        .expect("a screenshot");
    assert_eq!(png_size(&shot.png_base64), Some((1170, 2532)));
    phone.close().await.expect("closes");
    webkit.shutdown().await;
}

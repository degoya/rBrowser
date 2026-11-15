//! A core over the fake browser for the tests of the layout modes: three breakpoints in one row,
//! a canvas preset and a split preset, the grid area laid out at 1600 x 900.

use std::sync::Arc;

use rb_config::ProjectConfig;
use rb_engine::{CellRect, ViewHost, ViewId};
use serde_json::json;
use tokio::runtime::Handle;

use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::nav::marker_url;
use crate::host::ui::recording::RecordingUi;

/// The grid area of the tests.
pub(crate) const AREA: CellRect = CellRect {
    x: 0,
    y: 44,
    width: 1600,
    height: 900,
};

/// The config of the tests.
pub(crate) fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "startUrl": "http://localhost:3000/",
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "mobile", "width": 400, "height": 800 }
        ],
        "layouts": { "presets": {
            "board": { "mode": "canvas", "snap": 8, "items": [
                { "breakpoint": "mobile", "x": 0, "y": 20, "zoom": 0.5 }
            ] },
            "pair": { "mode": "split", "ratio": 0.6, "primary": ["desktop"] }
        } }
    }))
    .expect("the test config parses")
}

/// The core, its window and fake browser.
pub(crate) struct Setup {
    pub(crate) ui: Arc<RecordingUi>,
    pub(crate) core: Arc<Core>,
    _fake: Arc<FakeBrowser>,
}

/// A core with `config` open and the grid area laid out.
pub(crate) fn setup(config: ProjectConfig) -> Setup {
    setup_in(config, None)
}

/// [`setup`] for the project at `root`.
pub(crate) fn setup_in(config: ProjectConfig, root: Option<std::path::PathBuf>) -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config, root).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    crate::presets::on_start(&core);
    core.host().on_grid_area(AREA);
    Setup {
        ui,
        core,
        _fake: fake,
    }
}

/// The view ids, sorted.
pub(crate) fn view_ids(setup: &Setup) -> Vec<ViewId> {
    let mut ids: Vec<ViewId> = setup
        .core
        .host()
        .views()
        .iter()
        .map(|view| view.id)
        .collect();
    ids.sort();
    ids
}

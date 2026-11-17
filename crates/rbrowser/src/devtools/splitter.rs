//! The splitter between the grid and docked DevTools (RB-010-06). CEF Views give a panel no
//! mouse events, so the splitter is a thin browser view of its own (`cef_app/splitter.rs`)
//! showing [`page_url`]: a page of a few lines that captures the pointer on a press and reports
//! the drag in its title (`rb-splitter:move:<n>:<delta>`, `rb-splitter:end:<n>:<delta>`, the
//! sequence `n` makes every title new). The page has no `cefQuery`, loads nothing and opens
//! nothing; the title is all it can say, and only these two shapes are read.
//!
//! While dragged the dock follows live; the end of the drag becomes the setting
//! `devtools.dockHeight` (docked below) or `devtools.dockWidth` (docked right), kept in the
//! settings file.

use rb_config::DevtoolsSettings;
use rb_ipc::CoreToShell;
use serde_json::json;

use super::layout::{DockSizes, MAX_DOCK, MIN_DOCK, side_of};
use super::{Devtools, for_host};
use crate::cdp::lock;
use crate::control::Core;
use crate::host::ui::DockSide;

/// The splitter's thickness in DIP.
pub const THICKNESS: u32 = 6;
/// The title prefix of a move.
const MOVE: &str = "rb-splitter:move:";
/// The title prefix of the drag's end.
const END: &str = "rb-splitter:end:";
/// The largest delta read, in DIP: more than any screen.
const MAX_DELTA: i64 = 200_000;
/// The scheme and type of [`page_url`].
const DATA_PREFIX: &str = "data:text/html;charset=utf-8,";

/// The page: the bar, its cursor and the drag. Its axis follows its shape: wider than tall is
/// the bar below the grid (vertical drag), taller than wide the bar beside it.
const PAGE: &str = r"<!doctype html><meta charset=utf-8><style>html,body{margin:0;height:100%;overflow:hidden;background:#8080804d;cursor:ns-resize;user-select:none}html:hover{background:#80808099}@media (orientation:portrait){html,body{cursor:ew-resize}}</style><script>
let start=null,last=0,n=0,frame=0;
const axis=()=>innerWidth<innerHeight?'screenX':'screenY';
const say=(kind,delta)=>{document.title='rb-splitter:'+kind+':'+(++n)+':'+Math.round(delta)};
addEventListener('pointerdown',e=>{if(e.button!==0)return;start=e[axis()];last=0;document.documentElement.setPointerCapture(e.pointerId);e.preventDefault()});
addEventListener('pointermove',e=>{if(start===null)return;last=e[axis()]-start;if(!frame)frame=requestAnimationFrame(()=>{frame=0;if(start!==null)say('move',last)})});
const end=e=>{if(start===null)return;if(e.type==='pointerup')last=e[axis()]-start;start=null;say('end',last)};
addEventListener('pointerup',end);addEventListener('pointercancel',end);addEventListener('contextmenu',e=>e.preventDefault());
</script>";

/// What the page reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drag {
    /// The pointer moved by this many DIP from where the drag began.
    Move(i32),
    /// The drag ended this many DIP from where it began.
    End(i32),
}

/// The splitter page as a `data:` URL.
pub fn page_url() -> String {
    let mut url = String::from(DATA_PREFIX);
    for byte in PAGE.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~!*()=;:,/?@+&{}[]|^`$".contains(&byte) {
            url.push(char::from(byte));
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

/// Whether the splitter may load `url`: a page of its own kind only, never a web page. (Only a
/// load the browser starts reaches a top-level `data:` URL, and the splitter's is its own;
/// Chromium may re-escape it, so the scheme and type are what is compared.)
pub fn may_load(url: &str) -> bool {
    url.starts_with(DATA_PREFIX)
}

/// The report in `title`, when it is one; anything else is no report.
pub fn parse_title(title: &str) -> Option<Drag> {
    let (end, rest) = if let Some(rest) = title.strip_prefix(MOVE) {
        (false, rest)
    } else {
        (true, title.strip_prefix(END)?)
    };
    let (sequence, delta) = rest.split_once(':')?;
    sequence.parse::<u64>().ok()?;
    let delta: i64 = delta.parse().ok()?;
    if delta.abs() > MAX_DELTA {
        return None;
    }
    let delta = i32::try_from(delta).ok()?;
    Some(if end {
        Drag::End(delta)
    } else {
        Drag::Move(delta)
    })
}

/// The dock's size after a drag of `delta` DIP from `start`: towards the grid (up below it,
/// left beside it) the dock grows. Held between [`MIN_DOCK`] and [`MAX_DOCK`]; the dock's
/// delegate keeps the grid's share of the window on top.
pub fn dragged(start: u32, delta: i32) -> u32 {
    let size = i64::from(start) - i64::from(delta);
    u32::try_from(size.clamp(i64::from(MIN_DOCK), i64::from(MAX_DOCK))).unwrap_or(MIN_DOCK)
}

/// The dock sizes the settings give.
pub fn sizes_of(settings: &DevtoolsSettings) -> DockSizes {
    DockSizes {
        bottom: settings.dock_height,
        right: settings.dock_width,
    }
}

/// The splitter's page changed its title (CEF side, UI thread): a move lays the dock out live,
/// the end of a drag becomes the setting. Nothing happens while DevTools are not docked.
pub fn on_title(core: &Core, title: &str) {
    let Some(drag) = parse_title(title) else {
        return;
    };
    let Some(devtools) = for_host(core.host()) else {
        return;
    };
    let Some(side) = devtools.current().and_then(|open| side_of(open.mode)) else {
        *lock(devtools.drag_start()) = None;
        return;
    };
    let start = *lock(devtools.drag_start())
        .get_or_insert_with(|| sizes_of(&core.user_settings().devtools).size_for(side));
    match drag {
        Drag::Move(delta) => {
            devtools.redock(Some(dragged(start, delta)));
        }
        Drag::End(delta) => {
            *lock(devtools.drag_start()) = None;
            keep(core, &devtools, side, dragged(start, delta));
        }
    }
}

/// Makes `size` the setting of `side`, in the settings file too, tells the shell pages and lays
/// the dock out at it.
fn keep(core: &Core, devtools: &Devtools, side: DockSide, size: u32) {
    let mut user = core.user_settings();
    let key = match side {
        DockSide::Bottom => {
            user.devtools.dock_height = size;
            "dockHeight"
        }
        DockSide::Right => {
            user.devtools.dock_width = size;
            "dockWidth"
        }
    };
    core.replace_user_settings(user);
    // The file is written off the calling (UI) thread.
    if let Some(path) = core.user_settings_path().map(std::path::Path::to_path_buf) {
        core.runtime().spawn_blocking(move || {
            let written = crate::settings_window::user_file::update(&path, |root| {
                let section = root.entry("devtools").or_insert_with(|| json!({}));
                if !section.is_object() {
                    *section = json!({});
                }
                section[key] = json!(size);
            });
            if let Err(error) = written {
                tracing::warn!(%error, "the DevTools dock size was not saved");
            }
        });
    }
    // An open settings window shows the new value.
    core.send(&CoreToShell::SettingsChanged {
        keys: vec![format!("devtools.{key}")],
        values: None,
    });
    devtools.redock(None);
}

#[cfg(test)]
#[path = "splitter_tests.rs"]
mod tests;

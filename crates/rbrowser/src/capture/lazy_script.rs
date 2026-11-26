//! The page scripts of the lazy-loading pre-pass (RB-099-28, [`super::lazy`]). Both run in an
//! isolated world - the sync agent's or rBrowser's capture world - so the page neither sees nor
//! can stop them; every value in them is a number or JSON.
//!
//! - [`step`]: sets the sync agent's echo marker (`replay { ms }`; nothing the scroll causes is
//!   captured and sent to the other views), scrolls the document instantly to a position, waits
//!   for two animation frames (an `IntersectionObserver` reports after the frame that moved the
//!   page; [`FRAME_GIVE_UP_MS`] when the page paints none), and answers where the page is.
//! - [`images`]: waits until every image that is still loading and shown across the viewport's
//!   width is decoded (a broken one counts as done), at most the given time, and answers how
//!   many are still loading.

use std::time::Duration;

use rb_engine::EngineError;
use rb_sync::{AGENT_GLOBAL, AgentCommand};
use serde_json::Value;

use super::lazy::{Scroll, Seen};

/// The longest wait for the two frames after a scroll, in ms: a page out of sight paints none.
pub const FRAME_GIVE_UP_MS: u32 = 100;

/// `value` when finite, else 0.
fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

/// The step script: the echo marker for `marker_ms`, the scroll to `to` (none: only read), and
/// the answer `{x, y, height, viewport}` in CSS px.
pub fn step(to: Option<Scroll>, marker_ms: u32) -> Result<String, EngineError> {
    let marker = serde_json::to_string(&AgentCommand::Replay { ms: marker_ms })
        .map_err(|error| EngineError::Invalid(error.to_string()))?;
    let scroll = match to {
        Some(to) => format!(
            "window.scrollTo({{left:{},top:{},behavior:'instant'}});await frames();",
            finite(to.x),
            finite(to.y)
        ),
        None => String::new(),
    };
    Ok(format!(
        "(async()=>{{const agent=globalThis.{AGENT_GLOBAL};if(agent)agent.command({marker});\
         const frames=()=>new Promise((done)=>{{const late=setTimeout(done,{FRAME_GIVE_UP_MS});\
         requestAnimationFrame(()=>requestAnimationFrame(()=>{{clearTimeout(late);done();}}));}});\
         {scroll}const root=document.scrollingElement||document.documentElement;\
         return{{x:window.scrollX,y:window.scrollY,height:root.scrollHeight,\
         viewport:window.innerHeight}};}})()"
    ))
}

/// The image script: waits at most `wait` for the images still loading; answers `{pending}`.
pub fn images(wait: Duration) -> String {
    let ms = u64::try_from(wait.as_millis()).unwrap_or(u64::MAX);
    format!(
        "(async()=>{{const width=window.innerWidth;\
         const shown=(img)=>{{if(img.complete)return false;const box=img.getBoundingClientRect();\
         return box.width>0&&box.height>0&&box.right>0&&box.left<width;}};\
         const pending=Array.from(document.images).filter(shown);\
         if(pending.length===0)return{{pending:0}};\
         const loaded=Promise.all(pending.map((img)=>img.decode().catch(()=>undefined)));\
         const late=await Promise.race([loaded.then(()=>false),\
         new Promise((done)=>setTimeout(()=>done(true),{ms}))]);\
         return{{pending:late?pending.filter((img)=>!img.complete).length:0}};}})()"
    )
}

/// The answer of [`step`]; `None` for anything else (no viewport, a value not finite).
pub fn seen(value: &Value) -> Option<Seen> {
    let number = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_f64)
            .filter(|number| number.is_finite())
    };
    let viewport = number("viewport").filter(|viewport| *viewport >= 1.0)?;
    Some(Seen {
        at: Scroll {
            x: number("x")?,
            y: number("y")?,
        },
        height: number("height")?.max(0.0),
        viewport,
    })
}

/// The answer of [`images`]: the images still loading.
pub fn pending(value: &Value) -> Option<u32> {
    let pending = value.get("pending")?.as_u64()?;
    Some(u32::try_from(pending).unwrap_or(u32::MAX))
}

#[cfg(test)]
#[path = "lazy_script_tests.rs"]
mod tests;

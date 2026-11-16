//! The page's palette: the MCP tool `color_palette` (`[[tools]] palette`) and the panel's list.
//!
//! `src/palette.js` runs in each view's plugin world (`evaluate`) and counts the computed
//! `color` (of elements with text of their own), `background-color` and `border-color` (of
//! visible borders) of every rendered element by their computed text. Here the texts are read,
//! colours nearer than [`NEAR`] in OKLab are grouped under the most used one, and the groups
//! sorted by use. `mark` posts a group's members to every view's page script, which outlines
//! the elements using them.

use rb_plugin_api::{Failure, PluginHost};
use serde_json::{Value, json};

use crate::color::{Rgba, parse};
use crate::convert::{Formats, distance, oklab};
use crate::targets::{self, Default};
use crate::{bad_data, invalid};

/// The collector bundle: a function taking `{ maxElements }`.
pub const COLLECTOR: &str = include_str!("palette.js");
/// The most elements one view's collector reads.
pub const MAX_ELEMENTS: u64 = 20_000;
/// The most groups answered per view.
pub const MAX_GROUPS: usize = 48;
/// The most member texts a group keeps (and a mark takes).
pub const MAX_MEMBERS: usize = 16;
/// Colours nearer than this in OKLab (deltaE OK) are one group: not told apart by eye.
pub const NEAR: f64 = 0.02;

/// One group of the palette.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The most used member's colour.
    pub color: Rgba,
    /// Uses as text colour, background and border.
    pub uses: [u64; 3],
    /// The computed texts of the members, most used first.
    pub members: Vec<String>,
}

impl Group {
    #[must_use]
    pub fn count(&self) -> u64 {
        self.uses.iter().sum()
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        let formats = Formats::of(self.color);
        json!({
            "hex": formats.hex,
            "color": formats.to_json(),
            "count": self.count(),
            "uses": { "color": self.uses[0], "background": self.uses[1], "border": self.uses[2] },
            "members": self.members,
        })
    }
}

fn uses(entry: &Value) -> [u64; 3] {
    ["color", "background", "border"].map(|key| entry[key].as_u64().unwrap_or(0))
}

/// The collector's `entries`, grouped and sorted by use (most first, then by hex).
#[must_use]
pub fn group(entries: &[Value]) -> Vec<Group> {
    let mut read: Vec<(String, Rgba, [u64; 3])> = entries
        .iter()
        .filter_map(|entry| {
            let text = entry["value"].as_str()?;
            let color = parse(text).filter(|color| color.a > 0.0)?;
            let uses = uses(entry);
            (uses.iter().sum::<u64>() > 0).then(|| (text.to_owned(), color, uses))
        })
        .collect();
    read.sort_by(|a, b| {
        let total = |uses: &[u64; 3]| uses.iter().sum::<u64>();
        total(&b.2).cmp(&total(&a.2)).then_with(|| a.0.cmp(&b.0))
    });
    let mut groups: Vec<Group> = Vec::new();
    for (text, color, counts) in read {
        let lab = oklab(color);
        let near = groups.iter_mut().find(|group| {
            distance(oklab(group.color), lab) < NEAR && (group.color.a - color.a).abs() < 0.05
        });
        match near {
            Some(group) => {
                for (sum, add) in group.uses.iter_mut().zip(counts) {
                    *sum += add;
                }
                if group.members.len() < MAX_MEMBERS {
                    group.members.push(text);
                }
            }
            None => groups.push(Group {
                color,
                uses: counts,
                members: vec![text],
            }),
        }
    }
    groups.sort_by(|a, b| {
        b.count()
            .cmp(&a.count())
            .then_with(|| a.color.hex().cmp(&b.color.hex()))
    });
    groups.truncate(MAX_GROUPS);
    groups
}

/// The script `evaluate` runs: the collector, called.
#[must_use]
pub fn script() -> String {
    format!(
        "({})({})",
        COLLECTOR.trim(),
        json!({ "maxElements": MAX_ELEMENTS })
    )
}

/// The members a mark names: at most [`MAX_MEMBERS`] colour texts.
fn mark_values(mark: &Value) -> Result<Vec<String>, Failure> {
    let values = mark.as_array().ok_or_else(|| invalid("mark"))?;
    if values.len() > MAX_MEMBERS {
        return Err(invalid("mark"));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|text| parse(text).is_some())
                .map(str::to_owned)
                .ok_or_else(|| invalid("mark"))
        })
        .collect()
}

/// `color_palette`: the palette of the view `target` (the active view without one), or - with
/// `mark` - the elements using the named colours outlined in every view (`[]` clears).
///
/// # Errors
///
/// `invalid` for bad arguments; the views' failure.
pub fn tool(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    if !args["mark"].is_null() {
        let values = mark_values(&args["mark"])?;
        let message = if values.is_empty() {
            json!({ "type": "unmark" })
        } else {
            json!({ "type": "mark", "values": values })
        };
        return Ok(json!({
            "marked": values.len(),
            "$post": [{ "target": "all", "message": message }],
        }));
    }
    let views = targets::resolve(host, args, Default::Active)?;
    let script = script();
    let mut answers = Vec::new();
    for view in views {
        if !targets::supported(&view) {
            answers.push(targets::unsupported(&view));
            continue;
        }
        let read = host.evaluate(view.id, &script).and_then(|answer| {
            serde_json::from_str::<Value>(&answer).map_err(|_| bad_data("palette"))
        });
        answers.push(match read {
            Ok(page) => {
                let entries = page["entries"].as_array().cloned().unwrap_or_default();
                let groups: Vec<Value> = group(&entries).iter().map(Group::to_json).collect();
                json!({
                    "view": view.id,
                    "label": view.label,
                    "elements": page["elements"].as_u64().unwrap_or(0),
                    "truncated": page["truncated"].as_bool().unwrap_or(false),
                    "colors": groups,
                })
            }
            Err(failure) => targets::failed(&view, &failure),
        });
    }
    Ok(json!({ "views": answers }))
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;

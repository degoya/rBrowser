//! The width of the gallery's thumbnails (RB-090-86): the setting `thumbnailWidth`, held to the
//! plugin's own settings schema - its choices and its default - so nothing copies them.

/// The plugin's settings schema: the one source of the thumbnail width's choices and default
/// (the core reads the same file, `crates/rbrowser/src/sitemap/gallery_page.rs`).
pub const SCHEMA: &str = include_str!("../settings.schema.json");

/// The choices and the default of `thumbnailWidth` in [`SCHEMA`].
fn width_rule() -> (Vec<u64>, u64) {
    let rule = serde_json::from_str::<serde_json::Value>(SCHEMA)
        .ok()
        .and_then(|schema| {
            let key = schema.get("properties")?.get("thumbnailWidth")?.clone();
            let choices: Vec<u64> = key
                .get("enum")?
                .as_array()?
                .iter()
                .filter_map(serde_json::Value::as_u64)
                .collect();
            let default = key.get("default")?.as_u64()?;
            choices.contains(&default).then_some((choices, default))
        });
    rule.expect("settings.schema.json names thumbnailWidth's choices and default (gallery_tests)")
}

/// The width without a setting: the schema's default.
#[must_use]
pub fn default_thumbnail_width() -> u32 {
    u32::try_from(width_rule().1).unwrap_or(u32::MAX)
}

/// The thumbnail width of the setting's JSON text `setting`: one of the schema's choices, else
/// its default.
#[must_use]
pub fn thumbnail_width(setting: Option<&str>) -> u32 {
    let (choices, default) = width_rule();
    let width = setting
        .and_then(|text| text.trim().parse::<u64>().ok())
        .filter(|width| choices.contains(width))
        .unwrap_or(default);
    u32::try_from(width).unwrap_or(u32::MAX)
}

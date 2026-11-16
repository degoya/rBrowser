//! The export of the last measurement: `.rbrowser/out/coverage/coverage.json` (the whole
//! measurement) and `coverage.csv` (a row per view and file).

use rb_plugin_api::{Failure, PluginHost};

use crate::model::Run;

pub const JSON_FILE: &str = "coverage.json";
pub const CSV_FILE: &str = "coverage.csv";

/// One CSV field, quoted when it holds a separator, a quote or a line break; a leading `=`, `+`,
/// `-` or `@` is defused so a spreadsheet takes no URL for a formula.
#[must_use]
pub fn field(value: &str) -> String {
    let value = if value.starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

/// The CSV text of `run`.
#[must_use]
pub fn csv(run: &Run) -> String {
    let mut text = String::from(
        "view,label,breakpoint,width,status,page,file,kind,inline,total,used,unused,unusedPercent\n",
    );
    for view in &run.views {
        let head = [
            view.view.to_string(),
            field(&view.label),
            field(&view.breakpoint),
            view.width.to_string(),
            format!("{:?}", view.status).to_lowercase(),
            field(&view.url),
        ]
        .join(",");
        if view.files.is_empty() {
            text.push_str(&head);
            text.push_str(",,,,,,,\n");
        }
        for file in &view.files {
            let row = [
                head.clone(),
                field(&file.url),
                file.kind.as_str().to_owned(),
                file.inline.to_string(),
                file.total.to_string(),
                file.used.to_string(),
                file.total.saturating_sub(file.used).to_string(),
                format!("{}", file.unused_percent()),
            ]
            .join(",");
            text.push_str(&row);
            text.push('\n');
        }
    }
    text
}

/// Writes both files; their paths below the out folder.
///
/// # Errors
///
/// The host's failure to write.
pub fn write(host: &mut dyn PluginHost, run: &Run) -> Result<Vec<String>, Failure> {
    let json = serde_json::to_vec_pretty(run).unwrap_or_default();
    host.out_write(JSON_FILE, &json)?;
    host.out_write(CSV_FILE, csv(run).as_bytes())?;
    Ok(vec![JSON_FILE.to_owned(), CSV_FILE.to_owned()])
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;

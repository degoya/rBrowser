//! `report.html` of a comparison: one page without any network request - its images are the
//! run's own files beside it - with, per path, the matrix environment pair x breakpoint and the
//! share of differing pixels, and per cell the two shots side by side, a slider over both, the
//! diff image and the differing areas (RB-094-03). Each shot links to its page, so opened in a view a click loads the page
//! there. Every text comes through `text` (the core catalogue, `envcompare.report.*`); every
//! value from the run is escaped.

use std::fmt::Write as _;

use rb_plugin_types::html::escape;

use super::report::{EnvRun, PairDiff, PairStatus};

/// The texts the page uses, by key below `envcompare.report.`.
pub const TEXT_KEYS: [&str; 13] = [
    "title",
    "path",
    "breakpoint",
    "same",
    "differs",
    "size_mismatch",
    "failed",
    "pending",
    "side_by_side",
    "slider",
    "difference",
    "open_live",
    "regions",
];

const STYLE: &str = "body{font:14px system-ui,sans-serif;margin:16px;color:#222;background:#fff}\
table{border-collapse:collapse;margin:8px 0 16px}th,td{border:1px solid #ccc;padding:4px 8px;\
text-align:left}td.same{background:#e6f4ea}td.differs,td.sizeMismatch{background:#fce8e6}\
td.failed,td.pending{background:#f1f3f4}details{margin:4px 0 12px}.row{display:flex;gap:8px;\
flex-wrap:wrap}.row figure{margin:0;max-width:48%}img{max-width:100%;border:1px solid #ccc}\
.slider{position:relative;display:inline-block;max-width:100%}.slider img.top{position:absolute;\
left:0;top:0;clip-path:inset(0 50% 0 0)}input[type=range]{width:100%}";

const SCRIPT: &str = "document.querySelectorAll('.slider input').forEach(function(range){\
range.addEventListener('input',function(){var top=range.parentElement.querySelector('img.top');\
top.style.clipPath='inset(0 '+(100-range.value)+'% 0 0)';});});";

fn status_key(status: PairStatus) -> &'static str {
    match status {
        PairStatus::Pending => "pending",
        PairStatus::Same => "same",
        PairStatus::Differs => "differs",
        PairStatus::SizeMismatch => "size_mismatch",
        PairStatus::Failed => "failed",
    }
}

fn status_class(status: PairStatus) -> &'static str {
    match status {
        PairStatus::Pending => "pending",
        PairStatus::Same => "same",
        PairStatus::Differs => "differs",
        PairStatus::SizeMismatch => "sizeMismatch",
        PairStatus::Failed => "failed",
    }
}

/// The page of `run`; `text` gives the text of a key of [`TEXT_KEYS`].
pub fn render(run: &EnvRun, text: &dyn Fn(&str) -> String) -> String {
    let say = |key: &str| escape(&text(key));
    let mut page = String::new();
    let _ = write!(
        page,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{} {}</title>\
         <style>{STYLE}</style></head><body><h1>{} <small>{}</small></h1>",
        say("title"),
        escape(&run.run_id),
        say("title"),
        escape(&run.run_id)
    );
    for (p, path) in run.paths.iter().enumerate() {
        let diffs: Vec<&PairDiff> = run.diffs.iter().filter(|d| d.path == p).collect();
        let _ = write!(
            page,
            "<section><h2>{} <code>{}</code></h2><table><tr><th>{}</th>",
            say("path"),
            escape(&path.path),
            say("breakpoint")
        );
        let mut pairs: Vec<(usize, usize)> = diffs.iter().map(|d| (d.a, d.b)).collect();
        pairs.sort_unstable();
        pairs.dedup();
        for (a, b) in &pairs {
            let _ = write!(
                page,
                "<th>{} / {}</th>",
                escape(&environment_id(run, *a)),
                escape(&environment_id(run, *b))
            );
        }
        page.push_str("</tr>");
        for (b, breakpoint) in run.breakpoints.iter().enumerate() {
            let _ = write!(page, "<tr><th>{}</th>", escape(&breakpoint.label));
            for pair in &pairs {
                match diffs
                    .iter()
                    .find(|d| d.breakpoint == b && (d.a, d.b) == *pair)
                {
                    Some(diff) => {
                        let _ = write!(
                            page,
                            "<td class=\"{}\"><a href=\"#d{p}-{}-{}-{b}\">{:.2} %</a> {}</td>",
                            status_class(diff.status),
                            diff.a,
                            diff.b,
                            diff.ratio * 100.0,
                            say(status_key(diff.status))
                        );
                    }
                    None => page.push_str("<td></td>"),
                }
            }
            page.push_str("</tr>");
        }
        page.push_str("</table>");
        for diff in diffs {
            details(&mut page, run, diff, &say);
        }
        page.push_str("</section>");
    }
    let _ = writeln!(page, "<script>{SCRIPT}</script></body></html>");
    page
}

fn environment_id(run: &EnvRun, index: usize) -> String {
    run.environments
        .get(index)
        .map(|environment| environment.id.clone())
        .unwrap_or_default()
}

/// One pair's views: side by side, the slider and the difference.
fn details(page: &mut String, run: &EnvRun, diff: &PairDiff, say: &dyn Fn(&str) -> String) {
    let breakpoint = run
        .breakpoints
        .get(diff.breakpoint)
        .map(|breakpoint| breakpoint.label.clone())
        .unwrap_or_default();
    let shot = |environment: usize| {
        run.shots
            .get(run.shot_index(diff.path, environment, diff.breakpoint))
    };
    let _ = write!(
        page,
        "<details id=\"d{}-{}-{}-{}\"><summary>{} / {} - {} - {:.2} %</summary>",
        diff.path,
        diff.a,
        diff.b,
        diff.breakpoint,
        escape(&environment_id(run, diff.a)),
        escape(&environment_id(run, diff.b)),
        escape(&breakpoint),
        diff.ratio * 100.0
    );
    if let Some(error) = &diff.error {
        let _ = write!(page, "<p>{}</p>", escape(error));
    }
    let _ = write!(page, "<h3>{}</h3><div class=\"row\">", say("side_by_side"));
    for environment in [diff.a, diff.b] {
        let Some(shot) = shot(environment) else {
            continue;
        };
        let _ = write!(
            page,
            "<figure><figcaption>{} <a href=\"{}\">{}</a></figcaption>",
            escape(&environment_id(run, environment)),
            escape(&shot.url),
            say("open_live")
        );
        match (&shot.file, &shot.error) {
            (Some(file), _) => {
                let _ = write!(page, "<img src=\"{}\" alt=\"\">", escape(file));
            }
            (None, Some(error)) => {
                let _ = write!(page, "<p>{}</p>", escape(error));
            }
            (None, None) => {}
        }
        page.push_str("</figure>");
    }
    page.push_str("</div>");
    if let (Some(Some(left)), Some(Some(right))) = (
        shot(diff.a).map(|shot| shot.file.as_ref()),
        shot(diff.b).map(|shot| shot.file.as_ref()),
    ) {
        let _ = write!(
            page,
            "<h3>{}</h3><div class=\"slider\"><img src=\"{}\" alt=\"\">\
             <img class=\"top\" src=\"{}\" alt=\"\"><input type=\"range\" min=\"0\" max=\"100\" \
             value=\"50\"></div>",
            say("slider"),
            escape(right),
            escape(left)
        );
    }
    if let Some(image) = &diff.image {
        let _ = write!(
            page,
            "<h3>{}</h3><img src=\"{}\" alt=\"\">",
            say("difference"),
            escape(image)
        );
    }
    if !diff.regions.is_empty() {
        let _ = write!(page, "<h3>{}</h3><ul>", say("regions"));
        for area in &diff.regions {
            let _ = write!(
                page,
                "<li>x {} y {}: {} &times; {} px</li>",
                area.x, area.y, area.width, area.height
            );
        }
        page.push_str("</ul>");
    }
    page.push_str("</details>");
}

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;

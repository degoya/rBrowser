//! `index.*`: the page answers 200 without a detour and may be indexed - no `noindex` in a
//! meta robots tag or the `X-Robots-Tag` header, no `Disallow` for a search engine in
//! `robots.txt`.

use crate::audit::Page;
use crate::fetch::Stop;
use crate::page::{says_noindex, sel};
use crate::rules::{Element, Sink, short};

fn chain(page: &Page<'_>) -> String {
    let mut steps: Vec<String> = page
        .fetched
        .hops
        .iter()
        .map(|(url, status)| format!("{url} ({status})"))
        .collect();
    if let Some(response) = &page.fetched.response {
        steps.push(format!("{} ({})", response.url, response.status));
    }
    short(&steps.join(" -> "), 1500)
}

pub fn check(page: &Page<'_>, sink: &mut Sink<'_>) {
    let hops = page.fetched.hops.len().to_string();
    match &page.fetched.stop {
        Some(Stop::Loop) => sink.emit(
            "index.redirect_broken",
            "index.redirect_loop",
            None,
            &[("hops", hops.clone()), ("chain", chain(page))],
        ),
        Some(Stop::BadLocation(location)) => sink.emit(
            "index.redirect_broken",
            "index.redirect_location",
            None,
            &[("location", short(location, 300))],
        ),
        Some(Stop::Failed(code)) => sink.emit(
            "index.status",
            "index.fetch_failed",
            None,
            &[("code", code.clone())],
        ),
        None => {}
    }
    if !page.fetched.hops.is_empty() && page.fetched.response.is_some() {
        sink.emit(
            "index.redirect",
            "index.redirect",
            None,
            &[
                ("hops", hops),
                ("target", page.url.to_string()),
                ("chain", chain(page)),
            ],
        );
    }
    let Some(response) = &page.fetched.response else {
        return;
    };
    if response.status != 200 {
        sink.emit(
            "index.status",
            "index.status",
            None,
            &[("status", response.status.to_string())],
        );
    }
    if let Some(value) = response.header("x-robots-tag").find(|v| says_noindex(v)) {
        sink.emit(
            "index.noindex_header",
            "index.noindex_header",
            None,
            &[("value", short(value, 200))],
        );
    }
    if let Some((html, _)) = page.primary()
        && let Some(meta) = html
            .select(&sel("meta[name], meta[property]"))
            .find(|meta| {
                let name = meta.value().attr("name").unwrap_or_default();
                (name.eq_ignore_ascii_case("robots") || name.eq_ignore_ascii_case("googlebot"))
                    && says_noindex(meta.value().attr("content").unwrap_or_default())
            })
    {
        sink.emit(
            "index.noindex_meta",
            "index.noindex_meta",
            Some(Element::of(meta)),
            &[(
                "value",
                short(meta.value().attr("content").unwrap_or_default(), 200),
            )],
        );
    }
    if !page.site.engine.allows(&page.url) {
        sink.emit(
            "index.robots_blocked",
            "index.robots_blocked",
            None,
            &[("robots", page.site.robots_url.clone())],
        );
    }
}

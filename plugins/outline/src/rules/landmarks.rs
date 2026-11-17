//! The landmark rules, each where axe reports the same: no `main` (at `html`), more than one
//! `main`, `banner` or `contentinfo` (at the first), a `banner` or `contentinfo` inside another
//! landmark (at the inner one), landmarks of one role without a name that tells them apart (at the
//! first of them), and visible content outside every landmark (axe's `region`, at the outermost
//! such element).

use crate::page::{Landmark, PageOutline, cut};

use super::{
    CONTENT_OUTSIDE_LANDMARKS, LANDMARK_BANNER_MULTIPLE, LANDMARK_BANNER_NESTED,
    LANDMARK_CONTENTINFO_MULTIPLE, LANDMARK_CONTENTINFO_NESTED, LANDMARK_MAIN_MISSING,
    LANDMARK_MAIN_MULTIPLE, LANDMARK_UNIQUE, Page, Sink, root,
};

/// The roles a page may hold several of, each told apart by its name.
const NAMED_ROLES: [&str; 5] = ["navigation", "complementary", "region", "search", "form"];
/// The longest piece of content a finding quotes, in characters.
const QUOTE_CHARS: usize = 80;

/// Whether the element at path `inner` lies inside the one at `outer` (in its light or shadow
/// tree: `a > b`, `a >>> b`).
fn inside(inner: &str, outer: &str) -> bool {
    inner.len() > outer.len() && inner.starts_with(outer) && inner[outer.len()..].starts_with(" >")
}

pub(super) fn check(sink: &mut Sink<'_>, page: Page<'_>, outline: &PageOutline) {
    let exposed: Vec<&Landmark> = outline.landmarks.iter().filter(|l| !l.at.hidden).collect();
    let of = |role: &str| -> Vec<&Landmark> {
        exposed.iter().copied().filter(|l| l.role == role).collect()
    };
    let mains = of("main");
    match mains.first() {
        None => sink.emit(LANDMARK_MAIN_MISSING, page, Some(&root()), &[]),
        Some(first) if mains.len() > 1 => sink.emit(
            LANDMARK_MAIN_MULTIPLE,
            page,
            Some(&first.at),
            &[("count", mains.len().to_string())],
        ),
        Some(_) => {}
    }
    let pairs = [
        ("banner", LANDMARK_BANNER_MULTIPLE, LANDMARK_BANNER_NESTED),
        (
            "contentinfo",
            LANDMARK_CONTENTINFO_MULTIPLE,
            LANDMARK_CONTENTINFO_NESTED,
        ),
    ];
    for (role, multiple, _) in pairs {
        let all = of(role);
        if let Some(first) = all.first()
            && all.len() > 1
        {
            sink.emit(
                multiple,
                page,
                Some(&first.at),
                &[("count", all.len().to_string())],
            );
        }
    }
    for (role, _, nested) in pairs {
        for landmark in of(role) {
            // The innermost landmark around it names the problem best.
            let outer = exposed
                .iter()
                .filter(|other| inside(&landmark.at.path, &other.at.path))
                .max_by_key(|other| other.at.path.len());
            if let Some(outer) = outer {
                sink.emit(
                    nested,
                    page,
                    Some(&landmark.at),
                    &[("outer", outer.role.clone())],
                );
            }
        }
    }
    let mut told: Vec<(String, String)> = Vec::new();
    for landmark in exposed
        .iter()
        .filter(|l| NAMED_ROLES.contains(&l.role.as_str()))
    {
        let name = landmark.name.to_lowercase();
        if told
            .iter()
            .any(|(role, seen)| *role == landmark.role && *seen == name)
        {
            continue;
        }
        told.push((landmark.role.clone(), name.clone()));
        let count = exposed
            .iter()
            .filter(|other| other.role == landmark.role && other.name.to_lowercase() == name)
            .count();
        if count > 1 {
            sink.emit(
                LANDMARK_UNIQUE,
                page,
                Some(&landmark.at),
                &[
                    ("role", landmark.role.clone()),
                    ("name", landmark.name.clone()),
                    ("count", count.to_string()),
                ],
            );
        }
    }
    for content in outline.outside.iter().filter(|c| !c.at.hidden) {
        sink.emit(
            CONTENT_OUTSIDE_LANDMARKS,
            page,
            Some(&content.at),
            &[("text", cut(&content.text, QUOTE_CHARS))],
        );
    }
}

#[cfg(test)]
#[path = "landmarks_tests.rs"]
mod tests;

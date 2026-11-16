use std::collections::BTreeSet;

use super::*;

fn entry(view: ViewId, level: Level, kind: Kind, text: &str, time: f64) -> Entry {
    Entry {
        view,
        level,
        kind,
        text: text.to_owned(),
        url: Some("http://x.test/app.js?v=3".to_owned()),
        line: Some(12),
        column: None,
        stack: None,
        time,
    }
}

fn all_levels() -> Filter {
    Filter {
        levels: Level::ALL.into_iter().collect(),
        views: None,
        search: None,
    }
}

#[test]
fn the_same_message_of_every_view_is_one_group_with_every_view() {
    let entries: Vec<Entry> = [(1, 320), (2, 375), (3, 768), (4, 1280)]
        .into_iter()
        .enumerate()
        .map(|(i, (view, width))| {
            entry(
                view,
                Level::Warning,
                Kind::Console,
                &format!("Deprecated API at width {width}"),
                f64::from(u32::try_from(i).unwrap_or(0)),
            )
        })
        .collect();
    let groups = group(&entries);
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].views.keys().copied().collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(groups[0].count(), 4);
    // The first message's text is the one shown.
    assert_eq!(groups[0].text, "Deprecated API at width 320");
    assert_eq!(groups[0].first, 0.0);
    assert_eq!(groups[0].last, 3.0);
}

#[test]
fn level_kind_and_source_keep_groups_apart() {
    let base = entry(1, Level::Error, Kind::Console, "boom", 1.0);
    let warning = Entry {
        level: Level::Warning,
        ..base.clone()
    };
    let thrown = Entry {
        kind: Kind::Exception,
        ..base.clone()
    };
    let other_line = Entry {
        line: Some(13),
        ..base.clone()
    };
    let cache_buster = Entry {
        url: Some("http://x.test/app.js?v=4#top".to_owned()),
        ..base.clone()
    };
    let groups = group(&[base, warning, thrown, other_line, cache_buster]);
    assert_eq!(groups.len(), 4);
    assert_eq!(groups[0].count(), 2, "the query is not part of the source");
}

#[test]
fn repeats_in_one_view_count_up() {
    let entries = [
        entry(1, Level::Log, Kind::Console, "tick 1", 1.0),
        entry(1, Level::Log, Kind::Console, "tick 2", 2.0),
        entry(2, Level::Log, Kind::Console, "tick 3", 3.0),
    ];
    let groups = group(&entries);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].views.get(&1), Some(&2));
    assert_eq!(groups[0].views.get(&2), Some(&1));
}

#[test]
fn the_group_id_is_stable_and_differs_per_key() {
    let a = group(&[entry(1, Level::Log, Kind::Console, "a 1", 1.0)]);
    let b = group(&[entry(7, Level::Log, Kind::Console, "a 99", 5.0)]);
    let c = group(&[entry(1, Level::Log, Kind::Console, "b 1", 1.0)]);
    assert_eq!(a[0].id, b[0].id);
    assert_ne!(a[0].id, c[0].id);
    assert_eq!(a[0].id.len(), 16);
}

#[test]
fn filters_pick_levels_views_and_text() {
    let groups = group(&[
        entry(
            1,
            Level::Error,
            Kind::Exception,
            "Uncaught Error: narrow",
            1.0,
        ),
        entry(2, Level::Warning, Kind::Console, "Deprecated", 2.0),
    ]);
    let errors = Filter {
        levels: BTreeSet::from([Level::Error]),
        ..all_levels()
    };
    assert_eq!(groups.iter().filter(|g| errors.matches(g)).count(), 1);
    let view_two = Filter {
        views: Some(BTreeSet::from([2])),
        ..all_levels()
    };
    assert_eq!(groups.iter().filter(|g| view_two.matches(g)).count(), 1);
    let search = Filter {
        search: Some("NARROW".to_owned()),
        ..all_levels()
    };
    assert_eq!(groups.iter().filter(|g| search.matches(g)).count(), 1);
    let by_source = Filter {
        search: Some("x.test/app.js".to_owned()),
        ..all_levels()
    };
    assert_eq!(groups.iter().filter(|g| by_source.matches(g)).count(), 2);
}

#[test]
fn the_page_keeps_the_newest_and_counts_the_levels() {
    let entries: Vec<Entry> = (0..5)
        .map(|i| {
            entry(
                1,
                Level::Log,
                Kind::Console,
                &format!("line{}", "x".repeat(i)),
                f64::from(u8::try_from(i).unwrap_or(0)),
            )
        })
        .chain([entry(1, Level::Error, Kind::Exception, "boom", 9.0)])
        .collect();
    let groups = group(&entries);
    let logs = Filter {
        levels: BTreeSet::from([Level::Log]),
        ..all_levels()
    };
    let page = page(&groups, &logs, 2);
    assert_eq!(page["total"], 5);
    assert_eq!(page["shown"], 2);
    assert_eq!(page["truncated"], true);
    assert_eq!(page["entries"][0]["text"], "linexxx");
    assert_eq!(page["entries"][1]["text"], "linexxxx");
    assert_eq!(page["levels"]["log"], 5);
    assert_eq!(page["levels"]["error"], 1);
    assert_eq!(page["levels"]["debug"], 0);
    assert_eq!(page["entries"][1]["source"]["line"], 12);
    assert_eq!(page["entries"][1]["perView"]["1"], 1);
}

#[test]
fn the_page_stays_within_its_budget() {
    let long = "y".repeat(1900);
    let entries: Vec<Entry> = (0..100)
        .map(|i| {
            entry(
                1,
                Level::Log,
                Kind::Console,
                &format!("{long} {}", "z".repeat(i)),
                f64::from(u8::try_from(i).unwrap_or(0)),
            )
        })
        .collect();
    let page = page(&group(&entries), &all_levels(), MAX_LIMIT);
    let shown = page["shown"].as_u64().unwrap_or(0);
    assert!(shown > 0 && shown < 100);
    assert!(page["entries"].to_string().len() <= ANSWER_BUDGET);
    assert_eq!(page["truncated"], true);
}

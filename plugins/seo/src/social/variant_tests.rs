use super::*;
use crate::social::data;

fn shape(width: u32, height: u32) -> Option<Shape> {
    Some(Shape {
        width,
        height,
        bytes: 50_000,
    })
}

fn rule_of(id: &str) -> Rule {
    data()
        .platforms
        .into_iter()
        .find(|platform| platform.id == id)
        .map(|platform| platform.rule)
        .expect("platform")
}

fn param(variant: &Variant, key: &str) -> String {
    variant.reason.params.get(key).cloned().unwrap_or_default()
}

#[test]
fn without_an_image_every_platform_shows_text_alone() {
    for platform in data().platforms {
        let variant = choose(&platform.rule, Some(LARGE_CARD), false, None);
        assert_eq!(variant.layout, Layout::Text, "{}", platform.id);
        assert_eq!(variant.alternate, None);
        assert_eq!(variant.reason.code, "no_image");
    }
}

// Facebook: large from 600 x 315 on, smaller "much smaller".
#[test]
fn facebook_is_large_from_600_by_315() {
    let rule = rule_of("facebook");
    let large = choose(&rule, None, true, shape(600, 315));
    assert_eq!(
        (large.layout, large.alternate),
        (Layout::Large, Some(Layout::Small))
    );
    assert_eq!(large.reason.code, "meets");
    let small = choose(&rule, Some(LARGE_CARD), true, shape(400, 400));
    assert_eq!(small.layout, Layout::Small);
    assert_eq!(small.alternate, Some(Layout::Large));
    assert_eq!(small.reason.code, "below");
    assert_eq!(
        (
            param(&small, "width"),
            param(&small, "min_width"),
            param(&small, "min_height")
        ),
        ("400".to_owned(), "600".to_owned(), "315".to_owned())
    );
    assert_eq!(
        choose(&rule, None, true, shape(1200, 300)).layout,
        Layout::Small
    );
}

// X, Slack and Discord: the page's twitter:card decides.
#[test]
fn x_slack_and_discord_follow_twitter_card() {
    for id in ["x", "slack", "discord"] {
        let rule = rule_of(id);
        assert_eq!(rule, Rule::Card, "{id}");
        let large = choose(&rule, Some(" Summary_Large_Image "), true, shape(10, 10));
        assert_eq!(large.layout, Layout::Large);
        assert_eq!(large.reason.code, "card_large");
        let summary = choose(&rule, Some("summary"), true, shape(2000, 1000));
        assert_eq!(summary.layout, Layout::Small);
        assert_eq!(summary.reason.code, "card_other");
        assert_eq!(param(&summary, "card"), "summary");
        let missing = choose(&rule, None, true, None);
        assert_eq!((missing.layout, missing.unsure), (Layout::Small, false));
        assert_eq!(missing.reason.code, "card_missing");
    }
}

// LinkedIn: "Images less than 401 pixels wide display as a thumbnail image."
#[test]
fn linkedin_is_a_thumbnail_below_401_pixels_wide() {
    let rule = rule_of("linkedin");
    let small = choose(&rule, None, true, shape(400, 210));
    assert_eq!(small.layout, Layout::Small);
    assert_eq!(small.reason.code, "narrow");
    assert_eq!(param(&small, "min_width"), "401");
    assert_eq!(
        choose(&rule, None, true, shape(401, 100)).layout,
        Layout::Large
    );
}

// WhatsApp: at least 300 px wide, at most 4:1, under 600 KB.
#[test]
fn whatsapp_is_large_from_300_wide_up_to_4_to_1_under_600_kb() {
    let rule = rule_of("whatsapp");
    assert_eq!(
        choose(&rule, None, true, shape(300, 75)).layout,
        Layout::Large
    );
    assert_eq!(
        choose(&rule, None, true, shape(299, 299)).reason.code,
        "narrow"
    );
    let wide = choose(&rule, None, true, shape(1201, 300));
    assert_eq!(
        (wide.layout, wide.reason.code.as_str()),
        (Layout::Small, "ratio")
    );
    assert_eq!(param(&wide, "max_ratio"), "4");
    let heavy = Some(Shape {
        width: 1200,
        height: 630,
        bytes: 600_000,
    });
    let heavy = choose(&rule, None, true, heavy);
    assert_eq!(
        (heavy.layout, heavy.reason.code.as_str()),
        (Layout::Small, "heavy")
    );
    assert_eq!(param(&heavy, "max_bytes"), "600000");
}

// iMessage: narrower than 150 px "may be ignored or presented as icons".
#[test]
fn imessage_shows_an_image_narrower_than_150_pixels_small() {
    let rule = rule_of("imessage");
    assert_eq!(
        choose(&rule, None, true, shape(149, 149)).layout,
        Layout::Small
    );
    assert_eq!(
        choose(&rule, None, true, shape(150, 600)).layout,
        Layout::Large
    );
}

// Mastodon: large when the image is wider than high.
#[test]
fn mastodon_is_large_for_a_landscape_image() {
    let rule = rule_of("mastodon");
    assert_eq!(
        choose(&rule, None, true, shape(401, 400)).layout,
        Layout::Large
    );
    let square = choose(&rule, None, true, shape(400, 400));
    assert_eq!(
        (square.layout, square.reason.code.as_str()),
        (Layout::Small, "not_landscape")
    );
}

// Bluesky: always the large card while there is an image.
#[test]
fn bluesky_is_always_large_with_an_image() {
    let rule = rule_of("bluesky");
    let variant = choose(&rule, None, true, shape(16, 16));
    assert_eq!((variant.layout, variant.alternate), (Layout::Large, None));
    assert_eq!(variant.reason.code, "always_large");
}

// Review of RB-080-99 (2): the reason names the layout the rule always shows.
#[test]
fn an_always_rule_says_which_layout() {
    let small = choose(
        &Rule::Always {
            layout: Layout::Small,
        },
        None,
        true,
        shape(1200, 630),
    );
    assert_eq!(
        (small.layout, small.reason.code.as_str()),
        (Layout::Small, "always_small")
    );
    // A rule that always shows text alone makes no sense with an image: the table has none.
    for platform in data().platforms {
        if let Rule::Always { layout } = platform.rule {
            assert_ne!(layout, Layout::Text, "{}", platform.id);
        }
    }
}

// Signal: both sides at least 200 and not square within 5 %.
#[test]
fn signal_is_large_from_200_and_not_square() {
    let rule = rule_of("signal");
    assert_eq!(
        choose(&rule, None, true, shape(400, 210)).layout,
        Layout::Large
    );
    assert_eq!(
        choose(&rule, None, true, shape(400, 199)).reason.code,
        "below"
    );
    let square = choose(&rule, None, true, shape(410, 400));
    assert_eq!(
        (square.layout, square.reason.code.as_str()),
        (Layout::Small, "square")
    );
    assert_eq!(
        choose(&rule, None, true, shape(421, 400)).layout,
        Layout::Large
    );
}

// Telegram, Teams, Threads: no published rule - both variants, marked.
#[test]
fn an_unpublished_rule_shows_both_variants() {
    for id in ["telegram", "teams", "threads"] {
        let variant = choose(&rule_of(id), Some(LARGE_CARD), true, shape(1200, 630));
        assert!(variant.unsure, "{id}");
        assert_eq!(
            (variant.layout, variant.alternate),
            (Layout::Large, Some(Layout::Small))
        );
        assert_eq!(variant.reason.code, "unverified");
    }
}

#[test]
fn a_size_rule_without_a_known_size_shows_both() {
    let variant = choose(&rule_of("facebook"), None, true, None);
    assert!(variant.unsure);
    assert_eq!(variant.reason.code, "size_unknown");
}

#[test]
fn a_rule_reads_from_json_and_refuses_unknown_bounds() {
    let rule: Rule =
        serde_json::from_str(r#"{ "kind": "size", "minWidth": 1, "landscape": true }"#)
            .expect("rule");
    assert_eq!(
        rule,
        Rule::Size(Size {
            min_width: Some(1),
            landscape: true,
            ..Size::default()
        })
    );
    assert!(serde_json::from_str::<Rule>(r#"{ "kind": "size", "minWide": 1 }"#).is_err());
    assert!(serde_json::from_str::<Rule>(r#"{ "kind": "guess" }"#).is_err());
}

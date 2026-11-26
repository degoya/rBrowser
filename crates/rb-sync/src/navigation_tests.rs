use super::*;

const LEADER: ViewId = ViewId(1);
const MOBILE: ViewId = ViewId(2);
const TABLET: ViewId = ViewId(3);

fn nav() -> NavigationSync {
    NavigationSync::new(NavOptions::default())
}

fn follower(view: ViewId, url: &str) -> Follower {
    Follower {
        view,
        url: url.to_owned(),
        loading: false,
    }
}

fn loading(view: ViewId, url: &str) -> Follower {
    Follower {
        loading: true,
        ..follower(view, url)
    }
}

fn load(view: ViewId, url: &str) -> NavFollow {
    NavFollow {
        view,
        url: url.to_owned(),
        mode: FollowMode::Load,
    }
}

fn history(view: ViewId, url: &str) -> NavFollow {
    NavFollow {
        mode: FollowMode::History,
        ..load(view, url)
    }
}

/// The leader was on `from` and committed `to`; answers the settle check's generation.
fn lead(nav: &mut NavigationSync, from: &str, to: &str) -> u64 {
    assert!(nav.committed(LEADER, from, false).is_none());
    let settle = nav
        .committed(LEADER, to, true)
        .expect("the leader's navigation arms the check");
    assert_eq!(settle.delay, DEFAULT_SETTLE);
    settle.generation
}

/// RB-099-18: what a test waits on - every commit is noted, only a leading one arms a check.
#[test]
fn every_commit_is_noted_and_only_a_leading_one_counts_as_armed() {
    let mut nav = nav();
    assert_eq!((nav.url(MOBILE), nav.generation()), (None, 0));
    assert!(nav.committed(MOBILE, "http://a.test/x", false).is_none());
    assert_eq!(nav.url(MOBILE), Some("http://a.test/x"));
    assert_eq!(nav.generation(), 0, "a follower's commit arms nothing");
    let settle = nav
        .committed(LEADER, "http://a.test/y", true)
        .expect("armed");
    assert_eq!(nav.url(LEADER), Some("http://a.test/y"));
    assert_eq!(nav.generation(), settle.generation);
    assert_eq!(nav.generation(), 1);
}

#[test]
fn only_the_leaders_navigation_arms_the_settle_check() {
    let mut nav = nav();
    assert!(nav.committed(MOBILE, "http://a.test/x", false).is_none());
    assert_eq!(nav.pending_leader(), None);
    let settle = nav.committed(LEADER, "http://a.test/y", true);
    assert!(settle.is_some());
    assert_eq!(nav.pending_leader(), Some(LEADER));
}

#[test]
fn a_decoupled_leader_arms_nothing() {
    let mut nav = nav();
    nav.set_configured([LEADER]);
    assert!(nav.committed(LEADER, "http://a.test/y", true).is_none());
}

#[test]
fn a_page_that_is_not_followed_cancels_the_check() {
    let mut nav = nav();
    lead(&mut nav, "http://a.test/", "http://a.test/produkt");
    assert!(
        nav.committed(LEADER, "chrome-error://chromewebdata/", true)
            .is_none()
    );
    assert_eq!(nav.pending_leader(), None);
}

#[test]
fn a_follower_on_another_page_loads_the_leaders_url_after_the_settle_time() {
    let mut nav = nav();
    let generation = lead(&mut nav, "http://a.test/", "http://a.test/produkt");
    let outcome = nav.settle(
        generation,
        LEADER,
        "http://a.test/produkt",
        &[
            follower(LEADER, "http://a.test/produkt"),
            follower(MOBILE, "http://a.test/m/produkt"),
            follower(TABLET, "http://a.test/produkt"),
        ],
    );
    assert_eq!(outcome.follows, vec![load(MOBILE, "http://a.test/produkt")]);
    assert_eq!(outcome.retry, None);
    assert_eq!(nav.pending_leader(), None, "the check ran");
}

#[test]
fn a_decoupled_follower_stays_on_its_page() {
    let mut nav = nav();
    nav.set_configured([MOBILE]);
    let generation = lead(&mut nav, "http://a.test/", "http://a.test/b");
    let outcome = nav.settle(
        generation,
        LEADER,
        "http://a.test/b",
        &[follower(MOBILE, "http://a.test/c")],
    );
    assert!(outcome.follows.is_empty());
}

#[test]
fn a_stale_timer_or_another_leader_does_nothing() {
    let mut nav = nav();
    let first = lead(&mut nav, "http://a.test/", "http://a.test/redirect");
    // A redirect: the leader commits again before the settle time; the first timer is stale.
    let second = nav
        .committed(LEADER, "http://a.test/final", true)
        .expect("re-armed")
        .generation;
    assert_ne!(first, second);
    let followers = [follower(MOBILE, "http://a.test/")];
    assert!(
        nav.settle(first, LEADER, "http://a.test/final", &followers)
            .follows
            .is_empty()
    );
    // The active view changed meanwhile: the old leader's check is dropped.
    assert!(
        nav.settle(second, TABLET, "http://a.test/final", &followers)
            .follows
            .is_empty()
    );
    assert_eq!(nav.pending_leader(), None);
}

#[test]
fn a_re_armed_check_keeps_the_page_the_leader_came_from() {
    let mut nav = nav();
    nav.committed(LEADER, "http://a.test/list", false);
    nav.within_document(LEADER, "http://a.test/step-1");
    nav.committed(LEADER, "http://a.test/step-1", true);
    nav.within_document(LEADER, "http://a.test/step-2");
    let generation = nav
        .committed(LEADER, "http://a.test/step-2", true)
        .expect("re-armed")
        .generation;
    // The follower is still where the leader started: the same SPA document.
    let outcome = nav.settle(
        generation,
        LEADER,
        "http://a.test/step-2",
        &[follower(MOBILE, "http://a.test/list")],
    );
    assert_eq!(
        outcome.follows,
        vec![history(MOBILE, "http://a.test/step-2")]
    );
}

#[test]
fn a_spa_route_follows_through_the_history_api() {
    let mut nav = nav();
    nav.committed(LEADER, "http://a.test/#/", false);
    nav.within_document(LEADER, "http://a.test/about");
    let generation = nav
        .committed(LEADER, "http://a.test/about", true)
        .expect("armed")
        .generation;
    let outcome = nav.settle(
        generation,
        LEADER,
        "http://a.test/about",
        &[
            follower(MOBILE, "http://a.test/"),
            follower(TABLET, "http://a.test/elsewhere"),
        ],
    );
    assert_eq!(
        outcome.follows,
        vec![
            history(MOBILE, "http://a.test/about"),
            load(TABLET, "http://a.test/about"),
        ]
    );
}

#[test]
fn a_full_navigation_is_never_followed_through_the_history_api() {
    let mut nav = nav();
    nav.within_document(LEADER, "http://a.test/about");
    nav.cross_document(LEADER);
    let generation = lead(&mut nav, "http://a.test/", "http://a.test/about");
    let outcome = nav.settle(
        generation,
        LEADER,
        "http://a.test/about",
        &[follower(MOBILE, "http://a.test/")],
    );
    assert_eq!(outcome.follows, vec![load(MOBILE, "http://a.test/about")]);
}

#[test]
fn hash_changes_follow_only_with_include_hash() {
    let mut nav = nav();
    nav.committed(LEADER, "http://a.test/doc", false);
    nav.within_document(LEADER, "http://a.test/doc#part-2");
    let generation = nav
        .committed(LEADER, "http://a.test/doc#part-2", true)
        .expect("armed")
        .generation;
    let followers = [follower(MOBILE, "http://a.test/doc")];
    assert!(
        nav.settle(generation, LEADER, "http://a.test/doc#part-2", &followers)
            .follows
            .is_empty(),
        "sync.navigationIncludeHash off: the same page"
    );

    let mut nav = NavigationSync::new(NavOptions {
        include_hash: true,
        ..NavOptions::default()
    });
    nav.committed(LEADER, "http://a.test/doc", false);
    nav.within_document(LEADER, "http://a.test/doc#part-2");
    let generation = nav
        .committed(LEADER, "http://a.test/doc#part-2", true)
        .expect("armed")
        .generation;
    assert_eq!(
        nav.settle(generation, LEADER, "http://a.test/doc#part-2", &followers)
            .follows,
        vec![history(MOBILE, "http://a.test/doc#part-2")]
    );
}

#[test]
fn a_loading_follower_gets_more_time_then_is_loaded() {
    let mut nav = nav();
    let generation = lead(&mut nav, "http://a.test/", "http://a.test/b");
    let outcome = nav.settle(
        generation,
        LEADER,
        "http://a.test/b",
        &[
            loading(MOBILE, "http://a.test/"),
            follower(TABLET, "http://a.test/c"),
        ],
    );
    assert_eq!(outcome.follows, vec![load(TABLET, "http://a.test/b")]);
    let retry = outcome.retry.expect("the loading view gets another round");
    assert_eq!(retry.generation, generation);
    // The second round: still loading, still waiting; the tablet is not loaded twice.
    let followers = [
        loading(MOBILE, "http://a.test/"),
        loading(TABLET, "http://a.test/c"),
    ];
    let outcome = nav.settle(generation, LEADER, "http://a.test/b", &followers);
    assert!(outcome.follows.is_empty());
    assert!(outcome.retry.is_some());
    // The last round loads it.
    let outcome = nav.settle(generation, LEADER, "http://a.test/b", &followers);
    assert_eq!(outcome.follows, vec![load(MOBILE, "http://a.test/b")]);
    assert_eq!(outcome.retry, None);
    assert_eq!(nav.pending_leader(), None);
}

#[test]
fn a_follower_in_another_environment_keeps_its_origin() {
    let mut nav = NavigationSync::new(NavOptions {
        environments: vec![
            "http://localhost:5173".to_owned(),
            "https://staging.test".to_owned(),
        ],
        ..NavOptions::default()
    });
    let generation = lead(
        &mut nav,
        "http://localhost:5173/",
        "http://localhost:5173/produkt",
    );
    let outcome = nav.settle(
        generation,
        LEADER,
        "http://localhost:5173/produkt",
        &[
            follower(MOBILE, "https://staging.test/"),
            follower(TABLET, "https://staging.test/produkt"),
        ],
    );
    assert_eq!(
        outcome.follows,
        vec![load(MOBILE, "https://staging.test/produkt")]
    );
}

#[test]
fn coupling_overrides_the_config_until_it_changes() {
    let mut nav = nav();
    nav.set_configured([MOBILE]);
    assert!(!nav.is_coupled(MOBILE));
    assert!(nav.is_coupled(TABLET));
    assert!(nav.couple(MOBILE, true), "changed");
    assert!(!nav.couple(MOBILE, true), "no change");
    assert!(nav.couple(TABLET, false));
    assert_eq!(nav.decoupled(&[LEADER, MOBILE, TABLET]), vec![TABLET]);
    // A new layout keeps the runtime changes; a changed config drops them.
    nav.set_configured([MOBILE]);
    assert_eq!(nav.decoupled(&[LEADER, MOBILE, TABLET]), vec![TABLET]);
    nav.clear_overrides();
    assert_eq!(nav.decoupled(&[LEADER, MOBILE, TABLET]), vec![MOBILE]);
}

#[test]
fn views_that_are_gone_are_forgotten() {
    let mut nav = nav();
    nav.couple(MOBILE, false);
    lead(&mut nav, "http://a.test/", "http://a.test/b");
    nav.retain(&[MOBILE]);
    assert_eq!(nav.pending_leader(), None, "the leader is gone");
    assert!(!nav.is_coupled(MOBILE));
    nav.retain(&[]);
    assert!(nav.is_coupled(MOBILE));
}

#[test]
fn cancel_drops_the_check() {
    let mut nav = nav();
    let generation = lead(&mut nav, "http://a.test/", "http://a.test/b");
    nav.cancel();
    assert!(
        nav.settle(
            generation,
            LEADER,
            "http://a.test/b",
            &[follower(MOBILE, "http://a.test/")]
        )
        .follows
        .is_empty()
    );
}

#[test]
fn the_settle_time_follows_the_options() {
    let mut nav = nav();
    nav.set_options(NavOptions {
        settle: Duration::from_millis(250),
        ..NavOptions::default()
    });
    let settle = nav
        .committed(LEADER, "http://a.test/b", true)
        .expect("armed");
    assert_eq!(settle.delay, Duration::from_millis(250));
    assert_eq!(nav.options().settle, Duration::from_millis(250));
}

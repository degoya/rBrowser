use super::*;
use crate::manifest::tests::manifest;

const WINDOWS: Target = Target {
    platform: "windows",
    arch: "x86_64",
    kind: kind::ZIP,
};

#[test]
fn the_newest_newer_release_is_offered_with_this_builds_file() {
    let manifests = [
        manifest(Channel::Stable, "0.9.0", 1),
        manifest(Channel::Beta, "0.9.1-beta.1", 2),
    ];
    let offer = newest_offer("0.9.0-beta.2", Channel::Beta, &manifests, &WINDOWS).expect("offer");
    assert_eq!(offer.version, "0.9.1-beta.1");
    assert_eq!(offer.size(), Some(1024));
    let stable =
        newest_offer("0.9.0-beta.2", Channel::Stable, &manifests, &WINDOWS).expect("the release");
    assert_eq!(stable.version, "0.9.0", "no beta on stable");
}

#[test]
fn nothing_older_or_equal_is_offered() {
    let manifests = [manifest(Channel::Stable, "0.9.0", 1)];
    assert_eq!(
        newest_offer("0.9.0", Channel::Stable, &manifests, &WINDOWS),
        None
    );
    assert_eq!(
        newest_offer("1.0.0", Channel::Beta, &manifests, &WINDOWS),
        None
    );
    assert_eq!(
        newest_offer("not a version", Channel::Beta, &manifests, &WINDOWS),
        None
    );
}

#[test]
fn a_min_version_above_the_running_one_offers_nothing() {
    let mut gated = manifest(Channel::Stable, "1.0.0", 1);
    gated.min_version = Some("0.9.5".to_owned());
    assert_eq!(
        newest_offer("0.9.0", Channel::Stable, &[gated], &WINDOWS),
        None
    );
}

#[test]
fn another_platform_is_offered_without_a_file() {
    let manifests = [manifest(Channel::Stable, "1.0.0", 1)];
    let linux = Target {
        platform: "linux",
        arch: "x86_64",
        kind: kind::ZIP,
    };
    let offer = newest_offer("0.9.0", Channel::Stable, &manifests, &linux).expect("offer");
    assert_eq!(offer.artifact, None);
    assert_eq!(offer.size(), None);
}

/// `updates.channel = auto` (the default): a `-beta.N` build reads beta, a release build
/// stable; a channel the person chose wins (RB-090-14).
#[test]
fn auto_follows_the_build_and_a_chosen_channel_wins() {
    assert_eq!(effective_channel("0.9.0-beta.2", None), Channel::Beta);
    assert_eq!(effective_channel("0.9.0", None), Channel::Stable);
    assert_eq!(
        effective_channel("0.9.0-beta.2", Some(Channel::Stable)),
        Channel::Stable
    );
    assert_eq!(
        effective_channel("0.9.0", Some(Channel::Beta)),
        Channel::Beta
    );
}

#[test]
fn the_current_target_names_this_build() {
    let target = Target::current();
    assert_eq!(target.arch, std::env::consts::ARCH);
    assert_eq!(target.kind, kind::ZIP);
}

use super::*;

#[test]
fn wslinfo_answers() {
    assert_eq!(parse_mode("mirrored\n"), Some(NetworkingMode::Mirrored));
    assert_eq!(parse_mode("nat\r\n"), Some(NetworkingMode::Nat));
    assert_eq!(parse_mode("  NAT "), Some(NetworkingMode::Nat));
    assert_eq!(parse_mode("virtioproxy"), Some(NetworkingMode::VirtioProxy));
    assert_eq!(parse_mode("none"), Some(NetworkingMode::None));
    assert_eq!(parse_mode("bridged"), Some(NetworkingMode::Bridged));
    assert_eq!(parse_mode(""), None);
    assert_eq!(
        parse_mode("wslinfo: unknown option --networking-mode"),
        None
    );
}

#[test]
fn a_mirrored_wslconfig() {
    let text = "# my settings\r\n[wsl2]\r\nmemory=8GB\r\nnetworkingMode=mirrored\r\n";
    assert_eq!(from_wslconfig(text), Some(NetworkingMode::Mirrored));
}

#[test]
fn a_nat_wslconfig_and_one_without_the_key() {
    assert_eq!(
        from_wslconfig("[wsl2]\nnetworkingmode = \"NAT\" # default\n"),
        Some(NetworkingMode::Nat)
    );
    assert_eq!(from_wslconfig("[wsl2]\nmemory=4GB\n"), None);
    assert_eq!(from_wslconfig(""), None);
}

#[test]
fn the_key_counts_only_in_its_sections() {
    assert_eq!(from_wslconfig("[other]\nnetworkingMode=mirrored\n"), None);
    assert_eq!(
        from_wslconfig("[experimental]\nnetworkingMode=mirrored\n"),
        Some(NetworkingMode::Mirrored)
    );
    assert_eq!(
        from_wslconfig("[wsl2]\nnetworkingMode=nat\n[experimental]\nnetworkingMode=mirrored\n"),
        Some(NetworkingMode::Nat)
    );
    assert_eq!(
        from_wslconfig("[WSL2]\n; networkingMode=mirrored\n#networkingMode=mirrored\n"),
        None
    );
}

#[test]
fn the_sources_in_their_order() {
    use NetworkingMode::{Mirrored, Nat};
    let detection = |mode, source| Detection { mode, source };
    assert_eq!(
        resolve(Some(Nat), Some(Mirrored), Some(Mirrored)),
        detection(Nat, ModeSource::Override)
    );
    assert_eq!(
        resolve(None, Some(Mirrored), Some(Nat)),
        detection(Mirrored, ModeSource::Wslinfo)
    );
    assert_eq!(
        resolve(None, None, Some(Mirrored)),
        detection(Mirrored, ModeSource::Wslconfig)
    );
    assert_eq!(
        resolve(None, None, None),
        detection(Nat, ModeSource::Default)
    );
}

#[test]
fn only_mirrored_reaches_windows_on_localhost() {
    assert!(NetworkingMode::Mirrored.reaches_windows_localhost());
    for mode in [
        NetworkingMode::Nat,
        NetworkingMode::VirtioProxy,
        NetworkingMode::None,
        NetworkingMode::Bridged,
    ] {
        assert!(!mode.reaches_windows_localhost(), "{mode:?}");
    }
}

#[test]
fn the_cache_is_per_distribution_ignoring_case() {
    let cache = ModeCache::default();
    let mirrored = resolve(None, Some(NetworkingMode::Mirrored), None);
    assert_eq!(cache.get("Ubuntu"), None);
    cache.insert("Ubuntu", mirrored);
    assert_eq!(cache.get("ubuntu"), Some(mirrored));
    assert_eq!(cache.get("Debian"), None);
    cache.forget("UBUNTU");
    assert_eq!(cache.get("Ubuntu"), None);
}

#[test]
fn the_wslconfig_lives_in_the_profile() {
    assert_eq!(
        wslconfig_path(Some(r"C:\Users\me")),
        Some(std::path::PathBuf::from(r"C:\Users\me").join(".wslconfig"))
    );
    assert_eq!(wslconfig_path(None), None);
    assert_eq!(wslconfig_path(Some("")), None);
}

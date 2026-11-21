use super::*;

fn entry() -> ToolEntry {
    ToolEntry {
        name: "ffmpeg".to_owned(),
        platform: WINDOWS_X64.to_owned(),
        version: "n9.0.2-49-gc179411bbb".to_owned(),
        url: "https://example.test/ffmpeg.zip".to_owned(),
        sha256: "9ccd65428fb0e60a0d574b76145df41f402b754edd2d06c2e24fa506fc1c2b02".to_owned(),
        size: 1024,
        archive: Archive::Zip,
        executable: "ffmpeg.exe".to_owned(),
        members: vec!["ffmpeg.exe".to_owned(), "LICENSE.txt".to_owned()],
        license: "LGPL-3.0-or-later".to_owned(),
        source_url: "https://example.test/ffmpeg-source.tar.gz".to_owned(),
        upstream_url: None,
        upstream_source_url: None,
    }
}

#[test]
fn the_embedded_manifest_pins_an_lgpl_ffmpeg_for_windows() {
    let manifest = Manifest::embedded().expect("the embedded manifest is valid");
    let ffmpeg = manifest
        .entry(FFMPEG, WINDOWS_X64)
        .expect("ffmpeg for Windows");
    assert!(ffmpeg.url.starts_with("https://"), "{}", ffmpeg.url);
    assert!(ffmpeg.source_url.starts_with("https://"));
    assert_eq!(ffmpeg.license, "LGPL-3.0-or-later");
    assert_eq!(ffmpeg.executable, "ffmpeg.exe");
    assert_eq!(ffmpeg.sha256.len(), 64);
    assert_ne!(ffmpeg.sha256, PLACEHOLDER_SHA256, "the pin is set");
    assert!(ffmpeg.size > 0 && ffmpeg.size <= MAX_SIZE);
}

/// RB-095-02: the Android Platform Tools are downloaded from Google after the person's consent,
/// never shipped; the entry takes `adb.exe` with the two USB libraries and the threads runtime
/// it loads, and the licence notice.
#[test]
fn the_embedded_manifest_pins_googles_platform_tools_for_windows() {
    let manifest = Manifest::embedded().expect("the embedded manifest is valid");
    let adb = manifest.entry(ADB, WINDOWS_X64).expect("adb for Windows");
    assert!(
        adb.url
            .starts_with("https://dl.google.com/android/repository/platform-tools_r"),
        "{}",
        adb.url
    );
    assert!(adb.url.ends_with("-win.zip"), "{}", adb.url);
    assert_eq!(adb.license, ANDROID_SDK_LICENSE);
    assert_eq!(adb.executable, "adb.exe");
    for member in ["AdbWinApi.dll", "AdbWinUsbApi.dll", "NOTICE.txt"] {
        assert!(adb.members.iter().any(|m| m == member), "{member}");
    }
    assert_ne!(adb.sha256, PLACEHOLDER_SHA256, "the pin is set");
    assert!(adb.size > 0 && adb.size <= MAX_SIZE);
    assert!(
        adb.source_url
            .starts_with("https://android.googlesource.com/")
    );
}

/// Besides an LGPL build only the Android SDK licence is taken (RB-095-02), and only for adb:
/// another tool under it is refused like any other licence.
#[test]
fn the_android_sdk_licence_is_taken_for_adb_only() {
    let adb = ToolEntry {
        name: ADB.to_owned(),
        executable: "adb.exe".to_owned(),
        members: vec!["adb.exe".to_owned()],
        license: ANDROID_SDK_LICENSE.to_owned(),
        ..entry()
    };
    assert_eq!(adb.validate(), Ok(()));
    let other = ToolEntry {
        license: ANDROID_SDK_LICENSE.to_owned(),
        ..entry()
    };
    assert!(other.validate().is_err(), "ffmpeg under the SDK licence");
    let gpl_adb = ToolEntry {
        license: "GPL-2.0".to_owned(),
        ..adb
    };
    assert!(gpl_adb.validate().is_err());
}

#[test]
fn a_valid_entry_passes() {
    assert_eq!(entry().validate(), Ok(()));
}

/// The pin may only be a real digest: an unset, placeholder or malformed hash is refused, so a
/// manifest that was never pinned cannot download anything.
#[test]
fn a_placeholder_or_malformed_hash_is_refused() {
    for sha256 in [
        "",
        PLACEHOLDER_SHA256,
        "TODO",
        "9ccd65428fb0e60a0d574b76145df41f402b754edd2d06c2e24fa506fc1c2b0",
        "9CCD65428FB0E60A0D574B76145DF41F402B754EDD2D06C2E24FA506FC1C2B02",
        "zzcd65428fb0e60a0d574b76145df41f402b754edd2d06c2e24fa506fc1c2b02",
    ] {
        let entry = ToolEntry {
            sha256: sha256.to_owned(),
            ..entry()
        };
        assert!(entry.validate().is_err(), "{sha256:?} was taken");
    }
}

#[test]
fn only_https_addresses_are_taken() {
    let plain = ToolEntry {
        url: "http://example.test/ffmpeg.zip".to_owned(),
        ..entry()
    };
    assert!(plain.validate().is_err());
    let source = ToolEntry {
        source_url: "ftp://example.test/src.tar.gz".to_owned(),
        ..entry()
    };
    assert!(source.validate().is_err());
}

#[test]
fn a_gpl_or_unnamed_licence_is_refused() {
    for license in ["GPL-3.0-or-later", "GPL-2.0", "", "proprietary"] {
        let entry = ToolEntry {
            license: license.to_owned(),
            ..entry()
        };
        assert!(entry.validate().is_err(), "{license:?} was taken");
    }
}

#[test]
fn names_that_could_leave_the_tools_folder_are_refused() {
    for version in ["..", "../x", "a/b", "a\\b", ".hidden", ""] {
        let entry = ToolEntry {
            version: version.to_owned(),
            ..entry()
        };
        assert!(entry.validate().is_err(), "version {version:?}");
    }
    let member = ToolEntry {
        members: vec!["ffmpeg.exe".to_owned(), "../evil.dll".to_owned()],
        ..entry()
    };
    assert!(member.validate().is_err());
    let missing = ToolEntry {
        members: vec!["LICENSE.txt".to_owned()],
        ..entry()
    };
    assert!(missing.validate().is_err(), "the executable is a member");
}

#[test]
fn a_size_of_zero_or_over_the_ceiling_is_refused() {
    for size in [0, MAX_SIZE + 1] {
        let entry = ToolEntry { size, ..entry() };
        assert!(entry.validate().is_err(), "{size}");
    }
}

#[test]
fn a_manifest_of_another_version_or_with_unknown_fields_is_refused() {
    let text = serde_json::json!({ "version": 2, "tools": [] }).to_string();
    assert!(Manifest::parse(&text).is_err());
    let mut tool = serde_json::to_value(entry()).expect("entry");
    tool["extra"] = serde_json::json!(true);
    let text = serde_json::json!({ "version": 1, "tools": [tool] }).to_string();
    assert!(Manifest::parse(&text).is_err());
}

#[test]
fn a_tool_without_a_build_for_the_platform_is_named() {
    let manifest = Manifest::embedded().expect("manifest");
    let error = manifest
        .entry(FFMPEG, "aarch64-apple-darwin")
        .expect_err("no build");
    assert!(
        error.to_string().contains("aarch64-apple-darwin"),
        "{error}"
    );
}

#[test]
fn the_size_reads_in_megabytes() {
    assert_eq!(megabytes(168_580_960), "168.6");
    assert_eq!(megabytes(0), "0.0");
}

/// BtbN deletes a daily build after 14 days and keeps the last build of each month for two
/// years, so a BtbN pin is a release dated the last day of its month (rDownloader RD-1101-13).
#[test]
fn every_btbn_pin_is_a_month_end_release() {
    const BTBN: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-";
    let manifest = Manifest::embedded().expect("the embedded manifest is valid");
    for entry in &manifest.tools {
        let Some(tag) = entry.url.strip_prefix(BTBN) else {
            continue;
        };
        let date: Vec<u32> = tag
            .get(..10)
            .map(|date| {
                date.split('-')
                    .filter_map(|part| part.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        let [year, month, day] = date[..] else {
            panic!("{} is not a dated autobuild: {}", entry.name, entry.url);
        };
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let last = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        assert_eq!(
            day, last,
            "{} is pinned to a daily build: {}",
            entry.name, entry.url
        );
    }
}

/// RB-096-05: cloudflared, the presentation's tunnel, is the release's bare program (no archive)
/// under Apache-2.0, pinned like ffmpeg.
#[test]
fn the_embedded_manifest_pins_cloudflared_for_windows() {
    let manifest = Manifest::embedded().expect("the embedded manifest is valid");
    let cloudflared = manifest
        .entry(CLOUDFLARED, WINDOWS_X64)
        .expect("cloudflared for Windows");
    assert!(
        cloudflared
            .url
            .starts_with("https://github.com/cloudflare/cloudflared/releases/download/"),
        "{}",
        cloudflared.url
    );
    assert_eq!(cloudflared.archive, Archive::Exe);
    assert_eq!(cloudflared.executable, "cloudflared.exe");
    assert_eq!(cloudflared.members, ["cloudflared.exe"]);
    assert_eq!(cloudflared.license, "Apache-2.0");
    assert_ne!(cloudflared.sha256, PLACEHOLDER_SHA256, "the pin is set");
    assert!(cloudflared.size > 0 && cloudflared.size <= MAX_SIZE);
}

/// The licences a managed tool may have: an LGPL (ffmpeg) or Apache-2.0 (cloudflared); a GPL
/// still never.
#[test]
fn apache_2_0_is_taken_beside_the_lgpl() {
    let apache = ToolEntry {
        license: "Apache-2.0".to_owned(),
        ..entry()
    };
    assert_eq!(apache.validate(), Ok(()));
    for refused in ["GPL-3.0-only", "Apache-2.0 WITH x", "apache-2.0"] {
        let other = ToolEntry {
            license: refused.to_owned(),
            ..entry()
        };
        assert!(other.validate().is_err(), "{refused:?} was taken");
    }
}

/// A bare program is its one member: the executable, nothing beside it.
#[test]
fn a_bare_program_names_only_its_executable() {
    let bare = ToolEntry {
        archive: Archive::Exe,
        members: vec!["ffmpeg.exe".to_owned()],
        ..entry()
    };
    assert_eq!(bare.validate(), Ok(()));
    let more = ToolEntry {
        archive: Archive::Exe,
        ..entry()
    };
    assert!(
        more.validate().is_err(),
        "a bare program has no LICENSE.txt"
    );
}

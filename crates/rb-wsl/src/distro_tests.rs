use super::*;
use crate::decode::decode;

const EN: &[u8] = include_bytes!("../tests/fixtures/wsl-l-v.en.utf16le");
const EN_UTF8: &[u8] = include_bytes!("../tests/fixtures/wsl-l-v.en.utf8");
const DE: &[u8] = include_bytes!("../tests/fixtures/wsl-l-v.de.utf16le");
const NONE: &[u8] = include_bytes!("../tests/fixtures/wsl-l-v.none.utf16le");
const RUNNING: &[u8] = include_bytes!("../tests/fixtures/wsl-l-running-q.utf16le");

fn distro(name: &str, is_default: bool, state: DistroState, version: u8) -> Distro {
    Distro {
        name: name.into(),
        is_default,
        state,
        version: Some(version),
    }
}

#[test]
fn the_utf16_list_of_wsl_l_v() {
    assert_eq!(
        parse_verbose(&decode(EN)),
        vec![
            distro("Ubuntu-24.04", true, DistroState::Running, 2),
            distro("Debian", false, DistroState::Stopped, 2),
            distro("docker-desktop", false, DistroState::Stopped, 2),
            distro("Legacy", false, DistroState::Stopped, 1),
        ]
    );
    assert_eq!(parse_verbose(&decode(EN_UTF8)), parse_verbose(&decode(EN)));
}

#[test]
fn a_translated_list_reads_names_versions_and_the_default() {
    let mut list = parse_verbose(&decode(DE));
    assert_eq!(
        list,
        vec![
            distro("Ubuntu", true, DistroState::Unknown, 2),
            distro("Debian", false, DistroState::Unknown, 2),
        ]
    );
    apply_running(
        &mut list,
        &parse_quiet(&decode(&b"U\0b\0u\0n\0t\0u\0\r\0\n\0"[..])),
    );
    assert_eq!(list[0].state, DistroState::Running);
    assert_eq!(list[1].state, DistroState::Stopped);
}

#[test]
fn the_running_list_is_the_authority_on_the_state() {
    let mut list = parse_verbose(&decode(EN));
    let running = parse_quiet(&decode(RUNNING));
    assert_eq!(running, vec!["Ubuntu-24.04".to_owned()]);
    list[1].state = DistroState::Running;
    apply_running(&mut list, &running);
    assert_eq!(list[0].state, DistroState::Running);
    assert_eq!(list[1].state, DistroState::Stopped);
}

#[test]
fn the_message_without_distributions_lists_none() {
    assert!(parse_verbose(&decode(NONE)).is_empty());
    assert!(parse_quiet(&decode(NONE)).is_empty());
    assert!(parse_verbose("").is_empty());
}

#[test]
fn quiet_names_once_each_ignoring_case() {
    assert_eq!(
        parse_quiet("Ubuntu\r\nubuntu\r\n\r\nDebian\n"),
        vec!["Ubuntu".to_owned(), "Debian".to_owned()]
    );
}

#[test]
fn choosing_a_distribution() {
    let list = parse_verbose(&decode(EN));
    assert_eq!(
        choose(&list, None).map(|d| d.name.as_str()),
        Ok("Ubuntu-24.04")
    );
    assert_eq!(
        choose(&list, Some("debian")).map(|d| d.name.as_str()),
        Ok("Debian")
    );
    assert_eq!(
        choose(&list, Some("Arch")),
        Err(WslError::DistroNotFound("Arch".into()))
    );
    assert_eq!(choose(&[], None), Err(WslError::NoDistro));
    let no_default: Vec<Distro> = list.iter().skip(1).cloned().collect();
    assert_eq!(
        choose(&no_default, None).map(|d| d.name.as_str()),
        Ok("Debian")
    );
}

#[test]
fn wsl_exe_lives_in_system32() {
    assert_eq!(
        wsl_exe_in(Some(r"C:\Windows")),
        r"C:\Windows\System32\wsl.exe"
    );
    assert_eq!(
        wsl_exe_in(Some(r"D:\WINDOWS\")),
        r"D:\WINDOWS\System32\wsl.exe"
    );
    assert_eq!(wsl_exe_in(None), r"C:\Windows\System32\wsl.exe");
    assert_eq!(wsl_exe_in(Some("relative")), r"C:\Windows\System32\wsl.exe");
    assert_eq!(
        wsl_exe_in(Some(r"\\server\share")),
        r"C:\Windows\System32\wsl.exe"
    );
}

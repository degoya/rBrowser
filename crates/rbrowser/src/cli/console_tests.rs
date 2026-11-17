use super::*;

#[test]
fn what_the_console_twin_hands_over_is_kept() {
    // rbrowser.com: the terminal's input, a pipe for the output and one for the errors.
    assert_eq!(slots([true, true, true], true), [Slot::Keep; 3]);
    assert_eq!(slots([true, true, true], false), [Slot::Keep; 3]);
}

#[test]
fn a_call_straight_from_a_shell_writes_to_its_console_but_never_reads_it() {
    // PowerShell or cmd started rbrowser.exe itself: no handles, the shell's console attached.
    // The shell reads that console too, so `init` must not ask there.
    assert_eq!(
        slots([false, false, false], true),
        [Slot::Empty, Slot::Console, Slot::Console]
    );
}

#[test]
fn without_a_console_to_attach_nothing_is_made_up() {
    // Started from Explorer or a shortcut: no parent console.
    assert_eq!(slots([false, false, false], false), [Slot::Empty; 3]);
}

#[test]
fn a_redirected_output_stays_redirected_and_the_rest_goes_to_the_console() {
    // `rbrowser.exe diagnostics > out.txt` in cmd.
    assert_eq!(
        slots([false, true, false], true),
        [Slot::Empty, Slot::Keep, Slot::Console]
    );
}

#[test]
fn attach_and_release_run_outside_a_runtime_and_twice() {
    // Called first thing in the browser process, before any runtime exists (the RUNTIME RULE),
    // and harmless where there is no console to attach (Linux, the tests).
    attach();
    release();
    release();
}

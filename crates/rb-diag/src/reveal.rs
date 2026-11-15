//! Shows a file in the file manager: Explorer with the file selected on Windows, the folder
//! through `xdg-open` elsewhere. Never with a console window (`rb-process`).

use std::io;
use std::path::Path;

/// Opens the folder of `path` with `path` selected where the platform can. Waits for the
/// launcher, which hands over and exits at once (Explorer exits with 1 even on success, so the
/// status is not read).
pub fn reveal(path: &Path) -> io::Result<()> {
    platform::reveal(path)
}

#[cfg(windows)]
mod platform {
    use std::io;
    use std::os::windows::process::CommandExt as _;
    use std::path::Path;
    use std::process::Command;

    use rb_process::NoConsoleWindow as _;

    pub(super) fn reveal(path: &Path) -> io::Result<()> {
        // Explorer parses its own command line: `/select,"<path>"` as one raw argument.
        Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .no_console_window()
            .status()
            .map(drop)
    }
}

#[cfg(not(windows))]
mod platform {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    use rb_process::NoConsoleWindow as _;

    pub(super) fn reveal(path: &Path) -> io::Result<()> {
        let folder = path.parent().unwrap_or(path);
        Command::new("xdg-open")
            .arg(folder)
            .no_console_window()
            .status()
            .map(drop)
    }
}

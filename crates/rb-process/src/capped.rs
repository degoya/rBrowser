//! A child's output read with a cap (RB-095-05): `Command::output` keeps whatever a program
//! prints, so a program that prints without end (a broken `adb`, a hostile one on `PATH`) would
//! fill the memory. [`output_capped`] reads standard output and standard error side by side, at
//! most `max` bytes each; one byte more ends the child and is an error - never a cut answer,
//! which a parser could misread.

use std::io;
use std::process::{Output, Stdio};

use tokio::io::{AsyncRead, AsyncReadExt as _};
use tokio::process::Command;

/// The cap of the programs rBrowser reads (adb, xcrun, system_profiler, defaults).
pub const MAX_OUTPUT: usize = 1024 * 1024;

/// Runs `command` to its end, its standard output and standard error captured, at most `max`
/// bytes each. The caller sets stdin and the deadline.
///
/// # Errors
///
/// The program did not start or could not be awaited, or it printed more than `max` bytes on
/// either stream (`ErrorKind::OutOfMemory`; the child is killed).
pub async fn output_capped(command: &mut Command, max: usize) -> io::Result<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let read = tokio::try_join!(
        read_capped(child.stdout.take(), max),
        read_capped(child.stderr.take(), max)
    );
    let (stdout, stderr) = match read {
        Ok(streams) => streams,
        Err(error) => {
            // Ended, so it neither blocks on a full pipe nor outlives the caller.
            if child.start_kill().is_ok() {
                let _ended = child.wait().await;
            }
            return Err(error);
        }
    };
    let status = child.wait().await?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// All of `reader` up to `max` bytes; one more is an error.
async fn read_capped<R: AsyncRead + Unpin>(reader: Option<R>, max: usize) -> io::Result<Vec<u8>> {
    let Some(reader) = reader else {
        return Ok(Vec::new());
    };
    let mut bytes = Vec::new();
    let limit = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    reader.take(limit).read_to_end(&mut bytes).await?;
    if bytes.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::OutOfMemory,
            format!("the program printed more than {max} bytes"),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "capped_tests.rs"]
mod tests;

rBrowser for macOS
==================

rBrowser shows every breakpoint of a site at once, in one window. This build
is for macOS 13 or later, one per processor: "aarch64" for Apple Silicon (M1
and later), "x86_64" for Intel Macs. It is signed ad hoc, not by a registered
developer, and not notarised yet. VERSION.txt (in
rBrowser.app/Contents/Resources) names the version, the commit and the CEF
build.


1. Install
----------
From the DMG: open it and drag rBrowser into Applications. From the ZIP:
double-click it and move rBrowser.app into Applications.


2. First start - Gatekeeper
---------------------------
macOS refuses an app from the internet that is not notarised. Either:

  - Open System Settings > Privacy & Security, scroll to the message about
    rBrowser and click "Open Anyway"; then start rBrowser again and confirm.
    On macOS 14 and earlier, a right-click on rBrowser and "Open" does the
    same.

  - Or, in Terminal, remove the quarantine flag once:

      xattr -dr com.apple.quarantine /Applications/rBrowser.app

macOS remembers the decision.


3. Command line
---------------
The executable inside the bundle takes the same commands as on Windows:

  /Applications/rBrowser.app/Contents/MacOS/rbrowser --help
  /Applications/rBrowser.app/Contents/MacOS/rbrowser open ~/path/to/project


4. Not on macOS yet
-------------------
  - WebKit views, flows and Lighthouse (the Node runtime and the sidecar).
  - Firefox and WebKit windows inside the grid cells: they open as windows of
    their own.
  - The right mouse button on a label strip, the middle-button pan of the
    canvas, the title bar in rBrowser's dark theme.
  - The Chromium sandbox of the helper processes.


5. Updates
----------
rBrowser does not replace itself on macOS: when a new release is out, it says
so and opens the release page; download the new DMG or ZIP and replace the
app. Installed with Homebrew: brew upgrade rbrowser.


6. Logs and diagnostics
-----------------------
  ~/.local/state/rBrowser/logs        the app's log and CEF's
  rbrowser diagnostics --out <folder> a ZIP with logs and settings (secrets
                                      masked) for a bug report

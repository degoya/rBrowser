rBrowser for Linux x64
======================

rBrowser shows a site at every breakpoint at once, in one window. This is the
portable archive; the deb and rpm packages install the same files into
/usr/lib/rbrowser with a menu entry and the sandbox set up.

1. Start
--------
Unpack the archive anywhere and start the program from a graphical session
(X11, or Wayland through XWayland):

  ./rbrowser
  ./rbrowser open ~/path/to/project
  ./rbrowser --help

2. The Chromium sandbox
-----------------------
Chromium runs its pages in a sandbox: the kernel's unprivileged user
namespaces, or the setuid helper chrome-sandbox beside the program. Ubuntu
23.10 and later restrict the namespaces with AppArmor; there, and on any
system without them, rBrowser stops with a message unless the helper is
root's and setuid. Make it so once (the deb and rpm packages do it):

  sudo chown root:root chrome-sandbox
  sudo chmod 4755 chrome-sandbox

Or start without the sandbox, at your own risk: ./rbrowser --no-sandbox, or
switch engines.chromium.sandbox off in the settings.

3. What the system needs
------------------------
The libraries of Chromium: on Ubuntu and Debian

  sudo apt install libnss3 libgbm1 libxkbcommon0 libasound2t64 libatk-bridge2.0-0t64 \
    libcups2t64 libxcomposite1 libxdamage1 libxrandr2 libpango-1.0-0 libcairo2

(on Debian 12 without the t64 suffixes); on Fedora

  sudo dnf install nss mesa-libgbm libxkbcommon alsa-lib at-spi2-atk cups-libs \
    libXcomposite libXdamage libXrandr pango cairo

Firefox views use the installed Firefox (/usr/bin/firefox, /snap/bin/firefox
or the path in engines.firefox.path). The Snap edition may not reach rBrowser's
profile folder under ~/.local/state; the Flatpak edition is not supported.
Mozilla's own deb or tarball always works. WebKit views need the system
libraries of Playwright's WebKit build (Playwright's `install-deps webkit`
lists them); rBrowser does not check them yet.

4. Not on Linux yet
-------------------
  - Firefox and WebKit windows inside the grid cells: they open as windows of
    their own.
  - The right mouse button on a label strip, the middle-button pan of the
    canvas, the title bar in rBrowser's theme.
  - Video recording (ffmpeg is not offered for Linux yet).

5. Updates
----------
The archive does not replace itself: when a new release is out, rBrowser
says so and opens the release page. Installed from the deb or rpm package,
apt or dnf update it.

6. Logs and diagnostics
-----------------------
  ~/.local/state/rBrowser/logs        the app's log and CEF's
  ~/.config/rbrowser                  the settings
  ./rbrowser diagnostics --out <dir>  a ZIP with logs and settings (secrets
                                      masked) for a bug report

rBrowser - internal test package (Windows x64)
==============================================

rBrowser shows every breakpoint of a site at once, in one window. This ZIP is an
internal build for team tests: portable, no installer, not code-signed yet.
VERSION.txt names the version, the commit and the bundled CEF and Node runtimes.


1. Unpack
---------
Unpack the whole ZIP into a folder of your own, for example
C:\Tools\rbrowser-<version>-windows-x64. Do not start rbrowser.exe from inside
the ZIP, and do not run it from a WSL share (\\wsl.localhost\...): the browser
sandbox refuses both.


2. Start - SmartScreen
----------------------
Double-click rbrowser.exe. The package is not signed yet, so Windows Defender
SmartScreen may show "Windows protected your PC":

  - Click "More info".
  - Click "Run anyway".

Windows remembers the decision for this file. Your antivirus may scan the folder
on the first start, which can take a few seconds.

From PowerShell or a command prompt, use "rbrowser" without ".exe": it runs
rbrowser.com, the console twin, which waits for a command and shows its output
(rbrowser.exe is a window program; a shell neither waits for it nor shows what
it prints). In the app folder, PowerShell needs ".\" in front:

  .\rbrowser init --url https://example.com
  .\rbrowser open C:\path\to\your\project
  .\rbrowser --help

Or put the folder on your PATH (Settings > System > About > Advanced system
settings > Environment Variables > Path) and type "rbrowser ..." anywhere.
"rbrowser init" lists the views it proposes and asks before it writes
.rbrowser\config.json; "--yes" writes without asking. A call that opens the
window gives the prompt back at once; the window stays open.


3. Where settings and logs go
-----------------------------
Installed (the default):
  settings   %APPDATA%\rBrowser
  logs       %LOCALAPPDATA%\rBrowser\logs

Installed with the MSI (rbrowser-<version>-windows-x64.msi, per user, no admin
rights): the program lives in %LOCALAPPDATA%\Programs\rBrowser and the Start
menu has an "rBrowser" entry. Uninstalling it (Settings > Apps) removes the
program only; settings and logs stay where they are listed above.

Portable mode: create an empty file named "portable" (no extension) next to
rbrowser.exe. Settings, logs and browser profiles then live in the "data"
folder next to rbrowser.exe instead. Delete the file to switch back; the
"data" folder stays until you delete it.


4. Firefox (optional)
---------------------
rBrowser does not need Firefox. Firefox views use the Firefox installed on this
machine; without one, those views are simply unavailable.


5. Updates
----------
rBrowser looks for a new version at every start and shows it in the status bar
with its size and notes. "Install and restart" downloads it, checks its
signature and SHA-256, replaces the program folder (your own files and the
portable "data" folder stay) and starts the new version; if that does not
start, the previous version comes back by itself. Settings > General > Updates
turns the check off, picks the channel, or installs a downloaded update when
you quit. Keep rbrowser-updater.exe next to rbrowser.exe: it does the switch.
Installed with Scoop or winget, update with "scoop update rbrowser" or
"winget upgrade degoya.rBrowser" instead.


6. When something goes wrong
----------------------------
Create a diagnostics ZIP and send it to the rBrowser team:

  - in rBrowser: Ctrl+Shift+Space, then "," and "D" (Create diagnostics ZIP), or
  - from a command prompt in the folder: .\rbrowser diagnostics

The ZIP holds the logs, crash dumps, versions and the project config with
secrets masked. Nothing is uploaded anywhere - crash dumps stay on this machine
and you decide whom to send the ZIP to.


7. Licences
-----------
rBrowser is licensed under the GNU General Public License v3.0 or later
(LICENSE). It bundles the Chromium Embedded Framework, Chromium and Node.js;
NOTICE names them and their licences. THIRD-PARTY-LICENSES.txt holds the full
licence texts of the Rust crates and npm packages rBrowser is built from.

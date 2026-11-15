//! `rbrowser-helper`: the executable of the five helper apps of the macOS bundle
//! (`rbrowser Helper.app`, `(GPU)`, `(Renderer)`, `(Plugin)`, `(Alerts)` in
//! `rBrowser.app/Contents/Frameworks`, RB-090-15). CEF starts its subprocesses from them; each
//! runs the app's CEF application object, so the app's scheme and the renderer's handler are
//! the same in every process. Elsewhere it only says that it belongs in the bundle.

fn main() {
    std::process::exit(rbrowser::run_helper());
}

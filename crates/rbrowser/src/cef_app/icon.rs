//! Gives a window rBrowser's icon ([`crate::window_icon`]): the title bar's and the taskbar's.
//! Without it CEF's Views windows show Chromium's default. UI thread.

use cef::*;

use crate::window_icon::{LARGE, SMALL};

/// Sets `window`'s title bar and taskbar icon.
pub fn apply(window: &Window) {
    if let Some(mut small) = image(&SMALL) {
        window.set_window_icon(Some(&mut small));
    }
    if let Some(mut large) = image(&LARGE) {
        window.set_window_app_icon(Some(&mut large));
    }
}

fn image(representations: &[(f32, &[u8])]) -> Option<Image> {
    let image = image_create()?;
    for (scale, png) in representations {
        if image.add_png(*scale, Some(png)) == 0 {
            tracing::warn!(scale, "the window icon could not be read");
            return None;
        }
    }
    Some(image)
}

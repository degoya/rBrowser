//! RB-080-47: the colour a breakpoint gets in the settings reaches its footer at once.

use rb_engine::ViewHost as _;

use super::tests::{classic, config, setup};
use crate::view_footer::{LIGHT_TEXT, contrast};

#[tokio::test]
async fn a_colour_saved_in_the_settings_colours_the_footer_without_a_restart() {
    let setup = setup(Some(classic()));
    let view = setup.host.views()[0].id;
    // What the config watcher does after the settings window wrote `breakpoints`.
    let mut next = config();
    next.breakpoints[0].color = Some("#14532d".to_owned());
    setup.host.open_config(next, None).expect("reloads");
    let grid = setup.ui.last_grid().expect("grid");
    let footer = grid.cells[0].footer.clone().expect("footer");
    assert_eq!(footer.background, 0x0014_532d);
    // A dark green: light text, and it reads.
    assert_eq!(footer.foreground, LIGHT_TEXT);
    assert!(contrast(footer.background, footer.foreground) >= 4.5);
    assert_eq!(
        setup.host.state().footer.sent[&view].background,
        0x0014_532d
    );
}

#[tokio::test]
async fn the_colour_is_the_one_of_the_config_in_effect_not_of_the_view_s_copy() {
    let setup = setup(Some(classic()));
    let view = setup.host.views()[1].id;
    // A config put in effect without a new grid: the view keeps the breakpoint it was opened
    // with, the footer takes the colour of the config.
    let mut next = config();
    next.breakpoints[1].color = Some("#1e3a8a".to_owned());
    setup.host.replace_config(next);
    let sent = setup.host.state().footer.sent[&view].clone();
    assert_eq!(sent.background, 0x001e_3a8a);
    assert_eq!(sent.foreground, LIGHT_TEXT);
}

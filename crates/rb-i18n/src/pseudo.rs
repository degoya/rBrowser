//! The pseudo-locale `en-XA`: English with accented letters, in brackets, at least 40 % longer,
//! so a label the layout cuts off or a string that is not externalised shows at a glance. The
//! shell implements the same rule (`docs/i18n.md`): vowels and `c`, `n`, `y` get an accent
//! (literal text only - arguments stay as given), then the formatted text `t` of `n` characters
//! becomes `[` + `t` + `~` x (ceil(0.4 n) - 2) + `]`.

/// The accented look-alike of `c`, or `c` itself.
fn accented(c: char) -> char {
    match c {
        'a' => '\u{e1}',
        'c' => '\u{e7}',
        'e' => '\u{e9}',
        'i' => '\u{ed}',
        'n' => '\u{f1}',
        'o' => '\u{f3}',
        'u' => '\u{fa}',
        'y' => '\u{fd}',
        'A' => '\u{c1}',
        'C' => '\u{c7}',
        'E' => '\u{c9}',
        'I' => '\u{cd}',
        'N' => '\u{d1}',
        'O' => '\u{d3}',
        'U' => '\u{da}',
        'Y' => '\u{dd}',
        other => other,
    }
}

/// Appends `text` with its letters accented to `out`.
pub(crate) fn accent(text: &str, out: &mut String) {
    out.extend(text.chars().map(accented));
}

/// `text` in brackets, padded with `~` to at least 140 % of its length; empty stays empty.
pub(crate) fn pad(text: &str) -> String {
    let length = text.chars().count();
    if length == 0 {
        return String::new();
    }
    let extra = (length * 2).div_ceil(5);
    let tildes = extra.saturating_sub(2);
    let mut out = String::with_capacity(text.len() + tildes + 2);
    out.push('[');
    out.push_str(text);
    out.extend(std::iter::repeat_n('~', tildes));
    out.push(']');
    out
}

/// `text` as the pseudo-locale shows it: accented, bracketed, padded.
pub fn pseudo_localize(text: &str) -> String {
    let mut accented = String::with_capacity(text.len());
    accent(text, &mut accented);
    pad(&accented)
}

#[cfg(test)]
#[path = "pseudo_tests.rs"]
mod tests;

//! Terminal colors for the `pleno` tools.
//!
//! Output is colored only on a terminal. `NO_COLOR` turns colors off and
//! `FORCE_COLOR` turns them on, following <https://no-color.org> and
//! <https://force-color.org>; `TERM=dumb` also turns them off.

use std::io::IsTerminal;
use std::sync::OnceLock;

/// The Pleno logo gradient: red, magenta, violet.
pub const BRAND: [(u8, u8, u8); 3] = [(0xEF, 0x48, 0x57), (0xD2, 0x11, 0xEC), (0x7F, 0x52, 0xFF)];

/// An output stream, which may or may not be a terminal.
#[derive(Clone, Copy)]
pub enum Stream {
    Stdout,
    Stderr,
}

fn decide(is_terminal: bool) -> bool {
    let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
    if set("NO_COLOR") {
        return false;
    }
    if set("FORCE_COLOR") {
        return true;
    }
    is_terminal && std::env::var("TERM").map_or(true, |term| term != "dumb")
}

/// Whether to color what is written to `stream`.
pub fn enabled(stream: Stream) -> bool {
    static STDOUT: OnceLock<bool> = OnceLock::new();
    static STDERR: OnceLock<bool> = OnceLock::new();
    match stream {
        Stream::Stdout => *STDOUT.get_or_init(|| decide(std::io::stdout().is_terminal())),
        Stream::Stderr => *STDERR.get_or_init(|| decide(std::io::stderr().is_terminal())),
    }
}

/// `text` wrapped in an SGR code (`"1"` bold, `"32"` green, ...) when
/// `stream` is colored.
pub fn paint(stream: Stream, code: &str, text: &str) -> String {
    if enabled(stream) {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn bold(stream: Stream, text: &str) -> String {
    paint(stream, "1", text)
}

pub fn dim(stream: Stream, text: &str) -> String {
    paint(stream, "2", text)
}

pub fn red(stream: Stream, text: &str) -> String {
    paint(stream, "1;31", text)
}

pub fn green(stream: Stream, text: &str) -> String {
    paint(stream, "32", text)
}

pub fn yellow(stream: Stream, text: &str) -> String {
    paint(stream, "1;33", text)
}

pub fn cyan(stream: Stream, text: &str) -> String {
    paint(stream, "36", text)
}

/// `text` colored character by character along a gradient through `stops`
/// (24-bit color), bold. Plain text when `stream` is not colored.
pub fn gradient(stream: Stream, text: &str, stops: &[(u8, u8, u8)]) -> String {
    if !enabled(stream) || stops.is_empty() {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let last = chars.len().saturating_sub(1).max(1) as f32;
    let mut out = String::from("\x1b[1m");
    for (i, ch) in chars.iter().enumerate() {
        if ch.is_whitespace() {
            out.push(*ch);
            continue;
        }
        let (r, g, b) = color_at(stops, i as f32 / last);
        out.push_str(&format!("\x1b[38;2;{r};{g};{b}m{ch}"));
    }
    out.push_str("\x1b[0m");
    out
}

/// The color a fraction `t` (0 to 1) of the way along `stops`.
fn color_at(stops: &[(u8, u8, u8)], t: f32) -> (u8, u8, u8) {
    if stops.len() == 1 {
        return stops[0];
    }
    let scaled = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let index = (scaled.floor() as usize).min(stops.len() - 2);
    let local = scaled - index as f32;
    let (from, to) = (stops[index], stops[index + 1]);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * local).round() as u8;
    (mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_endpoints_are_the_stops() {
        assert_eq!(color_at(&BRAND, 0.0), BRAND[0]);
        assert_eq!(color_at(&BRAND, 0.5), BRAND[1]);
        assert_eq!(color_at(&BRAND, 1.0), BRAND[2]);
    }

    #[test]
    fn no_color_wins_over_force_color() {
        // `decide` reads the environment; with neither set it follows the terminal.
        if std::env::var_os("NO_COLOR").is_none() && std::env::var_os("FORCE_COLOR").is_none() {
            assert!(!decide(false));
        }
    }
}

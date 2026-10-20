use crate::{line::tab_separator, LinePart};
use ansi_term::{ANSIString, ANSIStrings};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use zellij_tile::prelude::*;
use zellij_tile_utils::style;

fn cursors<'a>(
    focused_clients: &'a [ClientId],
    colors: MultiplayerColors,
) -> (Vec<ANSIString<'a>>, usize) {
    // cursor section, text length
    let mut len = 0;
    let mut cursors = vec![];
    for client_id in focused_clients.iter() {
        if let Some(color) = client_id_to_colors(*client_id, colors) {
            cursors.push(style!(color.1, color.0).paint(" "));
            len += 1;
        }
    }
    len += 2; // 2 for the brackets: [ and ]
    (cursors, len)
}

#[derive(Clone, Copy, PartialEq)]
pub enum IconState {
    Idle,
    Working,
    Done, // finished and not yet seen
    Attn, // Claude waits for an answer (permission / question)
}

pub struct Icon {
    pub glyph: String,
    pub color: PaletteColor,
    pub race: Option<(String, PaletteColor)>,
    // race.0 is a 2-cell initials badge (no colour icon font): drawn as a coloured chip, flush left
    pub badge: bool,
    pub state: IconState,
    // race accent: when set, the whole tab is tinted (dim when inactive, bright when active)
    pub tint: Option<(u8, u8, u8)>,
    pub dim: f32,
    pub blink: Option<bool>, // Some(bright?) while an unseen finished tab blinks
    pub underline: bool, // underline the active tinted tab (config active_underline)
}

fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

pub fn rel_lum(c: (u8, u8, u8)) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
}

// Icon-font glyphs (race icons U+1F7E0..E6, character icons U+1FA70..U+1FAD6) are emoji codepoints: wide
// (2 cells) for unicode-width and drawn as 2-cell colour bitmaps, so no extra padding is needed. Private-use
// codepoints do not work: kitty draws them as 1-cell monochrome alpha masks.
// Only codepoints that really are 2 cells wide count (some unused ones in the range report width 1).
fn is_icon_font_char(c: char) -> bool {
    (('\u{1F7E0}'..='\u{1F7E6}').contains(&c) || ('\u{1FA70}'..='\u{1FAFF}').contains(&c)) && c.width() == Some(2)
}

pub fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let (x, y) = (rel_lum(a), rel_lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

// Tab colours for a race accent (bg, fg), independent of the theme's tab background:
//  active   = accent lightened 15%, black or white text (best contrast, always >= 4.5:1)
//  inactive = accent scaled to `dim` (default 0.4) of its brightness (same hue), with light text
//             mixed toward the background until the contrast is just 4.5:1 (muted but readable)
pub fn tint_colors(accent: (u8, u8, u8), active: bool, dim: f32) -> ((u8, u8, u8), (u8, u8, u8)) {
    let white = (255u8, 255u8, 255u8);
    if active {
        let bg = mix(accent, white, 0.15);
        return (bg, best_text(bg));
    }
    let bg = mix((0, 0, 0), accent, dim);
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if contrast(bg, mix(bg, white, mid)) >= 4.5 { hi = mid } else { lo = mid }
    }
    (bg, mix(bg, white, hi))
}

// Inactive tabs without a tint (plain tabs): the theme's ribbon background blended toward the bar background
// by `dim` (1.0 = unchanged), text = the darker/lighter end that reaches just 4.5:1 on it, so plain tabs
// recede like the dimmed tinted ones and stay readable.
pub fn plain_colors(bar: (u8, u8, u8), ribbon: (u8, u8, u8), dim: f32) -> ((u8, u8, u8), (u8, u8, u8)) {
    let bg = mix(bar, ribbon, dim);
    let (white, black) = ((255, 255, 255), (0, 0, 0));
    let end = if contrast(bg, white) >= contrast(bg, black) { white } else { black };
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if contrast(bg, mix(bg, end, mid)) >= 4.5 { hi = mid } else { lo = mid }
    }
    (bg, mix(bg, end, hi))
}

fn best_text(bg: (u8, u8, u8)) -> (u8, u8, u8) {
    let (black, white) = ((0u8, 0u8, 0u8), (255u8, 255u8, 255u8));
    if contrast(bg, black) >= contrast(bg, white) { black } else { white }
}

// Initials badge (no icon font): a chip of the character's accent that stands out from the tab it sits on,
// brighter than an inactive (dimmed) tab and darker than the active (bright) one; black or white text,
// whichever contrasts more (>= 4.4:1 for every accent of the RACES table).
pub fn badge_colors(accent: (u8, u8, u8), look_active: bool, dim: f32) -> ((u8, u8, u8), (u8, u8, u8)) {
    let k = if look_active { 0.55 } else { (dim + 0.35).min(1.0) };
    let bg = mix((0, 0, 0), accent, k);
    (bg, best_text(bg))
}

pub fn render_tab(
    text: String,
    icon: Option<Icon>,
    tab: &TabInfo,
    is_alternate_tab: bool,
    palette: Styling,
    separator: &str,
    dimmed: bool,
    show_other_clients: bool,
    plain_dim: f32,
) -> LinePart {
    let focused_clients: &[ClientId] = if show_other_clients { tab.other_focused_clients.as_slice() } else { &[] };
    let separator_width = separator.width();
    let alternate_tab_color = if is_alternate_tab {
        palette.ribbon_unselected.emphasis_1
    } else {
        palette.ribbon_unselected.background
    };
    // a blinking tab alternates between the active (bright) and inactive look; this includes the active
    // tab when its agent finished while the terminal window was not OS-focused
    let look_active = icon.as_ref().and_then(|i| i.blink).unwrap_or(tab.active);
    let background_color = if look_active {
        palette.ribbon_selected.background
    } else if is_alternate_tab {
        alternate_tab_color
    } else {
        palette.ribbon_unselected.background
    };
    let foreground_color = if tab.is_flashing_bell {
        if look_active {
            palette.ribbon_selected.emphasis_3
        } else {
            palette.ribbon_unselected.emphasis_3
        }
    } else if look_active {
        palette.ribbon_selected.base
    } else {
        palette.ribbon_unselected.base
    };
    let separator_fill_color = palette.text_unselected.background;
    let background_color = if dimmed {
        palette.ribbon_unselected.background
    } else {
        background_color
    };
    let tint = match (icon.as_ref().and_then(|i| i.tint), dimmed || tab.is_flashing_bell) {
        (Some(acc), false) => Some(tint_colors(acc, look_active, icon.as_ref().map_or(0.4, |i| i.dim))),
        _ => None,
    };
    let (background_color, foreground_color) = match tint {
        Some((bg, fg)) => (PaletteColor::Rgb(bg), PaletteColor::Rgb(fg)),
        None => (background_color, foreground_color),
    };
    // inactive plain tab (no tint): subdue the theme's bright ribbon colours (option plain_dim)
    let (background_color, foreground_color) = match (
        tint.is_some() || dimmed || look_active || tab.is_flashing_bell || plain_dim >= 1.0,
        palette.text_unselected.background,
        background_color,
    ) {
        (false, PaletteColor::Rgb(bar), PaletteColor::Rgb(rb)) => {
            let (bg, fg) = plain_colors(bar, rb, plain_dim);
            (PaletteColor::Rgb(bg), PaletteColor::Rgb(fg))
        }
        _ => (background_color, foreground_color),
    };
    let text_style = if dimmed {
        style!(palette.ribbon_unselected.base, background_color).italic()
    } else if tint.is_some() && !look_active {
        style!(foreground_color, background_color) // inactive race tabs recede: no bold
    } else if tint.is_some() && icon.as_ref().map_or(false, |i| i.underline) {
        style!(foreground_color, background_color).bold().underline()
    } else {
        style!(foreground_color, background_color).bold()
    };
    let left_separator = style!(separator_fill_color, background_color).paint(separator);
    let icon_width = icon
        .as_ref()
        .map_or(0, |i| (if i.glyph.is_empty() { 0 } else { i.glyph.width() + 1 }) + i.race.as_ref().map_or(0, |r| r.0.width() + 1));
    // a colour-font race icon sits flush against the left edge of the ribbon: no leading padding space
    // (so does the initials badge, which has the same 2-cell width)
    let flush = icon.as_ref().map_or(false, |i| i.race.is_some() && i.badge)
        || icon
            .as_ref()
            .and_then(|i| i.race.as_ref())
            .map_or(false, |r| r.0.chars().next().map_or(false, |c| is_icon_font_char(c)));
    let mut tab_text_len = text.width() + icon_width + (separator_width * 2) + if flush { 1 } else { 2 }; // padding

    let tab_styled_text = if let Some(icon) = icon {
        let icon_style = if tint.is_some() {
            // state stays visible on the tinted tab: working = italic, unseen finish = inverted badge
            match icon.state {
                IconState::Idle => text_style,
                IconState::Working => text_style.italic(),
                IconState::Done => style!(background_color, foreground_color).bold(),
                IconState::Attn => style!(background_color, foreground_color).bold().underline(),
            }
        } else {
            text_style.fg(match icon.color {
                PaletteColor::Rgb((r, g, b)) => ansi_term::Color::RGB(r, g, b),
                PaletteColor::EightBit(c) => ansi_term::Color::Fixed(c),
            })
        };
        let race_part = match icon.race {
            Some((g, c)) if icon.badge && !dimmed => {
                let (bg, fg) = match (icon.tint, tint) {
                    (Some(acc), Some(_)) => badge_colors(acc, look_active, icon.dim),
                    _ => {
                        let rgb = match c {
                            PaletteColor::Rgb(x) => x,
                            PaletteColor::EightBit(_) => (128, 128, 128),
                        };
                        (rgb, best_text(rgb))
                    }
                };
                format!("{}{}", style!(PaletteColor::Rgb(fg), PaletteColor::Rgb(bg)).bold().paint(g), text_style.paint(" "))
            }
            Some((g, c)) => {
                let c = if tint.is_some() { foreground_color } else { c };
                let rc = match c {
                    PaletteColor::Rgb((r, g, b)) => ansi_term::Color::RGB(r, g, b),
                    PaletteColor::EightBit(c) => ansi_term::Color::Fixed(c),
                };
                let pad = " ";
                format!("{}{}", text_style.fg(rc).paint(g), text_style.paint(pad))
            }
            None => String::new(),
        };
        let parts = [
            text_style.paint(if flush { "" } else { " " }),
            ANSIString::from(race_part),
            icon_style.paint(icon.glyph.clone()),
            text_style.paint(if icon.glyph.is_empty() { format!("{} ", text) } else { format!(" {} ", text) }),
        ];
        ANSIStrings(&parts).to_string()
    } else {
        text_style.paint(format!(" {} ", text)).to_string()
    };

    let right_separator = style!(background_color, separator_fill_color).paint(separator);
    let tab_styled_text = if !focused_clients.is_empty() {
        let (cursor_section, extra_length) =
            cursors(focused_clients, palette.multiplayer_user_colors);
        tab_text_len += extra_length;
        let mut s = String::new();
        let cursor_beginning = text_style.paint("[").to_string();
        let cursor_section = ANSIStrings(&cursor_section).to_string();
        let cursor_end = text_style.paint("]").to_string();
        s.push_str(&left_separator.to_string());
        s.push_str(&tab_styled_text);
        s.push_str(&cursor_beginning);
        s.push_str(&cursor_section);
        s.push_str(&cursor_end);
        s.push_str(&right_separator.to_string());
        s
    } else {
        format!("{}{}{}", left_separator, tab_styled_text, right_separator)
    };

    LinePart {
        part: tab_styled_text,
        len: tab_text_len,
        tab_index: Some(tab.position),
    }
}

pub fn tab_style(
    mut tabname: String,
    icon: Option<Icon>,
    tab: &TabInfo,
    mut is_alternate_tab: bool,
    palette: Styling,
    capabilities: PluginCapabilities,
    dimmed: bool,
    show_other_clients: bool,
    plain_dim: f32,
) -> LinePart {
    let separator = tab_separator(capabilities);

    if tab.is_fullscreen_active {
        tabname.push_str(" (FULLSCREEN)");
    } else if tab.is_sync_panes_active {
        tabname.push_str(" (SYNC)");
    }
    if tab.has_bell_notification || tab.is_flashing_bell {
        tabname.push_str(" [!]");
    }
    // we only color alternate tabs differently if we can't use the arrow fonts to separate them
    if !capabilities.arrow_fonts {
        is_alternate_tab = false;
    }

    render_tab(tabname, icon, tab, is_alternate_tab, palette, separator, dimmed, show_other_clients, plain_dim)
}

pub(crate) fn get_tab_to_focus(
    tab_line: &[LinePart],
    active_tab_idx: usize,
    mouse_click_col: usize,
) -> Option<usize> {
    let clicked_line_part = get_clicked_line_part(tab_line, mouse_click_col)?;
    let clicked_tab_idx = clicked_line_part.tab_index?;
    // tabs are indexed starting from 1 so we need to add 1
    let clicked_tab_idx = clicked_tab_idx + 1;
    if clicked_tab_idx != active_tab_idx {
        return Some(clicked_tab_idx);
    }
    None
}

pub(crate) fn get_clicked_line_part(
    tab_line: &[LinePart],
    mouse_click_col: usize,
) -> Option<&LinePart> {
    let mut len = 0;
    for tab_line_part in tab_line {
        if mouse_click_col >= len && mouse_click_col < len + tab_line_part.len {
            return Some(tab_line_part);
        }
        len += tab_line_part.len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_colors_blend() {
        let bar = (0x23, 0x20, 0x22);
        let rib = (0xd5, 0xb9, 0xbb);
        assert_eq!(plain_colors(bar, rib, 1.0).0, rib);
        assert_eq!(plain_colors(bar, rib, 0.0).0, bar);
        let mut prev = 255.0;
        for d in [1.0f32, 0.7, 0.5, 0.4, 0.25, 0.1] {
            let (bg, fg) = plain_colors(bar, rib, d);
            println!("plain_dim {d}: bg {bg:?} fg {fg:?} contrast {:.2}", contrast(bg, fg));
            assert!(contrast(bg, fg) >= 4.4);
            assert!(rel_lum(bg) < prev + 1e-6);
            prev = rel_lum(bg);
        }
        let (bg, _) = plain_colors(bar, rib, 0.4);
        assert!(rel_lum(bg) < rel_lum(rib) / 2.0);
    }
}

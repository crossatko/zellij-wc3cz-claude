// Tab bar replacement for zellij that marks tabs running a Claude Code session.
//
// State comes from Claude Code hooks (see ../../claude-hook.sh): the hook writes
// <zellij tmp dir>/zellij-claude/<session>/<terminal pane id> ("<state> <epoch ns>"; plugin sees it as /tmp/zellij-claude) and pokes the
// plugin with `zellij pipe --name claude_poke`. Every tab has its own instance of this
// plugin, so state lives in files and each instance re-reads it. Instances in inactive tabs
// receive no Tab/PaneUpdate events at all, so the acting instance (lowest plugin id) asks the server for a fresh
// snapshot (get_session_list) before it dings, switches tabs or acknowledges finished tabs.
use std::collections::{BTreeMap, HashMap, HashSet};
use zellij_tile::prelude::*;

mod clipboard_utils;
mod line;
mod tab;

use tab::{get_tab_to_focus, tab_style, Icon, IconState};

pub static ARROW_SEPARATOR: &str = "\u{e0b0}";

#[derive(Debug, Default)]
pub struct LinePart {
    pub part: String,
    pub len: usize,
    pub tab_index: Option<usize>,
}

pub struct TabRenderData {
    pub tabs: Vec<LinePart>,
    pub active_tab_index: usize,
    pub active_swap_layout_name: Option<String>,
    pub is_swap_layout_dirty: bool,
}

const ICON: &str = "\u{2733}"; // eight-spoked asterisk, the glyph Claude Code itself uses
// Race identities. Edit this table to change glyphs / accent colours (rgb is blended with the theme).
const RACES: &[(&str, &str, (u8, u8, u8), Option<&str>)] = &[
    // name, symbol glyph, accent, colour-font codepoint (race_font; 2 cells wide, from the WC3 icon font)
    ("human", "\u{2694}", (212, 167, 58), Some("\u{1F7E0}")),   // crossed swords, gold
    ("orc", "\u{2620}", (95, 174, 74), Some("\u{1F7E1}")),      // skull and crossbones, green
    ("undead", "\u{263D}", (155, 106, 214), Some("\u{1F7E2}")), // first quarter moon, purple
    ("nightelf", "\u{2726}", (53, 185, 176), Some("\u{1F7E3}")), // four pointed star, teal
    ("naga", "\u{223F}", (46, 134, 193), Some("\u{1F7E4}")),    // sine wave, sea blue
    ("demon", "\u{26E7}", (214, 69, 65), Some("\u{1F7E5}")),    // inverted pentagram, red
    ("neutral", "\u{2691}", (160, 160, 160), Some("\u{1F7E6}")), // flag, grey
    ("campaign", "\u{272A}", (230, 126, 34), None),              // circled star, orange
];
// Unknown groups get GENERIC_GLYPH and a colour derived from their name.
const GENERIC_GLYPH: &str = "\u{25C6}";
// empty = play-phrase.sh picks the packaged library: sounds/wc3 next to the script
const DEFAULT_SOUNDS_DIR: &str = "";
// The wasm cannot know where it was loaded from. The package dir (play-phrase.sh, claude-agents.sh,
// os-focus.sh, sounds/) is, in this order: the `package_dir` option; the path claude-hook.sh writes on
// every event to <state root>/pkgdir (its own resolved directory: plugin cache, checkout, ...); this
// legacy default (the link older setup.sh versions created).
const DEFAULT_PACKAGE_DIR: &str = "~/.config/zellij/plugins/claude-tabs";

// seconds a published library file stays fresh / a follower waits for the leader's file before scanning itself
const LIBRARY_STALE_S: u64 = 100;
const LIBRARY_GRACE_S: u64 = 15;
// a get_session_list() snapshot lags behind Tab/PaneUpdate (the server refreshes it on a ~1 s tick), so for
// this long after such an event the event's tabs / panes win over a snapshot (else the bar of the tab just
// switched to showed the previous tab as active again)
const EVENT_HOLD_MS: u64 = 3000;
// an agent scan (claude-agents.sh) result shared through <pane>.agents counts for this long
const AGENTS_FRESH_S: u64 = 10;

// Package dir: the `package_dir` option wins; else keep the current dir while a scan confirmed it (the
// hook's global pkgdir file flips between package copies on every event, each flip would force a rescan);
// else follow the file; else the legacy default.
fn choose_pkg(opt: Option<&str>, file: Option<&str>, current: &str, current_ok: bool) -> String {
    if let Some(o) = opt {
        return o.to_string();
    }
    if current_ok && !current.is_empty() {
        return current.to_string();
    }
    file.map_or_else(|| DEFAULT_PACKAGE_DIR.to_string(), |f| f.to_string())
}

// script `name` next to phrase_script; with `phrase_script ""` (clips off) next to the package dir
fn sibling_of(phrase_script: &str, pkg_dir: &str, name: &str) -> String {
    match phrase_script.rfind('/') {
        Some(i) => format!("{}/{}", &phrase_script[..i], name),
        None if !pkg_dir.is_empty() => format!("{}/{}", pkg_dir, name),
        None => name.to_string(),
    }
}

// glyph + accent colour for any group name
fn race_style(name: &str, font: bool, mac: bool) -> (String, (u8, u8, u8)) {
    if let Some(r) = RACES.iter().find(|r| r.0 == name) {
        // macOS terminals may draw emoji-capable symbols (swords, skull) as double-width colour emoji: use chess pieces there
        let sym = match (mac, name) {
            (true, "human") => "\u{265E}",
            (true, "orc") => "\u{265C}",
            _ => r.1,
        };
        return (if font { r.3.unwrap_or(sym) } else { sym }.to_string(), r.2);
    }
    // deterministic hue from the name (FNV-1a), fixed saturation / value
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    let hue = (h % 360) as f32;
    let (sat, val) = (0.55f32, 0.85f32);
    let c = val * sat;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = val - c;
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (GENERIC_GLYPH.to_string(), (((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8))
}

fn valid_race(name: &str) -> bool {
    !name.is_empty() && name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

// A character name is any non-empty text without control characters (names have spaces, diacritics, quotes).
fn valid_char(name: &str) -> bool {
    !name.is_empty() && name.chars().count() <= 60 && !name.chars().any(|c| c.is_control()) && name.trim() == name
}

// One WC3 character from assets/wc3-icons/characters.tsv (embedded at build time)
#[derive(Clone, Debug)]
struct CharDef {
    race: String,
    name: String,
    glyph: Option<char>, // icon font codepoint; None when the character has no icon yet (race icon is used)
    ord: usize,          // index among the characters of its race
    of: usize,           // number of characters of its race
    role: String,        // "hero" / "unit" / "" (unknown), for assignment diversity
    sex: String,         // "m" / "f" / "" (unknown or a group)
    theme: String,       // coarse voice/style family ("demon", "arcane", "troll", ...) or ""
}

// default plain_dim: plain bg luminance = median inactive Claude tab (test plain_matches_dimmed_claude_tabs)
const PLAIN_DIM_DEFAULT: f32 = 0.18;
const CHARACTERS_TSV: &str = include_str!("../assets/wc3-icons/characters.tsv");

fn parse_characters(tsv: &str) -> Vec<CharDef> {
    let mut v: Vec<CharDef> = Vec::new();
    for l in tsv.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 3 || f[1].trim().is_empty() || !valid_race(f[0].trim()) {
            continue;
        }
        let cp = f[2].trim().trim_start_matches("U+").trim_start_matches("u+");
        let glyph = if f.get(4).map_or(false, |x| !x.trim().is_empty()) {
            u32::from_str_radix(cp, 16).ok().and_then(char::from_u32)
        } else {
            None
        };
        let meta = |i: usize| f.get(i).map(|x| x.trim()).filter(|x| *x != "-").unwrap_or("").to_string();
        v.push(CharDef { race: f[0].trim().to_string(), name: f[1].trim().to_string(), glyph, ord: 0, of: 0, role: meta(5), sex: meta(6), theme: meta(7) });
    }
    let races: Vec<String> = v.iter().map(|c| c.race.clone()).collect();
    for i in 0..v.len() {
        let same: Vec<usize> = (0..v.len()).filter(|&j| races[j] == races[i]).collect();
        v[i].ord = same.iter().position(|&j| j == i).unwrap_or(0);
        v[i].of = same.len();
    }
    v
}

// Diversity of a new character against the characters already in the session (see assign_chars).
// `ctx` = (character, tab distance to the new tab), `recent` = newest-last history of recent picks.
// Penalty per live character: same race 1.0, same role 0.5, same sex 0.5, same theme 1.5 (unknown never
// matches); x2 for the adjacent tab. Weight = 1 / (1 + penalty)^2, x0.4 when among the last 8 picks;
// then divided by (candidates of its race)^0.5 so small races are not starved.
const DIV_RACE: f64 = 1.0;
const DIV_ROLE: f64 = 0.5;
const DIV_SEX: f64 = 0.5;
const DIV_THEME: f64 = 1.5;
const DIV_ADJACENT: f64 = 2.0;
const DIV_RECENT: f64 = 0.4;
const DIV_RECENT_N: usize = 8;
const DIV_FLATTEN: f64 = 0.5; // race-size exponent: weight / (candidates of the race)^0.5

fn same(a: &str, b: &str) -> bool {
    !a.is_empty() && a == b
}

// Hard rule for the adjacent tab: never the same race with the same role or the same theme.
fn clashes_adjacent(a: &CharDef, b: &CharDef) -> bool {
    a.race == b.race && (same(&a.role, &b.role) || same(&a.theme, &b.theme))
}

fn diversity_weight(defs: &[CharDef], cand: &str, ctx: &[(String, usize)], recent: &[String]) -> f64 {
    let Some(c) = defs.iter().find(|d| d.name == cand) else { return 1.0 };
    let mut pen = 0.0;
    for (n, dist) in ctx {
        let Some(o) = defs.iter().find(|d| d.name == *n) else { continue };
        let mut p = 0.0;
        if c.race == o.race {
            p += DIV_RACE;
        }
        if same(&c.role, &o.role) {
            p += DIV_ROLE;
        }
        if same(&c.sex, &o.sex) {
            p += DIV_SEX;
        }
        if same(&c.theme, &o.theme) {
            p += DIV_THEME;
        }
        pen += if *dist == 1 { p * DIV_ADJACENT } else { p };
    }
    let mut w = 1.0 / ((1.0 + pen) * (1.0 + pen));
    if recent.iter().rev().take(DIV_RECENT_N).any(|r| r == cand) {
        w *= DIV_RECENT;
    }
    w
}

// Weighted random choice among `cands`; `r` in [0,1). Adjacent clashes are dropped when anything else is left.
fn diverse_pick(defs: &[CharDef], cands: &[String], ctx: &[(String, usize)], recent: &[String], r: f64) -> Option<String> {
    let adj: Vec<&CharDef> = ctx.iter().filter(|c| c.1 == 1).filter_map(|c| defs.iter().find(|d| d.name == c.0)).collect();
    let mut pool: Vec<&String> = cands
        .iter()
        .filter(|n| defs.iter().find(|d| d.name == **n).map_or(true, |d| !adj.iter().any(|a| clashes_adjacent(d, a))))
        .collect();
    if pool.is_empty() {
        pool = cands.iter().collect();
    }
    // flatten the race sizes: a race's total weight grows with the square root of its candidates
    // (21 human vs 2 demon characters would otherwise make demons a 1-in-45 event)
    let race_of = |n: &String| defs.iter().find(|d| d.name == *n).map(|d| d.race.as_str()).unwrap_or("");
    let ws: Vec<f64> = pool
        .iter()
        .map(|n| {
            let k = pool.iter().filter(|m| race_of(m) == race_of(n)).count() as f64;
            diversity_weight(defs, n, ctx, recent) / k.powf(DIV_FLATTEN)
        })
        .collect();
    let total: f64 = ws.iter().sum();
    let mut x = r.clamp(0.0, 0.999_999_9) * total;
    for (n, w) in pool.iter().zip(&ws) {
        if x < *w {
            return Some((*n).clone());
        }
        x -= w;
    }
    pool.last().map(|n| (*n).clone())
}

fn hash_name(name: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    h
}

fn rgb_to_hsl(c: (u8, u8, u8)) -> (f32, f32, f32) {
    let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    (f(r), f(g), f(b))
}

// Accent colour of a character: the race colour with a per-character shift, so neighbours differ but the
// race stays recognisable: hue +-12 deg, lightness +-8 % (+-12 % for the grey neutral race), saturation
// +-8 %. Characters of a race are spread with two golden-ratio sequences over their index in characters.tsv
// (unlisted characters: hashed). tint_colors (tab.rs) enforces text contrast whatever comes out.
fn character_accent(race_rgb: (u8, u8, u8), name: &str, ord: Option<(usize, usize)>) -> (u8, u8, u8) {
    let frac = |x: f32| x - x.floor();
    let (a, b, c) = match ord {
        Some((i, _)) => (frac(i as f32 * 0.618_034), frac(i as f32 * 0.754_878 + 0.3), frac(i as f32 * 0.569_840 + 0.6)),
        None => {
            let h = hash_name(name);
            ((h & 0xff) as f32 / 255.0, ((h >> 8) & 0xff) as f32 / 255.0, ((h >> 16) & 0xff) as f32 / 255.0)
        }
    };
    let (h, s, l) = rgb_to_hsl(race_rgb);
    let grey = s < 0.15;
    let dl = (b - 0.5) * if grey { 0.24 } else { 0.16 };
    let ds = if grey { 0.0 } else { (c - 0.5) * 0.16 };
    hsl_to_rgb(h + (a - 0.5) * 24.0, (s + ds).clamp(0.0, 1.0), (l + dl).clamp(0.12, 0.85))
}

// Fallback badge (no colour icon font): the character's initials, exactly 2 cells of width-1 letters.
// Candidates in order of preference: first letter of word 1 + first letter of word 2 (single word: its
// 2nd letter), then word 1's first letter + each further letter (word 2, rest of word 1, other words).
// Words = whitespace separated, reduced to letters (so "-" and "(" vanish, "Kel'Thuzad" is one word).
fn badge_candidates(name: &str) -> Vec<String> {
    use unicode_width::UnicodeWidthChar;
    let one = |c: char| UnicodeWidthChar::width(c) == Some(1);
    let up = |c: char| {
        let mut u = c.to_uppercase();
        match (u.next(), u.next()) {
            (Some(x), None) if one(x) => x,
            _ => c, // multi-char uppercase (e.g. German sharp s): keep the letter
        }
    };
    let words: Vec<Vec<char>> = name
        .split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphabetic() && one(*c)).collect::<Vec<char>>())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        // no letters at all: the first two visible width-1 characters, padded to 2 cells
        let s: String = name.chars().filter(|c| !c.is_whitespace() && one(*c)).take(2).collect();
        return vec![format!("{:<2}", if s.is_empty() { "?".to_string() } else { s })];
    }
    let first = up(words[0][0]);
    let mut rest: Vec<char> = Vec::new();
    if words.len() > 1 {
        rest.extend(&words[1]);
    }
    rest.extend(&words[0][1..]);
    for w in words.iter().skip(2) {
        rest.extend(w);
    }
    let mut out: Vec<String> = Vec::new();
    for c in rest {
        let s = format!("{}{}", first, up(c));
        if !out.contains(&s) {
            out.push(s);
        }
    }
    if out.is_empty() {
        out.push(format!("{} ", first)); // one-letter name
    }
    out
}

// Badges for a list of names (characters.tsv order first, so they never depend on the pool or the
// session). A name keeps its first candidate unless an earlier name has the same one; it then takes its
// first candidate that is nobody's first choice and not taken yet (all 92 characters end up unique).
fn badge_table(names: &[String]) -> HashMap<String, String> {
    let mut uniq: Vec<&String> = Vec::new();
    for n in names {
        if !uniq.contains(&n) {
            uniq.push(n);
        }
    }
    let cands: Vec<Vec<String>> = uniq.iter().map(|n| badge_candidates(n)).collect();
    let mut holder: HashMap<&str, usize> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        holder.entry(c[0].as_str()).or_insert(i);
    }
    let mut used: HashSet<String> = cands.iter().map(|c| c[0].clone()).collect();
    let mut out = HashMap::new();
    for (i, n) in uniq.iter().enumerate() {
        let c = &cands[i];
        let pick = if holder[c[0].as_str()] == i {
            c[0].clone()
        } else {
            c.iter().skip(1).find(|x| !used.contains(*x)).cloned().unwrap_or_else(|| c[0].clone())
        };
        used.insert(pick.clone());
        out.insert((*n).clone(), pick);
    }
    out
}

// Claude Code's own spinner frames (ping-pong: forward then back, period 10); all one cell wide
const SPIN: [&str; 10] = ["\u{B7}", "\u{2722}", "\u{2733}", "\u{2736}", "\u{273B}", "\u{273D}", "\u{273B}", "\u{2736}", "\u{2733}", "\u{2722}"];

const STATE_ROOT: &str = "/tmp/zellij-claude";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Idle,    // session running, waiting for the user (start)
    Working, // processing a prompt
    Done,    // finished, waiting for the user
    Attn,    // needs an answer (permission prompt / question / elicitation): overlay file <pane>.attn
}

#[derive(Clone, Debug)]
struct PaneState {
    st: St,
    ns: u128,
    subs: usize, // running subagents (one marker file each in <pane>.sub/)
    deferred: bool, // the Stop hook found running subagents: not a real finish yet
    quiet: bool,    // the Stop came while a question was pending: no finish ding
    away: bool,     // the Stop hook found the terminal window not OS-focused (os-focus.sh)
    sess: Option<String>, // directory with the session's agent-<id>.jsonl transcripts (written by the hook)
    attn: Option<(u128, bool)>, // <pane>.attn: (ns, away) while Claude waits for an answer
    recent_activity: bool, // a hook event (any kind) newer than the state file, within working_hold_s
}

#[derive(Default)]
struct State {
    session: String,
    mode_info: ModeInfo,
    tab_line: Vec<LinePart>,
    breadcrumb_range: Option<(usize, usize)>,
    text_copy_destination: Option<CopyDestination>,
    display_system_clipboard_failure: bool,
    tabs: Vec<TabInfo>,
    manifest: Option<PaneManifest>,
    perms: bool,
    // set_selectable(false) done (only once the permission question is resolved, see load())
    unselectable: bool,
    perms_denied: bool,
    files: HashMap<u32, PaneState>,
    // last "done" timestamp we reacted to (ding/auto switch) and last one shown as seen
    reacted: HashMap<u32, u128>,
    clock: bool,
    tab_chars: HashMap<usize, String>, // tab id -> character (files in characters/)
    races_opt: Vec<String>,  // `races` option: only characters of these races
    chars_opt: Vec<String>,  // `characters` option: fixed character pool
    defs: Vec<CharDef>,      // characters.tsv (embedded), filled in load()
    scanned: Vec<(String, String)>, // (race, character) pairs that have clips in the library
    scan_done: bool,
    scan_asked: u64,
    scan_first: u64, // when this instance first wanted a scan (followers wait a little for the leader's file)
    pkg_ok: bool,    // the current package dir was confirmed by a scan (play-phrase.sh exists there)
    race_icons: bool,
    race_tab_colors: bool,
    race_dim: f32,
    plain_dim: f32,
    show_other_clients: bool,
    blink: bool,
    spinner: bool,
    task_running: HashMap<u32, Vec<String>>,
    // when each task_running entry was last refreshed (secs); only the polling instance keeps it fresh
    task_at: HashMap<u32, u64>,
    agents_asked: u64,
    agents_script: String,
    working_hold_s: u64,
    last_tick_ms: u64,
    last_next_ms: u64,
    names_asked: u64,
    spin_ms: u64,
    ding_with_subagents: bool,
    subagent_grace_s: u64,
    subagent_stale_s: u64,
    zero_seen: HashMap<u32, (u128, u64)>, // pane -> (done ns, first time seen without subagents)
    last_key: String,
    blink_ms: u64,
    blink_timeout_s: u64,
    race_font_opt: Option<bool>, // None = auto: the hook's <state root>/font flag (WC3 Icons installed)
    font_flag: bool,             // <state root>/font says 1
    badges: HashMap<String, String>, // character -> 2-cell initials badge (used without the icon font)
    pkg_opt: Option<String>,     // `package_dir` option (non-empty)
    phrase_opt: Option<String>,  // `phrase_script` option as given (Some("") = clips off)
    pkg_dir: String,             // effective package dir
    pkg_missing: String,         // package dir whose play-phrase.sh was not found (logged once)
    os: String,
    os_asked: u64,
    subagent_count: bool,
    active_underline: bool,
    tab_numbers: bool,
    race_all_tabs: bool,
    diversity: bool, // option `diversity`: weigh new characters against the session's (default on)
    rng: u64,
    sounds_dir: String,
    phrase_script: String,
    player: String,
    lead_in_ms: u64,   // silence played before a clip (wakes a sleeping sink), 0 = off
    dup_window_ms: u64, // same-character dings closer than this are one
    max_clip_s: String, // longest clip to play, seconds (fractional ok), empty = no limit
    tz_offset: Option<i64>,
    tz_asked: u64,
    my_id: u32,
    snap_at: u64,
    tabs_ev_ms: u64,  // last TabUpdate (see EVENT_HOLD_MS)
    panes_ev_ms: u64, // last PaneUpdate
    plugin_ids: Vec<u32>,
    primed: bool,
    cwd_names: HashMap<usize, String>,        // tab position -> basename of its main pane's cwd
    cwd_pending: HashMap<usize, (String, u64)>,
    user_names: HashMap<usize, String>, // tab id -> name the user gave (files in names/)
    last_names: HashMap<usize, String>,
    last_render: u64,
    rename_seen: u64,
    auto_name: bool,
    // config
    ding_command: String,
    ding_when_focused: bool,
    attention: bool,         // ding + marker when Claude asks something (default true)
    attention_repeat_s: u64, // 0 = ding once per request
    attn_reacted: HashMap<u32, (u128, u64)>, // pane -> (attn ns, secs of the last ding)
    ding_when_os_unfocused: bool,
    os_focus_ms: u64,    // last time os-focus.sh said the terminal is focused (or could not tell)
    os_focus_asked: u64,
    auto_switch: bool,
    show_icon_fallback: bool,
}

register_plugin!(State);

fn truthy(v: Option<&String>, default: bool) -> bool {
    match v.map(|s| s.as_str()) {
        Some("true") | Some("1") | Some("yes") | Some("on") => true,
        Some("false") | Some("0") | Some("no") | Some("off") => false,
        _ => default,
    }
}

impl State {
    fn read_files(&mut self) {
        self.files.clear();
        self.read_chars();
        self.read_user_names();
        if self.session.is_empty() {
            return;
        }
        let dir = format!("{}/{}", STATE_ROOT, self.session);
        let Ok(rd) = std::fs::read_dir(&dir) else { return };
        for e in rd.flatten() {
            let Ok(pane) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
            let Ok(txt) = std::fs::read_to_string(e.path()) else { continue };
            let mut it = txt.split_whitespace();
            let st = match it.next() {
                Some("working") => St::Working,
                Some("done") => St::Done,
                Some("idle") => St::Idle,
                _ => continue,
            };
            let ns = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            // optional flags after the timestamp: `sub` (subagents still running), `away` (OS focus elsewhere)
            let flags: Vec<&str> = it.collect();
            let deferred = flags.contains(&"sub");
            let away = flags.contains(&"away");
            let quiet = flags.contains(&"quiet");
            // running agents = hook markers (SubagentStart/Stop) UNION agents whose transcripts say they are
            // still running (claude-agents.sh, polled): neither source alone is reliable
            let mut ids = subagent_markers(&format!("{}/{}.sub", dir, pane), self.subagent_stale_s);
            // ignore a transcript scan older than AGENTS_FRESH_S: only the focused tab's instance (and the
            // leader) polls, so a stale non-empty list in another instance would keep a finished tab "Working".
            // The polling instances share their result through <pane>.agents, so every instance (also one whose
            // tab was just focused and has not polled yet) shows the same count.
            let fresh = self.task_at.get(&pane).map_or(false, |a| now_secs().saturating_sub(*a) < AGENTS_FRESH_S);
            let mut scanned: Vec<String> = self.task_running.get(&pane).filter(|_| fresh).cloned().unwrap_or_default();
            let shared = format!("{}/{}.agents", dir, pane);
            let shared_age = age_of(std::fs::metadata(&shared).ok());
            if shared_age.map_or(false, |a| a.as_secs() < AGENTS_FRESH_S) {
                scanned.extend(std::fs::read_to_string(&shared).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
            }
            for id in scanned {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            let subs = ids.len();
            let sess = std::fs::read_to_string(format!("{}/{}.sess", dir, pane)).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
            // safety net: any hook event newer than the last state change, within working_hold_s, means busy
            let st_age = age_of(e.metadata().ok());
            let ev_age = age_of(std::fs::metadata(format!("{}/{}.ev", dir, pane)).ok());
            let recent_activity = match (st_age, ev_age) {
                (Some(sa), Some(ea)) => {
                    ea.as_secs() < self.working_hold_s && sa > ea + std::time::Duration::from_secs(1)
                }
                _ => false,
            };
            // attention overlay: `<ns> [away]`; ignored when disabled or older than 12 h (a crashed session)
            let attn = std::fs::read_to_string(format!("{}/{}.attn", dir, pane)).ok().and_then(|t| {
                let mut w = t.split_whitespace();
                let ns: u128 = w.next()?.parse().ok()?;
                let away = w.any(|x| x == "away");
                let old = now_secs().saturating_sub((ns / 1_000_000_000) as u64) > 12 * 3600;
                if self.attention && !old { Some((ns, away)) } else { None }
            });
            self.files.insert(pane, PaneState { st, ns, subs, deferred, away, quiet, recent_activity, sess, attn });
        }
    }

    fn chars_dir(&self) -> String {
        format!("{}/{}/characters", STATE_ROOT, self.session)
    }

    fn read_chars(&mut self) {
        self.tab_chars.clear();
        if self.session.is_empty() {
            return;
        }
        let Ok(rd) = std::fs::read_dir(self.chars_dir()) else { return };
        for e in rd.flatten() {
            let Ok(id) = e.file_name().to_string_lossy().parse::<usize>() else { continue };
            if let Ok(r) = std::fs::read_to_string(e.path()) {
                let r = r.trim().to_string();
                if valid_char(&r) {
                    self.tab_chars.insert(id, r);
                }
            }
        }
    }

    fn set_char(&mut self, tab_id: usize, character: &str, overwrite: bool) -> bool {
        let dir = self.chars_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = format!("{}/{}", dir, tab_id);
        let ok = if overwrite {
            std::fs::write(&path, character).is_ok()
        } else {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .and_then(|mut f| f.write_all(character.as_bytes()))
                .is_ok()
        };
        if ok {
            self.tab_chars.insert(tab_id, character.to_string());
        }
        ok
    }

    // Cross-session history of recent picks (newest last, 16 lines), used to damp repeats.
    fn read_recent(&self) -> Vec<String> {
        std::fs::read_to_string(format!("{}/recent-characters", STATE_ROOT))
            .map(|t| t.lines().filter(|l| valid_char(l)).map(String::from).collect())
            .unwrap_or_default()
    }

    fn push_recent(&self, name: &str) {
        let mut v = self.read_recent();
        v.push(name.to_string());
        let skip = v.len().saturating_sub(16);
        let path = format!("{}/recent-characters", STATE_ROOT);
        let tmp = format!("{}.{}", path, std::process::id());
        let _ = std::fs::write(&tmp, v[skip..].join("\n") + "\n").and_then(|_| std::fs::rename(&tmp, &path));
    }

    // xorshift64*, seeded from the clock; [0,1)
    fn next_rand(&mut self) -> f64 {
        if self.rng == 0 {
            let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
            self.rng = n | 1;
        }
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        (x.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn def_of(&self, name: &str) -> Option<&CharDef> {
        self.defs.iter().find(|d| d.name == name)
    }

    // Library characters (race, name): the scan result resolved against characters.tsv. The scan reports
    // the filename part before the first " - ", which is a prefix of the full name for the few
    // characters whose name contains " - " ("Trollí čarodějnice - lékařka"); unknown names stay as they are.
    fn resolve_scan(&self, raw: &[(String, String)]) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for (race, pre) in raw {
            let full = format!("{} - ", pre);
            let mut hit = false;
            for d in self.defs.iter().filter(|d| d.race == *race && (d.name == *pre || d.name.starts_with(&full))) {
                hit = true;
                out.push((d.race.clone(), d.name.clone()));
            }
            if !hit {
                out.push((race.clone(), pre.clone()));
            }
        }
        out.sort();
        out.dedup();
        out
    }

    // Characters that may be assigned: the `characters` option, else every character that has clips
    // (the scanned library; before the first scan or with no library: all of characters.tsv, the sound
    // then falls back to the ding), narrowed by the `races` option.
    fn effective_chars(&self) -> Vec<String> {
        let mut pool: Vec<String> = if !self.chars_opt.is_empty() {
            self.chars_opt.clone()
        } else if !self.scanned.is_empty() {
            self.scanned.iter().map(|c| c.1.clone()).collect()
        } else {
            self.defs.iter().map(|d| d.name.clone()).collect()
        };
        if !self.races_opt.is_empty() {
            pool.retain(|n| self.race_of(n).map_or(false, |r| self.races_opt.contains(&r)));
        }
        pool.sort();
        pool.dedup();
        pool
    }

    fn race_of(&self, name: &str) -> Option<String> {
        self.def_of(name)
            .map(|d| d.race.clone())
            .or_else(|| self.scanned.iter().find(|c| c.1 == name).map(|c| c.0.clone()))
    }

    // glyph, accent colour, and whether the glyph needs a padding cell for a character
    fn char_style(&self, name: &str, font: bool, mac: bool) -> (String, (u8, u8, u8)) {
        let def = self.def_of(name);
        let race = def.map(|d| d.race.clone()).or_else(|| self.race_of(name)).unwrap_or_default();
        let (race_glyph, race_rgb) = race_style(&race, font, mac);
        let glyph = match def.and_then(|d| d.glyph) {
            Some(g) if font => g.to_string(),
            _ => race_glyph,
        };
        (glyph, character_accent(race_rgb, name, def.map(|d| (d.ord, d.of))))
    }

    // wasm cannot call uname: ask once (retry every 5 s until answered)
    fn ask_os(&mut self) {
        if !self.os.is_empty() || !self.perms {
            return;
        }
        let now = now_secs();
        if now.saturating_sub(self.os_asked) < 5 {
            return;
        }
        self.os_asked = now;
        let ctx = BTreeMap::from([("k".to_string(), "os".to_string())]);
        run_command(&["uname", "-s"], ctx);
    }

    fn race_font_on(&self) -> bool {
        self.race_font_opt.unwrap_or(self.font_flag)
    }

    // Package dir and font flag from the state root (written by claude-hook.sh); cheap, every tick.
    // A new package dir re-derives phrase_script and triggers a library scan at once (no restart needed).
    fn refresh_paths(&mut self) -> bool {
        let file = std::fs::read_to_string(format!("{}/pkgdir", STATE_ROOT))
            .ok()
            .map(|v| v.trim_end_matches(&['\n', '\r'][..]).trim_end_matches('/').to_string())
            .filter(|v| v.starts_with('/') && !v.chars().any(|c| c.is_control()));
        let pkg = choose_pkg(self.pkg_opt.as_deref(), file.as_deref(), &self.pkg_dir, self.pkg_ok);
        let mut changed = false;
        if pkg != self.pkg_dir {
            eprintln!("claude-tabs: package dir {}", pkg);
            self.pkg_dir = pkg;
            self.pkg_ok = false;
            if self.phrase_opt.is_none() {
                self.phrase_script = format!("{}/play-phrase.sh", self.pkg_dir);
            }
            self.scan_asked = 0; // scan the new location now
            self.scan_first = 0; // followers wait for the leader's new result again
            changed = true;
        }
        let font = std::fs::read_to_string(format!("{}/font", STATE_ROOT)).map_or(false, |v| v.trim() == "1");
        if font != self.font_flag {
            self.font_flag = font;
            changed = true;
        }
        changed
    }

    // badges for every known name: characters.tsv first (stable), then other names (option, library)
    fn rebuild_badges(&mut self) {
        let mut names: Vec<String> = self.defs.iter().map(|d| d.name.clone()).collect();
        names.extend(self.chars_opt.iter().cloned());
        let mut extra: Vec<String> = self.scanned.iter().map(|c| c.1.clone()).collect();
        extra.sort();
        names.extend(extra);
        self.badges = badge_table(&names);
    }

    fn badge_of(&self, name: &str) -> String {
        self.badges.get(name).cloned().unwrap_or_else(|| badge_candidates(name)[0].clone())
    }

    fn library_file(&self) -> String {
        format!("{}/library-{}", STATE_ROOT, self.session)
    }

    // The leader scans the sound library through the shell (wasm cannot read $HOME) every 30 s and
    // publishes the result in <state root>/library-<session>; the other instances read that file and only
    // scan themselves when it is missing or stale (no leader scanning), after a short grace period.
    fn ask_scan(&mut self) {
        if !self.perms || self.phrase_script.is_empty() {
            return;
        }
        let now = now_secs();
        if self.scan_first == 0 {
            self.scan_first = now;
        }
        if !self.is_leader() && !self.session.is_empty() {
            if let Ok(txt) = std::fs::read_to_string(self.library_file()) {
                if let Some((ts, body)) = txt.split_once('\n') {
                    if ts.trim().parse::<u64>().map_or(false, |t| now.saturating_sub(t) < LIBRARY_STALE_S) {
                        self.pkg_ok = true; // the leader's scan found play-phrase.sh: keep the dir, no flipping
                        if self.apply_scan(body) {
                            self.rebuild_badges();
                        }
                        return;
                    }
                }
            }
            if now.saturating_sub(self.scan_first) < LIBRARY_GRACE_S {
                return;
            }
        }
        if self.scan_asked != 0 && now.saturating_sub(self.scan_asked) < 30 {
            return;
        }
        self.scan_asked = now;
        let ctx = BTreeMap::from([("k".to_string(), "scan".to_string()), ("p".to_string(), self.phrase_script.clone())]);
        // exit 99 = no play-phrase.sh there (yet): no hook has written pkgdir, or it moved
        run_command(
            &[
                "sh",
                "-c",
                concat!(
                    "p=$1; d=$2; pl=$3; ",
                    "case $p in '~/'*) p=$HOME/${p#'~/'};; esac; case $d in '~'|'~/'*) d=$HOME${d#'~'};; esac; ",
                    "case $pl in '~/'*) pl=$HOME/${pl#'~/'};; esac; ",
                    "[ -f \"$p\" ] || exit 99; exec sh \"$p\" --list \"$d\" \"$pl\""
                ),
                "_",
                &self.phrase_script,
                &self.sounds_dir,
                &self.player,
            ],
            ctx,
        );
    }

    // parse "race<TAB>character" lines (play-phrase.sh --list); true when the library changed
    fn apply_scan(&mut self, out: &str) -> bool {
        let raw: Vec<(String, String)> = out
            .lines()
            .filter_map(|l| {
                let (r, c) = l.split_once('\t')?;
                let (r, c) = (r.trim(), c.trim());
                (valid_race(r) && !r.starts_with('_') && valid_char(c)).then(|| (r.to_string(), c.to_string()))
            })
            .collect();
        let v = self.resolve_scan(&raw);
        let changed = v != self.scanned;
        self.scanned = v;
        self.scan_done = true;
        changed
    }

    // Give every tab that runs Claude a race, preferring races no other tab has. The first writer wins
    // (create_new), so instances racing each other agree.
    // Only the leader (lowest plugin id) assigns, from a fresh snapshot, so two tabs can never be
    // handed the same race by concurrent instances. Least-used race first, random among ties; an
    // assignment is permanent (file per tab id) until the tab closes.
    fn assign_chars(&mut self) {
        // wait for the first library scan so the pool is the characters that have clips
        if !self.scan_done && self.perms && !self.phrase_script.is_empty() && self.chars_opt.is_empty() {
            return;
        }
        let allowed = self.effective_chars();
        if allowed.is_empty() || self.session.is_empty() {
            return;
        }
        let mut tabs: Vec<(usize, usize)> = self.tabs.iter().map(|t| (t.tab_id, t.position)).collect();
        tabs.sort_by_key(|t| t.1);
        for (id, pos) in tabs {
            if self.tab_chars.contains_key(&id) || (!self.race_all_tabs && self.tab_claude(pos).is_none()) {
                continue;
            }
            let live: HashSet<usize> = self.tabs.iter().map(|t| t.tab_id).collect();
            let count = |r: &String| self.tab_chars.iter().filter(|(k, v)| live.contains(k) && *v == r).count();
            let min = allowed.iter().map(|r| count(r)).min().unwrap_or(0);
            let mut cands: Vec<&String> = allowed.iter().filter(|r| count(r) == min).collect();
            // among equally used characters prefer those that have sound clips
            if cands.iter().any(|r| self.scanned.iter().any(|c| &c.1 == *r)) {
                cands.retain(|r| self.scanned.iter().any(|c| &c.1 == *r));
            }
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos() as usize);
            let pick = if self.diversity {
                let ctx: Vec<(String, usize)> = self
                    .tabs
                    .iter()
                    .filter_map(|t| Some((self.tab_chars.get(&t.tab_id)?.clone(), t.position.abs_diff(pos))))
                    .collect();
                let recent = self.read_recent();
                let names: Vec<String> = cands.iter().map(|c| (*c).clone()).collect();
                let r = self.next_rand();
                diverse_pick(&self.defs, &names, &ctx, &recent, r).unwrap_or_else(|| cands[nanos % cands.len()].clone())
            } else {
                cands[nanos % cands.len()].clone()
            };
            eprintln!("claude-tabs: tab {} -> {}", id, pick);
            if !self.set_char(id, &pick, false) {
                self.read_chars(); // already assigned elsewhere
            } else {
                self.push_recent(&pick);
            }
        }
    }

    // Leader duty: refresh from the server, hand out races, drop state of closed tabs / panes.
    fn leader_tick(&mut self) {
        let interval = if self.is_leader() { 2 } else { 30 };
        if self.snapshot_age() < interval {
            return;
        }
        let ok = self.fetch_snapshot();
        if ok && self.is_leader() {
            self.cleanup_stale();
            self.assign_chars();
        }
    }

    // State with the subagent rule applied: a Stop that left subagents running is not a finish. It stays
    // `Working` while subagents run and for `subagent_grace_s` after the last one stopped (the main agent
    // is normally woken and ends with a Stop of its own); only then it counts as finished.
    fn eff_state(&self, pane: u32, f: &PaneState) -> St {
        if f.attn.is_some() {
            return St::Attn; // beats the working hold, subagent counts and the grace timer
        }
        if f.subs > 0 || (f.recent_activity && f.st != St::Working && !(f.st == St::Done && f.deferred)) {
            return St::Working;
        }
        if f.st == St::Done && f.deferred && !self.ding_with_subagents {
            let since = self.zero_seen.get(&pane).filter(|(ns, _)| *ns == f.ns).map_or(now_secs(), |v| v.1);
            if now_secs().saturating_sub(since) < self.subagent_grace_s {
                return St::Working;
            }
        }
        f.st
    }

    // tab position of a terminal pane
    fn pane_tab(&self, pane: u32) -> Option<usize> {
        let m = self.manifest.as_ref()?;
        m.panes
            .iter()
            .find(|(_, v)| v.iter().any(|p| !p.is_plugin && p.id == pane))
            .map(|(k, _)| *k)
    }

    fn active_tab(&self) -> Option<usize> {
        self.tabs.iter().find(|t| t.active).map(|t| t.position)
    }

    // Per tab: best state among its panes (Done > Working > Idle) plus the newest done ns.
    fn tab_claude(&self, pos: usize) -> Option<(St, u128, usize)> {
        let m = self.manifest.as_ref()?;
        let panes = m.panes.get(&pos)?;
        let mut best: Option<(St, u128)> = None;
        let mut count = 0usize;
        let rank = |s: St| match s {
            St::Attn => 3,
            St::Working => 2,
            St::Done => 1,
            St::Idle => 0,
        };
        for p in panes.iter().filter(|p| !p.is_plugin) {
            let cand = if let Some(f) = self.files.get(&p.id) {
                count += f.subs;
                Some((self.eff_state(p.id, f), f.attn.map_or(f.ns, |a| a.0)))
            } else if self.show_icon_fallback && looks_like_claude(p) {
                Some((St::Idle, 0))
            } else {
                None
            };
            if let Some(c) = cand {
                best = match best {
                    Some(b) if rank(b.0) > rank(c.0) || (rank(b.0) == rank(c.0) && b.1 >= c.1) => Some(b),
                    _ => Some(c),
                };
            }
        }
        best.map(|(s, n)| (s, n, count))
    }

    fn refresh(&mut self) {
        // a reloaded instance in an inactive tab never gets a ModeUpdate, so learn the session name from the server
        if self.session.is_empty() && (self.snapshot_age() < 2 || !self.fetch_snapshot()) {
            return;
        }
        self.read_files();
        let now = now_secs();
        for (pane, f) in self.files.iter() {
            if f.st == St::Done && f.deferred {
                if f.subs > 0 {
                    self.zero_seen.remove(pane);
                } else if self.zero_seen.get(pane).map_or(true, |(ns, _)| *ns != f.ns) {
                    self.zero_seen.insert(*pane, (f.ns, now));
                }
            }
        }
        self.leader_tick();
        if !self.primed {
            // finished sessions that exist when we first look must not ding
            self.prime();
            return;
        }
        // Only a Done file needs action (ding / auto switch / acknowledge). Instances in inactive
        // tabs have stale tab/pane data, so the acting instance asks the server for a fresh snapshot.
        let new_done = self
            .files
            .iter()
            .any(|(p, f)| self.eff_state(*p, f) == St::Done && self.reacted.get(p).map_or(true, |n| *n != f.ns));
        let any_done = self.files.iter().any(|(p, f)| self.eff_state(*p, f) == St::Done);
        let any_attn = self.files.values().any(|f| f.attn.is_some());
        let new_attn = self.files.iter().any(|(p, f)| f.attn.map_or(false, |a| self.attn_due(*p, a.0)));
        let new_done = new_done || new_attn;
        let any_done = any_done || any_attn;
        if !new_done && !any_done {
            return;
        }
        // known non-leaders only react to a NEW finish; the leader keeps acknowledging seen tabs
        if !new_done && !self.plugin_ids.is_empty() && !self.is_leader() {
            return;
        }
        if (new_done || self.snapshot_age() >= 1) && !self.fetch_snapshot() {
            return;
        }
        if !self.is_leader() {
            self.prime(); // someone else acts; do not repeat it if we become leader later
            return;
        }
        self.cleanup_stale();
        let active = self.active_tab();
        let mut react: Vec<(u32, usize)> = Vec::new();
        let mut ack: Vec<(u32, u128)> = Vec::new();
        let mut need_os = false;
        let mut react_attn: Vec<(u32, usize)> = Vec::new();
        let mut ack_attn: Vec<(u32, u128)> = Vec::new();
        for (pane, f) in self.files.iter() {
            if let Some((ans, aaway)) = f.attn {
                let tab = self.pane_tab(*pane);
                if self.attn_due(*pane, ans) {
                    if let Some(t) = tab {
                        react_attn.push((*pane, t));
                    }
                }
                if tab.is_some() && tab == active {
                    // asked while the user was in another window: stays until the terminal has the OS focus
                    if self.ding_when_os_unfocused && aaway && self.os_focus_ms < (ans / 1_000_000) as u64 {
                        need_os = true;
                    } else {
                        ack_attn.push((*pane, ans));
                    }
                }
                continue;
            }
            if self.eff_state(*pane, f) != St::Done {
                continue;
            }
            let tab = self.pane_tab(*pane);
            if self.reacted.get(pane).map_or(true, |n| *n != f.ns) {
                if let Some(t) = tab {
                    react.push((*pane, t));
                }
            }
            if tab.is_some() && tab == active {
                // finished while the user was in another window: it stays unseen (blinks) until the
                // terminal window has the OS focus again (polled through os-focus.sh)
                if self.ding_when_os_unfocused && f.away && self.os_focus_ms < (f.ns / 1_000_000) as u64 {
                    need_os = true;
                } else {
                    ack.push((*pane, f.ns));
                }
            }
        }
        if need_os {
            self.ask_os_focus();
        }
        for (pane, tab) in react {
            let ns = self.files[&pane].ns;
            let away = self.files[&pane].away;
            let quiet = self.files[&pane].quiet;
            self.reacted.insert(pane, ns);
            if quiet {
                continue;
            }
            let focused = Some(tab) == active;
            if !focused || self.ding_when_focused || (self.ding_when_os_unfocused && away) {
                let race = self.tabs.iter().find(|t| t.position == tab).and_then(|t| self.tab_chars.get(&t.tab_id)).cloned();
                self.ding(race.as_deref(), "finish", Some(pane));
            }
            if self.auto_switch && !focused {
                switch_tab_to(tab as u32 + 1);
            }
        }
        let now = now_secs();
        for (pane, tab) in react_attn {
            let (ns, away) = self.files[&pane].attn.unwrap_or((0, false));
            self.attn_reacted.insert(pane, (ns, now));
            if Some(tab) != active || self.ding_when_focused || (self.ding_when_os_unfocused && away) {
                let ch = self.tabs.iter().find(|t| t.position == tab).and_then(|t| self.tab_chars.get(&t.tab_id)).cloned();
                self.ding(ch.as_deref(), "attention", Some(pane));
            }
        }
        // an attention request on the tab in view counts as seen (the question is on screen): drop the overlay
        for (pane, ns) in ack_attn {
            let path = format!("{}/{}/{}.attn", STATE_ROOT, self.session, pane);
            if std::fs::read_to_string(&path).unwrap_or_default().contains(&ns.to_string()) {
                let _ = std::fs::remove_file(&path);
            }
        }
        // a finished tab that is now in view counts as seen: Done -> Idle (shared through the file)
        for (pane, ns) in ack {
            let path = format!("{}/{}/{}", STATE_ROOT, self.session, pane);
            let cur = std::fs::read_to_string(&path).unwrap_or_default();
            if cur.starts_with("done") && cur.contains(&ns.to_string()) {
                let _ = std::fs::write(format!("{}.tmp", path), format!("idle {}\n", ns));
                let _ = std::fs::rename(format!("{}.tmp", path), &path);
            }
        }
    }

    // a new attention request (or, with attention_repeat_s, a repeat of one that is still unanswered)
    fn attn_due(&self, pane: u32, ns: u128) -> bool {
        match self.attn_reacted.get(&pane) {
            Some((n, _)) if *n != ns => true,
            Some((_, at)) => self.attention_repeat_s > 0 && now_secs().saturating_sub(*at) >= self.attention_repeat_s,
            None => true,
        }
    }

    fn snapshot_age(&self) -> u64 {
        now_secs().saturating_sub(self.snap_at)
    }

    // Fresh tabs / panes of the current session straight from the server.
    fn fetch_snapshot(&mut self) -> bool {
        if !self.perms {
            return false;
        }
        let Ok(snap) = get_session_list() else { return false };
        let Some(si) = snap.live_sessions.into_iter().find(|s| s.is_current_session) else { return false };
        self.snap_at = now_secs();
        self.session = si.name.clone();
        // live claude-tabs panes only (the server's plugin table can still list instances from before a reload)
        self.plugin_ids = si
            .panes
            .panes
            .values()
            .flatten()
            .filter(|p| p.is_plugin && (p.title.contains("claude-tabs") || p.plugin_url.as_deref().map_or(false, |u| u.contains("claude-tabs"))))
            .map(|p| p.id)
            .collect();
        // the snapshot lags behind Tab/PaneUpdate: keep what an event delivered lately
        let t = now_ms();
        if !event_holds(self.tabs_ev_ms, t) {
            self.tabs = si.tabs;
        }
        if !event_holds(self.panes_ev_ms, t) {
            self.manifest = Some(si.panes);
        }
        true
    }

    // exactly one instance acts: the one with the lowest plugin id
    fn is_leader(&self) -> bool {
        self.plugin_ids.iter().min().map_or(false, |m| *m == self.my_id)
    }

    fn prime(&mut self) {
        for (pane, f) in self.files.iter() {
            self.reacted.insert(*pane, f.ns);
            if let Some((ns, _)) = f.attn {
                self.attn_reacted.insert(*pane, (ns, now_secs()));
            }
        }
        self.primed = true;
    }

    // drop state of panes that no longer exist (closed without a SessionEnd)
    fn cleanup_stale(&mut self) {
        let Some(m) = self.manifest.as_ref() else { return };
        if m.panes.is_empty() {
            return;
        }
        let live: HashSet<u32> = m.panes.values().flatten().filter(|p| !p.is_plugin).map(|p| p.id).collect();
        let dead: Vec<u32> = self.files.keys().filter(|k| !live.contains(k)).cloned().collect();
        let live_tabs: HashSet<usize> = self.tabs.iter().map(|t| t.tab_id).collect();
        let stale_races: Vec<usize> = self.tab_chars.keys().filter(|k| !live_tabs.contains(k)).cloned().collect();
        for k in stale_races {
            let _ = std::fs::remove_file(format!("{}/{}", self.chars_dir(), k));
            self.tab_chars.remove(&k);
        }
        for k in dead {
            let dir = format!("{}/{}", STATE_ROOT, self.session);
            let _ = std::fs::remove_file(format!("{}/{}", dir, k));
            let _ = std::fs::remove_dir_all(format!("{}/{}.sub", dir, k));
            let _ = std::fs::remove_file(format!("{}/{}.ev", dir, k));
            let _ = std::fs::remove_file(format!("{}/{}.attn", dir, k));
            let _ = std::fs::remove_file(format!("{}/{}.attn-seen", dir, k));
            let _ = std::fs::remove_file(format!("{}/{}.sess", dir, k));
            let _ = std::fs::remove_file(format!("{}/{}.agents", dir, k));
            // the hook's duplicate-event markers <pane>.once.<key>
            if let Ok(rd) = std::fs::read_dir(&dir) {
                let pre = format!("{}.once.", k);
                for e in rd.flatten() {
                    if e.file_name().to_string_lossy().starts_with(&pre) {
                        let _ = std::fs::remove_dir(e.path());
                    }
                }
            }
            self.files.remove(&k);
        }
    }

    // basename of the cwd of each tab's main terminal pane (focused tiled one, else the first)
    // Main terminal pane of every tab: the focused tiled one, else the lowest id. Never plugins,
    // floating or suppressed panes.
    fn main_panes(&self) -> Vec<(usize, u32)> {
        let Some(m) = self.manifest.as_ref() else { return Vec::new() };
        let mut out = Vec::new();
        for (pos, panes) in m.panes.iter() {
            let mut cands: Vec<&PaneInfo> =
                panes.iter().filter(|p| !p.is_plugin && !p.is_suppressed && !p.is_floating).collect();
            cands.sort_by_key(|p| (!p.is_focused, p.id));
            if let Some(p) = cands.first() {
                out.push((*pos, p.id));
            }
        }
        out
    }

    // Ask for the cwd of each tab's main pane. This is the cwd of the process zellij started in the
    // pane (the shell, or claude itself), read from /proc through the shell: get_pane_cwd (and zellij's
    // own tab naming) follow the FOREGROUND process, so a child of claude running `cd /tmp` flips it.
    fn refresh_names(&mut self) {
        if !self.auto_name || !self.perms {
            return;
        }
        // get_pane_cwd is zellij's own (portable: Linux /proc, macOS libproc) lookup of the pane's ROOT
        // process; it does not follow children, unlike zellij's tab auto-naming
        for (pos, id) in self.main_panes() {
            if let Ok(path) = get_pane_cwd(PaneId::Terminal(id)) {
                self.observe_cwd(pos, &path.to_string_lossy());
            }
        }
    }

    // Debounce: a new folder name is shown only after it has been seen for 2 s in a row.
    fn observe_cwd(&mut self, pos: usize, path: &str) -> bool {
        let before = self.cwd_names.get(&pos).cloned();
        let name = std::path::Path::new(path.trim())
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        let now = now_secs();
        if self.cwd_names.get(&pos) == Some(&name) {
            self.cwd_pending.remove(&pos);
        } else if !self.cwd_names.contains_key(&pos) {
            self.cwd_names.insert(pos, name);
        } else {
            match self.cwd_pending.get(&pos) {
                Some((n, since)) if *n == name => {
                    if now.saturating_sub(*since) >= 2 {
                        self.cwd_names.insert(pos, name);
                        self.cwd_pending.remove(&pos);
                    }
                }
                _ => {
                    self.cwd_pending.insert(pos, (name, now));
                }
            }
        }
        self.cwd_names.get(&pos).cloned() != before
    }

    fn names_dir(&self) -> String {
        format!("{}/{}/names", STATE_ROOT, self.session)
    }

    fn read_user_names(&mut self) {
        self.user_names.clear();
        if self.session.is_empty() {
            return;
        }
        let Ok(rd) = std::fs::read_dir(self.names_dir()) else { return };
        for e in rd.flatten() {
            if let (Ok(id), Ok(n)) = (e.file_name().to_string_lossy().parse::<usize>(), std::fs::read_to_string(e.path())) {
                self.user_names.insert(id, n);
            }
        }
    }

    // Zellij renames tabs by itself (it follows the cwd of the FOREGROUND process, so a child running
    // `cd /tmp` flips the name), and there is no API to tell that apart from a user rename. A name
    // counts as the user's only when it changed while the interactive rename mode (RenameTab) was
    // active or within 3 s after it; it is remembered in a file per tab id. Renames made by other
    // means (zellij action rename-tab) are not detected and are shown as the folder name.
    fn track_user_names(&mut self, new: &[TabInfo]) {
        if self.session.is_empty() {
            return;
        }
        let intent = self.mode_info.mode == InputMode::RenameTab || now_secs().saturating_sub(self.rename_seen) <= 3;
        for t in new {
            let Some(prev) = self.last_names.get(&t.tab_id) else { continue };
            if *prev == t.name || !intent || !t.active {
                continue;
            }
            let path = format!("{}/{}", self.names_dir(), t.tab_id);
            let default = t.name.is_empty() || (t.name.starts_with("Tab #") && t.name[5..].chars().all(|c| c.is_ascii_digit()));
            if default {
                let _ = std::fs::remove_file(&path);
            } else {
                let _ = std::fs::create_dir_all(self.names_dir());
                let _ = std::fs::write(&path, &t.name);
            }
        }
    }

    // Name shown for a tab: the (debounced) folder of its main pane unless the user named the tab.
    fn display_name(&self, t: &TabInfo) -> String {
        // a name the user typed wins even if zellij later overwrote it with a folder name
        if let Some(n) = self.user_names.get(&t.tab_id) {
            return n.clone();
        }
        if self.auto_name {
            if let Some(n) = self.cwd_names.get(&t.position) {
                return n.clone();
            }
        }
        t.name.clone()
    }

    // Play a random clip of the character (play-phrase.sh); fall back to ding_command when there is none.
    fn ding(&self, character: Option<&str>, trigger: &str, pane: Option<u32>) {
        if !self.perms {
            return;
        }
        let ctx = BTreeMap::from([("k".to_string(), "ding".to_string())]);
        // a clip of the character (play-phrase.sh); else ding_command; else the packaged ding.wav. A missing
        // play-phrase.sh (no hook has told us the package dir yet) is skipped silently.
        let ch = character.filter(|c| valid_char(c)).unwrap_or("");
        let who = match pane {
            Some(p) => format!("{}/{}", self.session, p),
            None => self.session.clone(),
        };
        // play-phrase.sh serialises, de-duplicates and logs (ding.log). The packaged ding / ding_command run
        // only when it reports "nothing to play / no player" (exit 3) or there is no character or script; a
        // transient player failure after a clip may have started does NOT add a second sound.
        run_command(
            &[
                "sh",
                "-c",
                concat!(
                    "p=$1; d=$2; c=$3; pl=$4; dc=$5; ",
                    "export CLAUDE_TABS_LEAD_IN_MS=$6 CLAUDE_TABS_DUP_MS=$7 CLAUDE_TABS_TRIGGER=$8 CLAUDE_TABS_WHO=$9; ",
                    "[ -z \"${10}\" ] || export ZELLIJ_CLAUDE_MAX_CLIP_S=${10}; ",
                    "case $p in '~/'*) p=$HOME/${p#'~/'};; esac; case $d in '~'|'~/'*) d=$HOME${d#'~'};; esac; ",
                    "case $pl in '~/'*) pl=$HOME/${pl#'~/'};; esac; ",
                    "if [ -n \"$c\" ] && [ -n \"$p\" ] && [ -f \"$p\" ]; then sh \"$p\" \"$d\" \"$c\" \"$pl\"; r=$?; [ $r -eq 3 ] || exit $r; fi; ",
                    "if [ -n \"$dc\" ]; then eval \"$dc\"; exit $?; fi; ",
                    "if [ -n \"$p\" ] && [ -f \"$p\" ]; then exec sh \"$p\" --ding \"$pl\"; fi; exit 0"
                ),
                "_",
                &self.phrase_script,
                &self.sounds_dir,
                ch,
                &self.player,
                &self.ding_command,
                &self.lead_in_ms.to_string(),
                &self.dup_window_ms.to_string(),
                trigger,
                &who,
                &self.max_clip_s,
            ],
            ctx,
        );
    }
}

// max_clip_s option -> value for play-phrase.sh: seconds as plain decimal text, empty = no limit
// (unset, 0, negative, non-numeric or non-finite).
fn parse_max_clip_s(v: &str) -> String {
    match v.trim().parse::<f64>() {
        Ok(x) if x.is_finite() && x > 0.0 => {
            let ms = (x.min(86400.0) * 1000.0).round() as u64;
            if ms == 0 {
                String::new()
            } else if ms % 1000 == 0 {
                (ms / 1000).to_string()
            } else {
                format!("{}.{:03}", ms / 1000, ms % 1000).trim_end_matches('0').to_string()
            }
        }
        _ => String::new(),
    }
}

// Running subagents of a pane: one marker file each. SubagentStop removes its marker; a marker
// whose stop never arrived (crash, killed) expires so the count cannot stick forever.
fn subagent_markers(dir: &str, stale_s: u64) -> Vec<String> {
    let max_age = std::time::Duration::from_secs(6 * 3600); // long-expiry fallback for single markers
    let stale = std::time::Duration::from_secs(stale_s);
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut ages = Vec::new();
    for e in rd.flatten() {
        let age = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
        match age {
            Some(a) if a > max_age => {
                let _ = std::fs::remove_file(e.path());
            }
            Some(a) => ages.push((e.path(), a)),
            None => ages.push((e.path(), std::time::Duration::ZERO)),
        }
    }
    // a missed SubagentStop must not silence dings forever: if even the newest marker is older than
    // subagent_stale_s, nothing is running any more
    if stale_s > 0 && !ages.is_empty() && ages.iter().all(|(_, a)| *a > stale) {
        for (p, _) in &ages {
            let _ = std::fs::remove_file(p);
        }
        return Vec::new();
    }
    ages.iter().filter_map(|(p, _)| p.file_name().map(|n| n.to_string_lossy().to_string())).collect()
}

// age of a file from its metadata (None = unknown / in the future)
fn age_of(m: Option<std::fs::Metadata>) -> Option<std::time::Duration> {
    m.and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok())
}

// a Tab/PaneUpdate received at `ev_ms` still wins over a session snapshot taken at `now_ms`
fn event_holds(ev_ms: u64, now_ms: u64) -> bool {
    ev_ms > 0 && now_ms.saturating_sub(ev_ms) < EVENT_HOLD_MS
}

// accent colour mixed 75/25 with the theme foreground
fn blend(rgb: (u8, u8, u8), theme: PaletteColor) -> PaletteColor {
    match theme {
        PaletteColor::Rgb((r, g, b)) => {
            let m = |a: u8, t: u8| ((a as u16 * 3 + t as u16) / 4) as u8;
            PaletteColor::Rgb((m(rgb.0, r), m(rgb.1, g), m(rgb.2, b)))
        }
        _ => PaletteColor::Rgb(rgb),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn looks_like_claude(p: &PaneInfo) -> bool {
    // the command line of the pane, or the title Claude Code sets ("✳ ..." / "Claude Code")
    p.terminal_command.as_deref().map_or(false, |c| c.to_lowercase().contains("claude"))
        || p.title.starts_with('\u{2733}')
        || p.title.contains("Claude Code")
}

impl State {
    // the permission question is answered (or was cached / survived a reload) and granted
    fn permission_ok(&mut self) {
        self.make_unselectable();
        if !self.perms {
            self.perms = true;
            self.perms_denied = false;
            self.ask_tz();
            self.ask_scan();
            self.ask_os();
            self.refresh_names();
        }
    }

    fn make_unselectable(&mut self) {
        if !self.unselectable {
            self.unselectable = true;
            set_selectable(false);
        }
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, cfg: BTreeMap<String, String>) {
        self.ding_command = cfg.get("ding_command").cloned().unwrap_or_default();
        self.ding_when_focused = truthy(cfg.get("ding_when_focused"), false);
        self.ding_when_os_unfocused = truthy(cfg.get("ding_when_os_unfocused"), true);
        self.attention = truthy(cfg.get("attention"), true);
        self.attention_repeat_s = cfg.get("attention_repeat_s").and_then(|v| v.parse().ok()).unwrap_or(0);
        self.auto_switch = truthy(cfg.get("auto_switch"), false);
        self.show_icon_fallback = truthy(cfg.get("title_fallback"), true);
        self.auto_name = truthy(cfg.get("auto_name"), true);
        // false until PermissionRequestResult(Granted) or an event that needs ReadApplicationState arrives
        // (permission_ok); a synchronous host call (get_session_list etc.) panics the plugin while denied
        self.perms = false;
        self.clock = truthy(cfg.get("clock"), true);
        self.race_icons = truthy(cfg.get("race_icons"), true);
        self.race_tab_colors = truthy(cfg.get("race_tab_colors"), true);
        self.show_other_clients = truthy(cfg.get("show_other_clients"), false);
        // auto (default) = the colour font iff claude-hook.sh found "WC3 Icons" installed (<state root>/font)
        self.race_font_opt = match cfg.get("race_font").map(|v| v.as_str()) {
            Some("true") | Some("1") | Some("yes") | Some("on") => Some(true),
            Some("false") | Some("0") | Some("no") | Some("off") => Some(false),
            _ => None,
        };
        self.subagent_count = truthy(cfg.get("subagent_count"), true);
        self.blink = truthy(cfg.get("blink"), true);
        self.spinner = truthy(cfg.get("spinner"), true);
        self.agents_script = cfg.get("agents_script").cloned().unwrap_or_default();
        self.working_hold_s = cfg.get("working_hold_s").and_then(|v| v.parse().ok()).unwrap_or(20);
        self.spin_ms = cfg.get("spin_ms").and_then(|v| v.parse::<u64>().ok()).map_or(160, |v| v.clamp(60, 2000));
        self.ding_with_subagents = truthy(cfg.get("ding_with_subagents"), false);
        self.subagent_grace_s = cfg.get("subagent_grace_s").and_then(|v| v.parse().ok()).unwrap_or(8);
        self.subagent_stale_s = cfg.get("subagent_stale_s").and_then(|v| v.parse().ok()).unwrap_or(900);
        self.blink_ms = cfg.get("blink_ms").and_then(|v| v.parse::<u64>().ok()).map_or(900, |v| v.clamp(200, 5000));
        self.blink_timeout_s = cfg.get("blink_timeout_s").and_then(|v| v.parse::<u64>().ok()).unwrap_or(600);
        self.active_underline = truthy(cfg.get("active_underline"), false);
        self.tab_numbers = truthy(cfg.get("tab_numbers"), false);
        self.race_dim = cfg.get("race_dim").and_then(|v| v.parse::<f32>().ok()).map_or(0.4, |v| v.clamp(0.1, 1.0));
        self.plain_dim = cfg.get("plain_dim").and_then(|v| v.parse::<f32>().ok()).map_or(PLAIN_DIM_DEFAULT, |v| v.clamp(0.1, 1.0));
        self.race_all_tabs = truthy(cfg.get("race_all_tabs"), false);
        self.diversity = truthy(cfg.get("diversity"), true);
        self.races_opt = cfg
            .get("races")
            .map(|v| v.split(|c: char| c == ',' || c.is_whitespace()).filter(|x| valid_race(x)).map(String::from).collect::<Vec<_>>())
            .unwrap_or_default();
        // names contain spaces: comma separated
        self.chars_opt = cfg
            .get("characters")
            .map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| valid_char(x)).collect::<Vec<_>>())
            .unwrap_or_default();
        self.defs = parse_characters(CHARACTERS_TSV);
        self.rebuild_badges();
        self.sounds_dir = cfg.get("sounds_dir").cloned().unwrap_or_else(|| DEFAULT_SOUNDS_DIR.to_string());
        // empty/unset package_dir = the dir claude-hook.sh reports (<state root>/pkgdir), else the legacy default
        self.pkg_opt = cfg.get("package_dir").map(|v| v.trim().trim_end_matches('/').to_string()).filter(|v| !v.is_empty());
        // an explicit empty phrase_script disables clips (only ding_command); unset = <package dir>/play-phrase.sh
        self.phrase_opt = cfg.get("phrase_script").cloned();
        if let Some(p) = &self.phrase_opt {
            self.phrase_script = p.clone();
        }
        self.refresh_paths();
        self.player = cfg.get("player").cloned().unwrap_or_default(); // empty = autodetect in play-phrase.sh
        self.lead_in_ms = cfg.get("lead_in_ms").and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(0).min(3000);
        self.dup_window_ms = cfg.get("dup_window_ms").and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(2000).min(30000);
        self.max_clip_s = parse_max_clip_s(cfg.get("max_clip_s").map(|v| v.as_str()).unwrap_or(""));
        self.my_id = get_plugin_ids().plugin_id;
        // stay selectable until the permission question is resolved: zellij draws its one-time
        // "Allow? (y/n)" prompt in this pane and only a focused pane gets the `y`. Made unselectable on
        // PermissionRequestResult (zellij 0.45.1 also sends it with a cached grant and after a reload) or,
        // as a fallback, on the first event that needs a granted permission. zellij holds back all events
        // while the prompt is open.
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::RunCommands,
            PermissionType::ReadCliPipes,
        ]);
        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::ModeUpdate,
            EventType::Mouse,
            EventType::Timer,
            EventType::CopyToClipboard,
            EventType::RunCommandResult,
            EventType::InputReceived,
            EventType::SystemClipboardFailure,
            EventType::PermissionRequestResult,
        ]);
        set_timeout(1.0);
    }

    fn update(&mut self, event: Event) -> bool {
        match &event {
            // zellij delivers these only with the permissions granted
            Event::TabUpdate(_) | Event::PaneUpdate(_) | Event::ModeUpdate(_) | Event::RunCommandResult(..) => self.permission_ok(),
            _ => {}
        }
        match event {
            Event::PermissionRequestResult(s) => {
                if s == PermissionStatus::Granted {
                    self.permission_ok();
                } else {
                    self.perms = false;
                    self.perms_denied = true;
                    self.make_unselectable();
                }
                true
            }
            Event::ModeUpdate(mi) => {
                if mi.mode == InputMode::RenameTab {
                    self.rename_seen = now_secs();
                }
                let changed = self.mode_info != mi;
                if let Some(n) = mi.session_name.clone() {
                    self.session = n;
                }
                self.mode_info = mi;
                self.refresh();
                changed
            }
            Event::TabUpdate(t) => {
                let changed = self.tabs != t;
                self.track_user_names(&t);
                for x in &t {
                    self.last_names.insert(x.tab_id, x.name.clone());
                }
                self.tabs = t;
                self.tabs_ev_ms = now_ms();
                self.refresh();
                changed
            }
            Event::PaneUpdate(m) => {
                self.manifest = Some(m);
                self.panes_ev_ms = now_ms();
                self.refresh_names();
                self.refresh();
                true
            }
            Event::Timer(_) => {
                // several timer chains can end up alive (reloads, early fires); drop a tick that comes
                // much sooner than the interval we asked for, which lets the extra chains die out
                let t_now = now_ms();
                if t_now.saturating_sub(self.last_tick_ms) < self.last_next_ms / 2 {
                    return false;
                }
                self.last_tick_ms = t_now;
                // 1 s housekeeping tick; while a tab blinks, re-arm at the next phase boundary instead
                let mut next = 1.0f64;
                // fast cadence only in the instance that is actually being drawn (render() ran lately);
                // instances of inactive tabs keep the 1 s tick
                if now_secs().saturating_sub(self.last_render) < 3 && self.in_focused_tab() {
                    if self.any_blinking() {
                        let to_boundary = self.blink_ms - now_ms() % self.blink_ms;
                        next = next.min(to_boundary.max(60) as f64 / 1000.0);
                    }
                    self.ask_agents();
                    if self.any_working() {
                        let to_boundary = self.spin_ms - now_ms() % self.spin_ms;
                        next = next.min(to_boundary.max(30) as f64 / 1000.0);
                    }
                }
                // the leader decides about dings from these lists too, whichever tab it sits in
                if self.is_leader() {
                    self.ask_agents();
                }
                self.last_next_ms = (next * 1000.0) as u64;
                set_timeout(next);
                self.refresh_paths();
                self.ask_tz();
                self.ask_scan();
                self.ask_os();
                // only the instance that is being drawn (render() ran lately) polls folder names
                let now = now_secs();
                if now.saturating_sub(self.last_render) < 10 && now.saturating_sub(self.names_asked) >= 2 {
                    self.names_asked = now;
                    self.refresh_names();
                }
                self.refresh();
                let key = self.render_key();
                let changed = key != self.last_key;
                self.last_key = key;
                changed
            }
            Event::Mouse(Mouse::LeftClick(_, col)) => {
                if matches!(self.breadcrumb_range, Some((a, b)) if col >= a && col < b) {
                    focus_host_session();
                } else if let Some(idx) = get_tab_to_focus(&self.tab_line, self.active_idx(), col) {
                    switch_tab_to(idx as u32);
                }
                false
            }
            Event::Mouse(Mouse::ScrollUp(_)) => {
                switch_tab_to((self.active_idx() + 1).min(self.tabs.len()) as u32);
                false
            }
            Event::Mouse(Mouse::ScrollDown(_)) => {
                switch_tab_to(self.active_idx().saturating_sub(1).max(1) as u32);
                false
            }
            Event::RunCommandResult(code, out, err, ctx) => {
                let mut redraw = false;
                match ctx.get("k").map(|s| s.as_str()) {
                    Some("tz") => {
                        // `date +%z` -> +HHMM / -HHMM
                        let t = String::from_utf8_lossy(&out).trim().to_string();
                        if t.len() == 5 {
                            let sign = if t.starts_with('-') { -1 } else { 1 };
                            if let (Ok(h), Ok(m)) = (t[1..3].parse::<i64>(), t[3..5].parse::<i64>()) {
                                let o = Some(sign * (h * 3600 + m * 60));
                                redraw = o != self.tz_offset;
                                self.tz_offset = o;
                            }
                        }
                    }
                    Some("agents") => {
                        if let Some(pane) = ctx.get("pane").and_then(|v| v.parse::<u32>().ok()) {
                            let mut v: Vec<String> = String::from_utf8_lossy(&out)
                                .lines()
                                .map(|l| l.trim().to_string())
                                .filter(|l| !l.is_empty())
                                .collect();
                            v.sort();
                            self.task_at.insert(pane, now_secs());
                            // share with the other instances (atomic: temp file + rename)
                            if !self.session.is_empty() {
                                let path = format!("{}/{}/{}.agents", STATE_ROOT, self.session, pane);
                                let tmp = format!("{}.{}", path, self.my_id);
                                let body: String = v.iter().map(|l| format!("{}\n", l)).collect();
                                if std::fs::write(&tmp, body).is_ok() {
                                    let _ = std::fs::rename(&tmp, &path);
                                }
                            }
                            if self.task_running.get(&pane) != Some(&v) {
                                self.task_running.insert(pane, v);
                                redraw = true;
                            }
                        }
                    }
                    Some("osfocus") => {
                        // anything but a clear "unfocused" (focused, unknown, failure) counts as back
                        if String::from_utf8_lossy(&out).trim() != "unfocused" {
                            self.os_focus_ms = now_ms();
                        }
                    }
                    Some("os") if code == Some(0) => {
                        let o = String::from_utf8_lossy(&out).trim().to_string();
                        redraw = o != self.os;
                        self.os = o;
                    }
                    Some("scan") if code == Some(99) => {
                        // no play-phrase.sh at the package dir (yet): assign from characters.tsv meanwhile
                        // and look again in 5 s (a hook writes pkgdir on its next event)
                        let p = ctx.get("p").cloned().unwrap_or_default();
                        if p != self.pkg_missing {
                            eprintln!("claude-tabs: no {} yet (waiting for a Claude Code hook to report the package dir)", p);
                            self.pkg_missing = p;
                        }
                        self.scan_done = true;
                        self.pkg_ok = false;
                        self.scan_asked = now_secs().saturating_sub(25);
                    }
                    Some("scan") => {
                        self.pkg_missing.clear();
                        self.pkg_ok = true;
                        let txt = String::from_utf8_lossy(&out).to_string();
                        redraw = self.apply_scan(&txt);
                        eprintln!("claude-tabs: scan found {} characters", self.scanned.len());
                        if redraw {
                            self.rebuild_badges();
                        }
                        if self.is_leader() && !self.session.is_empty() {
                            // publish for the other instances (atomic: temp file + rename)
                            let path = self.library_file();
                            let tmp = format!("{}.{}", path, self.my_id);
                            if std::fs::write(&tmp, format!("{}\n{}", now_secs(), txt)).is_ok() {
                                let _ = std::fs::rename(&tmp, &path);
                            }
                        }
                    }
                    Some("ding") if code != Some(0) => {
                        eprintln!("claude-tabs: ding_command failed ({:?}): {}", code, String::from_utf8_lossy(&err));
                    }
                    _ => {}
                }
                redraw
            }
            Event::CopyToClipboard(dest) => {
                let changed = self.text_copy_destination != Some(dest);
                self.text_copy_destination = Some(dest);
                changed
            }
            Event::SystemClipboardFailure => {
                self.display_system_clipboard_failure = true;
                true
            }
            Event::InputReceived => {
                let changed = self.text_copy_destination.is_some() || self.display_system_clipboard_failure;
                self.text_copy_destination = None;
                self.display_system_clipboard_failure = false;
                changed
            }
            _ => false,
        }
    }

    fn pipe(&mut self, msg: PipeMessage) -> bool {
        if let PipeSource::Cli(id) = &msg.source {
            unblock_cli_pipe_input(id);
        }
        if msg.name == "claude_ding" {
            // manual test: payload = character name, or `active` for the active tab's character.
            // `zellij pipe` also sends an empty message at stdin EOF: ignore it.
            if msg.payload.as_deref().map_or(true, |p| p.trim().is_empty()) {
                return false;
            }
            if self.fetch_snapshot() && self.is_leader() {
                let given = msg.payload.as_deref().map(str::trim).filter(|r| *r != "active" && valid_char(r)).map(String::from);
                let ch = given.or_else(|| {
                    let t = self.tabs.iter().find(|t| t.active)?;
                    self.tab_chars.get(&t.tab_id).cloned()
                });
                self.ding(ch.as_deref(), "ctl", None);
            }
            return false;
        }
        if msg.name == "claude_reshuffle" {
            // `claude-ctl.sh reshuffle`: forget all assignments and hand them out again (unique first)
            if msg.payload.as_deref().map_or(false, |p| p.trim() == "now") && self.fetch_snapshot() && self.is_leader() {
                if let Ok(rd) = std::fs::read_dir(self.chars_dir()) {
                    for e in rd.flatten() {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
                self.tab_chars.clear();
                self.assign_chars();
            }
            return true;
        }
        if msg.name == "claude_character" || msg.name == "claude_race" {
            // `zellij pipe --name claude_character -- "Arthas Menethill"` sets the character of the active tab;
            // claude_race with a race name picks that race's least-used character instead
            let r = msg.payload.as_deref().map(str::trim).unwrap_or("");
            if valid_char(r) && self.fetch_snapshot() {
                self.read_chars();
                let chosen = if msg.name == "claude_character" {
                    Some(r.to_string())
                } else {
                    let live: HashSet<usize> = self.tabs.iter().map(|t| t.tab_id).collect();
                    let count = |n: &String| self.tab_chars.iter().filter(|(k, v)| live.contains(k) && *v == n).count();
                    let mut c: Vec<String> = self.effective_chars().into_iter().filter(|n| self.race_of(n).as_deref() == Some(r)).collect();
                    if c.is_empty() {
                        c = self.defs.iter().filter(|d| d.race == r).map(|d| d.name.clone()).collect();
                    }
                    let min = c.iter().map(|n| count(n)).min().unwrap_or(0);
                    c.retain(|n| count(n) == min);
                    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as usize);
                    if c.is_empty() { None } else { Some(c[nanos % c.len()].clone()) }
                };
                if let (Some(ch), Some(id)) = (chosen, self.tabs.iter().find(|t| t.active).map(|t| t.tab_id)) {
                    self.set_char(id, &ch, true);
                }
            }
            return true;
        }
        if msg.name == "claude_poke" {
            self.refresh();
            return true;
        }
        false
    }

    fn render(&mut self, _rows: usize, cols: usize) {
        self.last_render = now_secs();
        if let Some(dest) = self.text_copy_destination {
            let hint = clipboard_utils::text_copied_hint(dest).part;
            self.render_background_with_text(&hint);
        } else if self.perms_denied {
            self.render_background_with_text(" claude-tabs: permissions denied (README: Troubleshooting)");
        } else if self.display_system_clipboard_failure {
            let hint = clipboard_utils::system_clipboard_error().part;
            self.render_background_with_text(&hint);
        } else {
            self.render_tabs(cols);
        }
    }
}

impl State {
    // `date +%z` once and then hourly (DST); wasm has no local timezone
    fn ask_tz(&mut self) {
        if !self.clock || !self.perms {
            return;
        }
        let now = now_secs();
        let wait = if self.tz_offset.is_none() { 5 } else { 3600 };
        if now.saturating_sub(self.tz_asked) < wait {
            return;
        }
        self.tz_asked = now;
        let ctx = BTreeMap::from([("k".to_string(), "tz".to_string())]);
        run_command(&["date", "+%z"], ctx);
    }

    fn clock_text(&self) -> Option<String> {
        if !self.clock {
            return None;
        }
        let off = self.tz_offset?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
        let local = (now + off).rem_euclid(86400);
        Some(format!("{:02}:{:02}", local / 3600, (local % 3600) / 60))
    }

    // Blink of a finished-and-unseen tab that is not the active one (or is, when it finished while the
    // terminal window was not OS-focused): Some(true) = bright phase.
    // The phase comes from the wall clock, so every per-tab instance agrees. After blink_timeout_s the
    // tab stays steadily bright.
    fn tab_blink(&self, t: &TabInfo) -> Option<bool> {
        if !self.blink || (t.active && !self.tab_away(t.position)) {
            return None;
        }
        let (st, ns, _) = self.tab_claude(t.position)?;
        if st != St::Done && st != St::Attn {
            return None;
        }
        let age_s = now_secs().saturating_sub((ns / 1_000_000_000) as u64);
        if self.blink_timeout_s > 0 && age_s > self.blink_timeout_s {
            return Some(true);
        }
        Some(((now_ms() + 25) / self.blink_ms) % 2 == 0)
    }

    // True when this instance sits in the tab the user is looking at. Asked fresh from the server
    // (instances of inactive tabs get no tab events, so their own data is stale); own position comes
    // from the manifest. Unknown = true.
    // the tab holds a finish that happened while the terminal window was not OS-focused
    fn tab_away(&self, pos: usize) -> bool {
        if !self.ding_when_os_unfocused {
            return false;
        }
        let Some(m) = self.manifest.as_ref() else { return false };
        let Some(panes) = m.panes.get(&pos) else { return false };
        panes.iter().filter(|p| !p.is_plugin).any(|p| {
            self.files.get(&p.id).map_or(false, |f| match f.attn {
                Some((_, a)) => a,
                None => f.away && self.eff_state(p.id, f) == St::Done,
            })
        })
    }

    // script next to phrase_script (the package dir)
    fn sibling_script(&self, name: &str) -> String {
        sibling_of(&self.phrase_script, &self.pkg_dir, name)
    }

    // Ask os-focus.sh whether the terminal window of this session has the OS focus (every 2 s at most).
    fn ask_os_focus(&mut self) {
        if !self.perms {
            return;
        }
        let now = now_secs();
        if now.saturating_sub(self.os_focus_asked) < 2 {
            return;
        }
        self.os_focus_asked = now;
        let ctx = BTreeMap::from([("k".to_string(), "osfocus".to_string())]);
        let script = self.sibling_script("os-focus.sh");
        run_command(
            &["sh", "-c", "s=$1; case $s in '~/'*) s=$HOME/${s#'~/'};; esac; [ -f \"$s\" ] || exit 0; exec sh \"$s\" \"$2\"", "_", &script, &self.session],
            ctx,
        );
    }

    fn in_focused_tab(&self) -> bool {
        // NOTE get_focused_pane_info() returns the tab ID, the manifest is keyed by tab POSITION; they
        // differ as soon as a tab was closed, so compare ids
        if !self.perms {
            return false;
        }
        let Ok((focused_id, _)) = get_focused_pane_info() else { return true };
        let mine_pos = self.manifest.as_ref().and_then(|m| {
            m.panes
                .iter()
                .find(|(_, v)| v.iter().any(|p| p.is_plugin && p.id == self.my_id))
                .map(|(k, _)| *k)
        });
        let mine_id = mine_pos.and_then(|pos| self.tabs.iter().find(|t| t.position == pos).map(|t| t.tab_id));
        mine_id.map_or(true, |id| id == focused_id)
    }

    // frame of the working spinner (wall clock, so all instances agree)
    fn spin_frame(&self) -> usize {
        (((now_ms() + 25) / self.spin_ms) % SPIN.len() as u64) as usize
    }

    fn any_working(&self) -> bool {
        self.spinner && self.tabs.iter().any(|t| matches!(self.tab_claude(t.position), Some((St::Working, _, _))))
    }

    fn any_blinking(&self) -> bool {
        self.blink && self.tabs.iter().any(|t| self.tab_blink(t).is_some())
    }

    // Poll the transcripts of the session's background agents (claude-agents.sh) for every Claude pane;
    // only the focused tab's instance does it, every 3 s.
    fn ask_agents(&mut self) {
        if !self.perms {
            return;
        }
        let now = now_secs();
        if now.saturating_sub(self.agents_asked) < 3 {
            return;
        }
        self.agents_asked = now;
        let script = if self.agents_script.is_empty() {
            self.sibling_script("claude-agents.sh")
        } else {
            self.agents_script.clone()
        };
        for (pane, f) in self.files.iter() {
            if let Some(dir) = &f.sess {
                let ctx = BTreeMap::from([("k".to_string(), "agents".to_string()), ("pane".to_string(), pane.to_string())]);
                run_command(
                    &["sh", "-c", "s=$1; case $s in '~/'*) s=$HOME/${s#'~/'};; esac; [ -f \"$s\" ] || exit 0; exec sh \"$s\" \"$2\"", "_", &script, dir],
                    ctx,
                );
            }
        }
    }

    // 1-based index of the active tab (0 if unknown)
    fn active_idx(&self) -> usize {
        self.tabs.iter().find(|t| t.active).map_or(0, |t| t.position + 1)
    }

    fn render_background_with_text(&self, text: &str) {
        match self.mode_info.style.colors.text_unselected.background {
            PaletteColor::Rgb((r, g, b)) => print!("{}\u{1b}[48;2;{};{};{}m\u{1b}[0K", text, r, g, b),
            PaletteColor::EightBit(c) => print!("{}\u{1b}[48;5;{}m\u{1b}[0K", text, c),
        }
    }

    fn render_tabs(&mut self, cols: usize) {
        if self.tabs.is_empty() {
            return;
        }
        let data = self.prepare_tab_data();
        let out = line::tab_line(&self.mode_info, data, cols, self.clock_text());
        self.tab_line = out.parts;
        self.breadcrumb_range = out.breadcrumb_range;
        let text: String = self.tab_line.iter().map(|p| p.part.as_str()).collect();
        self.render_background_with_text(&text);
    }

    fn prepare_tab_data(&self) -> TabRenderData {
        let mut tabs = self.tabs.clone();
        tabs.sort_by_key(|t| t.position);
        let colors = self.mode_info.style.colors;
        let dimmed = self.mode_info.session_ascended == Some(true) || self.mode_info.session_dimmed == Some(true);
        let renaming = self.mode_info.mode == InputMode::RenameTab;
        let mut all = Vec::new();
        let mut active_tab_index = 0;
        let mut swap_name = None;
        let mut swap_dirty = false;
        let mut alt = false;
        for t in &tabs {
            let mut name = if renaming && t.active { t.name.clone() } else { self.display_name(t) };
            if self.tab_numbers && !(renaming && t.active) {
                name = format!("{} {}", t.position + 1, name);
            }
            if t.active && renaming && name.is_empty() {
                name = "Enter name...".to_string();
            }
            if t.active {
                active_tab_index = t.position;
                if !renaming {
                    swap_dirty = t.is_swap_layout_dirty;
                    swap_name = t.active_swap_layout_name.clone();
                }
            }
            let ribbon = if t.active { colors.ribbon_selected } else { colors.ribbon_unselected };
            let claude = self.tab_claude(t.position);
            let icon = claude.or(if self.race_all_tabs && self.tab_chars.contains_key(&t.tab_id) { Some((St::Idle, 0, 0)) } else { None }).map(|(st, _ns, count)| {
                let is_claude = claude.is_some();
                let mut glyph = if is_claude {
                    if self.spinner && st == St::Working {
                        SPIN[self.spin_frame()].to_string()
                    } else if st == St::Attn {
                        "?".to_string()
                    } else {
                        ICON.to_string()
                    }
                } else {
                    String::new()
                };
                if count > 0 && self.subagent_count {
                    glyph.push_str(&count.to_string());
                }
                let color = match st {
                    St::Working => ribbon.emphasis_0,
                    St::Done => ribbon.emphasis_2,
                    St::Attn => ribbon.emphasis_3,
                    St::Idle => ribbon.base,
                };
                // colour icon font: the character's glyph; otherwise a 2-cell initials badge (any terminal)
                let font = self.race_font_on();
                let race = if self.race_icons {
                    self.tab_chars.get(&t.tab_id).map(|r| {
                        let (g, rgb) = self.char_style(r, font, self.os == "Darwin");
                        (if font { g } else { self.badge_of(r) }, blend(rgb, ribbon.base))
                    })
                } else {
                    None
                };
                let badge = race.is_some() && !font;
                let state = match st {
                    St::Working => IconState::Working,
                    St::Done => IconState::Done,
                    St::Attn => IconState::Attn,
                    St::Idle => IconState::Idle,
                };
                let tint = if self.race_tab_colors && race.is_some() {
                    self.tab_chars.get(&t.tab_id).map(|r| self.char_style(r, false, false).1)
                } else {
                    None
                };
                Icon { glyph, color, race, badge, state, tint, dim: self.race_dim, blink: self.tab_blink(t), underline: self.active_underline }
            });
            all.push(tab_style(name, icon, t, alt, colors, self.mode_info.capabilities, dimmed, self.show_other_clients, self.plain_dim));
            alt = !alt;
        }
        TabRenderData {
            tabs: all,
            active_tab_index,
            active_swap_layout_name: swap_name,
            is_swap_layout_dirty: swap_dirty,
        }
    }

    fn render_key(&self) -> String {
        let mut s = self.clock_text().unwrap_or_default();
        s.push_str(if self.race_font_on() { "F1" } else { "F0" });
        if self.any_working() {
            s.push_str(&format!("S{}", self.spin_frame()));
        }
        for t in &self.tabs {
            if let Some(b) = self.tab_blink(t) {
                s.push_str(if b { "B1" } else { "B0" });
            }
        }
        for t in &self.tabs {
            s.push_str(&self.display_name(t));
            if let Some(r) = self.tab_chars.get(&t.tab_id) {
                s.push_str(r);
            }
            if let Some((st, ns, n)) = self.tab_claude(t.position) {
                s.push_str(&format!("{}:{:?}:{}:{};", t.position, st, ns, n));
            }
            s.push_str(&self.display_name(t));
            s.push('|');
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    // deterministic xorshift for the simulations
    fn sim_rng(seed: u64) -> impl FnMut() -> f64 {
        let mut x = seed | 1;
        move || {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            (x.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    // sessions of 2-8 tabs appended one by one (unique characters, Pit Lord excluded like the real pool)
    fn simulate(diverse: bool, sessions: usize) -> (usize, usize, Vec<(String, usize)>, usize) {
        let defs = parse_characters(CHARACTERS_TSV);
        let all: Vec<String> = defs.iter().filter(|d| d.name != "Pit Lord").map(|d| d.name.clone()).collect();
        let mut rnd = sim_rng(0xC0FFEE);
        let (mut clash, mut picks, mut same_theme_adj) = (0, 0, 0);
        let mut races: HashMap<String, usize> = HashMap::new();
        let mut recent: Vec<String> = Vec::new();
        for s in 0..sessions {
            let n = 2 + s % 7;
            let mut live: Vec<String> = Vec::new();
            for pos in 0..n {
                let cands: Vec<String> = all.iter().filter(|c| !live.contains(c)).cloned().collect();
                let ctx: Vec<(String, usize)> = live.iter().enumerate().map(|(i, c)| (c.clone(), pos - i)).collect();
                let pick = if diverse {
                    diverse_pick(&defs, &cands, &ctx, &recent, rnd()).unwrap()
                } else {
                    cands[(rnd() * cands.len() as f64) as usize].clone()
                };
                let d = defs.iter().find(|d| d.name == pick).unwrap();
                if let Some(prev) = live.last().and_then(|p| defs.iter().find(|x| x.name == *p)) {
                    // an alternative without a clash existed?
                    let alt = cands.iter().any(|c| !clashes_adjacent(defs.iter().find(|x| x.name == *c).unwrap(), prev));
                    if clashes_adjacent(d, prev) && alt {
                        clash += 1;
                    }
                    if same(&d.theme, &prev.theme) {
                        same_theme_adj += 1;
                    }
                }
                *races.entry(d.race.clone()).or_default() += 1;
                picks += 1;
                recent.push(pick.clone());
                live.push(pick);
            }
        }
        let mut r: Vec<(String, usize)> = races.into_iter().collect();
        r.sort();
        (clash, picks, r, same_theme_adj)
    }

    #[test]
    fn diversity_metadata_is_complete() {
        let defs = parse_characters(CHARACTERS_TSV);
        assert!(defs.iter().all(|d| d.role == "hero" || d.role == "unit"), "every row has a role");
        assert!(defs.iter().any(|d| d.sex == "f") && defs.iter().any(|d| d.sex == "m"));
    }

    #[test]
    fn diversity_never_clashes_with_adjacent_tab() {
        let (clash, picks, races, theme_adj) = simulate(true, 1000);
        let (_, _, base_races, base_theme_adj) = simulate(false, 1000);
        eprintln!("diverse: {} picks, {} adjacent same-race+role/theme clashes, {} adjacent same-theme", picks, clash, theme_adj);
        eprintln!("diverse races: {:?}\nrandom  races: {:?}\nrandom adjacent same-theme: {}", races, base_races, base_theme_adj);
        assert_eq!(clash, 0);
        assert!(theme_adj < base_theme_adj);
    }

    #[test]
    fn diversity_spreads_races() {
        // share of each race among the picks must be closer to even than the 92-character pool is
        let (_, picks, races, _) = simulate(true, 1000);
        let (_, _, base, _) = simulate(false, 1000);
        let spread = |r: &Vec<(String, usize)>| {
            let mean = picks as f64 / r.len() as f64;
            r.iter().map(|x| (x.1 as f64 - mean).abs()).sum::<f64>() / picks as f64
        };
        assert!(spread(&races) < spread(&base), "diverse {} vs random {}", spread(&races), spread(&base));
    }

    #[test]
    fn diversity_penalises_same_family() {
        let defs = parse_characters(CHARACTERS_TSV);
        let ctx = vec![("Illidan Stormrage".to_string(), 1)];
        let w = |n: &str| diversity_weight(&defs, n, &ctx, &[]);
        assert!(w("Lovec démonů") < w("Thrall"));
        assert!(w("Lovec démonů") < w("Tichondrius"));
        assert!(w("Tichondrius") < w("Thrall"));
        // recent picks are damped
        assert!(diversity_weight(&defs, "Thrall", &[], &["Thrall".to_string()]) < diversity_weight(&defs, "Thrall", &[], &[]));
    }

    // relative luminance of every inactive Claude tab background (race_dim 0.4), all characters
    fn claude_bgs(dim: f32) -> Vec<(String, (u8, u8, u8))> {
        parse_characters(CHARACTERS_TSV)
            .iter()
            .map(|d| {
                let (_, rgb) = race_style(&d.race, false, false);
                let acc = character_accent(rgb, &d.name, Some((d.ord, d.of)));
                (d.name.clone(), crate::tab::tint_colors(acc, false, dim).0)
            })
            .collect()
    }

    #[test]
    fn plain_matches_dimmed_claude_tabs() {
        let mut l: Vec<f32> = claude_bgs(0.4).iter().map(|(_, c)| crate::tab::rel_lum(*c)).collect();
        l.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = l[l.len() / 2];
        let mut bgs = claude_bgs(0.4);
        bgs.sort_by(|a, b| crate::tab::rel_lum(a.1).partial_cmp(&crate::tab::rel_lum(b.1)).unwrap());
        println!("claude bg min {:?} median {:?} max {:?}", bgs[0].1, bgs[bgs.len() / 2].1, bgs[bgs.len() - 1].1);
        let mean = l.iter().sum::<f32>() / l.len() as f32;
        println!("claude inactive bg luminance: n {} min {:.4} q1 {:.4} median {:.4} mean {:.4} q3 {:.4} max {:.4}",
            l.len(), l[0], l[l.len() / 4], med, mean, l[l.len() * 3 / 4], l[l.len() - 1]);
        for d in [0.4f32, 0.3, 0.25, 0.2, 0.15] {
            let (bg, fg) = crate::tab::plain_colors((0x23, 0x20, 0x22), (0xd5, 0xb9, 0xbb), d);
            println!("plain_dim {d}: bg {bg:?} lum {:.4} fg {fg:?}", crate::tab::rel_lum(bg));
        }
        let (bg, _) = crate::tab::plain_colors((0x23, 0x20, 0x22), (0xd5, 0xb9, 0xbb), PLAIN_DIM_DEFAULT);
        let pl = crate::tab::rel_lum(bg);
        println!("default {PLAIN_DIM_DEFAULT}: bg {bg:?} lum {pl:.4}");
        assert!((pl - med).abs() / med < 0.15, "plain lum {pl} vs median {med}");
    }

    #[test]
    fn max_clip_s_parsing() {
        assert_eq!(parse_max_clip_s(""), "");
        assert_eq!(parse_max_clip_s("0"), "");
        assert_eq!(parse_max_clip_s("-3"), "");
        assert_eq!(parse_max_clip_s("abc"), "");
        assert_eq!(parse_max_clip_s("NaN"), "");
        assert_eq!(parse_max_clip_s("inf"), "");
        assert_eq!(parse_max_clip_s("3"), "3");
        assert_eq!(parse_max_clip_s(" 12.5 "), "12.5");
        assert_eq!(parse_max_clip_s("8.0"), "8");
        assert_eq!(parse_max_clip_s("0.25"), "0.25");
        assert_eq!(parse_max_clip_s("1e1"), "10");
    }

    // cargo test --target x86_64-unknown-linux-gnu -- --nocapture   (prints the badge table)
    #[test]
    fn badges_unique_two_cells() {
        let defs = parse_characters(CHARACTERS_TSV);
        let names: Vec<String> = defs.iter().map(|d| d.name.clone()).collect();
        let t = badge_table(&names);
        let mut seen: HashMap<String, String> = HashMap::new();
        for d in &defs {
            let b = &t[&d.name];
            println!("{}\t{}\t{}", d.race, d.name, b);
            assert_eq!(b.width(), 2, "{} -> {:?}", d.name, b);
            assert_eq!(b.chars().count(), 2, "{} -> {:?}", d.name, b);
            assert!(b.chars().all(|c| (c as u32) < 0x10000), "{} -> {:?}", d.name, b);
            if let Some(o) = seen.insert(b.clone(), d.name.clone()) {
                panic!("collision {} {} {}", b, o, d.name);
            }
        }
        assert_eq!(seen.len(), defs.len());
        assert_eq!(badge_candidates("Čarodějka")[0], "ČA");
        assert_eq!(badge_candidates("Trollí čarodějnice - lékařka")[0], "TČ");
        assert_eq!(badge_candidates("Arthas (Nemrtví)")[0], "AN");
        assert_eq!(badge_candidates("X")[0], "X ");
        assert_eq!(badge_candidates("42")[0], "42");
        assert_eq!(badge_candidates("straße")[0], "ST");
    }

    #[test]
    fn events_win_over_snapshot_briefly() {
        assert!(!event_holds(0, 10_000)); // no event yet: snapshot applies
        assert!(event_holds(9_000, 10_000));
        assert!(event_holds(10_000, 10_000));
        assert!(!event_holds(10_000 - EVENT_HOLD_MS, 10_000));
        assert!(event_holds(10_500, 10_000)); // clock step back: keep the event
    }

    #[test]
    fn pkg_dir_sticks_while_confirmed() {
        // option always wins
        assert_eq!(choose_pkg(Some("/o"), Some("/f"), "/c", true), "/o");
        // confirmed current dir is kept although the hook file flips
        assert_eq!(choose_pkg(None, Some("/b"), "/a", true), "/a");
        // not confirmed (missing / default): follow the file
        assert_eq!(choose_pkg(None, Some("/b"), "/a", false), "/b");
        // nothing known: legacy default
        assert_eq!(choose_pkg(None, None, "", false), DEFAULT_PACKAGE_DIR);
    }

    #[test]
    fn siblings_without_phrase_script() {
        assert_eq!(sibling_of("/p/play-phrase.sh", "/q", "os-focus.sh"), "/p/os-focus.sh");
        assert_eq!(sibling_of("", "/q", "claude-agents.sh"), "/q/claude-agents.sh");
    }

    #[test]
    fn character_glyphs_are_two_cells() {
        use unicode_width::UnicodeWidthChar;
        let defs = parse_characters(CHARACTERS_TSV);
        assert!(defs.iter().any(|d| d.glyph.is_some()));
        for d in &defs {
            if let Some(g) = d.glyph {
                assert_eq!(g.width(), Some(2), "{} U+{:X}", d.name, g as u32);
            }
        }
        for r in RACES.iter().filter_map(|r| r.3) {
            assert_eq!(r.chars().next().and_then(|c| c.width()), Some(2), "race icon {:?}", r);
        }
    }
}

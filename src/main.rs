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
// The wasm cannot know where it was loaded from, so the helper scripts are found through the
// `package_dir` option (the package checkout). setup.sh links ~/.config/zellij/plugins/claude-tabs to
// the checkout, which is this default.
const DEFAULT_PACKAGE_DIR: &str = "~/.config/zellij/plugins/claude-tabs";

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

// Claude Code's own spinner frames (ping-pong: forward then back, period 10); all one cell wide
const SPIN: [&str; 10] = ["\u{B7}", "\u{2722}", "\u{2733}", "\u{2736}", "\u{273B}", "\u{273D}", "\u{273B}", "\u{2736}", "\u{2733}", "\u{2722}"];

const STATE_ROOT: &str = "/tmp/zellij-claude";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Idle,    // session running, waiting for the user (start)
    Working, // processing a prompt
    Done,    // finished, waiting for the user
}

#[derive(Clone, Debug)]
struct PaneState {
    st: St,
    ns: u128,
    subs: usize, // running subagents (one marker file each in <pane>.sub/)
    deferred: bool, // the Stop hook found running subagents: not a real finish yet
    away: bool,     // the Stop hook found the terminal window not OS-focused (os-focus.sh)
    sess: Option<String>, // directory with the session's agent-<id>.jsonl transcripts (written by the hook)
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
    files: HashMap<u32, PaneState>,
    // last "done" timestamp we reacted to (ding/auto switch) and last one shown as seen
    reacted: HashMap<u32, u128>,
    clock: bool,
    races: HashMap<usize, String>, // tab id -> race (files in races/)
    races_opt: Vec<String>,  // `races` option (empty = use the scanned sounds_dir folders)
    scanned: Vec<String>,    // sounds_dir subfolders that hold audio files
    scan_done: bool,
    scan_asked: u64,
    race_icons: bool,
    race_tab_colors: bool,
    race_dim: f32,
    show_other_clients: bool,
    blink: bool,
    spinner: bool,
    task_running: HashMap<u32, Vec<String>>,
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
    race_font_opt: Option<bool>, // None = auto: on for Linux, off elsewhere (macOS CoreText has no CBDT bitmap fonts)
    os: String,
    os_asked: u64,
    subagent_count: bool,
    active_underline: bool,
    tab_numbers: bool,
    race_all_tabs: bool,
    sounds_dir: String,
    phrase_script: String,
    player: String,
    tz_offset: Option<i64>,
    tz_asked: u64,
    my_id: u32,
    snap_at: u64,
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
        self.read_races();
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
            // running agents = hook markers (SubagentStart/Stop) UNION agents whose transcripts say they are
            // still running (claude-agents.sh, polled): neither source alone is reliable
            let mut ids = subagent_markers(&format!("{}/{}.sub", dir, pane), self.subagent_stale_s);
            if let Some(t) = self.task_running.get(&pane) {
                for id in t {
                    if !ids.contains(id) {
                        ids.push(id.clone());
                    }
                }
            }
            let subs = ids.len();
            let sess = std::fs::read_to_string(format!("{}/{}.sess", dir, pane)).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
            // safety net: any hook event newer than the last state change, within working_hold_s, means busy
            let age = |m: Option<std::fs::Metadata>| m.and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok());
            let st_age = age(e.metadata().ok());
            let ev_age = age(std::fs::metadata(format!("{}/{}.ev", dir, pane)).ok());
            let recent_activity = match (st_age, ev_age) {
                (Some(sa), Some(ea)) => {
                    ea.as_secs() < self.working_hold_s && sa > ea + std::time::Duration::from_secs(1)
                }
                _ => false,
            };
            self.files.insert(pane, PaneState { st, ns, subs, deferred, away, recent_activity, sess });
        }
    }

    fn races_dir(&self) -> String {
        format!("{}/{}/races", STATE_ROOT, self.session)
    }

    fn read_races(&mut self) {
        self.races.clear();
        if self.session.is_empty() {
            return;
        }
        let Ok(rd) = std::fs::read_dir(self.races_dir()) else { return };
        for e in rd.flatten() {
            let Ok(id) = e.file_name().to_string_lossy().parse::<usize>() else { continue };
            if let Ok(r) = std::fs::read_to_string(e.path()) {
                let r = r.trim().to_string();
                if valid_race(&r) {
                    self.races.insert(id, r);
                }
            }
        }
    }

    fn set_race(&mut self, tab_id: usize, race: &str, overwrite: bool) -> bool {
        let dir = self.races_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = format!("{}/{}", dir, tab_id);
        let ok = if overwrite {
            std::fs::write(&path, race).is_ok()
        } else {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .and_then(|mut f| f.write_all(race.as_bytes()))
                .is_ok()
        };
        if ok {
            self.races.insert(tab_id, race.to_string());
        }
        ok
    }

    // Groups that may be assigned: the `races` option, else the sounds_dir folders that contain audio.
    // With a library present, a group without clips is never assigned. With no library at all the
    // option (or the built-in four) is used so the icons still work and the sound falls back to the ding.
    fn effective_races(&self) -> Vec<String> {
        // pool = the `races` option, else every group of the sounds library plus every group that has
        // an icon (all of them, so races stay unique for as long as possible: 7 icon groups)
        let mut pool: Vec<String> = if !self.races_opt.is_empty() {
            self.races_opt.clone()
        } else {
            let mut v = self.scanned.clone();
            for r in RACES.iter().filter(|r| r.3.is_some()) {
                if !v.iter().any(|x| x == r.0) {
                    v.push(r.0.to_string());
                }
            }
            v
        };
        pool.dedup();
        pool
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
        self.race_font_opt.unwrap_or(self.os == "Linux")
    }

    // lazily (re)scan <sounds_dir> through the shell: wasm cannot read $HOME
    fn ask_scan(&mut self) {
        if !self.perms || self.phrase_script.is_empty() {
            return;
        }
        let now = now_secs();
        if self.scan_asked != 0 && now.saturating_sub(self.scan_asked) < 30 {
            return;
        }
        self.scan_asked = now;
        let ctx = BTreeMap::from([("k".to_string(), "scan".to_string())]);
        let cmd = format!(
            "\"{}\" --list \"{}\" \"{}\"",
            expand_home(&self.phrase_script),
            expand_home(&self.sounds_dir),
            expand_home(&self.player)
        );
        run_command(&["sh", "-c", &cmd], ctx);
    }

    // Give every tab that runs Claude a race, preferring races no other tab has. The first writer wins
    // (create_new), so instances racing each other agree.
    // Only the leader (lowest plugin id) assigns, from a fresh snapshot, so two tabs can never be
    // handed the same race by concurrent instances. Least-used race first, random among ties; an
    // assignment is permanent (file per tab id) until the tab closes.
    fn assign_races(&mut self) {
        let allowed = self.effective_races();
        if allowed.is_empty() || self.session.is_empty() {
            return;
        }
        let mut tabs: Vec<(usize, usize)> = self.tabs.iter().map(|t| (t.tab_id, t.position)).collect();
        tabs.sort_by_key(|t| t.1);
        for (id, pos) in tabs {
            if self.races.contains_key(&id) || (!self.race_all_tabs && self.tab_claude(pos).is_none()) {
                continue;
            }
            let live: HashSet<usize> = self.tabs.iter().map(|t| t.tab_id).collect();
            let count = |r: &String| self.races.iter().filter(|(k, v)| live.contains(k) && *v == r).count();
            let min = allowed.iter().map(|r| count(r)).min().unwrap_or(0);
            let mut cands: Vec<&String> = allowed.iter().filter(|r| count(r) == min).collect();
            // among equally used groups prefer those that have sound clips
            if cands.iter().any(|r| self.scanned.contains(r)) {
                cands.retain(|r| self.scanned.contains(r));
            }
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos() as usize);
            let pick = cands[nanos % cands.len()].clone();
            eprintln!("claude-tabs: tab {} -> {}", id, pick);
            if !self.set_race(id, &pick, false) {
                self.read_races(); // already assigned elsewhere
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
            self.assign_races();
        }
    }

    // State with the subagent rule applied: a Stop that left subagents running is not a finish. It stays
    // `Working` while subagents run and for `subagent_grace_s` after the last one stopped (the main agent
    // is normally woken and ends with a Stop of its own); only then it counts as finished.
    fn eff_state(&self, pane: u32, f: &PaneState) -> St {
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
            St::Working => 2,
            St::Done => 1,
            St::Idle => 0,
        };
        for p in panes.iter().filter(|p| !p.is_plugin) {
            let cand = if let Some(f) = self.files.get(&p.id) {
                count += f.subs;
                Some((self.eff_state(p.id, f), f.ns))
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
        for (pane, f) in self.files.iter() {
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
            self.reacted.insert(pane, ns);
            let focused = Some(tab) == active;
            if !focused || self.ding_when_focused || (self.ding_when_os_unfocused && away) {
                let race = self.tabs.iter().find(|t| t.position == tab).and_then(|t| self.races.get(&t.tab_id)).cloned();
                self.ding(race.as_deref());
            }
            if self.auto_switch && !focused {
                switch_tab_to(tab as u32 + 1);
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

    fn snapshot_age(&self) -> u64 {
        now_secs().saturating_sub(self.snap_at)
    }

    // Fresh tabs / panes of the current session straight from the server.
    fn fetch_snapshot(&mut self) -> bool {
        let Ok(snap) = get_session_list() else { return false };
        let Some(si) = snap.live_sessions.into_iter().find(|s| s.is_current_session) else { return false };
        self.snap_at = now_secs();
        self.session = si.name.clone();
        self.tabs = si.tabs;
        self.manifest = Some(si.panes);
        // live claude-tabs panes only (the server's plugin table can still list instances from before a reload)
        self.plugin_ids = self
            .manifest
            .as_ref()
            .map(|m| {
                m.panes
                    .values()
                    .flatten()
                    .filter(|p| p.is_plugin && (p.title.contains("claude-tabs") || p.plugin_url.as_deref().map_or(false, |u| u.contains("claude-tabs"))))
                    .map(|p| p.id)
                    .collect()
            })
            .unwrap_or_default();
        true
    }

    // exactly one instance acts: the one with the lowest plugin id
    fn is_leader(&self) -> bool {
        self.plugin_ids.iter().min().map_or(false, |m| *m == self.my_id)
    }

    fn prime(&mut self) {
        for (pane, f) in self.files.iter() {
            self.reacted.insert(*pane, f.ns);
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
        let stale_races: Vec<usize> = self.races.keys().filter(|k| !live_tabs.contains(k)).cloned().collect();
        for k in stale_races {
            let _ = std::fs::remove_file(format!("{}/{}", self.races_dir(), k));
            self.races.remove(&k);
        }
        for k in dead {
            let dir = format!("{}/{}", STATE_ROOT, self.session);
            let _ = std::fs::remove_file(format!("{}/{}", dir, k));
            let _ = std::fs::remove_dir_all(format!("{}/{}.sub", dir, k));
            let _ = std::fs::remove_file(format!("{}/{}.ev", dir, k));
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

    // Play a random clip for the race (play-phrase.sh); fall back to ding_command when there is none.
    fn ding(&self, race: Option<&str>) {
        if !self.perms {
            return;
        }
        let ctx = BTreeMap::from([("k".to_string(), "ding".to_string())]);
        let default_ding = format!("\"{}\" --ding \"{}\"", expand_home(&self.phrase_script), expand_home(&self.player));
        let fallback = if self.ding_command.is_empty() {
            if self.phrase_script.is_empty() { "true" } else { default_ding.as_str() }
        } else {
            self.ding_command.as_str()
        };
        let cmd = match race {
            Some(r) if !self.phrase_script.is_empty() && valid_race(r) => {
                let q = |v: &str| format!("\"{}\"", expand_home(v));
                format!("{} {} {} {} || {{ {}; }}", q(&self.phrase_script), q(&self.sounds_dir), r, q(&self.player), fallback)
            }
            _ => fallback.to_string(),
        };
        run_command(&["sh", "-c", &cmd], ctx);
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

fn expand_home(v: &str) -> String {
    // resolved by the shell: ~ becomes $HOME (inside double quotes only $HOME works)
    match v.strip_prefix('~') {
        Some(rest) => format!("$HOME{}", rest),
        None => v.to_string(),
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

impl ZellijPlugin for State {
    fn load(&mut self, cfg: BTreeMap<String, String>) {
        self.ding_command = cfg.get("ding_command").cloned().unwrap_or_default();
        self.ding_when_focused = truthy(cfg.get("ding_when_focused"), false);
        self.ding_when_os_unfocused = truthy(cfg.get("ding_when_os_unfocused"), true);
        self.auto_switch = truthy(cfg.get("auto_switch"), false);
        self.show_icon_fallback = truthy(cfg.get("title_fallback"), true);
        self.auto_name = truthy(cfg.get("auto_name"), true);
        self.perms = true; // PermissionRequestResult is not re-sent to a reloaded instance; only a denial turns this off
        self.clock = truthy(cfg.get("clock"), true);
        self.race_icons = truthy(cfg.get("race_icons"), true);
        self.race_tab_colors = truthy(cfg.get("race_tab_colors"), true);
        self.show_other_clients = truthy(cfg.get("show_other_clients"), false);
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
        self.race_all_tabs = truthy(cfg.get("race_all_tabs"), false);
        self.races_opt = cfg
            .get("races")
            .map(|v| v.split(|c: char| c == ',' || c.is_whitespace()).filter(|x| valid_race(x)).map(String::from).collect::<Vec<_>>())
            .unwrap_or_default();
        self.sounds_dir = cfg.get("sounds_dir").cloned().unwrap_or_else(|| DEFAULT_SOUNDS_DIR.to_string());
        let pkg = cfg.get("package_dir").map(|v| v.trim_end_matches('/').to_string()).filter(|v| !v.is_empty());
        let pkg = pkg.unwrap_or_else(|| DEFAULT_PACKAGE_DIR.to_string());
        // an explicit empty phrase_script disables clips (only ding_command); unset = <package_dir>/play-phrase.sh
        self.phrase_script = cfg.get("phrase_script").cloned().unwrap_or_else(|| format!("{}/play-phrase.sh", pkg));
        self.player = cfg.get("player").cloned().unwrap_or_default(); // empty = autodetect in play-phrase.sh
        self.my_id = get_plugin_ids().plugin_id;
        set_selectable(false);
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
        match event {
            Event::PermissionRequestResult(s) => {
                self.perms = s == PermissionStatus::Granted;
                self.ask_tz();
                self.ask_scan();
                self.ask_os();
                self.refresh_names();
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
                self.refresh();
                changed
            }
            Event::PaneUpdate(m) => {
                self.manifest = Some(m);
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
                self.last_next_ms = (next * 1000.0) as u64;
                set_timeout(next);
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
                    Some("scan") => {
                        let mut v: Vec<String> = String::from_utf8_lossy(&out)
                            .lines()
                            .map(|l| l.trim().to_string())
                            .filter(|l| valid_race(l) && !l.starts_with('_')) // `_excluded` etc. are not races
                            .collect();
                        v.sort();
                        eprintln!("claude-tabs: scan found {:?}", v);
                        redraw = v != self.scanned;
                        self.scanned = v;
                        self.scan_done = true;
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
            // manual test: payload = race / group name, or `active` for the active tab's race.
            // `zellij pipe` also sends an empty message at stdin EOF: ignore it.
            if msg.payload.as_deref().map_or(true, |p| p.trim().is_empty()) {
                return false;
            }
            if self.fetch_snapshot() && self.is_leader() {
                // payload = race name; empty = the active tab's race
                let given = msg.payload.as_deref().map(str::trim).filter(|r| valid_race(r)).map(String::from);
                let race = given.or_else(|| {
                    let t = self.tabs.iter().find(|t| t.active)?;
                    self.races.get(&t.tab_id).cloned()
                });
                self.ding(race.as_deref());
            }
            return false;
        }
        if msg.name == "claude_reshuffle" {
            // `claude-ctl.sh reshuffle`: forget all assignments and hand them out again (unique first)
            if msg.payload.as_deref().map_or(false, |p| p.trim() == "now") && self.fetch_snapshot() && self.is_leader() {
                if let Ok(rd) = std::fs::read_dir(self.races_dir()) {
                    for e in rd.flatten() {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
                self.races.clear();
                self.assign_races();
            }
            return true;
        }
        if msg.name == "claude_race" {
            // `zellij pipe --name claude_race -- orc` sets the race of the active tab
            let r = msg.payload.as_deref().map(str::trim).unwrap_or("");
            if valid_race(r) && self.fetch_snapshot() {
                self.read_races();
                if let Some(id) = self.tabs.iter().find(|t| t.active).map(|t| t.tab_id) {
                    self.set_race(id, r, true);
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
        if st != St::Done {
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
            self.files.get(&p.id).map_or(false, |f| f.away && self.eff_state(p.id, f) == St::Done)
        })
    }

    // script next to phrase_script (the package dir)
    fn sibling_script(&self, name: &str) -> String {
        match self.phrase_script.rfind('/') {
            Some(i) => format!("{}/{}", &self.phrase_script[..i], name),
            None => name.to_string(),
        }
    }

    // Ask os-focus.sh whether the terminal window of this session has the OS focus (every 2 s at most).
    fn ask_os_focus(&mut self) {
        if !self.perms || self.phrase_script.is_empty() {
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
            &["sh", "-c", "s=$1; case $s in '~/'*) s=$HOME/${s#'~/'};; esac; exec \"$s\" \"$2\"", "_", &script, &self.session],
            ctx,
        );
    }

    fn in_focused_tab(&self) -> bool {
        // NOTE get_focused_pane_info() returns the tab ID, the manifest is keyed by tab POSITION; they
        // differ as soon as a tab was closed, so compare ids
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
        if !self.perms || self.phrase_script.is_empty() {
            return;
        }
        let now = now_secs();
        if now.saturating_sub(self.agents_asked) < 3 {
            return;
        }
        self.agents_asked = now;
        let script = if self.agents_script.is_empty() {
            match self.phrase_script.rfind('/') {
                Some(i) => format!("{}/claude-agents.sh", &self.phrase_script[..i]),
                None => "claude-agents.sh".to_string(),
            }
        } else {
            self.agents_script.clone()
        };
        for (pane, f) in self.files.iter() {
            if let Some(dir) = &f.sess {
                let ctx = BTreeMap::from([("k".to_string(), "agents".to_string()), ("pane".to_string(), pane.to_string())]);
                run_command(
                    &["sh", "-c", "s=$1; case $s in '~/'*) s=$HOME/${s#'~/'};; esac; exec \"$s\" \"$2\"", "_", &script, dir],
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
            let icon = claude.or(if self.race_all_tabs && self.races.contains_key(&t.tab_id) { Some((St::Idle, 0, 0)) } else { None }).map(|(st, _ns, count)| {
                let is_claude = claude.is_some();
                let mut glyph = if is_claude {
                    if self.spinner && st == St::Working { SPIN[self.spin_frame()].to_string() } else { ICON.to_string() }
                } else {
                    String::new()
                };
                if count > 0 && self.subagent_count {
                    glyph.push_str(&count.to_string());
                }
                let color = match st {
                    St::Working => ribbon.emphasis_0,
                    St::Done => ribbon.emphasis_2,
                    St::Idle => ribbon.base,
                };
                let race = if self.race_icons {
                    self.races.get(&t.tab_id).map(|r| {
                        let (g, rgb) = race_style(r, self.race_font_on(), self.os == "Darwin");
                        (g, blend(rgb, ribbon.base))
                    })
                } else {
                    None
                };
                let state = match st {
                    St::Working => IconState::Working,
                    St::Done => IconState::Done,
                    St::Idle => IconState::Idle,
                };
                let tint = if self.race_tab_colors && race.is_some() {
                    self.races.get(&t.tab_id).map(|r| race_style(r, false, false).1)
                } else {
                    None
                };
                Icon { glyph, color, race, state, tint, dim: self.race_dim, blink: self.tab_blink(t), underline: self.active_underline }
            });
            all.push(tab_style(name, icon, t, alt, colors, self.mode_info.capabilities, dimmed, self.show_other_clients));
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
            if let Some(r) = self.races.get(&t.tab_id) {
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

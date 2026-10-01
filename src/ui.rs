//! Drawing. The whole screen is built in memory first and written in one go,
//! so nothing flickers.

use crate::content::{all_species, Kind, FINAL_FLOOR};
use crate::game::{Game, ItemKind, Status, Tone};
use crate::map::{idx, Tile, MAP_H, MAP_W};
use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Attribute, Color, Print, SetAttribute, SetForegroundColor},
    terminal,
};
use std::io::{self, Write};

pub const MIN_W: u16 = 64;
pub const MIN_H: u16 = 22;
const SIDEBAR: i32 = 24;
const LOG_LINES: i32 = 4;

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    ch: char,
    fg: Color,
    bold: bool,
    dim: bool,
}

const BLANK: Cell = Cell {
    ch: ' ',
    fg: Color::Reset,
    bold: false,
    dim: false,
};

pub struct Frame {
    w: i32,
    h: i32,
    cells: Vec<Cell>,
}

impl Frame {
    pub fn new(w: i32, h: i32) -> Frame {
        Frame {
            w,
            h,
            cells: vec![BLANK; (w * h) as usize],
        }
    }

    fn put(&mut self, x: i32, y: i32, ch: char, fg: Color, bold: bool, dim: bool) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.cells[(y * self.w + x) as usize] = Cell { ch, fg, bold, dim };
        }
    }

    fn text(&mut self, x: i32, y: i32, s: &str, fg: Color, bold: bool, dim: bool) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as i32, y, ch, fg, bold, dim);
        }
    }

    /// Plain text of one row, for tests.
    #[cfg(test)]
    pub fn row(&self, y: i32) -> String {
        (0..self.w)
            .map(|x| self.cells[(y * self.w + x) as usize].ch)
            .collect()
    }

    pub fn write_to(&self, out: &mut impl Write) -> io::Result<()> {
        for y in 0..self.h {
            queue!(
                out,
                MoveTo(0, y as u16),
                SetAttribute(Attribute::Reset),
                SetForegroundColor(Color::Reset)
            )?;
            let mut run = String::new();
            let mut style: Option<(Color, bool, bool)> = None;
            for x in 0..self.w {
                let c = self.cells[(y * self.w + x) as usize];
                let s = (c.fg, c.bold, c.dim);
                if style != Some(s) {
                    if !run.is_empty() {
                        queue!(out, Print(&run))?;
                        run.clear();
                    }
                    queue!(
                        out,
                        SetAttribute(Attribute::Reset),
                        SetForegroundColor(c.fg)
                    )?;
                    if c.bold {
                        queue!(out, SetAttribute(Attribute::Bold))?;
                    }
                    if c.dim {
                        queue!(out, SetAttribute(Attribute::Dim))?;
                    }
                    style = Some(s);
                }
                run.push(c.ch);
            }
            if !run.is_empty() {
                queue!(out, Print(&run))?;
            }
        }
        queue!(
            out,
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::Reset)
        )?;
        out.flush()
    }
}

#[derive(Default)]
pub struct Ui {
    /// 0 = closed, 1 = controls and items, 2 = the bugs.
    pub help: u8,
    pub confirm_quit: bool,
}

fn monster_color(kind: Kind) -> Color {
    match kind {
        Kind::Typo => Color::Green,
        Kind::NullPointer => Color::Red,
        Kind::OffByOne => Color::Yellow,
        Kind::RaceCondition => Color::Magenta,
        Kind::MemoryLeak => Color::Cyan,
        Kind::Segfault => Color::DarkRed,
        Kind::Deadlock => Color::AnsiValue(39),
        Kind::LegacyCode => Color::AnsiValue(208),
    }
}

fn item_color(kind: ItemKind) -> Color {
    match kind {
        ItemKind::Coffee => Color::AnsiValue(172),
        ItemKind::Overflow => Color::Cyan,
        ItemKind::Refactor => Color::Green,
        ItemKind::UnitTest => Color::AnsiValue(75),
        ItemKind::CtrlZ => Color::Magenta,
    }
}

fn tone_color(tone: Tone) -> (Color, bool) {
    match tone {
        Tone::Info => (Color::Grey, false),
        Tone::Good => (Color::Green, false),
        Tone::Bad => (Color::Red, false),
        Tone::Item => (Color::Cyan, false),
        Tone::Danger => (Color::Yellow, true),
    }
}

fn camera(player: i32, view: i32, map: i32) -> i32 {
    if map <= view {
        -((view - map) / 2)
    } else {
        (player - view / 2).clamp(0, map - view)
    }
}

fn draw_map(f: &mut Frame, game: &Game, view_w: i32, view_h: i32) {
    let cx = camera(game.player.x, view_w, MAP_W);
    let cy = camera(game.player.y, view_h, MAP_H);
    for vy in 0..view_h {
        for vx in 0..view_w {
            let (mx, my) = (cx + vx, cy + vy);
            if mx < 0 || my < 0 || mx >= MAP_W || my >= MAP_H {
                continue;
            }
            let i = idx(mx, my);
            if !game.map.explored[i] {
                continue;
            }
            let seen = game.map.visible[i];
            let (ch, fg, bold, dim) = match game.map.tile(mx, my) {
                Tile::Wall => (
                    '#',
                    if seen { Color::Grey } else { Color::DarkGrey },
                    false,
                    !seen,
                ),
                Tile::Floor => (
                    '.',
                    if seen {
                        Color::DarkYellow
                    } else {
                        Color::DarkGrey
                    },
                    false,
                    !seen,
                ),
                Tile::Stairs => ('>', Color::Yellow, true, !seen),
            };
            f.put(vx, 1 + vy, ch, fg, bold, dim);
        }
    }
    for item in &game.items {
        let i = idx(item.x, item.y);
        if game.map.explored[i] {
            let (vx, vy) = (item.x - cx, item.y - cy);
            f.put(
                vx,
                1 + vy,
                item.kind.glyph(),
                item_color(item.kind),
                true,
                !game.map.visible[i],
            );
        }
    }
    for m in &game.monsters {
        if game.map.visible[idx(m.x, m.y)] {
            let (vx, vy) = (m.x - cx, m.y - cy);
            f.put(
                vx,
                1 + vy,
                crate::content::species(m.kind).glyph,
                monster_color(m.kind),
                true,
                false,
            );
        }
    }
    f.put(
        game.player.x - cx,
        1 + game.player.y - cy,
        '@',
        Color::White,
        true,
        false,
    );
}

fn draw_sidebar(f: &mut Frame, game: &Game, x0: i32, label: &str) {
    let p = &game.player;
    let mut y = 1;
    f.text(x0, y, "STACKDIVE", Color::Cyan, true, false);
    y += 2;
    f.text(
        x0,
        y,
        &format!("Floor {}/{}", game.floor, FINAL_FLOOR),
        Color::White,
        true,
        false,
    );
    y += 1;
    f.text(x0, y, game.floor_name(), Color::Yellow, false, false);
    y += 2;
    f.text(
        x0,
        y,
        &format!("HP {}/{}", p.hp, p.max_hp),
        Color::White,
        true,
        false,
    );
    y += 1;
    let width = 14;
    let filled = ((p.hp.max(0) * width + p.max_hp - 1) / p.max_hp).clamp(0, width);
    let color = if p.hp * 2 > p.max_hp {
        Color::Green
    } else if p.hp * 4 > p.max_hp {
        Color::Yellow
    } else {
        Color::Red
    };
    f.put(x0, y, '[', Color::DarkGrey, false, false);
    for i in 0..width {
        let (ch, c) = if i < filled {
            ('#', color)
        } else {
            ('-', Color::DarkGrey)
        };
        f.put(x0 + 1 + i, y, ch, c, false, false);
    }
    f.put(x0 + 1 + width, y, ']', Color::DarkGrey, false, false);
    y += 1;
    f.text(
        x0,
        y,
        &format!("Damage +{}", p.bonus),
        Color::Green,
        false,
        false,
    );
    y += 2;
    let coffee = if p.coffees > 0 {
        Color::AnsiValue(172)
    } else {
        Color::DarkGrey
    };
    f.text(
        x0,
        y,
        &format!("[c] Coffee x{}", p.coffees),
        coffee,
        false,
        false,
    );
    y += 1;
    let answer = if p.scrolls > 0 {
        Color::Cyan
    } else {
        Color::DarkGrey
    };
    f.text(
        x0,
        y,
        &format!("[o] Answer x{}", p.scrolls),
        answer,
        false,
        false,
    );
    y += 1;
    if p.ctrl_z {
        f.text(x0, y, "CTRL+Z ready", Color::Magenta, true, false);
    } else {
        f.text(x0, y, "CTRL+Z used", Color::DarkGrey, false, false);
    }
    y += 2;
    f.text(
        x0,
        y,
        &format!("Bugs   {}", game.kills),
        Color::Grey,
        false,
        false,
    );
    y += 1;
    f.text(
        x0,
        y,
        &format!("Turns  {}", game.turns),
        Color::Grey,
        false,
        false,
    );
    y += 2;
    f.text(x0, y, label, Color::DarkGrey, false, false);
}

fn draw_log(f: &mut Frame, game: &Game, top: i32, w: i32) {
    let shown: Vec<_> = game.log.iter().rev().take(LOG_LINES as usize).collect();
    for (i, msg) in shown.iter().enumerate() {
        let y = top + LOG_LINES - 1 - i as i32;
        let (color, bold) = tone_color(msg.tone);
        let text: String = msg.text.chars().take((w - 2).max(0) as usize).collect();
        f.text(1, y, &text, color, bold && i == 0, i > 1);
    }
}

fn overlay(f: &mut Frame, title: &str, lines: &[(String, Color)]) {
    let inner = lines
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(title.chars().count()) as i32;
    let w = inner + 4;
    let h = lines.len() as i32 + 4;
    let x0 = ((f.w - w) / 2).max(0);
    let y0 = ((f.h - h) / 2).max(0);
    for y in 0..h {
        for x in 0..w {
            let edge_x = x == 0 || x == w - 1;
            let edge_y = y == 0 || y == h - 1;
            let ch = match (edge_x, edge_y) {
                (true, true) => '+',
                (true, false) => '|',
                (false, true) => '-',
                _ => ' ',
            };
            f.put(x0 + x, y0 + y, ch, Color::Grey, false, false);
        }
    }
    f.text(x0 + 2, y0, &format!(" {title} "), Color::Cyan, true, false);
    for (i, (line, color)) in lines.iter().enumerate() {
        f.text(x0 + 2, y0 + 2 + i as i32, line, *color, false, false);
    }
}

fn help_bugs() -> Vec<(String, Color)> {
    let mut v = Vec::new();
    for s in all_species() {
        v.push((
            format!("{}  {:<15} {}", s.glyph, s.name, s.blurb),
            monster_color(s.kind),
        ));
    }
    v.push((String::new(), Color::Grey));
    v.push(("Press any key to go back.".to_string(), Color::DarkGrey));
    v
}

fn help_controls() -> Vec<(String, Color)> {
    let g = Color::Grey;
    let mut v = vec![
        ("Move       arrows, h j k l or w a s d".to_string(), g),
        ("Diagonal   y u b n".to_string(), g),
        ("Wait       . or space".to_string(), g),
        ("Stairs     >  (stand on the > first)".to_string(), g),
        ("Coffee     c   heals 8 HP".to_string(), g),
        ("Answer     o   reveals the whole floor".to_string(), g),
        ("Quit       q".to_string(), g),
        (String::new(), g),
        ("Walk into a bug to hit it.".to_string(), Color::DarkGrey),
        (String::new(), g),
    ];
    for k in [
        ItemKind::Coffee,
        ItemKind::Overflow,
        ItemKind::Refactor,
        ItemKind::UnitTest,
        ItemKind::CtrlZ,
    ] {
        let what = match k {
            ItemKind::Coffee => "heals you later",
            ItemKind::Overflow => "reveals the floor",
            ItemKind::Refactor => "+1 damage, for good",
            ItemKind::UnitTest => "+3 max HP, for good",
            ItemKind::CtrlZ => "undoes one death",
        };
        v.push((
            format!("{}  {:<15} {}", k.glyph(), k.name(), what),
            item_color(k),
        ));
    }
    v.push((String::new(), g));
    v.push((
        "Press any key to see the bugs.".to_string(),
        Color::DarkGrey,
    ));
    v
}

/// Builds one screen. `size` is the terminal size.
pub fn render(game: &Game, ui: &Ui, label: &str, demo: bool, size: (u16, u16)) -> Frame {
    let (w, h) = (i32::from(size.0), i32::from(size.1));
    let mut f = Frame::new(w, h);
    if size.0 < MIN_W || size.1 < MIN_H {
        f.text(
            1,
            1,
            "The terminal is too small.",
            Color::Yellow,
            true,
            false,
        );
        f.text(
            1,
            2,
            &format!("Needs {MIN_W}x{MIN_H}, has {w}x{h}."),
            Color::Grey,
            false,
            false,
        );
        return f;
    }
    let view_w = w - SIDEBAR - 1;
    let view_h = h - 2 - LOG_LINES;
    draw_map(&mut f, game, view_w, view_h);
    draw_sidebar(&mut f, game, view_w + 2, label);
    for y in 0..=view_h {
        f.put(view_w, y, '|', Color::DarkGrey, false, true);
    }
    let log_top = h - 1 - LOG_LINES;
    draw_log(&mut f, game, log_top, w);
    let hints: &[&str] = if demo {
        &["A bot is playing.  Press q to stop."]
    } else {
        &[
            "arrows/hjkl move   yubn diagonal   > stairs   c coffee   o answer   . wait   ? help   q quit",
            "arrows move   > stairs   c coffee   o answer   ? help   q quit",
            "? help   q quit",
        ]
    };
    let hint = hints
        .iter()
        .find(|h| h.len() as i32 <= w - 2)
        .unwrap_or(&"? help");
    f.text(1, h - 1, hint, Color::DarkGrey, false, false);

    match &game.status {
        Status::Dead(by) => {
            let lines = vec![
                (
                    format!(
                        "Floor {}/{}: {}",
                        game.floor,
                        FINAL_FLOOR,
                        game.floor_name()
                    ),
                    Color::Grey,
                ),
                (
                    format!("{} bugs squashed in {} turns", game.kills, game.turns),
                    Color::Grey,
                ),
                (format!("Killed by {by}"), Color::Red),
                (String::new(), Color::Grey),
                ("Press any key.".to_string(), Color::DarkGrey),
            ];
            overlay(&mut f, "YOU DIED", &lines);
        }
        Status::Won => {
            let lines = vec![
                ("The Legacy Code is refactored.".to_string(), Color::Green),
                (
                    format!("{} bugs squashed in {} turns", game.kills, game.turns),
                    Color::Grey,
                ),
                (String::new(), Color::Grey),
                ("Press any key.".to_string(), Color::DarkGrey),
            ];
            overlay(&mut f, "YOU WON", &lines);
        }
        Status::Playing => {
            if ui.help == 1 {
                overlay(&mut f, "HELP 1/2", &help_controls());
            } else if ui.help == 2 {
                overlay(&mut f, "HELP 2/2: THE BUGS", &help_bugs());
            } else if ui.confirm_quit {
                let lines = vec![("Quit this run?  y / n".to_string(), Color::Yellow)];
                overlay(&mut f, "QUIT", &lines);
            }
        }
    }
    f
}

pub fn draw(out: &mut impl Write, game: &Game, ui: &Ui, label: &str, demo: bool) -> io::Result<()> {
    let size = terminal::size().unwrap_or((80, 24));
    render(game, ui, label, demo, size).write_to(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    fn screen(f: &Frame) -> String {
        (0..f.h).map(|y| f.row(y)).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn the_main_screen_has_everything_on_it() {
        let g = Game::new(1);
        let f = render(&g, &Ui::default(), "seed 1", false, (80, 24));
        let text = screen(&f);
        assert!(text.contains("STACKDIVE"));
        assert!(text.contains("Floor 1/8"));
        assert!(text.contains("Frontend"));
        assert!(text.contains("HP 20/20"));
        assert!(text.contains("seed 1"));
        assert!(text.contains('@'), "the player is on the map");
        assert!(text.contains("The build is broken"));
    }

    #[test]
    fn it_works_at_every_size_from_the_smallest_up() {
        let g = Game::new(2);
        for (w, h) in [(64, 22), (80, 24), (100, 30), (200, 60), (120, 22)] {
            let f = render(&g, &Ui::default(), "seed 2", false, (w, h));
            assert!(screen(&f).contains('@'), "no player at {w}x{h}");
        }
    }

    #[test]
    fn a_small_terminal_gets_a_polite_message() {
        let g = Game::new(3);
        let text = screen(&render(&g, &Ui::default(), "seed 3", false, (40, 10)));
        assert!(text.contains("too small"));
        assert!(text.contains("64x22"));
    }

    #[test]
    fn the_map_follows_the_player() {
        let mut g = Game::new(4);
        g.map.reveal_all();
        let spots = [(2, 2), (MAP_W - 3, MAP_H - 3)];
        for (x, y) in spots {
            if g.map.passable(x, y) {
                g.player.x = x;
                g.player.y = y;
                assert!(screen(&render(&g, &Ui::default(), "s", false, (80, 24))).contains('@'));
            }
        }
        // And a spot far from the corner, wherever the player is on the map.
        let (rx, ry) = g.map.rooms[g.map.rooms.len() / 2].center();
        g.player.x = rx;
        g.player.y = ry;
        assert!(screen(&render(&g, &Ui::default(), "s", false, (70, 24))).contains('@'));
    }

    #[test]
    fn overlays_show_up() {
        let mut g = Game::new(5);
        let page1 = screen(&render(
            &g,
            &Ui {
                help: 1,
                confirm_quit: false,
            },
            "s",
            false,
            (80, 24),
        ));
        assert!(page1.contains("HELP 1/2"));
        assert!(page1.contains("CTRL+Z"));
        assert!(
            page1.contains("Press any key to see the bugs."),
            "page 1 fits on 24 rows"
        );
        let page2 = screen(&render(
            &g,
            &Ui {
                help: 2,
                confirm_quit: false,
            },
            "s",
            false,
            (80, 24),
        ));
        assert!(page2.contains("Race Condition"));
        assert!(page2.contains("Legacy Code"));
        assert!(
            page2.contains("Press any key to go back."),
            "page 2 fits on 24 rows"
        );
        let quit = render(
            &g,
            &Ui {
                help: 0,
                confirm_quit: true,
            },
            "s",
            false,
            (80, 24),
        );
        assert!(screen(&quit).contains("Quit this run?"));
        g.status = Status::Dead("a Typo".to_string());
        assert!(screen(&render(&g, &Ui::default(), "s", false, (80, 24))).contains("YOU DIED"));
        g.status = Status::Won;
        assert!(screen(&render(&g, &Ui::default(), "s", false, (80, 24))).contains("YOU WON"));
    }

    /// The part of the screen that is the map (no sidebar, no log).
    fn map_part(f: &Frame) -> String {
        let view_w = (f.w - SIDEBAR - 1) as usize;
        (1..f.h - 1 - LOG_LINES)
            .map(|y| f.row(y).chars().take(view_w).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn unseen_monsters_are_hidden_and_seen_ones_are_drawn() {
        let mut g = Game::new(6);
        let (px, py) = (g.player.x, g.player.y);
        g.monsters.clear();
        // Somewhere the player cannot see: far away, in another room.
        let far = g.map.rooms.last().expect("rooms").center();
        assert!(!g.map.visible[idx(far.0, far.1)]);
        let mut boss = crate::game::Monster::for_test(Kind::LegacyCode, far.0, far.1);
        boss.awake = false;
        g.monsters.push(boss);
        let hidden = render(&g, &Ui::default(), "s", false, (80, 24));
        assert!(
            !map_part(&hidden).contains('L'),
            "an unseen monster must not be drawn"
        );
        // The same monster right next to the player is drawn.
        let next = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|(dx, dy)| (px + dx, py + dy))
            .find(|&(x, y)| g.map.passable(x, y))
            .expect("open neighbor");
        g.monsters[0].x = next.0;
        g.monsters[0].y = next.1;
        let seen = render(&g, &Ui::default(), "s", false, (80, 24));
        assert!(
            map_part(&seen).contains('L'),
            "a visible monster must be drawn"
        );
    }

    #[test]
    fn the_hint_line_is_never_cut_off() {
        let g = Game::new(7);
        for w in [64u16, 70, 78, 80, 90, 100, 140] {
            let f = render(&g, &Ui::default(), "s", false, (w, 24));
            let last = f.row(23);
            let text = last.trim_end();
            assert!(
                text.len() < usize::from(w),
                "hint fills the line at width {w}: {text:?}"
            );
            assert!(
                text.ends_with("quit") || text.ends_with("quit"),
                "hint is cut at width {w}: {text:?}"
            );
        }
    }
}

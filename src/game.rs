//! The rules. Nothing in here knows about terminals, so it can be tested
//! and played by a bot.

use crate::content::{self, species, Kind, FINAL_FLOOR, FLOOR_NAMES};
use crate::map::{idx, Map, Pos, DIRS8, MAP_H, MAP_W};
use crate::rng::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Info,
    Good,
    Bad,
    Item,
    Danger,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub text: String,
    pub tone: Tone,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Move(i32, i32),
    Wait,
    Descend,
    Coffee,
    Scroll,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Status {
    Playing,
    Dead(String),
    Won,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Coffee,
    Overflow,
    Refactor,
    UnitTest,
    CtrlZ,
}

impl ItemKind {
    pub fn glyph(self) -> char {
        match self {
            ItemKind::Coffee => '!',
            ItemKind::Overflow => '?',
            ItemKind::Refactor => '+',
            ItemKind::UnitTest => ']',
            ItemKind::CtrlZ => 'z',
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ItemKind::Coffee => "Coffee",
            ItemKind::Overflow => "Stack Overflow answer",
            ItemKind::Refactor => "Refactor",
            ItemKind::UnitTest => "Unit Test",
            ItemKind::CtrlZ => "CTRL+Z",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub kind: ItemKind,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug)]
pub struct Player {
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub bonus: i32,
    pub coffees: i32,
    pub scrolls: i32,
    pub ctrl_z: bool,
    pub stunned: bool,
}

#[derive(Clone, Debug)]
pub struct Monster {
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub awake: bool,
    energy: i32,
    age: i32,
}

const START_HP: i32 = 20;
const COFFEE_HEAL: i32 = 8;
const PLAYER_HIT: i32 = 90;
const MONSTER_HIT: i32 = 85;
const MAX_LOG: usize = 200;

struct Floor {
    map: Map,
    start: Pos,
    exit: Pos,
    monsters: Vec<Monster>,
    items: Vec<Item>,
}

pub struct Game {
    pub seed: u64,
    pub floor: u32,
    pub map: Map,
    pub monsters: Vec<Monster>,
    pub items: Vec<Item>,
    pub player: Player,
    pub log: Vec<Message>,
    pub kills: u32,
    pub turns: u32,
    pub status: Status,
    pub exit: Pos,
    combat: Rng,
    ctrlz_floor: u32,
}

impl Monster {
    #[cfg(test)]
    pub fn for_test(kind: Kind, x: i32, y: i32) -> Monster {
        new_monster(kind, x, y)
    }
}

fn new_monster(kind: Kind, x: i32, y: i32) -> Monster {
    Monster {
        kind,
        x,
        y,
        hp: species(kind).hp,
        awake: false,
        energy: 0,
        age: 0,
    }
}

/// A random floor tile in a random room that nobody uses yet.
fn free_spot(
    rng: &mut Rng,
    map: &Map,
    taken: &[Pos],
    skip_first_room: bool,
    far_from: Option<(Pos, i32)>,
) -> Option<Pos> {
    let first = usize::from(skip_first_room);
    for _ in 0..60 {
        let ri = rng.range(first as i32, map.rooms.len() as i32 - 1) as usize;
        let r = map.rooms[ri];
        let x = rng.range(r.x, r.x + r.w - 1);
        let y = rng.range(r.y, r.y + r.h - 1);
        if map.tile(x, y) != crate::map::Tile::Floor || taken.contains(&(x, y)) {
            continue;
        }
        if let Some((from, min)) = far_from {
            if (x - from.0).abs().max((y - from.1).abs()) < min {
                continue;
            }
        }
        return Some((x, y));
    }
    None
}

fn build_floor(seed: u64, floor: u32, ctrlz_floor: u32) -> Floor {
    let last = floor == FINAL_FLOOR;
    let mut rng = Rng::child(seed, u64::from(floor));
    let g = Map::generate(&mut rng, last);
    let mut taken: Vec<Pos> = vec![g.start, g.exit];
    let mut monsters = Vec::new();
    let mut items = Vec::new();

    let pool = content::spawnable(floor);
    let total: i32 = pool.iter().map(|s| s.weight).sum();
    let count = 4 + floor as i32;
    for _ in 0..count {
        let mut roll = rng.range(1, total);
        let mut chosen = pool[0];
        for s in &pool {
            if roll <= s.weight {
                chosen = s;
                break;
            }
            roll -= s.weight;
        }
        if let Some(p) = free_spot(&mut rng, &g.map, &taken, true, Some((g.start, 7))) {
            taken.push(p);
            monsters.push(new_monster(chosen.kind, p.0, p.1));
        }
    }
    if last {
        monsters.push(new_monster(Kind::LegacyCode, g.exit.0, g.exit.1));
    }

    let mut drops: Vec<ItemKind> = Vec::new();
    drops.push(ItemKind::Coffee);
    if rng.chance(40) {
        drops.push(ItemKind::Coffee);
    }
    if rng.chance(40) {
        drops.push(ItemKind::Overflow);
    }
    if rng.chance(30) {
        drops.push(ItemKind::Refactor);
    }
    if rng.chance(35) {
        drops.push(ItemKind::UnitTest);
    }
    if floor == ctrlz_floor {
        drops.push(ItemKind::CtrlZ);
    }
    for kind in drops {
        if let Some(p) = free_spot(&mut rng, &g.map, &taken, false, None) {
            taken.push(p);
            items.push(Item {
                kind,
                x: p.0,
                y: p.1,
            });
        }
    }
    Floor {
        map: g.map,
        start: g.start,
        exit: g.exit,
        monsters,
        items,
    }
}

impl Game {
    pub fn new(seed: u64) -> Game {
        let ctrlz_floor = 3 + Rng::child(seed, 2000).range(0, 2) as u32;
        let f = build_floor(seed, 1, ctrlz_floor);
        let mut game = Game {
            seed,
            floor: 1,
            map: f.map,
            monsters: f.monsters,
            items: f.items,
            player: Player {
                x: f.start.0,
                y: f.start.1,
                hp: START_HP,
                max_hp: START_HP,
                bonus: 0,
                coffees: 0,
                scrolls: 0,
                ctrl_z: false,
                stunned: false,
            },
            log: Vec::new(),
            kills: 0,
            turns: 0,
            status: Status::Playing,
            exit: f.exit,
            combat: Rng::child(seed, 1000),
            ctrlz_floor,
        };
        game.map.update_fov((game.player.x, game.player.y));
        game.say(Tone::Info, "The build is broken. You are the developer.");
        game.say(Tone::Info, "Descend through the stack and squash the bugs.");
        game
    }

    pub fn say(&mut self, tone: Tone, text: &str) {
        self.log.push(Message {
            text: text.to_string(),
            tone,
        });
        if self.log.len() > MAX_LOG {
            self.log.remove(0);
        }
    }

    pub fn floor_name(&self) -> &'static str {
        FLOOR_NAMES[(self.floor - 1) as usize]
    }

    pub fn monster_at(&self, x: i32, y: i32) -> Option<usize> {
        self.monsters.iter().position(|m| m.x == x && m.y == y)
    }

    pub fn item_at(&self, x: i32, y: i32) -> Option<usize> {
        self.items.iter().position(|i| i.x == x && i.y == y)
    }

    pub fn on_stairs(&self) -> bool {
        self.map.tile(self.player.x, self.player.y) == crate::map::Tile::Stairs
    }

    /// Does one thing. Returns true if it used up a turn or changed the floor.
    pub fn act(&mut self, action: Action) -> bool {
        if self.status != Status::Playing {
            return false;
        }
        if action == Action::Descend {
            return self.descend();
        }
        if self.player.stunned {
            self.player.stunned = false;
            self.say(Tone::Bad, "You are deadlocked and lose your turn.");
            self.end_turn();
            return true;
        }
        let used = match action {
            Action::Wait => true,
            Action::Move(dx, dy) => self.try_move(dx, dy),
            Action::Coffee => self.drink_coffee(),
            Action::Scroll => self.read_scroll(),
            Action::Descend => unreachable!(),
        };
        if used && self.status == Status::Playing {
            self.end_turn();
        }
        used
    }

    fn descend(&mut self) -> bool {
        if !self.on_stairs() {
            self.say(Tone::Info, "There are no stairs here.");
            return false;
        }
        self.floor += 1;
        let f = build_floor(self.seed, self.floor, self.ctrlz_floor);
        self.map = f.map;
        self.monsters = f.monsters;
        self.items = f.items;
        self.exit = f.exit;
        self.player.x = f.start.0;
        self.player.y = f.start.1;
        self.map.update_fov(f.start);
        let line = format!("Floor {}: {}.", self.floor, self.floor_name());
        self.say(Tone::Good, &line);
        if self.floor == FINAL_FLOOR {
            self.say(
                Tone::Danger,
                "The Legacy Code is here. Nobody knows what it does.",
            );
        }
        true
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        if (dx, dy) == (0, 0) {
            return false;
        }
        let (nx, ny) = (self.player.x + dx, self.player.y + dy);
        if let Some(i) = self.monster_at(nx, ny) {
            self.player_attack(i);
            return true;
        }
        if !self.map.passable(nx, ny) {
            return false;
        }
        self.player.x = nx;
        self.player.y = ny;
        if let Some(i) = self.item_at(nx, ny) {
            let item = self.items.remove(i);
            self.pick_up(item.kind);
        }
        self.map.update_fov((nx, ny));
        if self.on_stairs() {
            self.say(Tone::Info, "Stairs down. Press > to descend.");
        }
        true
    }

    fn pick_up(&mut self, kind: ItemKind) {
        match kind {
            ItemKind::Coffee => {
                self.player.coffees += 1;
                self.say(Tone::Item, "You find a Coffee. Press c to drink it.");
            }
            ItemKind::Overflow => {
                self.player.scrolls += 1;
                self.say(
                    Tone::Item,
                    "You find a Stack Overflow answer. Press o to use it.",
                );
            }
            ItemKind::Refactor => {
                self.player.bonus += 1;
                self.say(Tone::Item, "You refactor your code. +1 damage.");
            }
            ItemKind::UnitTest => {
                self.player.max_hp += 3;
                self.player.hp += 3;
                self.say(Tone::Item, "You write a Unit Test. +3 max HP.");
            }
            ItemKind::CtrlZ => {
                self.player.ctrl_z = true;
                self.say(Tone::Item, "You find CTRL+Z. One undo if you die.");
            }
        }
    }

    fn drink_coffee(&mut self) -> bool {
        if self.player.coffees == 0 {
            self.say(Tone::Info, "You have no coffee.");
            return false;
        }
        if self.player.hp >= self.player.max_hp {
            self.say(Tone::Info, "You are not tired yet.");
            return false;
        }
        self.player.coffees -= 1;
        let before = self.player.hp;
        self.player.hp = (self.player.hp + COFFEE_HEAL).min(self.player.max_hp);
        let line = format!("You drink a coffee. +{} HP.", self.player.hp - before);
        self.say(Tone::Good, &line);
        true
    }

    fn read_scroll(&mut self) -> bool {
        if self.player.scrolls == 0 {
            self.say(Tone::Info, "You have no Stack Overflow answer.");
            return false;
        }
        self.player.scrolls -= 1;
        self.map.reveal_all();
        self.say(
            Tone::Good,
            "You paste the answer without reading it. The map is revealed.",
        );
        true
    }

    fn player_attack(&mut self, i: usize) {
        let name = species(self.monsters[i].kind).name;
        if !self.combat.chance(PLAYER_HIT) {
            self.say(Tone::Info, &format!("You miss the {name}."));
            self.monsters[i].awake = true;
            return;
        }
        let dmg = self.combat.range(2, 4) + self.player.bonus;
        self.monsters[i].hp -= dmg;
        self.monsters[i].awake = true;
        if self.monsters[i].hp > 0 {
            self.say(Tone::Info, &format!("You hit the {name} for {dmg}."));
            return;
        }
        let kind = self.monsters[i].kind;
        self.monsters.remove(i);
        self.kills += 1;
        self.say(Tone::Good, species(kind).slain);
        if kind == Kind::LegacyCode {
            self.status = Status::Won;
        }
    }

    fn end_turn(&mut self) {
        self.turns += 1;
        if self.turns.is_multiple_of(15) && self.player.hp < self.player.max_hp {
            self.player.hp += 1;
        }
        let dist = self.map.distances((self.player.x, self.player.y));
        let count = self.monsters.len();
        for i in 0..count {
            if self.status != Status::Playing {
                break;
            }
            self.monsters[i].energy += species(self.monsters[i].kind).speed;
            while self.monsters[i].energy >= 100 && self.status == Status::Playing {
                self.monsters[i].energy -= 100;
                self.monster_act(i, &dist);
            }
        }
    }

    fn monster_act(&mut self, i: usize, dist: &[i32]) {
        let (mx, my) = (self.monsters[i].x, self.monsters[i].y);
        let kind = self.monsters[i].kind;
        self.monsters[i].age += 1;
        let age = self.monsters[i].age;
        if kind == Kind::MemoryLeak && age % 4 == 0 && self.monsters[i].hp < species(kind).hp {
            self.monsters[i].hp += 1;
        }
        if !self.monsters[i].awake {
            if self.map.visible[idx(mx, my)] {
                self.monsters[i].awake = true;
                let line = format!("The {} notices you.", species(kind).name);
                self.say(Tone::Danger, &line);
            }
            return;
        }
        if kind == Kind::LegacyCode && age % 7 == 0 {
            self.spawn_minion(i);
        }
        let (px, py) = (self.player.x, self.player.y);
        if (mx - px).abs().max((my - py).abs()) == 1 {
            self.monster_attack(i);
            return;
        }
        let here = dist[idx(mx, my)];
        if here < 0 {
            return;
        }
        let mut best: Option<(i32, Pos)> = None;
        for (dx, dy) in DIRS8 {
            let (nx, ny) = (mx + dx, my + dy);
            if !self.map.passable(nx, ny)
                || (nx, ny) == (px, py)
                || self.monster_at(nx, ny).is_some()
            {
                continue;
            }
            let d = dist[idx(nx, ny)];
            if d >= 0 && d < here && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, (nx, ny)));
            }
        }
        if let Some((_, (nx, ny))) = best {
            self.monsters[i].x = nx;
            self.monsters[i].y = ny;
        }
    }

    fn spawn_minion(&mut self, boss: usize) {
        let (bx, by) = (self.monsters[boss].x, self.monsters[boss].y);
        for (dx, dy) in DIRS8 {
            let (nx, ny) = (bx + dx, by + dy);
            if self.map.passable(nx, ny)
                && self.monster_at(nx, ny).is_none()
                && (nx, ny) != (self.player.x, self.player.y)
            {
                let mut m = new_monster(Kind::Typo, nx, ny);
                m.awake = true;
                self.monsters.push(m);
                self.say(Tone::Danger, "The Legacy Code spawns a Typo.");
                return;
            }
        }
    }

    fn monster_attack(&mut self, i: usize) {
        let kind = self.monsters[i].kind;
        let s = species(kind);
        if !self.combat.chance(MONSTER_HIT) {
            self.say(Tone::Info, &format!("The {} misses you.", s.name));
            return;
        }
        let dmg = if kind == Kind::OffByOne {
            if self.combat.chance(50) {
                1
            } else {
                3
            }
        } else {
            self.combat.range(s.dmg.0, s.dmg.1)
        };
        self.player.hp -= dmg;
        self.say(
            Tone::Bad,
            &format!("The {} {} you for {}.", s.name, s.verb, dmg),
        );
        if kind == Kind::Deadlock && !self.player.stunned && self.combat.chance(25) {
            self.player.stunned = true;
            self.say(Tone::Danger, "Deadlock! You cannot act next turn.");
        }
        if self.player.hp <= 0 {
            if self.player.ctrl_z {
                self.player.ctrl_z = false;
                self.player.hp = self.player.max_hp / 2;
                self.say(Tone::Good, "CTRL+Z! You undo your death.");
            } else {
                self.player.hp = 0;
                self.status = Status::Dead(s.killer.to_string());
                self.say(Tone::Danger, "You died.");
            }
        }
    }

    /// The text that people can paste into a chat.
    pub fn summary(&self, label: &str) -> String {
        let outcome = match &self.status {
            Status::Won => format!(
                "Refactored the Legacy Code on floor {FINAL_FLOOR}. {} bugs squashed in {} turns.",
                self.kills, self.turns
            ),
            Status::Dead(by) => format!(
                "Reached floor {}/{} ({}). {} bugs squashed in {} turns. Killed by {}.",
                self.floor,
                FINAL_FLOOR,
                self.floor_name(),
                self.kills,
                self.turns,
                by
            ),
            Status::Playing => format!(
                "On floor {}/{} ({}). {} bugs squashed in {} turns.",
                self.floor,
                FINAL_FLOOR,
                self.floor_name(),
                self.kills,
                self.turns
            ),
        };
        format!("stackdive {label}\n{outcome}")
    }

    // ----- the bot, used by --demo and by the tests -----

    fn step_towards(&self, dist: &[i32], target: Pos) -> Option<Action> {
        let origin = (self.player.x, self.player.y);
        let mut cur = target;
        loop {
            let d = dist[idx(cur.0, cur.1)];
            if d <= 0 {
                return None;
            }
            if d == 1 {
                return Some(Action::Move(
                    (cur.0 - origin.0).signum(),
                    (cur.1 - origin.1).signum(),
                ));
            }
            let mut next = None;
            for (dx, dy) in DIRS8 {
                let (nx, ny) = (cur.0 + dx, cur.1 + dy);
                if nx >= 0 && ny >= 0 && nx < MAP_W && ny < MAP_H && dist[idx(nx, ny)] == d - 1 {
                    next = Some((nx, ny));
                    break;
                }
            }
            cur = next?;
        }
    }

    fn nearest(&self, dist: &[i32], targets: &[Pos]) -> Option<Pos> {
        targets
            .iter()
            .copied()
            .filter(|&(x, y)| dist[idx(x, y)] > 0)
            .min_by_key(|&(x, y)| dist[idx(x, y)])
    }

    /// A simple player: fights what is next to it, drinks coffee when hurt,
    /// explores, and takes the stairs when there is nothing left to see.
    pub fn bot_action(&self) -> Action {
        let p = &self.player;
        let origin = (p.x, p.y);
        if p.hp * 100 <= p.max_hp * 45 && p.coffees > 0 {
            return Action::Coffee;
        }
        if p.scrolls > 0 {
            return Action::Scroll;
        }
        let adjacent = self
            .monsters
            .iter()
            .filter(|m| (m.x - p.x).abs().max((m.y - p.y).abs()) == 1)
            .min_by_key(|m| m.hp);
        if let Some(m) = adjacent {
            return Action::Move(m.x - p.x, m.y - p.y);
        }
        let dist = self.map.distances(origin);
        let seen: Vec<Pos> = self
            .monsters
            .iter()
            .filter(|m| self.map.visible[idx(m.x, m.y)])
            .map(|m| (m.x, m.y))
            .collect();
        let items: Vec<Pos> = self
            .items
            .iter()
            .filter(|i| self.map.explored[idx(i.x, i.y)])
            .map(|i| (i.x, i.y))
            .collect();
        let mut frontier: Vec<Pos> = Vec::new();
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                if !self.map.explored[idx(x, y)] || !self.map.passable(x, y) || dist[idx(x, y)] < 0
                {
                    continue;
                }
                let open_edge = DIRS8.iter().any(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    crate::map::in_bounds(nx, ny)
                        && self.map.passable(nx, ny)
                        && !self.map.explored[idx(nx, ny)]
                });
                if open_edge {
                    frontier.push((x, y));
                }
            }
        }
        for targets in [&seen, &items, &frontier] {
            if let Some(t) = self.nearest(&dist, targets) {
                if let Some(a) = self.step_towards(&dist, t) {
                    return a;
                }
            }
        }
        if self.on_stairs() {
            return Action::Descend;
        }
        if self.floor == FINAL_FLOOR {
            // Hunt the boss, wherever it is.
            if let Some(m) = self.monsters.iter().find(|m| m.kind == Kind::LegacyCode) {
                if let Some(a) = self.step_towards(&dist, (m.x, m.y)) {
                    return a;
                }
            }
        } else if let Some(a) = self.step_towards(&dist, self.exit) {
            return a;
        }
        Action::Wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn teleport(g: &mut Game, pos: Pos) {
        g.player.x = pos.0;
        g.player.y = pos.1;
        g.map.update_fov(pos);
    }

    /// A free floor tile next to the player.
    fn free_neighbor(g: &Game) -> Pos {
        for (dx, dy) in DIRS8 {
            let (x, y) = (g.player.x + dx, g.player.y + dy);
            if g.map.passable(x, y) && g.monster_at(x, y).is_none() && g.item_at(x, y).is_none() {
                return (x, y);
            }
        }
        panic!("no free neighbor");
    }

    fn put_monster(g: &mut Game, kind: Kind, pos: Pos) -> usize {
        let mut m = new_monster(kind, pos.0, pos.1);
        m.awake = true;
        g.monsters.push(m);
        g.monsters.len() - 1
    }

    #[test]
    fn a_new_game_starts_clean() {
        for seed in 0..100u64 {
            let g = Game::new(seed);
            assert!(g.map.passable(g.player.x, g.player.y));
            assert!(g.monster_at(g.player.x, g.player.y).is_none());
            assert!(g.item_at(g.player.x, g.player.y).is_none());
            assert_eq!(g.player.hp, START_HP);
            assert!(!g.monsters.is_empty());
            for m in &g.monsters {
                assert!(g.map.passable(m.x, m.y));
                let far = (m.x - g.player.x).abs().max((m.y - g.player.y).abs());
                assert!(far >= 7, "monster too close to the start");
            }
        }
    }

    #[test]
    fn walls_do_not_use_a_turn() {
        let mut g = Game::new(1);
        // Stand right next to a wall.
        let spot = (0..MAP_H)
            .flat_map(|y| (0..MAP_W).map(move |x| (x, y)))
            .find(|&(x, y)| {
                g.map.passable(x, y)
                    && g.monster_at(x, y).is_none()
                    && g.item_at(x, y).is_none()
                    && DIRS8.iter().any(|(dx, dy)| !g.map.passable(x + dx, y + dy))
            })
            .expect("a tile next to a wall");
        teleport(&mut g, spot);
        let mut moved_into_wall = false;
        for (dx, dy) in DIRS8 {
            if !g.map.passable(g.player.x + dx, g.player.y + dy) {
                let turns = g.turns;
                assert!(!g.act(Action::Move(dx, dy)));
                assert_eq!(g.turns, turns);
                moved_into_wall = true;
                break;
            }
        }
        assert!(moved_into_wall);
    }

    #[test]
    fn hitting_a_bug_kills_it_in_the_end() {
        let mut g = Game::new(2);
        let spot = free_neighbor(&g);
        let i = put_monster(&mut g, Kind::Typo, spot);
        g.monsters[i].hp = 1;
        let dir = (spot.0 - g.player.x, spot.1 - g.player.y);
        for _ in 0..30 {
            if g.monster_at(spot.0, spot.1).is_none() {
                break;
            }
            g.player.hp = g.player.max_hp;
            g.act(Action::Move(dir.0, dir.1));
        }
        assert!(g.monster_at(spot.0, spot.1).is_none());
        assert_eq!(g.kills, 1);
        assert!(g.log.iter().any(|m| m.text.contains("Typo")));
    }

    #[test]
    fn coffee_heals_and_is_not_wasted() {
        let mut g = Game::new(3);
        g.player.coffees = 1;
        assert!(!g.act(Action::Coffee), "full health: keep the coffee");
        assert_eq!(g.player.coffees, 1);
        g.player.hp = 5;
        assert!(g.act(Action::Coffee));
        assert_eq!(g.player.hp, 5 + COFFEE_HEAL);
        assert_eq!(g.player.coffees, 0);
        assert!(!g.act(Action::Coffee), "no coffee left");
    }

    #[test]
    fn ctrl_z_saves_you_once() {
        let mut g = Game::new(4);
        let spot = free_neighbor(&g);
        put_monster(&mut g, Kind::Segfault, spot);
        g.monsters[0].energy = 100;
        g.player.hp = 1;
        g.player.ctrl_z = true;
        for _ in 0..60 {
            g.act(Action::Wait);
            if !g.player.ctrl_z {
                break;
            }
        }
        assert!(!g.player.ctrl_z, "the undo was used");
        assert_eq!(g.status, Status::Playing);
        assert!(g.player.hp > 0);
        // Without it, the next deadly hit kills.
        g.player.hp = 1;
        for _ in 0..200 {
            g.act(Action::Wait);
            if g.status != Status::Playing {
                break;
            }
        }
        assert!(matches!(g.status, Status::Dead(ref by) if by.contains("Segfault")));
    }

    #[test]
    fn stairs_lead_down_and_only_from_the_stairs() {
        let mut g = Game::new(5);
        assert!(!g.act(Action::Descend));
        assert_eq!(g.floor, 1);
        let exit = g.exit;
        teleport(&mut g, exit);
        assert!(g.on_stairs());
        assert!(g.act(Action::Descend));
        assert_eq!(g.floor, 2);
        assert!(g.map.passable(g.player.x, g.player.y));
    }

    #[test]
    fn what_you_do_never_changes_the_next_floor() {
        let a = {
            let mut g = Game::new(77);
            let exit = g.exit;
            teleport(&mut g, exit);
            g.act(Action::Descend);
            g
        };
        let b = {
            let mut g = Game::new(77);
            let mut rng = Rng::new(9);
            for _ in 0..80 {
                let (dx, dy) = DIRS8[rng.range(0, 7) as usize];
                g.act(Action::Move(dx, dy));
                g.player.hp = g.player.max_hp;
            }
            let exit = g.exit;
            teleport(&mut g, exit);
            g.act(Action::Descend);
            g
        };
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                assert_eq!(a.map.tile(x, y), b.map.tile(x, y));
            }
        }
        let positions = |g: &Game| {
            g.monsters
                .iter()
                .map(|m| (m.kind, m.x, m.y))
                .collect::<Vec<_>>()
        };
        assert_eq!(positions(&a), positions(&b));
    }

    #[test]
    fn the_last_floor_has_a_boss_and_no_stairs() {
        let g = {
            let mut g = Game::new(6);
            g.floor = FINAL_FLOOR - 1;
            let exit = g.exit;
            g.map.reveal_all();
            teleport(&mut g, exit);
            g.act(Action::Descend);
            g
        };
        assert_eq!(g.floor, FINAL_FLOOR);
        assert!(g.monsters.iter().any(|m| m.kind == Kind::LegacyCode));
        assert!(!g.on_stairs());
    }

    #[test]
    fn killing_the_boss_wins() {
        let mut g = Game::new(8);
        let spot = free_neighbor(&g);
        let i = put_monster(&mut g, Kind::LegacyCode, spot);
        g.monsters[i].hp = 1;
        let dir = (spot.0 - g.player.x, spot.1 - g.player.y);
        for _ in 0..40 {
            if g.status == Status::Won {
                break;
            }
            g.player.hp = g.player.max_hp;
            g.act(Action::Move(dir.0, dir.1));
        }
        assert_eq!(g.status, Status::Won);
        assert!(g.summary("seed 8").contains("Refactored"));
        assert!(!g.act(Action::Wait), "nothing happens after the end");
    }

    #[test]
    fn the_summary_is_something_you_can_paste() {
        let mut g = Game::new(9);
        g.kills = 12;
        g.turns = 345;
        g.status = Status::Dead("a Segfault (core dumped)".to_string());
        let s = g.summary("daily 2026-10-01");
        assert!(s.starts_with("stackdive daily 2026-10-01\n"));
        assert!(s.contains("12 bugs squashed in 345 turns"));
        assert!(s.contains("Killed by a Segfault (core dumped)"));
        assert!(s.is_ascii());
    }

    #[test]
    fn same_seed_same_actions_same_run() {
        let play = |seed| {
            let mut g = Game::new(seed);
            for _ in 0..400 {
                let a = g.bot_action();
                g.act(a);
            }
            (g.floor, g.kills, g.turns, g.player.hp, g.log.len())
        };
        assert_eq!(play(31), play(31));
    }

    fn check_invariants(g: &Game) {
        assert!(g.player.hp <= g.player.max_hp);
        assert!(g.map.passable(g.player.x, g.player.y));
        for (i, m) in g.monsters.iter().enumerate() {
            assert!(g.map.passable(m.x, m.y), "monster in a wall");
            assert!(m.hp > 0);
            assert!(
                (m.x, m.y) != (g.player.x, g.player.y),
                "monster on the player"
            );
            for other in &g.monsters[i + 1..] {
                assert!((m.x, m.y) != (other.x, other.y), "two monsters on one tile");
            }
        }
    }

    #[test]
    fn random_play_never_breaks_the_rules() {
        let actions = [
            Action::Move(0, -1),
            Action::Move(1, 0),
            Action::Move(0, 1),
            Action::Move(-1, 0),
            Action::Move(1, 1),
            Action::Move(-1, -1),
            Action::Wait,
            Action::Descend,
            Action::Coffee,
            Action::Scroll,
        ];
        for seed in 0..60u64 {
            let mut g = Game::new(seed);
            let mut rng = Rng::new(seed + 1000);
            for _ in 0..1500 {
                let a = *{
                    let i = rng.range(0, actions.len() as i32 - 1) as usize;
                    &actions[i]
                };
                g.act(a);
                check_invariants(&g);
                if g.status != Status::Playing {
                    break;
                }
            }
        }
    }

    #[test]
    fn the_bot_always_finishes_a_run() {
        let mut won = 0;
        let mut died = 0;
        for seed in 0..120u64 {
            let mut g = Game::new(seed);
            let mut steps = 0;
            while g.status == Status::Playing {
                let a = g.bot_action();
                g.act(a);
                check_invariants(&g);
                steps += 1;
                assert!(
                    steps < 30_000,
                    "seed {seed}: the bot got stuck on floor {}",
                    g.floor
                );
            }
            match g.status {
                Status::Won => won += 1,
                _ => died += 1,
            }
        }
        eprintln!("bot: {won} won, {died} died");
        assert!(won + died == 120);
    }

    /// Prints how each seed goes for the bot. Run by hand with:
    /// cargo test --release bot_report -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bot_report() {
        for seed in 0..400u64 {
            let mut g = Game::new(seed);
            let mut steps = 0;
            let mut coffees = 0;
            while g.status == Status::Playing && steps < 30_000 {
                let a = g.bot_action();
                if a == Action::Coffee {
                    coffees += 1;
                }
                g.act(a);
                steps += 1;
            }
            println!(
                "seed {seed:3} {:?} floor {} kills {} turns {} coffees {}",
                g.status, g.floor, g.kills, g.turns, coffees
            );
        }
    }
}

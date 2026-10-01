//! The dungeon: rooms joined by corridors, plus line of sight.

use crate::rng::Rng;
use std::collections::VecDeque;

pub const MAP_W: i32 = 72;
pub const MAP_H: i32 = 34;
pub const SIGHT: i32 = 9;

pub type Pos = (i32, i32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Wall,
    Floor,
    Stairs,
}

#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Room {
    pub fn center(&self) -> Pos {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// True if the rooms touch or are closer than one tile.
    fn too_close(&self, other: &Room) -> bool {
        self.x - 1 <= other.x + other.w
            && self.x + self.w + 1 >= other.x
            && self.y - 1 <= other.y + other.h
            && self.y + self.h + 1 >= other.y
    }
}

pub struct Map {
    tiles: Vec<Tile>,
    pub explored: Vec<bool>,
    pub visible: Vec<bool>,
    pub rooms: Vec<Room>,
}

pub struct Generated {
    pub map: Map,
    pub start: Pos,
    /// The stairs down, or the place of the boss on the last floor.
    pub exit: Pos,
}

pub fn idx(x: i32, y: i32) -> usize {
    (y * MAP_W + x) as usize
}

pub fn in_bounds(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < MAP_W && y < MAP_H
}

pub const DIRS8: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];

impl Map {
    fn solid() -> Map {
        let n = (MAP_W * MAP_H) as usize;
        Map {
            tiles: vec![Tile::Wall; n],
            explored: vec![false; n],
            visible: vec![false; n],
            rooms: Vec::new(),
        }
    }

    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if in_bounds(x, y) {
            self.tiles[idx(x, y)]
        } else {
            Tile::Wall
        }
    }

    pub fn passable(&self, x: i32, y: i32) -> bool {
        self.tile(x, y) != Tile::Wall
    }

    fn set(&mut self, x: i32, y: i32, tile: Tile) {
        if in_bounds(x, y) {
            self.tiles[idx(x, y)] = tile;
        }
    }

    fn carve_room(&mut self, r: &Room) {
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                self.set(x, y, Tile::Floor);
            }
        }
    }

    fn carve_corridor(&mut self, a: Pos, b: Pos, horizontal_first: bool) {
        let (mut x, mut y) = a;
        let step = |from: i32, to: i32| (to - from).signum();
        if horizontal_first {
            while x != b.0 {
                self.set(x, y, Tile::Floor);
                x += step(x, b.0);
            }
            while y != b.1 {
                self.set(x, y, Tile::Floor);
                y += step(y, b.1);
            }
        } else {
            while y != b.1 {
                self.set(x, y, Tile::Floor);
                y += step(y, b.1);
            }
            while x != b.0 {
                self.set(x, y, Tile::Floor);
                x += step(x, b.0);
            }
        }
        self.set(b.0, b.1, Tile::Floor);
    }

    /// Distance in steps from `from` to every reachable tile, -1 elsewhere.
    pub fn distances(&self, from: Pos) -> Vec<i32> {
        let mut dist = vec![-1; (MAP_W * MAP_H) as usize];
        let mut queue = VecDeque::new();
        dist[idx(from.0, from.1)] = 0;
        queue.push_back(from);
        while let Some((x, y)) = queue.pop_front() {
            let d = dist[idx(x, y)];
            for (dx, dy) in DIRS8 {
                let (nx, ny) = (x + dx, y + dy);
                if self.passable(nx, ny) && dist[idx(nx, ny)] < 0 {
                    dist[idx(nx, ny)] = d + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
        dist
    }

    pub fn generate(rng: &mut Rng, last_floor: bool) -> Generated {
        loop {
            let mut map = Map::solid();
            let want = rng.range(9, 13) as usize;
            let mut tries = 0;
            while map.rooms.len() < want && tries < 400 {
                tries += 1;
                let w = rng.range(4, 10);
                let h = rng.range(3, 6);
                let x = rng.range(1, MAP_W - w - 2);
                let y = rng.range(1, MAP_H - h - 2);
                let room = Room { x, y, w, h };
                if map.rooms.iter().any(|r| r.too_close(&room)) {
                    continue;
                }
                map.carve_room(&room);
                if let Some(prev) = map.rooms.last() {
                    let horizontal = rng.chance(50);
                    let from = prev.center();
                    map.carve_corridor(from, room.center(), horizontal);
                }
                map.rooms.push(room);
            }
            if map.rooms.len() < 6 {
                continue;
            }
            // A few shortcuts, so the level is not one long snake.
            for _ in 0..3 {
                let a = rng.range(0, map.rooms.len() as i32 - 1) as usize;
                let b = rng.range(0, map.rooms.len() as i32 - 1) as usize;
                if a != b {
                    let horizontal = rng.chance(50);
                    let (ca, cb) = (map.rooms[a].center(), map.rooms[b].center());
                    map.carve_corridor(ca, cb, horizontal);
                }
            }

            let start = map.rooms[0].center();
            let dist = map.distances(start);
            // The exit goes into the room that is the longest walk away.
            let far = map
                .rooms
                .iter()
                .skip(1)
                .max_by_key(|r| {
                    let c = r.center();
                    dist[idx(c.0, c.1)]
                })
                .copied()
                .expect("at least six rooms");
            let exit = far.center();
            if dist[idx(exit.0, exit.1)] < 12 {
                continue;
            }
            if !last_floor {
                map.set(exit.0, exit.1, Tile::Stairs);
            }
            return Generated { map, start, exit };
        }
    }

    /// Field of view with recursive shadowcasting. Fills `visible` and adds
    /// everything seen to `explored`.
    pub fn update_fov(&mut self, origin: Pos) {
        for v in self.visible.iter_mut() {
            *v = false;
        }
        self.visible[idx(origin.0, origin.1)] = true;
        // The eight ways to turn and mirror the view: [xx, xy, yx, yy].
        const OCTANTS: [[i32; 4]; 8] = [
            [1, 0, 0, 1],
            [0, 1, 1, 0],
            [0, -1, 1, 0],
            [-1, 0, 0, 1],
            [-1, 0, 0, -1],
            [0, -1, -1, 0],
            [0, 1, -1, 0],
            [1, 0, 0, -1],
        ];
        for m in OCTANTS {
            self.cast_light(origin, 1, 1.0, 0.0, m);
        }
        for i in 0..self.visible.len() {
            if self.visible[i] {
                self.explored[i] = true;
            }
        }
    }

    fn cast_light(&mut self, origin: Pos, row: i32, mut start: f32, end: f32, m: [i32; 4]) {
        if start < end {
            return;
        }
        let [xx, xy, yx, yy] = m;
        let radius_sq = SIGHT * SIGHT;
        let mut new_start = 0.0;
        for j in row..=SIGHT {
            let (mut dx, dy) = (-j - 1, -j);
            let mut blocked = false;
            while dx <= 0 {
                dx += 1;
                let x = origin.0 + dx * xx + dy * xy;
                let y = origin.1 + dx * yx + dy * yy;
                let l_slope = (dx as f32 - 0.5) / (dy as f32 + 0.5);
                let r_slope = (dx as f32 + 0.5) / (dy as f32 - 0.5);
                if start < r_slope {
                    continue;
                }
                if end > l_slope {
                    break;
                }
                if dx * dx + dy * dy < radius_sq && in_bounds(x, y) {
                    self.visible[idx(x, y)] = true;
                }
                let wall = !self.passable(x, y);
                if blocked {
                    if wall {
                        new_start = r_slope;
                        continue;
                    }
                    blocked = false;
                    start = new_start;
                } else if wall && j < SIGHT {
                    blocked = true;
                    self.cast_light(origin, j + 1, start, l_slope, m);
                    new_start = r_slope;
                }
            }
            if blocked {
                break;
            }
        }
    }

    /// Marks the whole floor as seen, for the Stack Overflow answer.
    pub fn reveal_all(&mut self) {
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let near_floor = DIRS8.iter().any(|(dx, dy)| self.passable(x + dx, y + dy))
                    || self.passable(x, y);
                if near_floor {
                    self.explored[idx(x, y)] = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_floor_connects_start_and_exit() {
        for seed in 0..300u64 {
            for floor in 1..=8u64 {
                let mut rng = Rng::child(seed, floor);
                let g = Map::generate(&mut rng, floor == 8);
                let dist = g.map.distances(g.start);
                assert!(
                    dist[idx(g.exit.0, g.exit.1)] >= 12,
                    "seed {seed} floor {floor}"
                );
                assert!(g.map.rooms.len() >= 6);
                for r in &g.map.rooms {
                    let c = r.center();
                    assert!(dist[idx(c.0, c.1)] >= 0, "room unreachable, seed {seed}");
                }
                if floor < 8 {
                    assert_eq!(g.map.tile(g.exit.0, g.exit.1), Tile::Stairs);
                } else {
                    assert_eq!(g.map.tile(g.exit.0, g.exit.1), Tile::Floor);
                }
            }
        }
    }

    #[test]
    fn same_seed_same_dungeon() {
        let a = Map::generate(&mut Rng::child(5, 3), false);
        let b = Map::generate(&mut Rng::child(5, 3), false);
        assert_eq!(a.map.tiles, b.map.tiles);
        assert_eq!(a.start, b.start);
        assert_eq!(a.exit, b.exit);
    }

    #[test]
    fn border_is_always_wall() {
        for seed in 0..50u64 {
            let g = Map::generate(&mut Rng::child(seed, 1), false);
            for x in 0..MAP_W {
                assert_eq!(g.map.tile(x, 0), Tile::Wall);
                assert_eq!(g.map.tile(x, MAP_H - 1), Tile::Wall);
            }
            for y in 0..MAP_H {
                assert_eq!(g.map.tile(0, y), Tile::Wall);
                assert_eq!(g.map.tile(MAP_W - 1, y), Tile::Wall);
            }
        }
    }

    fn open_room() -> Map {
        let mut map = Map::solid();
        for y in 5..15 {
            for x in 5..25 {
                map.set(x, y, Tile::Floor);
            }
        }
        map
    }

    #[test]
    fn fov_sees_the_room_but_not_through_walls() {
        let mut map = open_room();
        map.update_fov((10, 10));
        assert!(map.visible[idx(10, 10)]);
        assert!(map.visible[idx(14, 10)]);
        assert!(map.visible[idx(4, 10)], "the wall of the room is visible");
        assert!(!map.visible[idx(2, 10)], "behind the wall is not");
        assert!(map.explored[idx(14, 10)]);
    }

    #[test]
    fn fov_is_blocked_by_a_pillar() {
        let mut map = open_room();
        map.set(12, 10, Tile::Wall);
        map.update_fov((10, 10));
        assert!(map.visible[idx(12, 10)], "the pillar itself is seen");
        assert!(!map.visible[idx(14, 10)], "the tile behind it is hidden");
        assert!(map.visible[idx(14, 12)], "off to the side is visible");
    }

    #[test]
    fn fov_has_a_limited_radius() {
        let mut map = open_room();
        map.update_fov((5, 10));
        assert!(map.visible[idx(5 + SIGHT - 1, 10)]);
        assert!(!map.visible[idx(5 + SIGHT + 2, 10)]);
    }
}

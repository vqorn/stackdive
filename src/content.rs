//! What lives in the dungeon: the bugs and the floors they live on.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Typo,
    NullPointer,
    OffByOne,
    RaceCondition,
    MemoryLeak,
    Segfault,
    Deadlock,
    LegacyCode,
}

pub struct Species {
    pub kind: Kind,
    pub name: &'static str,
    pub glyph: char,
    pub hp: i32,
    pub dmg: (i32, i32),
    /// 100 acts once per turn, 200 twice, 50 every other turn.
    pub speed: i32,
    pub first_floor: u32,
    pub last_floor: u32,
    pub weight: i32,
    pub verb: &'static str,
    pub slain: &'static str,
    pub killer: &'static str,
    pub blurb: &'static str,
}

pub const FINAL_FLOOR: u32 = 8;

pub const FLOOR_NAMES: [&str; FINAL_FLOOR as usize] = [
    "Frontend",
    "API Gateway",
    "Auth Service",
    "Cache",
    "Database",
    "Message Queue",
    "Kernel",
    "Legacy Code",
];

const SPECIES: [Species; 8] = [
    Species {
        kind: Kind::Typo,
        name: "Typo",
        glyph: 't',
        hp: 3,
        dmg: (1, 2),
        speed: 100,
        first_floor: 1,
        last_floor: 4,
        weight: 10,
        verb: "nibbles",
        slain: "You fix the Typo.",
        killer: "a Typo",
        blurb: "Weak, but there are many of them.",
    },
    Species {
        kind: Kind::NullPointer,
        name: "Null Pointer",
        glyph: 'n',
        hp: 5,
        dmg: (1, 3),
        speed: 100,
        first_floor: 1,
        last_floor: 6,
        weight: 8,
        verb: "dereferences",
        slain: "The Null Pointer is caught and handled.",
        killer: "a Null Pointer (undefined is not a hero)",
        blurb: "Points at nothing and still hurts.",
    },
    Species {
        kind: Kind::OffByOne,
        name: "Off-by-one",
        glyph: 'o',
        hp: 6,
        dmg: (1, 3),
        speed: 100,
        first_floor: 2,
        last_floor: 7,
        weight: 6,
        verb: "miscounts and hits",
        slain: "The Off-by-one is corrected. Almost.",
        killer: "an Off-by-one (so close)",
        blurb: "Hits for 1 or for 3. Never 2.",
    },
    Species {
        kind: Kind::RaceCondition,
        name: "Race Condition",
        glyph: 'r',
        hp: 5,
        dmg: (1, 3),
        speed: 200,
        first_floor: 3,
        last_floor: 8,
        weight: 5,
        verb: "overwrites",
        slain: "The Race Condition loses the race.",
        killer: "a Race Condition (it works on my machine)",
        blurb: "Fast. Moves twice per turn.",
    },
    Species {
        kind: Kind::MemoryLeak,
        name: "Memory Leak",
        glyph: 'm',
        hp: 8,
        dmg: (1, 2),
        speed: 100,
        first_floor: 4,
        last_floor: 8,
        weight: 5,
        verb: "drains",
        slain: "The Memory Leak is plugged.",
        killer: "a Memory Leak (out of memory)",
        blurb: "Heals itself. Do not let it live.",
    },
    Species {
        kind: Kind::Segfault,
        name: "Segfault",
        glyph: 's',
        hp: 10,
        dmg: (3, 5),
        speed: 50,
        first_floor: 5,
        last_floor: 8,
        weight: 5,
        verb: "crashes into",
        slain: "Segfault. Core dumped, but not yours.",
        killer: "a Segfault (core dumped)",
        blurb: "Slow, but hits like a truck.",
    },
    Species {
        kind: Kind::Deadlock,
        name: "Deadlock",
        glyph: 'd',
        hp: 12,
        dmg: (2, 3),
        speed: 100,
        first_floor: 6,
        last_floor: 8,
        weight: 4,
        verb: "locks up",
        slain: "The Deadlock is resolved. Everyone moves on.",
        killer: "a Deadlock (waiting forever)",
        blurb: "Can freeze you for a turn.",
    },
    Species {
        kind: Kind::LegacyCode,
        name: "Legacy Code",
        glyph: 'L',
        hp: 45,
        dmg: (3, 6),
        speed: 100,
        first_floor: FINAL_FLOOR,
        last_floor: FINAL_FLOOR,
        weight: 0,
        verb: "throws a 4000 line function at",
        slain: "The Legacy Code is refactored. Nobody dares to touch it again.",
        killer: "the Legacy Code (nobody knows what it does)",
        blurb: "The boss. Keeps spawning Typos.",
    },
];

pub fn species(kind: Kind) -> &'static Species {
    SPECIES
        .iter()
        .find(|s| s.kind == kind)
        .expect("every kind has a species")
}

/// All bugs that can show up on a floor, for random spawning.
pub fn spawnable(floor: u32) -> Vec<&'static Species> {
    SPECIES
        .iter()
        .filter(|s| s.weight > 0 && s.first_floor <= floor && floor <= s.last_floor)
        .collect()
}

pub fn all_species() -> &'static [Species] {
    &SPECIES
}

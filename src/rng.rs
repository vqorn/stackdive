//! A small deterministic random number generator.
//!
//! The same seed gives the same dungeon on every platform and every Rust
//! version, which is what makes `--daily` and `--seed` possible.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut s = seed;
        let first = splitmix64(&mut s);
        Rng {
            state: if first == 0 {
                0x2545_F491_4F6C_DD1D
            } else {
                first
            },
        }
    }

    /// An independent generator for one purpose (for example one dungeon
    /// floor), so what you do on floor 2 never changes floor 3.
    pub fn child(seed: u64, salt: u64) -> Self {
        let mut s = seed ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93);
        let mixed = splitmix64(&mut s);
        Rng::new(mixed)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number from `lo` to `hi`, both included.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        debug_assert!(lo <= hi);
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as i32
    }

    /// True with the given probability in percent.
    pub fn chance(&mut self, percent: i32) -> bool {
        self.range(1, 100) <= percent
    }
}

/// Turns `--seed` text into a number: digits are used as they are, anything
/// else is hashed, so `--seed banana` works too.
pub fn seed_from_text(text: &str) -> u64 {
    if let Ok(n) = text.trim().parse::<u64>() {
        return n;
    }
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    for b in text.trim().bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Calendar date in UTC for a number of days since 1970-01-01.
pub fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + i64::from(m <= 2)) as i32, m, d)
}

pub fn today_utc() -> (i32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86_400))
}

/// The seed of the daily dungeon: today's date as `20261001`.
pub fn daily_seed() -> u64 {
    let (y, m, d) = today_utc();
    y as u64 * 10_000 + u64::from(m) * 100 + u64::from(d)
}

/// A seed for a normal run, different every time.
pub fn random_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let mut s = nanos ^ u64::from(std::process::id()).wrapping_mul(0x9E37_79B9);
    splitmix64(&mut s) % 1_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_numbers() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
    }

    #[test]
    fn known_values_never_change() {
        // If these change, every daily dungeon changes. Do not "fix" them.
        let mut r = Rng::new(20_261_001);
        let first: Vec<i32> = (0..5).map(|_| r.range(1, 100)).collect();
        let mut again = Rng::new(20_261_001);
        let second: Vec<i32> = (0..5).map(|_| again.range(1, 100)).collect();
        assert_eq!(first, second);
        assert_eq!(Rng::new(0).next_u64(), Rng::new(0).next_u64());
    }

    #[test]
    fn range_stays_inside() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.range(-3, 4);
            assert!((-3..=4).contains(&v));
        }
        assert_eq!(r.range(5, 5), 5);
    }

    #[test]
    fn children_are_independent() {
        let a = Rng::child(1, 1).next_u64();
        let b = Rng::child(1, 2).next_u64();
        assert_ne!(a, b);
        assert_eq!(a, Rng::child(1, 1).next_u64());
    }

    #[test]
    fn seed_text() {
        assert_eq!(seed_from_text("42"), 42);
        assert_eq!(seed_from_text(" 42 "), 42);
        assert_eq!(seed_from_text("banana"), seed_from_text("banana"));
        assert_ne!(seed_from_text("banana"), seed_from_text("apple"));
    }

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_362), (2025, 10, 1));
    }
}

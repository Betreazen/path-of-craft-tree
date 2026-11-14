// The real leveling pace, without the game: experience per branch and play time of each player
// over one interval, named only by a short salted hash of their UUID. pace_log.rs feeds it from
// the server and writes it out once an interval (design.md, 14).
use std::collections::BTreeMap;

/// Hex digits of a player's hash in the pace lines: 32 bits, plenty for one server's players.
const HASH_DIGITS: usize = 8;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A player's short name in the pace lines: FNV-1a over the world's salt and the UUID, mixed by
/// the splitmix64 finalizer. The same in every interval of one world, unrelated across worlds,
/// and the UUID cannot be read back from it without the salt.
pub fn player_hash(salt: &[u8], player: u128) -> String {
    let mut hash = FNV_OFFSET;
    for byte in salt.iter().chain(&player.to_be_bytes()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^= hash >> 31;
    format!("{hash:016x}")[..HASH_DIGITS].to_owned()
}

/// A world's salt as kept in its file: hex digits; anything else is not a salt.
pub fn parse_salt(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if text.len() < 16 || !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

pub fn salt_text(salt: &[u8]) -> String {
    salt.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// One player's interval so far.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PlayerPace {
    /// Experience gained per branch, main and related together.
    pub xp: BTreeMap<String, f64>,
    /// The general scale's gain.
    pub general: f64,
    /// Seconds in the game outside creative mode.
    pub play_seconds: f64,
}

/// Every player's interval so far.
#[derive(Debug, Default)]
pub struct PaceCounter {
    players: BTreeMap<u128, PlayerPace>,
}

impl PaceCounter {
    /// Adds the change from `before` to `after` of a player's experience table.
    pub fn note_gain(
        &mut self,
        player: u128,
        before: &BTreeMap<String, f64>,
        after: &BTreeMap<String, f64>,
        general: f64,
    ) {
        let pace = self.players.entry(player).or_default();
        for (branch, now) in after {
            let gained = now - before.get(branch).copied().unwrap_or(0.0);
            if gained > 0.0 {
                *pace.xp.entry(branch.clone()).or_insert(0.0) += gained;
            }
        }
        if general > 0.0 {
            pace.general += general;
        }
    }

    pub fn note_play(&mut self, player: u128, seconds: f64) {
        if seconds > 0.0 {
            self.players.entry(player).or_default().play_seconds += seconds;
        }
    }

    /// The interval so far, for a look without ending it.
    pub fn so_far(&self) -> &BTreeMap<u128, PlayerPace> {
        &self.players
    }

    /// Ends the interval: everyone who played in it, and a fresh count. Experience without play
    /// time (gained while in creative or spectator mode only) gives no pace and is left out.
    pub fn take(&mut self) -> Vec<(u128, PlayerPace)> {
        std::mem::take(&mut self.players)
            .into_iter()
            .filter(|(_, pace)| pace.play_seconds > 0.0)
            .collect()
    }
}

/// The JSON line of one player's interval, as `pace.jsonl` keeps it. `at` is the interval's end
/// in Unix seconds; experience is rounded to tenths.
pub fn json_line(at: u64, interval_seconds: u64, hash: &str, pace: &PlayerPace) -> String {
    let xp: serde_json::Map<String, serde_json::Value> = pace
        .xp
        .iter()
        .map(|(branch, xp)| (branch.clone(), serde_json::json!(tenths(*xp))))
        .collect();
    serde_json::json!({
        "at": at,
        "interval_s": interval_seconds,
        "player": hash,
        "minutes": tenths(pace.play_seconds / 60.0),
        "general": tenths(pace.general),
        "xp": xp,
    })
    .to_string()
}

/// The server log's line of one player's interval.
pub fn log_line(hash: &str, pace: &PlayerPace) -> String {
    let branches: Vec<String> = pace
        .xp
        .iter()
        .map(|(branch, xp)| format!("{branch} {xp:.0}"))
        .collect();
    let branches = if branches.is_empty() {
        "nothing".to_owned()
    } else {
        branches.join(", ")
    };
    format!(
        "pace {hash}: {:.0} min, {branches}; general {:.0}",
        pace.play_seconds / 60.0,
        pace.general
    )
}

fn tenths(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALT: &[u8] = b"0123456789abcdef";
    const PLAYER: u128 = 0x0123_abcd_4567_89ef_0123_4567_89ab_cdef;

    fn table(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    #[test]
    fn a_hash_is_short_stable_per_salt_and_hides_the_uuid() {
        let hash = player_hash(SALT, PLAYER);
        assert_eq!(hash.len(), HASH_DIGITS);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash, player_hash(SALT, PLAYER));
        assert_ne!(hash, player_hash(b"another world salt", PLAYER));
        assert_ne!(hash, player_hash(SALT, PLAYER + 1));
        assert!(!format!("{PLAYER:032x}").contains(&hash));
    }

    #[test]
    fn a_salt_reads_back_from_its_text() {
        let salt = [7u8, 0, 255, 16, 3, 9, 200, 1];
        assert_eq!(parse_salt(&salt_text(&salt)), Some(salt.to_vec()));
        assert_eq!(
            parse_salt(" 00ff00ff00ff00ff\n"),
            Some(vec![0, 255, 0, 255, 0, 255, 0, 255])
        );
        for bad in ["", "abc", "00ff", "zz00ff00ff00ff00ff", "00ff00ff00ff00f"] {
            assert_eq!(parse_salt(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn gains_add_up_per_branch_and_only_upwards() {
        let mut counter = PaceCounter::default();
        let empty = table(&[]);
        counter.note_gain(PLAYER, &empty, &table(&[("mining", 4.0)]), 0.6);
        counter.note_gain(
            PLAYER,
            &table(&[("mining", 4.0), ("smithing", 10.0)]),
            &table(&[("mining", 9.0), ("smithing", 14.0)]),
            0.75,
        );
        // A respec lowers a table; that is not experience earned.
        counter.note_gain(
            PLAYER,
            &table(&[("mining", 9.0)]),
            &table(&[("mining", 0.0)]),
            0.0,
        );
        counter.note_play(PLAYER, 60.0);
        let pace = &counter.so_far()[&PLAYER];
        assert_eq!(pace.xp, table(&[("mining", 9.0), ("smithing", 4.0)]));
        assert!((pace.general - 1.35).abs() < 1e-9);
        assert_eq!(pace.play_seconds, 60.0);
    }

    #[test]
    fn taking_the_interval_starts_a_fresh_one_and_skips_players_who_did_not_play() {
        let mut counter = PaceCounter::default();
        counter.note_play(1, 30.0);
        counter.note_gain(2, &table(&[]), &table(&[("alchemy", 36.0)]), 5.4);
        let taken = counter.take();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].0, 1);
        assert!(counter.so_far().is_empty());
    }

    #[test]
    fn the_lines_carry_the_hash_minutes_and_branches_but_no_uuid() {
        let pace = PlayerPace {
            xp: table(&[("mining", 4613.27), ("crafting", 120.04)]),
            general: 709.93,
            play_seconds: 3420.0,
        };
        let json: serde_json::Value =
            serde_json::from_str(&json_line(1_763_200_000, 3600, "a1b2c3d4", &pace)).unwrap();
        assert_eq!(json["player"], "a1b2c3d4");
        assert_eq!(json["minutes"], 57.0);
        assert_eq!(json["interval_s"], 3600);
        assert_eq!(json["xp"]["mining"], 4613.3);
        assert_eq!(json["general"], 709.9);
        let line = log_line("a1b2c3d4", &pace);
        assert_eq!(
            line,
            "pace a1b2c3d4: 57 min, crafting 120, mining 4613; general 710"
        );
        assert!(!line.contains(&format!("{PLAYER:032x}")));
    }
}

// Limits against experience farms that are pure bookkeeping, without the game: an anvil take that
// is a repair, the alchemist's brew pace and the builder's place-break-place loop (design.md, 14).
// Times come in as arguments, so the harness can step them.
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

/// An anvil take pays the smith only as a repair: the same item came out with less damage.
/// A rename, or merging enchantments on a whole item, pays nothing.
pub fn is_anvil_repair(left: Option<(&str, u32)>, output: (&str, u32)) -> bool {
    left.is_some_and(|(item, damage)| item == output.0 && output.1 < damage)
}

/// Bottles a finished brew pays for: the filled bottle slots, at most three.
pub const MAX_BOTTLES: u32 = 3;

pub fn bottles_paid(filled: usize) -> u32 {
    (filled as u32).min(MAX_BOTTLES)
}

/// The least time between two paid brews of one player: a stand's own brewing time, so more
/// stands than one do not pay faster.
pub const BREW_PAY_GAP: Duration = Duration::from_secs(20);

/// When each player's last paid brew was.
#[derive(Default)]
pub struct BrewPace {
    last_paid: HashMap<u128, Instant>,
}

/// How many players the pace remembers before it forgets those past the gap.
const MAX_BREWERS: usize = 10_000;

impl BrewPace {
    /// Whether a brew of `player` finished `now` may pay; a paid one starts the gap again.
    pub fn may_pay(&mut self, player: u128, now: Instant) -> bool {
        let ready = self
            .last_paid
            .get(&player)
            .is_none_or(|at| now.saturating_duration_since(*at) >= BREW_PAY_GAP);
        if ready {
            if self.last_paid.len() >= MAX_BREWERS {
                self.last_paid
                    .retain(|_, at| now.saturating_duration_since(*at) < BREW_PAY_GAP);
            }
            self.last_paid.insert(player, now);
        }
        ready
    }

    pub fn forget(&mut self, player: u128) {
        self.last_paid.remove(&player);
    }
}

/// How long a broken paid block keeps the breaker's next placement unpaid: as long as a built
/// spot stays unpaid.
pub const REBUILD_MEMORY: Duration = Duration::from_secs(24 * 60 * 60);
/// The most unpaid placements one player can owe; more breaks than that add nothing.
const MAX_OWED: usize = 10_000;
/// How many players may owe placements before those whose debts all ran out are forgotten.
const MAX_DEBTORS: usize = 10_000;

/// Blocks a player broke where building paid in the last day. Each one makes one later placement
/// of that player pay nothing, so "place, break, place next to it" earns only once.
#[derive(Default)]
pub struct RebuildDebts {
    owed: HashMap<u128, VecDeque<Instant>>,
}

impl RebuildDebts {
    /// `player` broke a block whose placement was paid.
    pub fn note_break(&mut self, player: u128, now: Instant) {
        if self.owed.len() >= MAX_DEBTORS && !self.owed.contains_key(&player) {
            self.owed.retain(|_, debts| {
                debts
                    .back()
                    .is_some_and(|at| now.saturating_duration_since(*at) < REBUILD_MEMORY)
            });
        }
        let debts = self.owed.entry(player).or_default();
        if debts.len() < MAX_OWED {
            debts.push_back(now);
        }
    }

    /// Whether a placement of `player` that would pay is instead taken by an earlier break; one
    /// debt is used up then. Debts older than a day are dropped.
    pub fn take(&mut self, player: u128, now: Instant) -> bool {
        let Some(debts) = self.owed.get_mut(&player) else {
            return false;
        };
        while debts
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= REBUILD_MEMORY)
        {
            debts.pop_front();
        }
        let owed = debts.pop_front().is_some();
        if debts.is_empty() {
            self.owed.remove(&player);
        }
        owed
    }

    pub fn forget(&mut self, player: u128) {
        self.owed.remove(&player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SWORD: &str = "minecraft:iron_sword";

    #[test]
    fn only_a_take_with_less_damage_on_the_same_item_is_a_repair() {
        assert!(is_anvil_repair(Some((SWORD, 120)), (SWORD, 40)));
        assert!(
            !is_anvil_repair(Some((SWORD, 120)), (SWORD, 120)),
            "a rename"
        );
        assert!(
            !is_anvil_repair(Some((SWORD, 0)), (SWORD, 0)),
            "a whole item"
        );
        assert!(!is_anvil_repair(Some((SWORD, 40)), (SWORD, 120)));
        assert!(!is_anvil_repair(
            Some(("minecraft:book", 0)),
            ("minecraft:enchanted_book", 0)
        ));
        assert!(!is_anvil_repair(None, (SWORD, 0)));
    }

    #[test]
    fn a_brew_pays_its_filled_bottles_and_never_more_than_three() {
        assert_eq!(bottles_paid(0), 0);
        assert_eq!(bottles_paid(1), 1);
        assert_eq!(bottles_paid(3), 3);
        assert_eq!(bottles_paid(5), 3);
    }

    #[test]
    fn one_player_is_paid_one_brew_per_twenty_seconds() {
        let start = Instant::now();
        let mut pace = BrewPace::default();
        assert!(pace.may_pay(1, start));
        // Three more stands of the same player finish in the same tick.
        assert!(!pace.may_pay(1, start));
        assert!(!pace.may_pay(1, start + Duration::from_secs(19)));
        assert!(pace.may_pay(2, start), "another player has their own pace");
        assert!(pace.may_pay(1, start + BREW_PAY_GAP));
        assert!(!pace.may_pay(1, start + BREW_PAY_GAP + Duration::from_secs(5)));
    }

    #[test]
    fn refused_brews_do_not_push_the_next_paid_one_back() {
        let start = Instant::now();
        let mut pace = BrewPace::default();
        assert!(pace.may_pay(1, start));
        for second in 1..20 {
            assert!(!pace.may_pay(1, start + Duration::from_secs(second)));
        }
        assert!(pace.may_pay(1, start + Duration::from_secs(20)));
    }

    #[test]
    fn each_broken_paid_block_cancels_one_later_placement() {
        let start = Instant::now();
        let mut debts = RebuildDebts::default();
        assert!(!debts.take(1, start), "nothing broken, the placement pays");
        debts.note_break(1, start);
        debts.note_break(1, start);
        assert!(!debts.take(2, start), "another player owes nothing");
        assert!(debts.take(1, start));
        assert!(debts.take(1, start));
        assert!(!debts.take(1, start), "both debts are used up");
    }

    #[test]
    fn a_debt_runs_out_after_a_day() {
        let start = Instant::now();
        let mut debts = RebuildDebts::default();
        debts.note_break(1, start);
        debts.note_break(1, start + Duration::from_secs(3600));
        let next_day = start + REBUILD_MEMORY;
        assert!(debts.take(1, next_day), "the later break still counts");
        assert!(!debts.take(1, next_day));
    }

    #[test]
    fn forgetting_a_player_drops_their_debts_and_brew_pace() {
        let now = Instant::now();
        let mut debts = RebuildDebts::default();
        debts.note_break(1, now);
        debts.forget(1);
        assert!(!debts.take(1, now));
        let mut pace = BrewPace::default();
        assert!(pace.may_pay(1, now));
        pace.forget(1);
        assert!(pace.may_pay(1, now));
    }
}

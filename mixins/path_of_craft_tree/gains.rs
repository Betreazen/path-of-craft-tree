// Where each player's experience came from, for the statistics page of the tree window. The
// server adds up what every source paid and sends the sums every few seconds on its own channel
// (`path_of_craft_tree:gains`, session.rs has the shape). That needs no protocol change: a client
// without the page ignores the channel, and a client on a server without it shows no sources.
use crate::game::{Game, pct_glue as glue, pct_session as session};
use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

pub const CHANNEL: &str = "path_of_craft_tree:gains";
/// How often the sums leave, at most: the page needs no more.
const SEND_EVERY: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Pending {
    /// By player, then by branch and source.
    sums: BTreeMap<u128, BTreeMap<(String, String), f64>>,
    sent_at: Option<Instant>,
}

static PENDING: LazyLock<Mutex<Pending>> = LazyLock::new(Mutex::default);

fn pending() -> MutexGuard<'static, Pending> {
    PENDING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `player` earned `amount` in `branch` from `source`: an action (`act:fish`) or a loot table.
pub fn record(player: u128, branch: &str, source: &str, amount: f64) {
    if !amount.is_finite() || amount <= 0.0 || glue::is_test_player(player) {
        return;
    }
    let mut state = pending();
    let sums = state.sums.entry(player).or_default();
    match sums.get_mut(&(branch.to_owned(), source.to_owned())) {
        Some(sum) => *sum += amount,
        None => {
            sums.insert((branch.to_owned(), source.to_owned()), amount);
        }
    }
}

/// Every server tick: the sums leave once `SEND_EVERY` has passed, to players who are still on.
pub fn on_server_tick(game: &mut Game<'_>) {
    let due = {
        let mut state = pending();
        if state.sums.is_empty() || state.sent_at.is_some_and(|at| at.elapsed() < SEND_EVERY) {
            return;
        }
        state.sent_at = Some(Instant::now());
        std::mem::take(&mut state.sums)
    };
    for (player, sums) in due {
        if !glue::server().players.contains_key(&player) {
            continue;
        }
        let message = session::encode_gains(&sums);
        if let Err(error) = game.native_net_send_to_player(player, CHANNEL, &message) {
            log::warn!("path_of_craft_tree: gains not sent: {error}");
        }
    }
}

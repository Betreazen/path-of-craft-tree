// Anvil takes: one payment point for every host. Single player's server thread, a LAN host and a
// dedicated server all finish a guest's anvil take in `lan_station_combined`; the engine's
// `anvil_craft_post` announcement depends on the take being deferred (`Menu::anvil_hook`), which
// the engine only asks for when a trait mod is loaded. Paying where the take ends does not.
use crate::game::{
    Game, pct_actions as actions, pct_fx_craft as fx, pct_fx_craft_enchant as enchant,
};
use horizon_client::menu::{AnvilCraft, Menu};
use std::sync::{Mutex, PoisonError};

/// The menu's result slot.
const RESULT_SLOT: usize = 2;

/// The inputs and result of the anvil a guest is about to click, by connection: what a take in
/// this click takes, as the engine's `AnvilCraft` would report it.
static BEFORE: Mutex<Option<(u32, AnvilCraft)>> = Mutex::new(None);

fn before() -> std::sync::MutexGuard<'static, Option<(u32, AnvilCraft)>> {
    BEFORE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// An operation on a guest's anvil menu is about to run: remember what a take would take.
pub fn before_op(conn: u32, menu: &Menu) {
    let snapshot = menu.slot(RESULT_SLOT).cloned().map(|output| AnvilCraft {
        left: menu.slot(0).cloned(),
        right: menu.slot(1).cloned(),
        output,
    });
    *before() = snapshot.map(|craft| (conn, craft));
}

/// A guest took an anvil's result, paying `levels`: the anvil's experience, once per take.
/// `craft` is the engine's report of the take when it deferred it; otherwise what `before_op` saw.
pub fn on_take(
    game: &mut Game<'_>,
    player: u128,
    conn: u32,
    levels: i32,
    craft: Option<&AnvilCraft>,
) {
    let saved = before().take().filter(|(seen, _)| *seen == conn);
    let Some(craft) = craft.or(saved.as_ref().map(|(_, craft)| craft)) else {
        return;
    };
    fx::note_anvil_levels(player, levels);
    actions::on_anvil(game, player, craft.right.as_ref());
    enchant::on_anvil_post(game, player, craft);
}

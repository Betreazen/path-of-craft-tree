// The Странствия sector: what every player's tick keeps up for its melee, archery, defense and
// exploration nodes, and the hit reductions that apply to the damage hooks.
use crate::game::{
    Game, pct_fx_roam_bow as bow, pct_fx_roam_combat as combat, pct_fx_roam_rules as rules,
    pct_fx_roam_world as world,
    pct_mining::{effect, player_effects},
};
use horizon_client::mod_api::NativeServerPlayer;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const PERIOD_TICKS: u32 = 10;
const ATTRIBUTE_PERIOD_TICKS: u32 = 100;
const MAX_REMEMBERED: usize = 4_096;
const FALL_DAMAGE: &str = "minecraft:fall";

/// Where a player was seen last, and what their nearby-ally nodes reach.
#[derive(Clone)]
pub struct Seen {
    pub uuid: u128,
    pub dimension: String,
    pub position: [f64; 3],
    pub guard_radius: f64,
}

#[derive(Default)]
struct State {
    ticks: HashMap<u128, u32>,
    seen: HashMap<i32, Seen>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn next_tick(uuid: u128) -> u32 {
    let mut state = state();
    if state.ticks.len() >= MAX_REMEMBERED {
        state.ticks.clear();
    }
    let ticks = state.ticks.entry(uuid).or_default();
    *ticks = ticks.wrapping_add(1);
    *ticks
}

fn remember(acting: &NativeServerPlayer, guard_radius: f64) {
    let mut state = state();
    if state.seen.len() >= MAX_REMEMBERED {
        state.seen.clear();
    }
    state.seen.insert(
        acting.entity_id,
        Seen {
            uuid: acting.uuid,
            dimension: acting.dimension.clone(),
            position: acting.position,
            guard_radius,
        },
    );
}

/// Every player seen lately, by entity id.
pub fn seen_players() -> Vec<(i32, Seen)> {
    state()
        .seen
        .iter()
        .map(|(id, seen)| (*id, seen.clone()))
        .collect()
}

/// Each player's upkeep, spread over the ticks to stay cheap.
pub fn on_tick(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    combat::finish_blocks(game, acting);
    let ticks = next_tick(acting.uuid);
    if !ticks.is_multiple_of(PERIOD_TICKS) {
        return;
    }
    let effects = player_effects(acting.uuid);
    remember(acting, effect(&effects, "ally_guard"));
    bow::on_period(game, acting);
    world::run_jobs(game, acting, &effects);
    if ticks.is_multiple_of(ATTRIBUTE_PERIOD_TICKS) {
        world::keep_attributes(game, acting, &effects);
    }
    combat::on_period(game, acting, &effects);
    world::on_period(game, acting, &effects, ticks);
}

fn dist_sq(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

fn guarded_by_ally(victim: i32, seen: &Seen, all: &[(i32, Seen)]) -> bool {
    all.iter().any(|(id, ally)| {
        *id != victim
            && ally.guard_radius > 0.0
            && ally.dimension == seen.dimension
            && dist_sq(ally.position, seen.position) <= ally.guard_radius.powi(2)
    })
}

/// The nodes that reduce a hit, called for hits that are not PvP.
pub fn on_incoming_damage(entity: i32, damage_type: &str, from_mob: bool, amount: &mut f32) {
    let all = seen_players();
    let Some((_, seen)) = all.iter().find(|(id, _)| *id == entity) else {
        return;
    };
    if damage_type == FALL_DAMAGE {
        let effects = player_effects(seen.uuid);
        let saved = effect(&effects, "fall_save") as f32;
        *amount *= (1.0 - saved).clamp(0.0, 1.0);
    }
    if from_mob && guarded_by_ally(entity, seen, &all) {
        *amount *= 1.0 - rules::ALLY_GUARD_CUT;
    }
}

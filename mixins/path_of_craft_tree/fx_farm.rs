// The farming sector: shared state and the farmer's nodes. Husbandry, cooking and alchemy have
// their own modules and are reached from here.
use crate::game::{
    Game,
    native_hooks::NativeLootDrops,
    native_player_state::NativeEffect,
    pct_actions as actions, pct_fx_farm_alchemy as alchemy, pct_fx_farm_cooking as cooking,
    pct_fx_farm_husbandry as husbandry, pct_fx_farm_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects, random},
    pct_rules::{Effects, Tree},
};
use horizon_client::{items::ItemStack, mod_api::NativeServerPlayer};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const MAX_BODIES: usize = 4_096;
const MAX_REAPED: usize = 1_024;
const BLOCK_PREFIX: &str = "minecraft:blocks/";
const FARMLAND: &str = "minecraft:farmland";
const AIR_BLOCKS: [&str; 2] = ["minecraft:air", "minecraft:cave_air"];
const STING: &str = "minecraft:sting";
const WET_ZONE_RADIUS: i32 = 12;
const WET_ZONE_LIFETIME: Duration = Duration::from_secs(5);
const UP: [i32; 3] = [0, 1, 0];
const MAX_SCARED_MOBS: usize = 32;
const SCARE_PUSH: f64 = 0.4;
const SCARE_LIFT: f64 = 0.1;
const SCARE_REACH: i32 = 1;

/// What the sector's hooks leave for the player's next tick, and what they share.
#[derive(Default)]
pub struct State {
    pub ticks: HashMap<u128, u32>,
    pub bodies: HashMap<i32, u128>,
    pub reaped: HashSet<(String, [i32; 3])>,
    pub wet_zones: HashMap<u128, (i32, i32, Instant)>,
    pub stations: HashMap<[i32; 3], u128>,
    pub carry: HashMap<[i32; 3], f64>,
    pub finished: Vec<[i32; 3]>,
    pub throwers: Vec<(u128, Instant)>,
    pub stung: HashSet<u128>,
    pub pending_status: Vec<(u128, NativeEffect)>,
    pub cake_shares: Vec<cooking::CakeShare>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);
/// Set while the mod cuts a whole stalk, so those breaks do not start more cuts.
static CUTTING_STALK: AtomicBool = AtomicBool::new(false);

pub fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn status(id: &str, amplifier: u8, ticks: i32) -> NativeEffect {
    NativeEffect {
        id: id.to_owned(),
        amplifier,
        duration: ticks,
        ambient: false,
        particles: false,
    }
}

/// Queues a status for a player, delivered on their own tick where only they can be reached.
pub fn queue_status(player: u128, effect: NativeEffect) {
    let mut state = state();
    if state.pending_status.len() < rules::MAX_PENDING {
        state.pending_status.push((player, effect));
    }
}

fn take_mine<T>(list: &mut Vec<(u128, T)>, player: u128) -> Vec<T> {
    let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(list)
        .into_iter()
        .partition(|(target, _)| *target == player);
    *list = rest;
    mine.into_iter().map(|(_, value)| value).collect()
}

fn apply_pending(game: &mut Game<'_>, player: u128) {
    let due = take_mine(&mut state().pending_status, player);
    for effect in &due {
        game.native_add_effect(player, effect);
    }
}

fn floor_pos(position: [f64; 3]) -> [i32; 3] {
    position.map(|axis| axis.floor() as i32)
}

// The hooks the shared flow calls.

pub fn on_tick(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let ticks = {
        let mut state = state();
        if state.bodies.len() >= MAX_BODIES {
            state.bodies.clear();
        }
        state.bodies.insert(acting.entity_id, acting.uuid);
        let ticks = state.ticks.entry(acting.uuid).or_default();
        *ticks = ticks.wrapping_add(1);
        *ticks
    };
    apply_pending(game, acting.uuid);
    let effects = player_effects(acting.uuid);
    let Some(loaded) = glue::current_tree().filter(|_| !effects.is_empty()) else {
        return;
    };
    let tree = &loaded.tree;
    tick_farming(game, tree, acting, &effects, ticks);
    husbandry::on_tick(game, acting, &effects, ticks);
    cooking::on_tick(game, tree, acting, &effects, ticks);
    alchemy::on_tick(game, acting, &effects, ticks);
}

pub fn on_block_drops(
    _tree: &Tree,
    _table: &str,
    _tool: Option<&ItemStack>,
    branch: &str,
    effects: &Effects,
    drops: &mut Vec<ItemStack>,
) {
    if branch != "farming" || !chance(effect(effects, "extra_seeds")) {
        return;
    }
    let seed = drops
        .iter()
        .find(|stack| stack.item.ends_with("_seeds"))
        .map(|stack| stack.item.to_string());
    if let Some(seed) = seed {
        drops.push(ItemStack::new(&seed, 1));
    }
}

/// A bee keeper does not feel stings; the poison they bring goes on the next tick.
pub fn on_incoming_damage(entity: i32, damage_type: &str, _from_mob: bool, amount: &mut f32) {
    if damage_type != STING {
        return;
    }
    let Some(player) = state().bodies.get(&entity).copied() else {
        return;
    };
    if effect(&player_effects(player), "bee_friend") > 0.0 {
        *amount = 0.0;
        state().stung.insert(player);
    }
}

// Ticking work.

fn tick_farming(
    game: &mut Game<'_>,
    tree: &Tree,
    acting: &NativeServerPlayer,
    effects: &Effects,
    ticks: u32,
) {
    if effect(effects, "wet_soil") > 0.0 {
        let [x, _, z] = floor_pos(acting.position);
        state()
            .wet_zones
            .insert(acting.uuid, (x, z, Instant::now()));
    }
    if ticks.is_multiple_of(rules::GROWTH_PERIOD_TICKS) {
        grow_near(game, tree, acting, effects);
    }
    if ticks.is_multiple_of(rules::AREA_PERIOD_TICKS) {
        scare_mobs(game, tree, acting, effects);
    }
}

fn random_offset(radius: i32) -> i32 {
    (random() * f64::from(2 * radius + 1)) as i32 - radius
}

fn ripe_age(tree: &Tree, block: &str) -> Option<i32> {
    if rules::is_stem(block) {
        return Some(rules::stem_ripe_age());
    }
    tree.number("ripe_age", block).map(|age| age as i32)
}

fn crop_age(game: &Game<'_>, pos: [i32; 3]) -> Option<i32> {
    game.native_block_property(pos, "age")?.parse().ok()
}

fn grow_one(game: &mut Game<'_>, tree: &Tree, pos: [i32; 3]) {
    let block = game.native_block(pos);
    let (Some(ripe), Some(age)) = (ripe_age(tree, &block), crop_age(game, pos)) else {
        return;
    };
    if age < ripe {
        game.native_set_block_property(pos, "age", &(age + 1).to_string());
    }
}

/// Green Fingers: a few random blocks around the player grow a stage.
fn grow_near(game: &mut Game<'_>, tree: &Tree, acting: &NativeServerPlayer, effects: &Effects) {
    let radius = effect(effects, "growth_radius") as i32;
    if radius <= 0 {
        return;
    }
    let [x, y, z] = floor_pos(acting.position);
    for _ in 0..rules::growth_samples(radius) {
        let pos = [
            x + random_offset(radius),
            y + random_offset(rules::GROWTH_HEIGHT),
            z + random_offset(radius),
        ];
        grow_one(game, tree, pos);
    }
}

fn is_crop_ground(tree: &Tree, block: &str) -> bool {
    block == FARMLAND || ripe_age(tree, block).is_some()
}

/// The centre of the nearest crop ground around a mob's feet, if any.
fn crop_near(game: &Game<'_>, tree: &Tree, feet: [i32; 3]) -> Option<[i32; 2]> {
    rules::area_offsets(SCARE_REACH)
        .into_iter()
        .find(|[dx, dz]| {
            let at = |y: i32| [feet[0] + dx, y, feet[2] + dz];
            is_crop_ground(tree, &game.native_block(at(feet[1])))
                || is_crop_ground(tree, &game.native_block(at(feet[1] - 1)))
        })
        .map(|[dx, dz]| [feet[0] + dx, feet[2] + dz])
}

/// Scarecrow: hostile mobs that step onto crops around the player are pushed off them.
fn scare_mobs(game: &mut Game<'_>, tree: &Tree, acting: &NativeServerPlayer, effects: &Effects) {
    let radius = effect(effects, "scarecrow");
    if radius <= 0.0 {
        return;
    }
    let [cx, _, cz] = acting.position;
    let nearby: Vec<(u32, [f64; 3])> = game
        .mobs
        .iter()
        .filter(|m| !m.removed && m.alive() && m.kind.category == "monster")
        .filter(|m| {
            let (dx, dz) = (m.position.x - cx, m.position.z - cz);
            dx * dx + dz * dz <= radius * radius
        })
        .take(MAX_SCARED_MOBS)
        .map(|m| (m.id, [m.position.x, m.position.y, m.position.z]))
        .collect();
    for (id, at) in nearby {
        let Some([crop_x, crop_z]) = crop_near(game, tree, floor_pos(at)) else {
            continue;
        };
        let (dx, dz) = (
            at[0] - (f64::from(crop_x) + 0.5),
            at[2] - (f64::from(crop_z) + 0.5),
        );
        let length = (dx * dx + dz * dz).sqrt().max(0.01);
        let push = [
            dx / length * SCARE_PUSH,
            SCARE_LIFT,
            dz / length * SCARE_PUSH,
        ];
        if let Some(mob) = game.mobs.iter_mut().find(|m| m.id == id) {
            mob.ai.nav.stop();
            mob.ai.target = None;
        }
        game.native_push_mob(&acting.dimension, id, push);
    }
}

// Hooks with their own injections.

/// Reaper's click, and the blocks other branches answer on a right click.
pub fn on_right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let effects = player_effects(acting.uuid);
    if effects.is_empty() {
        return;
    }
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    if effect(&effects, "reap") > 0.0 && reap_area(game, tree, acting.uuid, &effects, pos) {
        *canceled = true;
        return;
    }
    husbandry::on_right_click(game, tree, pos, &effects);
    cooking::on_right_click(game, &acting, pos, &effects);
}

fn is_ripe(game: &Game<'_>, tree: &Tree, block: &str, pos: [i32; 3]) -> bool {
    match (tree.number("ripe_age", block), crop_age(game, pos)) {
        (Some(ripe), Some(age)) => f64::from(age) >= ripe,
        _ => false,
    }
}

fn reap_one(game: &mut Game<'_>, tree: &Tree, player: u128, pos: [i32; 3]) -> bool {
    let block = game.native_block(pos);
    if rules::seed_of(&block).is_none() || !is_ripe(game, tree, &block, pos) {
        return false;
    }
    let Some(young) = game.native_default_state(&block) else {
        return false;
    };
    let dimension = game.native_dimension();
    let key = (dimension.to_owned(), pos);
    {
        let mut state = state();
        if state.reaped.len() >= MAX_REAPED {
            state.reaped.clear();
        }
        state.reaped.insert(key.clone());
    }
    if !game.native_player_break_block(player, pos, false) {
        state().reaped.remove(&key);
        return false;
    }
    game.native_set_block(
        dimension,
        pos,
        young,
        horizon_world::level::flags::UPDATE_ALL,
    );
    true
}

fn reap_area(
    game: &mut Game<'_>,
    tree: &Tree,
    player: u128,
    effects: &Effects,
    pos: [i32; 3],
) -> bool {
    // The clicked crop decides: a click elsewhere must keep its usual meaning.
    let block = game.native_block(pos);
    if rules::seed_of(&block).is_none() || !is_ripe(game, tree, &block, pos) {
        return false;
    }
    let radius = effect(effects, "reap_area") as i32;
    let mut reaped = false;
    for [dx, dz] in rules::area_offsets(radius) {
        reaped |= reap_one(game, tree, player, [pos[0] + dx, pos[1], pos[2] + dz]);
    }
    reaped
}

/// A reaped crop pays for its own replanting with one of its seeds.
fn take_replant_seed(table: &str, drops: &mut Vec<ItemStack>) {
    let Some(seed) = table
        .strip_prefix(BLOCK_PREFIX)
        .and_then(|name| rules::seed_of(&format!("minecraft:{name}")))
    else {
        return;
    };
    let Some(index) = drops.iter().position(|stack| &*stack.item == seed) else {
        return;
    };
    drops[index].count -= 1;
    if drops[index].count == 0 {
        drops.remove(index);
    }
}

pub fn on_loot(context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::Loot);
    let Some(player) = context.player else {
        return;
    };
    if let Some((pos, _)) = context.block {
        let key = (context.dimension.to_owned(), pos);
        if state().reaped.remove(&key) {
            take_replant_seed(context.table, drops);
        }
    }
    husbandry::on_loot(context.table, player, drops);
}

/// Hoe in hand over dirt: a wider square, and bone meal pays the farmer.
pub fn on_use_item_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], face: [i32; 3]) {
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let item = &*stack.item;
    if item == "minecraft:bone_meal" {
        pay_bone_meal(game, acting.uuid, pos);
    } else if item.ends_with("_hoe") && face == UP {
        let effects = player_effects(acting.uuid);
        till_area(game, effect(&effects, "till_radius") as i32, pos);
    }
}

fn pay_bone_meal(game: &mut Game<'_>, player: u128, pos: [i32; 3]) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let block = game.native_block(pos);
    let growing = block.ends_with("_sapling")
        || match (ripe_age(&loaded.tree, &block), crop_age(game, pos)) {
            (Some(ripe), Some(age)) => age < ripe,
            _ => false,
        };
    if growing {
        actions::earn_once(game, player, "act:bone_meal", 1.0);
    }
}

fn till_area(game: &mut Game<'_>, radius: i32, pos: [i32; 3]) {
    if radius <= 0 || !rules::is_tillable(&game.native_block(pos)) {
        return;
    }
    let Some(farmland) = game.native_default_state(FARMLAND) else {
        return;
    };
    let dimension = game.native_dimension();
    for [dx, dz] in rules::area_offsets(radius) {
        let at = [pos[0] + dx, pos[1], pos[2] + dz];
        let above = game.native_block([at[0], at[1] + 1, at[2]]);
        if at != pos
            && rules::is_tillable(&game.native_block(at))
            && AIR_BLOCKS.contains(&above.as_str())
        {
            game.native_set_block(
                dimension,
                at,
                farmland,
                horizon_world::level::flags::UPDATE_ALL,
            );
        }
    }
}

/// Seed Stock: a planted seed may come straight back.
pub fn on_place_block(game: &mut Game<'_>, stack: &ItemStack) {
    if !rules::is_seed(&stack.item) {
        return;
    }
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    if !chance(effect(&player_effects(acting.uuid), "seed_save")) {
        return;
    }
    // Given back a few ticks later: until the client has heard the seed go, a count that ends
    // where it began is never sent, and the client keeps showing the seed as spent.
    crate::game::pct_fx_craft::give_later(acting.uuid, ItemStack::new(&*stack.item, 1));
}

/// Marks a stalk cut as being in progress until dropped, even if a break panics.
struct StalkGuard;

impl StalkGuard {
    fn start() -> StalkGuard {
        CUTTING_STALK.store(true, Ordering::Relaxed);
        StalkGuard
    }
}

impl Drop for StalkGuard {
    fn drop(&mut self) {
        CUTTING_STALK.store(false, Ordering::Relaxed);
    }
}

/// The rest of the column at `pos`, top first so that nothing falls before it is cut.
fn stalk_column(game: &Game<'_>, pos: [i32; 3], block: &str) -> Vec<[i32; 3]> {
    let mut column = Vec::new();
    for direction in [1, -1] {
        for step in 1..=rules::STALK_LIMIT {
            let next = [pos[0], pos[1] + direction * step, pos[2]];
            if game.native_block(next) != block {
                break;
            }
            column.push(next);
        }
    }
    column.sort_by_key(|at| std::cmp::Reverse(at[1]));
    column
}

/// Whole Stalk: the block broken takes the rest of its column with it.
pub fn on_destroy(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::Destroy);
    if CUTTING_STALK.load(Ordering::Relaxed) || *canceled {
        return;
    }
    let block = game.native_block(pos);
    if !rules::is_stalk(&block) {
        return;
    }
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let effects = player_effects(acting.uuid);
    if effect(&effects, "whole_stalk") <= 0.0 {
        return;
    }
    let into_inventory = effect(&effects, "stalk_inventory") > 0.0;
    let column = stalk_column(game, pos, &block);
    let _cutting = StalkGuard::start();
    for at in column {
        game.native_player_break_block(acting.uuid, at, into_inventory);
    }
    // The drops of the block itself only reach the inventory through a break of our own.
    if into_inventory && game.native_player_break_block(acting.uuid, pos, true) {
        *canceled = true;
    }
}

/// Eternal Field: a player with the node does not trample farmland.
pub fn on_trample(entity: i32, kind: &str, cancel: &mut bool) {
    if kind != "minecraft:player" {
        return;
    }
    let player = state().bodies.get(&entity).copied();
    if player.is_some_and(|player| effect(&player_effects(player), "no_trample") > 0.0) {
        *cancel = true;
    }
}

/// Eternal Field's upgrade: farmland around such a player does not dry out.
pub fn on_moisture_change(x: i32, z: i32, from: i32, to: i32, cancel: &mut bool) {
    if to >= from {
        return;
    }
    let mut state = state();
    state
        .wet_zones
        .retain(|_, (.., at)| at.elapsed() < WET_ZONE_LIFETIME);
    let wet = state.wet_zones.values().any(|(zone_x, zone_z, _)| {
        (x - zone_x).abs() <= WET_ZONE_RADIUS && (z - zone_z).abs() <= WET_ZONE_RADIUS
    });
    if wet {
        *cancel = true;
    }
}

/// Stations remember who opened them last: their speed nodes are that player's.
pub fn on_container_open(player: u128, menu: &str, pos: Option<[i32; 3]>) {
    let Some(pos) = pos else {
        return;
    };
    if menu != rules::BREWING_MENU && !rules::FURNACE_MENUS.contains(&menu) {
        return;
    }
    let mut state = state();
    if state.stations.len() >= rules::MAX_STATIONS {
        state.stations.clear();
        state.carry.clear();
    }
    state.stations.insert(pos, player);
}

// Node effects of the other branches: extra drops, passive statuses and damage cuts.
use crate::game::{
    Game,
    native_player_state::NativeEffect,
    pct_effect_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects, random},
    pct_rules::{Effects, Tree},
};
use horizon_client::{items::ItemStack, mod_api::NativeServerPlayer};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

// Regeneration heals when its remaining time is a multiple of 50 ticks, so refreshing it more
// often than that would heal once per refresh.
const STATUS_PERIOD_TICKS: u32 = 50;
const OVERWORLD: &str = "minecraft:overworld";
const MAX_BODIES: usize = 4_096;
const EYE_HEIGHT: f64 = 1.62;
const DAY_LENGTH: i64 = 24_000;
const NIGHT_START: i64 = 12_542;
const NIGHT_END: i64 = 23_460;
const IDLE_BEFORE_REGEN: Duration = Duration::from_secs(10);
// The PvP hook runs just before the damage hooks of the same hit.
const SAME_HIT: Duration = Duration::from_millis(20);
// Commands and the void must still kill: saving the player there only delays it.
const UNSTOPPABLE_DAMAGE: [&str; 2] = ["minecraft:generic_kill", "minecraft:out_of_world"];
const RESCUE_REGEN_TICKS: i32 = 100;
const RESCUE_REGEN_AMPLIFIER: u8 = 1;
const WATER_BLOCKS: [&str; 6] = [
    "minecraft:water",
    "minecraft:kelp",
    "minecraft:kelp_plant",
    "minecraft:seagrass",
    "minecraft:tall_seagrass",
    "minecraft:bubble_column",
];
const BLOCK_PREFIX: &str = "minecraft:blocks/";
const SILK_TOUCH: &str = "minecraft:silk_touch";
const LOOSE_BLOCKS: [&str; 4] = ["sand", "red_sand", "gravel", "clay"];
const TREASURE_BLOCKS: [&str; 6] = [
    "dirt",
    "grass_block",
    "coarse_dirt",
    "podzol",
    "mycelium",
    "rooted_dirt",
];
const TREASURE_COMMON: [&str; 4] = [
    "minecraft:bone",
    "minecraft:flint",
    "minecraft:gold_nugget",
    "minecraft:iron_nugget",
];
const TREASURE_RARE_CHANCE: f64 = 0.02;
const DIAMOND_SHARE: f64 = 0.25;
const FOREST_GIFTS: [&str; 3] = ["minecraft:apple", "minecraft:honeycomb", "minecraft:stick"];
const RICH_GIFT_SHARE: f64 = 0.1;
const SEEDLESS_HERBS: [&str; 3] = ["short_grass", "tall_grass", "fern"];
const MOB_HEADS: [(&str, &str); 5] = [
    ("minecraft:zombie", "minecraft:zombie_head"),
    ("minecraft:skeleton", "minecraft:skeleton_skull"),
    ("minecraft:creeper", "minecraft:creeper_head"),
    (
        "minecraft:wither_skeleton",
        "minecraft:wither_skeleton_skull",
    ),
    ("minecraft:piglin", "minecraft:piglin_head"),
];
const HIDE_ANIMALS: [&str; 4] = [
    "minecraft:cow",
    "minecraft:horse",
    "minecraft:llama",
    "minecraft:mooshroom",
];

struct Body {
    uuid: u128,
    y: f64,
    health: f32,
}

#[derive(Default)]
struct State {
    ticks: HashMap<u128, u32>,
    bodies: HashMap<i32, Body>,
    last_hurt: HashMap<u128, Instant>,
    last_rescue: HashMap<u128, Instant>,
    rescued: Vec<u128>,
    pvp_hit: Option<(i32, Instant)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn is_night(game: &Game<'_>) -> bool {
    let time = game.native_day_time().rem_euclid(DAY_LENGTH);
    (NIGHT_START..NIGHT_END).contains(&time)
}

/// Height for "underground" nodes, which only mean something in the overworld.
fn depth_y(acting: &NativeServerPlayer) -> f64 {
    if acting.dimension == OVERWORLD {
        acting.position[1]
    } else {
        f64::MAX
    }
}

fn surroundings(game: &Game<'_>, tree: &Tree, acting: &NativeServerPlayer) -> rules::Surroundings {
    let [x, y, z] = acting.position;
    let feet = [x.floor() as i32, y.floor() as i32, z.floor() as i32];
    let head = [feet[0], (y + EYE_HEIGHT).floor() as i32, feet[2]];
    let head_block = game.native_block(head);
    let biome = game
        .native_biome(feet)
        .map(|biome| biome.to_string())
        .unwrap_or_default();
    let idle = state()
        .last_hurt
        .get(&acting.uuid)
        .is_none_or(|at| at.elapsed() >= IDLE_BEFORE_REGEN);
    rules::Surroundings {
        y: depth_y(acting),
        head_in_water: WATER_BLOCKS.iter().any(|water| head_block == *water),
        in_forest: tree.in_list("forest_biomes", &biome),
        in_nether_forest: tree.in_list("nether_forest_biomes", &biome),
        night: is_night(game),
        idle,
    }
}

fn give_status(game: &mut Game<'_>, player: u128, id: &str, amplifier: u8, ticks: i32) {
    let status = NativeEffect {
        id: id.to_owned(),
        amplifier,
        duration: ticks,
        ambient: false,
        particles: false,
    };
    game.native_add_effect(player, &status);
}

fn apply_statuses(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let effects = player_effects(acting.uuid);
    let Some(loaded) = glue::current_tree().filter(|_| !effects.is_empty()) else {
        return;
    };
    let at = surroundings(game, &loaded.tree, acting);
    let get = |key: &str| effect(&effects, key);
    for status in rules::statuses(&get, &at) {
        give_status(game, acting.uuid, status.id, status.amplifier, status.ticks);
    }
}

/// Keeps what the damage hooks need, which run without the game, and refreshes statuses.
pub fn on_tick(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let (due, was_rescued) = {
        let mut state = state();
        if state.bodies.len() >= MAX_BODIES {
            state.bodies.clear();
        }
        let body = Body {
            uuid: acting.uuid,
            y: depth_y(acting),
            health: acting.health,
        };
        state.bodies.insert(acting.entity_id, body);
        let ticks = state.ticks.entry(acting.uuid).or_default();
        *ticks = ticks.wrapping_add(1);
        let due = ticks.is_multiple_of(STATUS_PERIOD_TICKS);
        let before = state.rescued.len();
        state.rescued.retain(|uuid| *uuid != acting.uuid);
        (due, state.rescued.len() != before)
    };
    if was_rescued {
        give_status(
            game,
            acting.uuid,
            "minecraft:regeneration",
            RESCUE_REGEN_AMPLIFIER,
            RESCUE_REGEN_TICKS,
        );
    }
    if due {
        apply_statuses(game, acting);
    }
}

pub fn on_pvp_hit(victim: i32) {
    state().pvp_hit = Some((victim, Instant::now()));
}

/// Whether the hit being processed right now is another player's, on this player.
pub fn was_pvp_hit_on(player: u128) -> bool {
    let state = state();
    state.pvp_hit.is_some_and(|(victim, at)| {
        at.elapsed() < SAME_HIT && state.bodies.get(&victim).is_some_and(|b| b.uuid == player)
    })
}

pub fn is_pvp_hit(entity: i32) -> bool {
    state()
        .pvp_hit
        .is_some_and(|(victim, at)| victim == entity && at.elapsed() < SAME_HIT)
}

/// Any hit, from anyone, keeps Recovery from starting.
pub fn note_hurt(entity: i32) {
    let mut state = state();
    if let Some(uuid) = state.bodies.get(&entity).map(|body| body.uuid) {
        state.last_hurt.insert(uuid, Instant::now());
    }
}

/// Applies a player's damage-reducing nodes to a hit that is not from another player.
pub fn cut_incoming(entity: i32, damage_type: &str, from_mob: bool, amount: &mut f32) {
    let Some((uuid, y, health)) = state().bodies.get(&entity).map(|b| (b.uuid, b.y, b.health))
    else {
        return;
    };
    let effects = player_effects(uuid);
    let get = |key: &str| effect(&effects, key);
    *amount = rules::after_cuts(&get, damage_type, from_mob, y, health, *amount);
}

/// Second Wind: a hit that would kill leaves the player at one health instead.
pub fn on_final_damage(entity: i32, kind: &str, damage_type: &str, amount: &mut f32) {
    if kind != "minecraft:player" || UNSTOPPABLE_DAMAGE.contains(&damage_type) || is_pvp_hit(entity)
    {
        return;
    }
    let Some((uuid, health)) = state().bodies.get(&entity).map(|b| (b.uuid, b.health)) else {
        return;
    };
    let effects = player_effects(uuid);
    if *amount < health || effect(&effects, "second_wind") <= 0.0 {
        return;
    }
    let get = |key: &str| effect(&effects, key);
    let cooldown = Duration::from_secs_f64(rules::second_wind_cooldown(&get));
    let mut state = state();
    if state
        .last_rescue
        .get(&uuid)
        .is_some_and(|at| at.elapsed() < cooldown)
    {
        return;
    }
    state.last_rescue.insert(uuid, Instant::now());
    state.rescued.push(uuid);
    *amount = (health - 1.0).max(0.0);
}

fn add(drops: &mut Vec<ItemStack>, item: &str) {
    drops.push(ItemStack::new(item, 1));
}

/// Adds one more of the first drop that `wanted` accepts.
fn duplicate_first(drops: &mut Vec<ItemStack>, wanted: impl Fn(&str) -> bool) {
    let item = drops
        .iter()
        .find(|stack| wanted(&stack.item))
        .map(|stack| stack.item.to_string());
    if let Some(item) = item {
        add(drops, &item);
    }
}

fn pick<'a>(items: &[&'a str]) -> &'a str {
    items[(random() * items.len() as f64) as usize % items.len()]
}

fn block_name(table: &str) -> Option<&str> {
    table.strip_prefix(BLOCK_PREFIX)
}

fn sapling_of(leaves: &str) -> Option<String> {
    let wood = leaves.strip_suffix("_leaves")?;
    Some(format!("minecraft:{wood}_sapling"))
}

fn apply_leaves(name: &str, effects: &Effects, drops: &mut Vec<ItemStack>) {
    let Some(sapling) = sapling_of(name) else {
        return;
    };
    if chance(effect(effects, "extra_sapling")) {
        duplicate_first(drops, |item| item == sapling);
    }
}

fn apply_herbs(tree: &Tree, name: &str, effects: &Effects, drops: &mut Vec<ItemStack>) {
    if !tree.in_list("herbs", &format!("minecraft:{name}")) {
        return;
    }
    if effect(effects, "herbs_double") > 0.0 {
        let extra = drops.clone();
        drops.extend(extra);
    }
    if SEEDLESS_HERBS.contains(&name) && chance(effect(effects, "herbs_seeds")) {
        add(drops, "minecraft:wheat_seeds");
    }
}

/// Effects of blocks that give no experience, such as leaves and flowers.
pub fn on_any_block(tree: &Tree, table: &str, effects: &Effects, drops: &mut Vec<ItemStack>) {
    let Some(name) = block_name(table) else {
        return;
    };
    apply_leaves(name, effects, drops);
    apply_herbs(tree, name, effects, drops);
}

fn add_treasure(effects: &Effects, drops: &mut Vec<ItemStack>) {
    let rare_bonus = effect(effects, "treasure_rare");
    if !chance(TREASURE_RARE_CHANCE * (1.0 + rare_bonus)) {
        add(drops, pick(&TREASURE_COMMON));
    } else if rare_bonus > 0.0 && chance(DIAMOND_SHARE) {
        add(drops, "minecraft:diamond");
    } else {
        add(drops, "minecraft:emerald");
    }
}

fn apply_digging(name: &str, effects: &Effects, drops: &mut Vec<ItemStack>) {
    if name == "gravel" && effect(effects, "flint_always") > 0.0 {
        for stack in drops
            .iter_mut()
            .filter(|stack| &*stack.item == "minecraft:gravel")
        {
            *stack = ItemStack::new("minecraft:flint", stack.count);
        }
    }
    if name == "gravel" && chance(effect(effects, "flint_double")) {
        add(drops, "minecraft:flint");
    }
    if LOOSE_BLOCKS.contains(&name) && chance(effect(effects, "extra_digging")) {
        duplicate_first(drops, |_| true);
    }
    if TREASURE_BLOCKS.contains(&name) && chance(effect(effects, "treasure_digging")) {
        add_treasure(effects, drops);
    }
}

fn is_log(item: &str) -> bool {
    item.ends_with("_log") || item.ends_with("_stem")
}

fn apply_woodcutting(effects: &Effects, drops: &mut Vec<ItemStack>) {
    if chance(effect(effects, "extra_log")) {
        duplicate_first(drops, is_log);
    }
    if chance(effect(effects, "charcoal")) {
        if let Some(stack) = drops.iter_mut().find(|stack| is_log(&stack.item)) {
            *stack = ItemStack::new("minecraft:charcoal", stack.count);
        }
    }
    if chance(effect(effects, "forest_gift")) {
        let rich = effect(effects, "forest_gift_rich") > 0.0 && chance(RICH_GIFT_SHARE);
        let gift = if rich {
            "minecraft:golden_apple"
        } else {
            pick(&FOREST_GIFTS)
        };
        add(drops, gift);
    }
}

fn apply_farming(name: &str, effects: &Effects, drops: &mut Vec<ItemStack>) {
    if chance(effect(effects, "extra_crop")) {
        duplicate_first(drops, |item| !item.ends_with("_seeds"));
    }
    let golden = match name {
        "carrots" => Some("minecraft:golden_carrot"),
        "melon" => Some("minecraft:glistering_melon_slice"),
        _ => None,
    };
    if let Some(item) = golden.filter(|_| chance(effect(effects, "golden_crop"))) {
        add(drops, item);
    }
}

/// Effects of a block that just earned experience in `branch`.
pub fn on_earning_block(
    table: &str,
    tool: Option<&ItemStack>,
    branch: &str,
    effects: &Effects,
    drops: &mut Vec<ItemStack>,
) {
    let Some(name) = block_name(table) else {
        return;
    };
    let silk = tool.is_some_and(|tool| tool.enchantment_level(SILK_TOUCH) > 0);
    match branch {
        "digging" if !silk => apply_digging(name, effects, drops),
        "woodcutting" => apply_woodcutting(effects, drops),
        "farming" => apply_farming(name, effects, drops),
        _ => {}
    }
}

/// Effects of a mob a player killed.
pub fn on_mob(tree: &Tree, kind: &str, effects: &Effects, drops: &mut Vec<ItemStack>) {
    if chance(effect(effects, "butcher")) {
        duplicate_first(drops, |item| {
            tree.in_list("raw_food", item) || tree.in_list("cooked_food", item)
        });
    }
    if HIDE_ANIMALS.contains(&kind) && chance(effect(effects, "butcher_hide")) {
        add(drops, "minecraft:leather");
    }
    let head = MOB_HEADS.iter().find(|(mob, _)| *mob == kind);
    if let Some((_, head)) = head.filter(|_| chance(effect(effects, "heads"))) {
        add(drops, head);
    }
}

// Experience for everything except breaking blocks: each hook names what the player did.
use crate::game::{
    Game,
    native_hooks::NativeLootDrops,
    pct_effects as effects, pct_glue as glue,
    pct_mining::player_effects,
    pct_rules::{Profile, Tree},
};
use horizon_client::items::ItemStack;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

const FISH_WINDOW: Duration = Duration::from_secs(6);
const MILK_COOLDOWN: Duration = Duration::from_secs(300);
const REBUILD_COOLDOWN: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_REMEMBERED_BUILDS: usize = 200_000;
const HIT_WINDOW: Duration = Duration::from_secs(1);
const MAX_HIT_XP_PER_WINDOW: f64 = 20.0;
const BOTTLES_PER_BREW: f64 = 3.0;
const DEFAULT_MOB_HEALTH: f64 = 20.0;
const FISH_CAUGHT: i32 = 2;
const CREATIVE: i32 = 1;
const FULL_HIVE: &str = "5";
const BREWING_MENU: &str = "minecraft:brewing_stand";
const TAME_TRIGGER: &str = "minecraft:tame_animal";
const MAX_REMEMBERED_MOBS: usize = 10_000;
const RANGED_DAMAGE_TYPES: [&str; 2] = ["minecraft:arrow", "minecraft:trident"];
const MOB_DAMAGE_TYPES: [&str; 14] = [
    "minecraft:mob_attack",
    "minecraft:mob_attack_no_aggro",
    "minecraft:mob_projectile",
    "minecraft:arrow",
    "minecraft:trident",
    "minecraft:thrown",
    "minecraft:fireball",
    "minecraft:wither_skull",
    "minecraft:sonic_boom",
    "minecraft:explosion",
    "minecraft:sting",
    "minecraft:spit",
    "minecraft:dragon_breath",
    "minecraft:wind_charge",
];

type Chunk = (String, i32, i32);

#[derive(Default)]
struct State {
    fishing: HashMap<u128, Instant>,
    brewers: HashMap<[i32; 3], u128>,
    finished_brews: Vec<[i32; 3]>,
    hits: Vec<(i32, f64)>,
    hit_budget: HashMap<u128, (Instant, f64)>,
    last_hit_ranged: HashMap<i32, bool>,
    milked: HashMap<i32, Instant>,
    built: HashMap<(String, [i32; 3]), Instant>,
    ripe: HashSet<(String, [i32; 3])>,
    current_chunk: HashMap<u128, Chunk>,
    visited: HashMap<u128, HashSet<Chunk>>,
    dirty_chunks: HashSet<u128>,
    game_modes: HashMap<u128, i32>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The acting player's mode, else the server's list; inside another player's turn (or a hook
/// with nobody's state in place) that list is empty, so the mode seen on their last tick counts.
fn is_creative(game: &Game<'_>, player: u128) -> bool {
    let acting = game
        .native_acting_player()
        .filter(|acting| acting.uuid == player);
    let found = acting.or_else(|| {
        game.native_server_players()
            .into_iter()
            .find(|p| p.uuid == player)
    });
    let mode = match found {
        Some(player) => Some(player.game_mode),
        None => state().game_modes.get(&player).copied(),
    };
    mode == Some(CREATIVE)
}

/// Pays `action` `times` over; `cross` also pays a related branch at its reduced share.
fn earn(
    game: &mut Game<'_>,
    tree: &Tree,
    player: u128,
    action: &str,
    times: f64,
    cross: Option<(&str, f64)>,
) {
    if is_creative(game, player) {
        return;
    }
    let Some((branch, base)) = tree.xp_for_action(action) else {
        return;
    };
    let branch = branch.to_owned();
    let amount = base * times;
    let change = |tree: &Tree, profile: &mut Profile| {
        tree.add_main_xp(profile, &branch, amount);
        if let Some((other, value)) = cross {
            tree.add_cross_xp(profile, other, value);
        }
    };
    glue::grant(game, tree, player, &branch, change, amount);
}

fn acting_uuid(game: &Game<'_>) -> Option<u128> {
    game.native_acting_player().map(|player| player.uuid)
}

/// A base value from another branch's table, for related work paid at the cross share.
fn base_of(tree: &Tree, action: &str) -> f64 {
    tree.xp_for_action(action).map_or(0.0, |(_, value)| value)
}

// Fishing: the catch hook does not name the item, so the next pickup classifies it.

pub fn on_player_fish(player: u128, fish_state: i32) {
    if fish_state == FISH_CAUGHT {
        state().fishing.insert(player, Instant::now());
    }
}

pub fn on_item_picked_up(game: &mut Game<'_>, player: u128, item: &str, count: u32) {
    let caught = {
        let mut state = state();
        state
            .fishing
            .remove(&player)
            .is_some_and(|at| at.elapsed() < FISH_WINDOW)
    };
    if !caught {
        return;
    }
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let action = if tree.in_list("fish", item) {
        "act:fish"
    } else if tree.in_list("treasure", item) {
        "act:treasure"
    } else {
        "act:junk"
    };
    earn(game, tree, player, action, f64::from(count), None);
}

/// For machines a player built: the tree's machine share of `action`'s experience. Experience
/// only reaches players who are online, so an offline owner earns nothing.
pub fn earn_for_owner(game: &mut Game<'_>, owner: u128, action: &str, times: f64) {
    if let Some(loaded) = glue::current_tree() {
        let share = loaded.tree.machine_share;
        earn(game, &loaded.tree, owner, action, times * share, None);
    }
}

/// A machine took `count` of `item` out of its furnace: the owner earns as for their own furnace.
pub fn on_machine_smelt(game: &mut Game<'_>, owner: u128, item: &str, count: u32) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let action = if tree.in_list("cooked_food", item) {
        "act:cook"
    } else if tree.in_list("ingots", item) {
        "act:ingot"
    } else {
        return;
    };
    earn(
        game,
        tree,
        owner,
        action,
        f64::from(count) * tree.machine_share,
        None,
    );
}

/// A machine ran `recipe` `crafts` times and made `count` of `item`: the owner earns as for their
/// own crafting, without the first-craft reward, which is for a player finding a recipe.
pub fn on_machine_craft(
    game: &mut Game<'_>,
    owner: u128,
    (recipe, item): (&str, &str),
    count: u32,
    crafts: u32,
) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    if tree.in_list("reversible_recipes", recipe) {
        return;
    }
    let share = tree.machine_share;
    let crafter = base_of(tree, "act:craft") * f64::from(crafts) * share;
    let action = if item == "minecraft:cake" {
        "act:cake"
    } else if tree.in_list("stews", item) {
        "act:stew"
    } else if tree.in_list("crafted_food", item) {
        "act:craft_food"
    } else if tree.has_suffix_in("gear_suffixes", item) {
        "act:gear"
    } else {
        earn(
            game,
            tree,
            owner,
            "act:craft",
            f64::from(crafts) * share,
            None,
        );
        return;
    };
    let times = f64::from(count) * share;
    earn(
        game,
        tree,
        owner,
        action,
        times,
        Some(("crafting", crafter)),
    );
}

/// A machine (a turret, a guard) killed mob `entity`: the owner earns as for their own ranged
/// kill, with the same cut for mobs from spawners.
pub fn on_machine_kill(game: &mut Game<'_>, owner: u128, entity: u32, kind: &str) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let health = tree
        .number("mob_health", kind)
        .unwrap_or(DEFAULT_MOB_HEALTH);
    let boss = if tree.in_list("bosses", kind) {
        base_of(tree, "act:boss")
    } else {
        1.0
    };
    let factor = crate::game::pct_fx_roam_combat::kill_xp_factor(entity);
    let times = health * boss * factor * tree.machine_share;
    earn(game, tree, owner, "act:shot", times, None);
}

// Animals.

pub fn earn_once(game: &mut Game<'_>, player: u128, action: &str, times: f64) {
    if let Some(loaded) = glue::current_tree() {
        earn(game, &loaded.tree, player, action, times, None);
    }
}

pub fn on_baby_spawn(game: &mut Game<'_>, player: u128) {
    earn_once(game, player, "act:breed", 1.0);
}

/// Wolves, cats, parrots and mounts all reach the `tame_animal` advancement as the acting player.
pub fn on_advancement_entity_trigger(game: &mut Game<'_>, trigger: &str) {
    if trigger != TAME_TRIGGER {
        return;
    }
    if let Some(player) = acting_uuid(game) {
        earn_once(game, player, "act:tame", 1.0);
    }
}

pub fn on_shear(game: &mut Game<'_>, player: u128) {
    earn_once(game, player, "act:shear", 1.0);
}

pub fn on_interact_entity(game: &mut Game<'_>, entity: i32, kind: &str, stack: Option<&ItemStack>) {
    let milking =
        kind == "minecraft:cow" && stack.is_some_and(|stack| &*stack.item == "minecraft:bucket");
    if !milking {
        return;
    }
    let ready = {
        let mut state = state();
        let ready = state
            .milked
            .get(&entity)
            .is_none_or(|at| at.elapsed() >= MILK_COOLDOWN);
        if ready {
            state.milked.insert(entity, Instant::now());
        }
        ready
    };
    if let (true, Some(player)) = (ready, acting_uuid(game)) {
        earn_once(game, player, "act:milk", 1.0);
    }
}

pub fn on_use_item_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    let (Some(player), Some(loaded)) = (acting_uuid(game), glue::current_tree()) else {
        return;
    };
    let tree = &loaded.tree;
    let block = game.native_block(pos);
    let item = &*stack.item;
    if block.ends_with("campfire") && tree.in_list("raw_food", item) {
        earn(game, tree, player, "act:campfire", 1.0, None);
    }
}

/// A beehive handles the click itself, before `use_item_on` is ever asked.
pub fn on_right_click_block(game: &mut Game<'_>, pos: [i32; 3]) {
    let (Some(player), Some(loaded)) = (acting_uuid(game), glue::current_tree()) else {
        return;
    };
    let tree = &loaded.tree;
    if !tree.in_list("hive_blocks", &game.native_block(pos)) {
        return;
    }
    let holds_tool = game
        .native_held()
        .is_some_and(|(item, _, _)| item == "minecraft:glass_bottle" || item == "minecraft:shears");
    if holds_tool && game.native_block_property(pos, "honey_level").as_deref() == Some(FULL_HIVE) {
        earn(game, tree, player, "act:honey", 1.0, None);
    }
}

// Furnaces, crafting, anvils and enchanting.

pub fn on_furnace_extract(game: &mut Game<'_>, player: u128, item: &str, count: u32) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let times = f64::from(count);
    if tree.in_list("cooked_food", item) {
        earn(game, tree, player, "act:cook", times, None);
    } else if tree.in_list("ingots", item) {
        earn(game, tree, player, "act:ingot", times, None);
    }
}

fn mark_first(player: u128, key: String) -> bool {
    glue::mark_seen(player, key)
}

pub fn on_item_crafted(game: &mut Game<'_>, player: u128, recipe: &str, stack: &ItemStack) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let item = &*stack.item;
    let count = f64::from(stack.count);
    let craft_units = crate::game::pct_fx_craft::craft_times(player);
    let crafter = base_of(tree, "act:craft") * craft_units;
    if item == "minecraft:cake" {
        earn(
            game,
            tree,
            player,
            "act:cake",
            count,
            Some(("crafting", crafter)),
        );
    } else if tree.in_list("stews", item) {
        earn(
            game,
            tree,
            player,
            "act:stew",
            count,
            Some(("crafting", crafter)),
        );
    } else if tree.in_list("crafted_food", item) {
        earn(
            game,
            tree,
            player,
            "act:craft_food",
            count,
            Some(("crafting", crafter)),
        );
    } else if tree.has_suffix_in("gear_suffixes", item) {
        earn(
            game,
            tree,
            player,
            "act:gear",
            count,
            Some(("crafting", crafter)),
        );
    } else if !tree.in_list("reversible_recipes", recipe) {
        // One craft pays once: a stack of four planks is still one piece of work.
        earn(game, tree, player, "act:craft", craft_units, None);
    }
    if !tree.in_list("reversible_recipes", recipe) && mark_first(player, format!("craft:{item}")) {
        earn(game, tree, player, "act:craft_first", 1.0, None);
    }
}

pub fn on_anvil(game: &mut Game<'_>, player: u128, right: Option<&ItemStack>) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let with_book = right.is_some_and(|stack| &*stack.item == "minecraft:enchanted_book");
    if with_book {
        earn(
            game,
            tree,
            player,
            "act:book",
            crate::game::pct_fx_craft::take_anvil_levels(player),
            Some(("smithing", base_of(tree, "act:repair"))),
        );
    } else {
        earn(game, tree, player, "act:repair", 1.0, None);
    }
}

pub fn on_enchant(game: &mut Game<'_>, player: u128, levels: i32) {
    earn_once(game, player, "act:enchant", f64::from(levels.max(0)));
}

pub fn on_eat(game: &mut Game<'_>, stack: &ItemStack) {
    let (Some(player), Some(loaded)) = (acting_uuid(game), glue::current_tree()) else {
        return;
    };
    let tree = &loaded.tree;
    let item = &*stack.item;
    if tree.in_list("cooked_food", item)
        || tree.in_list("crafted_food", item)
        || tree.in_list("stews", item)
    {
        earn(game, tree, player, "act:eat", 1.0, None);
    }
}

// Brewing: the stand does not name a player, so the last one who opened it gets the credit.

pub fn on_container_open(player: u128, menu: &str, pos: Option<[i32; 3]>) {
    if let (BREWING_MENU, Some(pos)) = (menu, pos) {
        state().brewers.insert(pos, player);
    }
}

pub fn on_brewing_complete(pos: [i32; 3]) {
    state().finished_brews.push(pos);
}

// Combat.

pub fn on_mob_killed(game: &mut Game<'_>, player: u128, entity: u32, kind: &str) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let health = tree
        .number("mob_health", kind)
        .unwrap_or(DEFAULT_MOB_HEALTH);
    let action = if tree.in_list("bosses", kind) {
        "act:boss"
    } else {
        "act:kill"
    };
    let cross = tree
        .in_list("animals", kind)
        .then_some(("husbandry", health));
    let factor = crate::game::pct_fx_roam_combat::kill_xp_factor(entity);
    earn(game, tree, player, action, health * factor, cross);
}

pub fn on_incoming_damage(entity: i32, kind: &str, damage_type: &str, amount: &mut f32) {
    if kind != "minecraft:player" {
        let mut state = state();
        if state.last_hit_ranged.len() >= MAX_REMEMBERED_MOBS {
            state.last_hit_ranged.clear();
        }
        state
            .last_hit_ranged
            .insert(entity, RANGED_DAMAGE_TYPES.contains(&damage_type));
        return;
    }
    // The hook reports no attacker for a player's damage, so the kind of damage decides.
    effects::note_hurt(entity);
    if effects::is_pvp_hit(entity) {
        return;
    }
    let from_mob = MOB_DAMAGE_TYPES.contains(&damage_type);
    if from_mob && *amount > 0.0 {
        state().hits.push((entity, f64::from(*amount)));
    }
    effects::cut_incoming(entity, damage_type, from_mob, amount);
    crate::game::pct_fx_farm::on_incoming_damage(entity, damage_type, from_mob, amount);
    crate::game::pct_fx_craft::on_incoming_damage(entity, damage_type, from_mob, amount);
    crate::game::pct_fx_roam::on_incoming_damage(entity, damage_type, from_mob, amount);
}

fn earn_mob_shot(game: &mut Game<'_>, player: u128, entity: u32, kind: &str) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let health = tree
        .number("mob_health", kind)
        .unwrap_or(DEFAULT_MOB_HEALTH);
    let boss = if tree.in_list("bosses", kind) {
        base_of(tree, "act:boss")
    } else {
        1.0
    };
    let factor = crate::game::pct_fx_roam_combat::kill_xp_factor(entity);
    earn(game, tree, player, "act:shot", health * boss * factor, None);
}

/// Handles drops that are not a block broken with a tool; true when this call dealt with them.
pub fn on_loot(
    game: &mut Game<'_>,
    context: &NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) -> bool {
    let Some(player) = context.player else {
        return false;
    };
    if let Some((entity, kind)) = context.entity.filter(|_| !context.used) {
        if state().last_hit_ranged.remove(&(entity as i32)) == Some(true) {
            earn_mob_shot(game, player, entity, kind);
        }
        if let Some(loaded) = glue::current_tree() {
            effects::on_mob(&loaded.tree, kind, &player_effects(player), drops);
        }
        return true;
    }
    let brushed = context.used
        && context
            .tool
            .is_some_and(|tool| &*tool.item == "minecraft:brush");
    if brushed && context.block.is_some() {
        earn_once(game, player, "act:brush", 1.0);
    }
    context.used
}

// Building.

fn build_action(tree: &Tree, block: &str) -> Option<&'static str> {
    if tree.in_list("not_building", block) {
        None
    } else if tree.has_suffix_in("deco_suffixes", block) {
        Some("act:deco")
    } else if tree.has_suffix_in("building_suffixes", block) {
        Some("act:build")
    } else {
        None
    }
}

fn first_build_here(dimension: &str, pos: [i32; 3]) -> bool {
    let mut state = state();
    let key = (dimension.to_owned(), pos);
    if state
        .built
        .get(&key)
        .is_some_and(|at| at.elapsed() < REBUILD_COOLDOWN)
    {
        return false;
    }
    if state.built.len() >= MAX_REMEMBERED_BUILDS {
        state.built.retain(|_, at| at.elapsed() < REBUILD_COOLDOWN);
    }
    state.built.insert(key, Instant::now());
    true
}

pub fn on_block_placed(game: &mut Game<'_>, pos: [i32; 3], block: &str) {
    let (Some(player), Some(loaded)) = (acting_uuid(game), glue::current_tree()) else {
        return;
    };
    let tree = &loaded.tree;
    let Some(action) = build_action(tree, block) else {
        return;
    };
    if first_build_here(game.native_dimension(), pos) {
        earn(game, tree, player, action, 1.0, None);
    }
}

// Crops.

/// Notes a crop being broken when it is fully grown; the loot hook comes after the block is gone.
pub fn remember_if_ripe(game: &Game<'_>, tree: &Tree, pos: [i32; 3]) {
    let block = game.native_block(pos);
    let Some(ripe_age) = tree.number("ripe_age", &block) else {
        return;
    };
    let age = game
        .native_block_property(pos, "age")
        .and_then(|age| age.parse::<f64>().ok());
    if age.is_some_and(|age| age >= ripe_age) {
        state()
            .ripe
            .insert((game.native_dimension().to_owned(), pos));
    }
}

/// Whether this break may pay: crops with an age must have been ripe.
pub fn take_ripe(tree: &Tree, table: &str, dimension: &str, pos: [i32; 3]) -> bool {
    let Some(block) = table.strip_prefix("minecraft:blocks/") else {
        return true;
    };
    if tree
        .number("ripe_age", &format!("minecraft:{block}"))
        .is_none()
    {
        return true;
    }
    state().ripe.remove(&(dimension.to_owned(), pos))
}

// Exploration.

fn chunks_file(game: &Game<'_>, player: u128) -> Option<PathBuf> {
    Some(
        glue::mod_dir(game)?
            .join("chunks")
            .join(format!("{player:032x}.json")),
    )
}

fn load_visited(game: &Game<'_>, player: u128) {
    if state().visited.contains_key(&player) {
        return;
    }
    let chunks: HashSet<Chunk> = chunks_file(game, player)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    state().visited.entry(player).or_insert(chunks);
}

pub fn save_chunks(game: &Game<'_>, player: u128) {
    let snapshot = {
        let mut state = state();
        if !state.dirty_chunks.remove(&player) {
            return;
        }
        state.visited.get(&player).cloned()
    };
    let (Some(chunks), Some(path)) = (snapshot, chunks_file(game, player)) else {
        return;
    };
    let result = serde_json::to_vec(&chunks)
        .map_err(|e| e.to_string())
        .and_then(|json| glue::write_in_folder(&path, &json));
    if let Err(error) = result {
        log::warn!("path_of_craft_tree: visited chunks not saved: {error}");
        state().dirty_chunks.insert(player);
    }
}

pub fn forget_player(player: u128) {
    let mut state = state();
    state.visited.remove(&player);
    state.current_chunk.remove(&player);
    state.dirty_chunks.remove(&player);
    state.fishing.remove(&player);
    state.hit_budget.remove(&player);
    state.game_modes.remove(&player);
}

fn enter_chunk(player: u128, chunk: &Chunk) -> Option<bool> {
    let mut state = state();
    if state.current_chunk.get(&player) == Some(chunk) {
        return None;
    }
    state.current_chunk.insert(player, chunk.clone());
    let is_new = state
        .visited
        .entry(player)
        .or_default()
        .insert(chunk.clone());
    if is_new {
        state.dirty_chunks.insert(player);
    }
    Some(is_new)
}

pub fn on_player_tick(game: &mut Game<'_>) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::PlayerTick);
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    state().game_modes.insert(acting.uuid, acting.game_mode);
    effects::on_tick(game, &acting);
    crate::game::pct_fx_gather::on_tick(game, &acting);
    crate::game::pct_fx_farm::on_tick(game, &acting);
    crate::game::pct_fx_craft::on_tick(game, &acting);
    crate::game::pct_fx_roam::on_tick(game, &acting);
    let [x, y, z] = acting.position;
    let chunk: Chunk = (acting.dimension.clone(), (x as i32) >> 4, (z as i32) >> 4);
    let player = acting.uuid;
    load_visited(game, player);
    let Some(is_new) = enter_chunk(player, &chunk) else {
        return;
    };
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    if is_new {
        earn(game, tree, player, "act:chunk", 1.0, None);
    }
    if let Some(biome) = game.native_biome([x as i32, y as i32, z as i32]) {
        if mark_first(player, format!("biome:{biome}")) {
            earn(game, tree, player, "act:biome", 1.0, None);
        }
    }
}

pub fn on_dimension_changed(game: &mut Game<'_>, player: u128, to: &str) {
    // Everyone starts in the overworld, so returning to it discovers nothing.
    if to != "minecraft:overworld" && mark_first(player, format!("dimension:{to}")) {
        earn_once(game, player, "act:dimension", 1.0);
    }
}

pub fn on_advancement(game: &mut Game<'_>, player: u128, advancement: &str) {
    // Recipe unlocks count as advancements in the game but are not achievements.
    if !advancement.contains("recipes/") && mark_first(player, format!("advancement:{advancement}"))
    {
        earn_once(game, player, "act:advancement", 1.0);
    }
}

pub fn on_trade(game: &mut Game<'_>) {
    if let Some(player) = acting_uuid(game) {
        earn_once(game, player, "act:trade", 1.0);
    }
}

// Work queued by hooks that cannot grant experience themselves.

fn drain_brews(game: &mut Game<'_>, tree: &Tree) {
    let brews = {
        let mut state = state();
        let positions = std::mem::take(&mut state.finished_brews);
        positions
            .into_iter()
            .filter_map(|pos| state.brewers.get(&pos).copied())
            .collect::<Vec<_>>()
    };
    for player in brews {
        earn(game, tree, player, "act:brew", BOTTLES_PER_BREW, None);
    }
}

/// How much of `xp` this player may still earn from being hit in the current window.
fn allowed_hit_xp(player: u128, xp: f64) -> f64 {
    let mut state = state();
    let budget = state
        .hit_budget
        .entry(player)
        .or_insert((Instant::now(), 0.0));
    if budget.0.elapsed() >= HIT_WINDOW {
        *budget = (Instant::now(), 0.0);
    }
    let allowed = xp.min(MAX_HIT_XP_PER_WINDOW - budget.1).max(0.0);
    budget.1 += allowed;
    allowed
}

fn drain_hits(game: &mut Game<'_>, tree: &Tree) {
    let hits = std::mem::take(&mut state().hits);
    if hits.is_empty() {
        return;
    }
    let players = game.native_server_players();
    let per_point = base_of(tree, "act:hit");
    for (entity, amount) in hits {
        let Some(player) = players.iter().find(|player| player.entity_id == entity) else {
            continue;
        };
        let allowed = allowed_hit_xp(player.uuid, amount * per_point);
        if allowed > 0.0 && per_point > 0.0 {
            earn(
                game,
                tree,
                player.uuid,
                "act:hit",
                allowed / per_point,
                None,
            );
        }
    }
}

pub fn on_server_tick(game: &mut Game<'_>) {
    {
        let mut state = state();
        state.ripe.clear();
    }
    if let Some(loaded) = glue::current_tree() {
        drain_brews(game, &loaded.tree);
        drain_hits(game, &loaded.tree);
        crate::game::pct_fx_roam_combat::provoke_all(game);
        crate::game::pct_fx_farm_cooking::share_cakes(game);
    }
}

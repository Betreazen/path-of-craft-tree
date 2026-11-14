// The miner branch: experience for ore and stone, and the effects of its nodes.
use crate::game::{
    Game,
    native_hooks::NativeLootDrops,
    native_player_state::NativeEffect,
    pct_actions as actions, pct_effects as effects, pct_fx_farm as farm, pct_fx_gather as gather,
    pct_glue as glue,
    pct_placed::{Position, RegionKey},
    pct_rules::{Effects, Tree},
};
use horizon_client::items::ItemStack;
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

const PICKAXE_SUFFIX: &str = "_pickaxe";
const TOOL_SUFFIXES: [&str; 4] = ["_pickaxe", "_shovel", "_axe", "_hoe"];
const DURABLE_SUFFIXES: [&str; 6] = [
    "_pickaxe",
    "_shovel",
    "_axe",
    "_hoe",
    "_fishing_rod",
    "shield",
];
const NETHER_WART_TABLE: &str = "minecraft:blocks/nether_wart";
const BLOCK_TABLE_PREFIX: &str = "minecraft:blocks/";
const FALLING_BLOCKS: [&str; 3] = ["minecraft:sand", "minecraft:red_sand", "minecraft:gravel"];
const NEIGHBOURS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
const TICKS_PER_SECOND: f64 = 20.0;

/// Set while the mod breaks a vein, so those breaks do not start more veins.
pub static BREAKING_VEIN: AtomicBool = AtomicBool::new(false);
static RANDOM: AtomicU64 = AtomicU64::new(0);

/// A number in 0..1 from a small xorshift generator; gameplay chances need nothing stronger.
pub fn random() -> f64 {
    let mut x = RANDOM.load(Ordering::Relaxed);
    if x == 0 {
        x = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |t| t.as_nanos() as u64)
            | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    RANDOM.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

pub fn chance(probability: f64) -> bool {
    probability > 0.0 && random() < probability
}

pub fn effect(effects: &Effects, key: &str) -> f64 {
    effects.get(key).copied().unwrap_or(0.0)
}

fn read_region(dir: Option<PathBuf>) -> impl FnOnce(&RegionKey) -> Option<Vec<u8>> {
    move |key| std::fs::read(dir?.join(key.file_name())).ok()
}

fn to_position(pos: [i32; 3]) -> Position {
    (pos[0], pos[1], pos[2])
}

/// Remembers a placed block, but only one that could earn experience when broken.
pub fn on_place(game: &mut Game<'_>, pos: [i32; 3]) {
    let block = game.native_block(pos);
    actions::on_block_placed(game, pos, &block);
    if FALLING_BLOCKS.iter().any(|falling| block == *falling) {
        count_placed_falling(game, &block);
        return;
    }
    mark_placed(game, game.native_dimension(), pos, &block);
}

/// Whether breaking `block` could pay, so placing it needs a mark.
fn earns_when_broken(tree: &Tree, block: &str) -> bool {
    // A planted crop must still pay when it ripens; `take_ripe` already refuses unripe ones.
    let is_crop = tree.number("ripe_age", block).is_some();
    !is_crop && block_table(block).is_some_and(|table| tree.earns_xp(&table))
        || block.ends_with("_leaves")
        || tree.in_list("herbs", block)
}

/// Marks a block a player or a machine put at `pos`, and the top half of a tall herb.
fn mark_placed(game: &Game<'_>, dimension: &str, pos: [i32; 3], block: &str) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    if !earns_when_broken(&loaded.tree, block) {
        return;
    }
    let dir = glue::placed_dir(game);
    let mut state = glue::server();
    state
        .placed
        .add(dimension, to_position(pos), read_region(dir.clone()));
    if loaded.tree.in_list("tall_herbs", block) {
        let top = (pos[0], pos[1] + 1, pos[2]);
        state.placed.add(dimension, top, read_region(dir));
    }
}

/// A piston's move, kept from the level's hook until the server tick can reach the marks' files.
struct PistonMove {
    dimension: &'static str,
    direction: [i32; 3],
    pushed: Vec<[i32; 3]>,
    destroyed: Vec<[i32; 3]>,
}

static PISTON_MOVES: Mutex<Vec<PistonMove>> = Mutex::new(Vec::new());

/// A piston is about to push or pull the blocks at `pushed` one step `direction` and destroy those
/// at `destroyed`. A placed block keeps its mark where it lands, so a piston can no longer turn a
/// placed ore back into one that pays. Applied on the server tick (`apply_piston_moves`).
pub fn on_piston_move(
    dimension: &'static str,
    direction: [i32; 3],
    pushed: Vec<[i32; 3]>,
    destroyed: Vec<[i32; 3]>,
) {
    if pushed.is_empty() && destroyed.is_empty() {
        return;
    }
    PISTON_MOVES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(PistonMove {
            dimension,
            direction,
            pushed,
            destroyed,
        });
}

/// Moves the marks of the blocks pistons moved since the last tick, and drops those of the blocks
/// they destroyed. Blocks move together: every mark is lifted before any is set down.
pub fn apply_piston_moves(game: &Game<'_>) {
    let moves = std::mem::take(&mut *PISTON_MOVES.lock().unwrap_or_else(PoisonError::into_inner));
    if moves.is_empty() {
        return;
    }
    let dir = glue::placed_dir(game);
    let mut state = glue::server();
    for step in moves {
        let mut lifted = Vec::new();
        for pos in &step.pushed {
            let position = to_position(*pos);
            if state
                .placed
                .contains(step.dimension, position, read_region(dir.clone()))
            {
                state
                    .placed
                    .remove(step.dimension, position, read_region(dir.clone()));
                lifted.push(*pos);
            }
        }
        for pos in &step.destroyed {
            state
                .placed
                .remove(step.dimension, to_position(*pos), read_region(dir.clone()));
        }
        for pos in lifted {
            let landed = [
                pos[0] + step.direction[0],
                pos[1] + step.direction[1],
                pos[2] + step.direction[2],
            ];
            state.placed.add(
                step.dimension,
                to_position(landed),
                read_region(dir.clone()),
            );
        }
    }
}

/// A machine put `block` at `pos` (a block placer, a growing ore vein): it pays nothing when
/// broken, as a block a player placed.
pub fn on_machine_place(game: &Game<'_>, dimension: &str, pos: [i32; 3], block: &str) {
    if FALLING_BLOCKS.contains(&block) {
        return;
    }
    mark_placed(game, dimension, pos, block);
}

/// A machine broke `block` at `pos`. A placed block loses its mark and pays nothing; any other
/// pays the owner the machine share of what breaking it with `tool` pays, while they are online.
/// The owner's tree effects (extra drops, wider mining) do not apply to machines.
pub fn on_machine_break(
    game: &mut Game<'_>,
    owner: u128,
    dimension: &str,
    pos: [i32; 3],
    block: &str,
    tool: crate::game::pct_machine_rules::Tool,
) {
    let position = to_position(pos);
    let dir = glue::placed_dir(game);
    let was_placed = {
        let mut state = glue::server();
        let placed = state
            .placed
            .contains(dimension, position, read_region(dir.clone()));
        if placed {
            state.placed.remove(dimension, position, read_region(dir));
        }
        placed
    };
    if was_placed {
        return;
    }
    let (Some(loaded), Some(table)) = (glue::current_tree(), block_table(block)) else {
        return;
    };
    let tree = &loaded.tree;
    let Some((branch, amount)) = tree.xp_for_break(&table, tool.item()) else {
        return;
    };
    let share = tree.machine_share;
    let cross = break_cross(tree, &table, 0).map(|(branch, xp)| (branch, xp * share));
    let branch = branch.to_owned();
    award_break(game, tree, owner, &branch, amount * share, cross);
}

/// A falling block lands away from its mark, so each one placed cancels one later reward instead.
/// The count lives in the player's profile, so it survives a restart.
fn count_placed_falling(game: &Game<'_>, block: &str) {
    if let Some(acting) = game.native_acting_player() {
        glue::count_placed_falling(acting.uuid, block);
    }
}

/// Whether this broken falling block is paid for by one the player placed earlier.
fn take_placed_falling(player: u128, table: &str) -> bool {
    let Some(block) = table
        .strip_prefix(BLOCK_TABLE_PREFIX)
        .map(|name| format!("minecraft:{name}"))
        .filter(|block| FALLING_BLOCKS.contains(&block.as_str()))
    else {
        return false;
    };
    glue::take_placed_falling(player, &block)
}

/// Forgets the placed mark of the block being broken and remembers it for the loot hook.
fn forget_placed(game: &Game<'_>, pos: [i32; 3]) {
    let dimension = game.native_dimension();
    let position = to_position(pos);
    let dir = glue::placed_dir(game);
    let mut state = glue::server();
    if state
        .placed
        .contains(dimension, position, read_region(dir.clone()))
    {
        state.placed.remove(dimension, position, read_region(dir));
        state.just_broken.insert((dimension.to_owned(), position));
    }
}

fn is_placed(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let dir = glue::placed_dir(game);
    glue::server()
        .placed
        .contains(game.native_dimension(), to_position(pos), read_region(dir))
}

/// The other ore blocks of the vein at `origin`, nearest first, at most `limit`.
fn find_vein(game: &Game<'_>, origin: [i32; 3], block: &str, limit: usize) -> Vec<[i32; 3]> {
    let mut seen = HashSet::from([origin]);
    let mut frontier = VecDeque::from([origin]);
    let mut vein = Vec::new();
    while let Some(at) = frontier.pop_front() {
        for offset in NEIGHBOURS {
            let next = [at[0] + offset[0], at[1] + offset[1], at[2] + offset[2]];
            if vein.len() >= limit {
                return vein;
            }
            if seen.insert(next) && game.native_block(next) == block && !is_placed(game, next) {
                vein.push(next);
                frontier.push_back(next);
            }
        }
    }
    vein
}

fn held_pickaxe(game: &Game<'_>) -> bool {
    game.native_held()
        .is_some_and(|(item, _, _)| item.ends_with(PICKAXE_SUFFIX))
}

pub fn player_effects(player: u128) -> Arc<Effects> {
    glue::server()
        .players
        .get(&player)
        .map(|entry| entry.effects.clone())
        .unwrap_or_default()
}

pub fn block_table(block: &str) -> Option<String> {
    let path = block.strip_prefix("minecraft:")?;
    Some(format!("{BLOCK_TABLE_PREFIX}{path}"))
}

fn start_vein(game: &mut Game<'_>, player: u128, tree: &Tree, pos: [i32; 3]) {
    let length = effect(&player_effects(player), "vein_len") as usize;
    if length == 0 || !held_pickaxe(game) {
        return;
    }
    let block = game.native_block(pos);
    let is_ore = block_table(&block).is_some_and(|table| tree.ores.contains_key(&table));
    if !is_ore {
        return;
    }
    let vein = find_vein(game, pos, &block, length.saturating_sub(1));
    let _breaking = VeinGuard::start();
    for next in vein {
        // The pickaxe can wear out partway: the rest of the vein stays in the ground.
        if !held_pickaxe(game) {
            break;
        }
        game.native_player_break_block(player, next, false);
    }
}

/// Marks a vein as being broken until dropped, even if a break panics.
struct VeinGuard;

impl VeinGuard {
    fn start() -> VeinGuard {
        BREAKING_VEIN.store(true, Ordering::Relaxed);
        VeinGuard
    }
}

impl Drop for VeinGuard {
    fn drop(&mut self) {
        BREAKING_VEIN.store(false, Ordering::Relaxed);
    }
}

pub fn on_destroy(game: &mut Game<'_>, pos: [i32; 3]) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::Destroy);
    forget_placed(game, pos);
    if let Some(loaded) = glue::current_tree() {
        actions::remember_if_ripe(game, &loaded.tree, pos);
    }
    if BREAKING_VEIN.load(Ordering::Relaxed) || gather::in_chain() {
        return;
    }
    let (Some(player), Some(loaded)) = (
        game.native_acting_player().map(|acting| acting.uuid),
        glue::current_tree(),
    ) else {
        return;
    };
    start_vein(game, player, &loaded.tree, pos);
}

fn add_item(drops: &mut Vec<ItemStack>, item: &str) {
    drops.push(ItemStack::new(item, 1));
}

fn count_item(drops: &[ItemStack], item: &str) -> u32 {
    drops
        .iter()
        .filter(|stack| &*stack.item == item)
        .map(|stack| stack.count)
        .sum()
}

/// Turns raw metal into ingots; the number of ingots made.
fn smelt_drops(drops: &mut [ItemStack], raw: &str, ingot: &str) -> u32 {
    let mut made = 0;
    for stack in drops.iter_mut().filter(|stack| &*stack.item == raw) {
        made += stack.count;
        *stack = ItemStack::new(ingot, stack.count);
    }
    made
}

fn start_rush(game: &mut Game<'_>, player: u128, effects: &Effects) {
    let seconds = effect(effects, "rush_seconds");
    if seconds <= 0.0 {
        return;
    }
    let duration = (seconds * TICKS_PER_SECOND) as i32;
    let give = |id: &str, amplifier: u8| NativeEffect {
        id: id.to_owned(),
        amplifier,
        duration,
        ambient: false,
        particles: false,
    };
    game.native_add_effect(
        player,
        &give("minecraft:haste", effect(effects, "rush_amp") as u8),
    );
    if effect(effects, "rush_speed") > 0.0 {
        game.native_add_effect(player, &give("minecraft:speed", 0));
    }
}

/// Applies the ore effects to `drops`; the ingots the player got straight from the ore.
fn apply_ore_effects(
    tree: &Tree,
    table: &str,
    effects: &Effects,
    drops: &mut Vec<ItemStack>,
) -> u32 {
    let Some(ore) = tree.ores.get(table) else {
        return 0;
    };
    if count_item(drops, &ore.drop) == 0 {
        return 0;
    }
    if chance(effect(effects, "extra_ore")) {
        add_item(drops, &ore.drop);
    }
    if ore.rich && chance(effect(effects, "greed")) {
        add_item(drops, &ore.drop);
    }
    let (Some(raw), Some(ingot)) = (&ore.raw, &ore.ingot) else {
        return 0;
    };
    if effect(effects, "smelt") <= 0.0 {
        return 0;
    }
    let made = smelt_drops(drops, raw, ingot);
    if chance(effect(effects, "smelt_extra")) {
        add_item(drops, ingot);
    }
    made
}

fn award_break(
    game: &mut Game<'_>,
    tree: &Tree,
    player: u128,
    branch: &str,
    amount: f64,
    cross: Option<(String, f64)>,
) {
    let change = |tree: &Tree, profile: &mut crate::game::pct_rules::Profile| {
        tree.add_main_xp(profile, branch, amount);
        if let Some((other, value)) = &cross {
            tree.add_cross_xp(profile, other, *value);
        }
    };
    glue::grant(game, tree, player, branch, change, amount);
}

/// The related branch a broken block also pays: smelting straight from the ore, nether wart.
pub fn break_cross(tree: &Tree, table: &str, ingots: u32) -> Option<(String, f64)> {
    if ingots > 0 {
        let cross = tree.smelt_cross.as_ref()?;
        return Some((cross.branch.clone(), cross.xp * f64::from(ingots)));
    }
    if table == NETHER_WART_TABLE {
        let (branch, xp) = tree.xp_for_action("act:nether_wart")?;
        return Some((branch.to_owned(), xp));
    }
    None
}

pub fn on_loot(game: &mut Game<'_>, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::Loot);
    if actions::on_loot(game, context, drops) {
        return;
    }
    let (Some(player), Some((pos, _))) = (context.player, context.block) else {
        return;
    };
    let placed_key = (context.dimension.to_owned(), to_position(pos));
    if glue::server().just_broken.remove(&placed_key)
        || context.table.is_empty()
        || take_placed_falling(player, context.table)
    {
        return;
    }
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let effects = player_effects(player);
    effects::on_any_block(tree, context.table, &effects, drops);
    gather::on_any_block_drops(tree, context.table, &effects, drops);
    let tool = context.tool.map_or("", |tool| &*tool.item);
    let Some((branch, amount)) = tree.xp_for_break(context.table, tool) else {
        return;
    };
    if !actions::take_ripe(tree, context.table, context.dimension, pos) {
        return;
    }
    effects::on_earning_block(context.table, context.tool, branch, &effects, drops);
    gather::on_block_drops(tree, context.table, context.tool, branch, &effects, drops);
    farm::on_block_drops(tree, context.table, context.tool, branch, &effects, drops);
    let ingots = apply_ore_effects(tree, context.table, &effects, drops);
    if tree.ores.contains_key(context.table) {
        start_rush(game, player, &effects);
    }
    let cross = break_cross(tree, context.table, ingots);
    let branch = branch.to_owned();
    award_break(game, tree, player, &branch, amount, cross);
}

fn durability_key(item: &str) -> Option<String> {
    DURABLE_SUFFIXES
        .iter()
        .find(|suffix| item.ends_with(**suffix))
        .map(|suffix| format!("durability:{suffix}"))
}

pub fn on_item_damaged(player: u128, item: &str, amount: &mut i32, cancel: &mut bool) {
    let effects = player_effects(player);
    let saved = durability_key(item).map_or(0.0, |key| effect(&effects, &key));
    if chance(saved) {
        *cancel = true;
        return;
    }
    if BREAKING_VEIN.load(Ordering::Relaxed) {
        let cost = (1.0 - effect(&effects, "vein_cost_cut")).clamp(0.0, 1.0);
        let kept = f64::from(*amount) * cost;
        let whole = kept.floor();
        *amount = whole as i32 + i32::from(chance(kept - whole));
    }
}

pub fn on_break_speed(game: &Game<'_>, speed: &mut f32) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::BreakSpeed);
    let Some(key) = game.native_held().and_then(|(item, _, _)| {
        TOOL_SUFFIXES
            .iter()
            .find(|suffix| item.ends_with(**suffix))
            .map(|suffix| format!("speed:{suffix}"))
    }) else {
        return;
    };
    // The server reads the digging player's progress; a remote client predicts its own.
    let bonus = match game.native_acting_player() {
        Some(player) => glue::server()
            .players
            .get(&player.uuid)
            .and_then(|e| e.effects.get(&key).copied()),
        None => glue::client().effects.get(&key).copied(),
    };
    *speed *= 1.0 + bonus.unwrap_or(0.0) as f32;
}

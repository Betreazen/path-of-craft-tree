// Gathering branches: the shared chain machinery and the drop, wear and tick effects.
use crate::game::{
    Game,
    native_hooks::NativeLootDrops,
    pct_fx_gather_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects},
    pct_placed::RegionKey,
    pct_rules::{Effects, Tree},
};
use horizon_client::{items::ItemStack, mod_api::NativeServerPlayer};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU8, Ordering},
    },
};

pub const KIND_NONE: u8 = 0;
pub const KIND_MINE_AREA: u8 = 1;
pub const KIND_DIG_AREA: u8 = 2;
pub const KIND_COLLAPSE: u8 = 3;
pub const KIND_TIMBER: u8 = 4;
pub const KIND_LEAVES: u8 = 5;
/// A break the mod makes that is no chain of its own, such as levelling a path.
pub const KIND_UTILITY: u8 = 6;

pub const SET_BLOCK_FLAGS: i32 = 3;

const BLOCK_PREFIX: &str = "minecraft:blocks/";
const SILK_TOUCH: &str = "minecraft:silk_touch";
const MOVEMENT_EFFICIENCY: &str = "minecraft:movement_efficiency";
const WORK_PERIOD_TICKS: u32 = 8;
const VANILLA_PICKUP_RADIUS: f64 = 1.5;
const PICKUP_PULL_SPEED: f64 = 0.35;
const PILE_DISTANCE: f64 = 2.0;
const PILE_PICKUP_DELAY: u32 = 20;
const CANOPY_APPLE_SHARE: f64 = 0.5;

/// Which chain the mod is breaking right now; its own breaks must not start more chains.
static CHAIN: AtomicU8 = AtomicU8::new(KIND_NONE);

pub enum Job {
    Break {
        pos: [i32; 3],
        kind: u8,
        block: String,
    },
    Plant {
        pos: [i32; 3],
        block: String,
        grow: f64,
    },
}

#[derive(Default)]
struct Earned {
    branch: String,
    planked: bool,
}

#[derive(Default)]
struct State {
    queue: HashMap<u128, VecDeque<Job>>,
    ticks: HashMap<u128, u32>,
    resin: HashMap<u128, u32>,
    repairs: HashMap<u128, u32>,
    cross_xp: HashMap<u128, f64>,
    efficient: HashSet<u128>,
    earned: Option<Earned>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// True while a chain of this module is breaking blocks.
pub fn in_chain() -> bool {
    CHAIN.load(Ordering::Relaxed) != KIND_NONE
}

/// Marks a chain as running until dropped, even if a break panics.
pub struct ChainGuard {
    previous: u8,
}

impl ChainGuard {
    pub fn start(kind: u8) -> ChainGuard {
        ChainGuard {
            previous: CHAIN.swap(kind, Ordering::Relaxed),
        }
    }
}

impl Drop for ChainGuard {
    fn drop(&mut self) {
        CHAIN.store(self.previous, Ordering::Relaxed);
    }
}

pub fn is_placed(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let dir = glue::placed_dir(game);
    let reader = move |key: &RegionKey| std::fs::read(dir?.join(key.file_name())).ok();
    glue::server()
        .placed
        .contains(game.native_dimension(), (pos[0], pos[1], pos[2]), reader)
}

/// Whether the block being broken was placed: its mark was already moved aside for the loot hook.
pub fn broken_was_placed(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let key = (game.native_dimension().to_owned(), (pos[0], pos[1], pos[2]));
    glue::server().just_broken.contains(&key)
}

/// The hook behind every block a player breaks: decides which chain, if any, starts.
pub fn on_destroy(game: &mut Game<'_>, pos: [i32; 3]) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::Destroy);
    let vein_running = crate::game::pct_mining::BREAKING_VEIN.load(Ordering::Relaxed);
    if in_chain() || vein_running {
        return;
    }
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let effects = player_effects(acting.uuid);
    if effects.is_empty() || acting.game_mode == 1 {
        return;
    }
    let Some((item, _, _)) = game.native_held() else {
        return;
    };
    if rules::is_pickaxe(&item) {
        crate::game::pct_fx_gather_dig::start_mine_area(game, &acting, pos, &effects);
    } else if rules::is_shovel(&item) {
        crate::game::pct_fx_gather_dig::start_shovel_chains(game, &acting, pos, &effects);
    } else if rules::is_axe(&item) {
        crate::game::pct_fx_gather_tree::start_tree(game, &acting, pos, &effects);
    }
}

// The job queue spreads a big tree over several ticks.

pub fn enqueue(player: u128, jobs: Vec<Job>) {
    if !jobs.is_empty() {
        state().queue.entry(player).or_default().extend(jobs);
    }
}

/// Runs the first breaks at once and leaves the rest, and any planting, for the next ticks.
pub fn run_chain(game: &mut Game<'_>, player: u128, jobs: Vec<Job>) {
    let mut jobs: VecDeque<Job> = jobs.into();
    for _ in 0..rules::BREAKS_PER_TICK {
        if !matches!(jobs.front(), Some(Job::Break { .. })) {
            break;
        }
        let Some(job) = jobs.pop_front() else {
            break;
        };
        if !run_job(game, player, job) {
            return;
        }
    }
    enqueue(player, jobs.into());
}

fn holds_axe(game: &Game<'_>) -> bool {
    game.native_held()
        .is_some_and(|(item, _, _)| rules::is_axe(&item))
}

/// Runs one job; false when the rest of the queue no longer makes sense.
fn run_job(game: &mut Game<'_>, player: u128, job: Job) -> bool {
    match job {
        Job::Break { pos, kind, block } => {
            if kind == KIND_TIMBER && !holds_axe(game) {
                return false;
            }
            if game.native_block(pos) == block && !is_placed(game, pos) {
                let _chain = ChainGuard::start(kind);
                game.native_player_break_block(player, pos, false);
            }
            true
        }
        Job::Plant { pos, block, grow } => {
            plant(game, pos, &block, grow);
            true
        }
    }
}

fn plant(game: &mut Game<'_>, pos: [i32; 3], block: &str, grow: f64) {
    let below = [pos[0], pos[1] - 1, pos[2]];
    if game.native_block(pos) != "minecraft:air" || !rules::is_soil(&game.native_block(below)) {
        return;
    }
    let Some(sapling) = game.native_default_state(block) else {
        return;
    };
    let dimension = game.native_dimension();
    if game.native_set_block(dimension, pos, sapling, SET_BLOCK_FLAGS) && chance(grow) {
        game.native_set_block_property(pos, "stage", "1");
    }
}

fn drain_jobs(game: &mut Game<'_>, player: u128) {
    let batch: Vec<Job> = {
        let mut state = state();
        let Some(queue) = state.queue.get_mut(&player) else {
            return;
        };
        let count = queue.len().min(rules::BREAKS_PER_TICK);
        queue.drain(..count).collect()
    };
    for job in batch {
        if !run_job(game, player, job) {
            state().queue.remove(&player);
            return;
        }
    }
    let mut state = state();
    if state.queue.get(&player).is_some_and(VecDeque::is_empty) {
        state.queue.remove(&player);
    }
}

// Work that needs the game but was learned about in a hook without it.

fn apply_repairs(game: &mut Game<'_>, player: u128) {
    let Some(points) = state().repairs.remove(&player) else {
        return;
    };
    let slot = game.selected as i32;
    if let Some(stack) = game.native_carried_stack(slot) {
        if rules::is_axe(&stack.item) {
            stack.damage = stack.damage.saturating_sub(points);
        }
    }
}

fn grant_cross(game: &mut Game<'_>, player: u128) {
    let Some(xp) = state().cross_xp.remove(&player) else {
        return;
    };
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let change = |tree: &Tree, profile: &mut crate::game::pct_rules::Profile| {
        tree.add_cross_xp(profile, "crafting", xp);
    };
    glue::grant(game, &loaded.tree, player, "crafting", change, xp);
}

fn sync_light_step(game: &mut Game<'_>, player: u128, effects: &Effects) {
    let wanted = effect(effects, "light_step") > 0.0;
    let applied = state().efficient.contains(&player);
    if wanted == applied {
        return;
    }
    let base = if wanted { 1.0 } else { 0.0 };
    if game.native_set_attribute_base(player, MOVEMENT_EFFICIENCY, base) {
        let mut state = state();
        if wanted {
            state.efficient.insert(player);
        } else {
            state.efficient.remove(&player);
        }
    }
}

/// Items inside the extended radius drift to the player; the game's own pickup then takes them.
fn attract_items(game: &mut Game<'_>, acting: &NativeServerPlayer, bonus: f64) {
    if acting.dimension != game.native_dimension() {
        return;
    }
    let [x, y, z] = acting.position;
    let radius = VANILLA_PICKUP_RADIUS + bonus;
    for item in game.dropped.iter_mut() {
        let (dx, dz) = (x - item.position.x, z - item.position.z);
        let in_vanilla_reach = dx.abs() < VANILLA_PICKUP_RADIUS && dz.abs() < VANILLA_PICKUP_RADIUS;
        let in_height = item.position.y > y - 1.0 && item.position.y < y + 2.4 + bonus;
        if item.pickup_delay > 0
            || item.target.is_some()
            || in_vanilla_reach
            || !in_height
            || dx.abs() >= radius
            || dz.abs() >= radius
        {
            continue;
        }
        let length = (dx * dx + dz * dz).sqrt().max(0.01);
        item.velocity.x = dx / length * PICKUP_PULL_SPEED;
        item.velocity.z = dz / length * PICKUP_PULL_SPEED;
        item.needs_sync = true;
    }
}

/// Called for every player each tick.
pub fn on_tick(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let player = acting.uuid;
    drain_jobs(game, player);
    crate::game::pct_fx_gather_fish::deliver_lava_catch(game, acting);
    let due = {
        let mut state = state();
        let ticks = state.ticks.entry(player).or_default();
        *ticks = ticks.wrapping_add(1);
        ticks.is_multiple_of(WORK_PERIOD_TICKS)
    };
    if !due {
        return;
    }
    apply_repairs(game, player);
    grant_cross(game, player);
    let effects = player_effects(player);
    sync_light_step(game, player, &effects);
    let bonus = effect(&effects, "pickup_radius");
    if bonus > 0.0 {
        attract_items(game, acting, bonus);
    }
}

pub fn forget_player(player: u128) {
    crate::game::pct_fx_gather_fish::forget_player(player);
    let mut state = state();
    state.queue.remove(&player);
    state.ticks.remove(&player);
    state.resin.remove(&player);
    state.repairs.remove(&player);
    state.cross_xp.remove(&player);
    state.efficient.remove(&player);
}

// Drops.

fn block_name(table: &str) -> Option<&str> {
    table.strip_prefix(BLOCK_PREFIX)
}

fn is_silk(tool: Option<&ItemStack>) -> bool {
    tool.is_some_and(|tool| tool.enchantment_level(SILK_TOUCH) > 0)
}

fn apply_glass(name: &str, effects: &Effects, drops: &mut Vec<ItemStack>) {
    if !matches!(name, "sand" | "red_sand") || effect(effects, "glass") <= 0.0 {
        return;
    }
    let mut made = false;
    for stack in drops
        .iter_mut()
        .filter(|stack| stack.item.ends_with("sand"))
    {
        *stack = ItemStack::new("minecraft:glass", stack.count);
        made = true;
    }
    if made && chance(effect(effects, "glass_extra")) {
        drops.push(ItemStack::new("minecraft:glass", 1));
    }
}

/// Turns logs into planks; true when something was converted.
fn apply_planks(effects: &Effects, drops: &mut Vec<ItemStack>) -> bool {
    if effect(effects, "planks") <= 0.0 {
        return false;
    }
    let per_log = 4 + effect(effects, "planks_extra") as u32;
    let mut made = false;
    for stack in drops.iter_mut() {
        if let Some(planks) = rules::planks_of(&stack.item) {
            *stack = ItemStack::new(&planks, stack.count * per_log);
            made = true;
        }
    }
    if made && chance(effect(effects, "planks_sticks")) {
        drops.push(ItemStack::new("minecraft:stick", 2));
    }
    made
}

/// A natural block earned experience in `branch`; the shared flow calls this.
pub fn on_block_drops(
    _tree: &Tree,
    table: &str,
    tool: Option<&ItemStack>,
    branch: &str,
    effects: &Effects,
    drops: &mut Vec<ItemStack>,
) {
    let Some(name) = block_name(table) else {
        return;
    };
    let mut planked = false;
    match branch {
        "digging" if !is_silk(tool) => apply_glass(name, effects, drops),
        "woodcutting" => planked = apply_planks(effects, drops),
        _ => {}
    }
    state().earned = Some(Earned {
        branch: branch.to_owned(),
        planked,
    });
}

/// Any natural block broken; only the leaves of a falling crown care.
pub fn on_any_block_drops(
    _tree: &Tree,
    table: &str,
    effects: &Effects,
    drops: &mut Vec<ItemStack>,
) {
    if CHAIN.load(Ordering::Relaxed) != KIND_LEAVES
        || !block_name(table).is_some_and(|name| name.ends_with("_leaves"))
    {
        return;
    }
    if chance(effect(effects, "canopy_bonus")) {
        let item = if chance(CANOPY_APPLE_SHARE) {
            "minecraft:apple"
        } else {
            "minecraft:stick"
        };
        drops.push(ItemStack::new(item, 1));
    }
    if chance(effect(effects, "canopy_golden")) {
        drops.push(ItemStack::new("minecraft:golden_apple", 1));
    }
}

/// What the earlier loot hook learned, consumed once per drop.
fn settle_earned(player: u128, effects: &Effects) {
    let Some(earned) = state().earned.take() else {
        return;
    };
    if earned.branch != "woodcutting" {
        return;
    }
    if earned.planked {
        let xp = glue::current_tree()
            .and_then(|loaded| loaded.tree.xp_for_action("act:craft").map(|(_, xp)| xp))
            .unwrap_or(0.0);
        *state().cross_xp.entry(player).or_default() += xp;
    }
    let Some(every) = rules::resin_period(&|key: &str| effect(effects, key)) else {
        return;
    };
    let mut state = state();
    let count = state.resin.entry(player).or_default();
    *count += 1;
    if *count >= every {
        *count = 0;
        *state.repairs.entry(player).or_default() += 1;
    }
}

fn into_inventory(game: &mut Game<'_>, player: u128, drops: &mut Vec<ItemStack>) {
    let taken = std::mem::take(drops);
    for stack in taken {
        if let Some(left) = game.native_give_stack(player, stack) {
            drops.push(left);
        }
    }
}

/// Stacks the drops at one spot in front of the player.
fn pile_up(game: &mut Game<'_>, acting: &NativeServerPlayer, drops: &mut Vec<ItemStack>) {
    let radians = f64::from(acting.yaw).to_radians();
    let [x, y, z] = acting.position;
    let spot = [
        x - radians.sin() * PILE_DISTANCE,
        y + 0.5,
        z + radians.cos() * PILE_DISTANCE,
    ];
    for stack in std::mem::take(drops) {
        game.spawn_item(stack, spot, [0.0; 3], PILE_PICKUP_DELAY);
    }
}

fn apply_brush_extra(context: &NativeLootDrops<'_>, effects: &Effects, drops: &mut Vec<ItemStack>) {
    let brushed = context
        .tool
        .is_some_and(|tool| &*tool.item == "minecraft:brush");
    if brushed && context.block.is_some() && chance(effect(effects, "brush_extra")) {
        let extra = drops.clone();
        drops.extend(extra);
    }
}

/// Runs after the shared loot flow, so the drops it sees are final.
pub fn on_final_loot(
    game: &mut Game<'_>,
    context: &NativeLootDrops<'_>,
    drops: &mut Vec<ItemStack>,
) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::Loot);
    let Some(player) = context.player else {
        return;
    };
    let effects = player_effects(player);
    settle_earned(player, &effects);
    if context.used {
        apply_brush_extra(context, &effects, drops);
        return;
    }
    if context.block.is_none() {
        return;
    }
    let Some(acting) = game
        .native_acting_player()
        .filter(|acting| acting.uuid == player)
    else {
        return;
    };
    let tool = context.tool.map_or("", |tool| &*tool.item);
    let kind = CHAIN.load(Ordering::Relaxed);
    let from_tree = kind == KIND_TIMBER || kind == KIND_LEAVES;
    if rules::is_pickaxe(tool) && effect(&effects, "magnet") > 0.0 {
        into_inventory(game, player, drops);
    } else if from_tree && effect(&effects, "stack_inventory") > 0.0 {
        into_inventory(game, player, drops);
    } else if from_tree && effect(&effects, "stack_pile") > 0.0 {
        pile_up(game, &acting, drops);
    }
}

// Wear.

/// What a chain does to one point of wear; None cancels the wear.
fn chain_wear_factor(kind: u8, effects: &Effects) -> Option<f64> {
    match kind {
        KIND_MINE_AREA => Some(effect(effects, "mine_area_cost")),
        KIND_DIG_AREA => Some(effect(effects, "dig_area_cost")),
        KIND_COLLAPSE => Some(1.0 - effect(effects, "collapse_cost_cut")),
        KIND_TIMBER => Some(1.0 - effect(effects, "timber_cost_cut")),
        KIND_LEAVES => None,
        _ => Some(1.0),
    }
}

pub fn on_item_damaged(player: u128, item: &str, amount: &mut i32, cancel: &mut bool) {
    let effects = player_effects(player);
    if item == "minecraft:fishing_rod" {
        let saved = effect(&effects, "fish_luck") + effect(&effects, "durability:_fishing_rod");
        if chance(saved) {
            *cancel = true;
        }
        return;
    }
    let kind = CHAIN.load(Ordering::Relaxed);
    if kind == KIND_NONE {
        return;
    }
    match chain_wear_factor(kind, &effects) {
        None => *cancel = true,
        Some(factor) => {
            *amount = rules::scaled_wear(*amount, factor, crate::game::pct_mining::random())
        }
    }
}

// Digging speed and Minecraft experience.

fn current_effect(game: &Game<'_>, key: &str) -> f64 {
    match game.native_acting_player() {
        Some(player) => effect(&player_effects(player.uuid), key),
        None => glue::client().effects.get(key).copied().unwrap_or(0.0),
    }
}

/// Archaeologist: whether this player's brush strokes come twice as often.
pub fn brush_fast(game: &Game<'_>) -> bool {
    current_effect(game, "brush_fast") > 0.0
}

/// Firm Hand: obsidian and ancient debris dig faster.
pub fn on_break_speed(game: &Game<'_>, block: &str, speed: &mut f32) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::BreakSpeed);
    let hard = matches!(block, "minecraft:obsidian" | "minecraft:ancient_debris");
    let deep = block == "minecraft:deepslate" || block.starts_with("minecraft:deepslate_");
    if !hard && !deep {
        return;
    }
    if !game
        .native_held()
        .is_some_and(|(item, _, _)| rules::is_pickaxe(&item))
    {
        return;
    }
    let factor = current_effect(game, "hard_hand");
    if factor <= 0.0 || (deep && current_effect(game, "hard_hand_deepslate") <= 0.0) {
        return;
    }
    *speed *= 1.0 + factor as f32;
}

/// More Minecraft experience from ore, or all of it at once with the Magnet upgrade.
pub fn on_block_experience(game: &mut Game<'_>, block: &str, experience: u32) -> u32 {
    if experience == 0 || !block.ends_with("_ore") {
        return experience;
    }
    let Some(player) = game.native_acting_player().map(|acting| acting.uuid) else {
        return experience;
    };
    let effects = player_effects(player);
    let scaled = f64::from(experience) * (1.0 + effect(&effects, "mc_xp"));
    let whole = scaled.floor();
    let total = whole as u32 + u32::from(chance(scaled - whole));
    if effect(&effects, "magnet_xp") <= 0.0 {
        return total;
    }
    let silk = game
        .held()
        .is_some_and(|tool| tool.enchantment_level(SILK_TOUCH) > 0);
    if !silk {
        game.native_give_experience(player, total);
    }
    0
}

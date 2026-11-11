// Builder nodes: tools acting on blocks and the bookkeeping of blocks a player placed.
use crate::game::{
    Game,
    native_hooks::NativeLootDrops,
    pct_fx_craft_rules as rules, pct_glue as glue,
    pct_mining::{self as mining, chance, effect, player_effects},
    pct_rules::{Effects, Tree},
};
use horizon_client::{
    first_person::Hand, items::ItemStack, mod_api::NativeServerPlayer, text::Text,
};
use horizon_world::registry::BlockStateId;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

const AIR_BLOCKS: [&str; 3] = ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"];
const SCAFFOLDING: &str = "minecraft:scaffolding";
const STICK: &str = "minecraft:stick";
const PAPER: &str = "minecraft:paper";
const SET_BLOCK_FLAGS: i32 = 3;
const CREATIVE: i32 = 1;
const MAX_OWN_BLOCKS: usize = 4_096;
const SCAFFOLD_SPEED: f32 = 3.0;
const INSTANT_SPEED: f32 = 30.0;
const NORMAL_USE_DELAY: f64 = 4.0;
const FAST_USE_DELAY: u32 = 2;
const BUILD_SPEED_WEIGHT: f64 = 3.0;
const OFF_HAND_WINDOW_TICKS: u64 = 4;
/// Blocks without a block entity that still open a menu: paper must not paste over them.
const MENU_BLOCKS: [&str; 9] = [
    "minecraft:crafting_table",
    "minecraft:cartography_table",
    "minecraft:smithing_table",
    "minecraft:stonecutter",
    "minecraft:loom",
    "minecraft:grindstone",
    "minecraft:anvil",
    "minecraft:chipped_anvil",
    "minecraft:damaged_anvil",
];

struct Blueprint {
    size: [i32; 3],
    cells: Vec<([i32; 3], BlockStateId, String)>,
}

#[derive(Default)]
struct Own {
    set: HashSet<[i32; 3]>,
    order: VecDeque<[i32; 3]>,
}

#[derive(Default)]
struct State {
    own: HashMap<u128, HashMap<String, Own>>,
    fill_points: HashMap<u128, [i32; 3]>,
    print_points: HashMap<u128, [i32; 3]>,
    blueprints: HashMap<u128, Blueprint>,
    last_click: Option<(u128, [i32; 3], u64, bool)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);
/// Set while a row is placed, so the row's own blocks do not start more rows.
static PLACING_LINE: AtomicBool = AtomicBool::new(false);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn is_air(block: &str) -> bool {
    AIR_BLOCKS.contains(&block)
}

fn add(pos: [i32; 3], by: [i32; 3], times: i32) -> [i32; 3] {
    [
        pos[0] + by[0] * times,
        pos[1] + by[1] * times,
        pos[2] + by[2] * times,
    ]
}

fn survival(acting: &NativeServerPlayer) -> bool {
    acting.game_mode != CREATIVE
}

fn tell(game: &mut Game<'_>, player: u128, line: &str) {
    game.native_action_bar(player, Text::literal(line));
}

fn remember_own(player: u128, dimension: &str, pos: [i32; 3]) {
    let mut state = state();
    let own = state
        .own
        .entry(player)
        .or_default()
        .entry(dimension.to_owned())
        .or_default();
    if own.set.insert(pos) {
        own.order.push_back(pos);
    }
    if own.order.len() > MAX_OWN_BLOCKS {
        if let Some(oldest) = own.order.pop_front() {
            own.set.remove(&oldest);
        }
    }
}

fn is_own(player: u128, dimension: &str, pos: [i32; 3]) -> bool {
    state()
        .own
        .get(&player)
        .and_then(|by_dimension| by_dimension.get(dimension))
        .is_some_and(|own| own.set.contains(&pos))
}

fn forget_own(player: u128, dimension: &str, pos: [i32; 3]) {
    let mut state = state();
    let own = state
        .own
        .get_mut(&player)
        .and_then(|by_dimension| by_dimension.get_mut(dimension));
    if let Some(own) = own {
        if own.set.remove(&pos) {
            own.order.retain(|known| *known != pos);
        }
    }
}

/// Places a block the way a player would count it: marked as placed, and paying building work.
fn place(game: &mut Game<'_>, dimension: &str, pos: [i32; 3], state: BlockStateId) -> bool {
    if !game.native_set_block(dimension, pos, state, SET_BLOCK_FLAGS) {
        return false;
    }
    mining::on_place(game, pos);
    true
}

fn tracks_own(effects: &Effects) -> bool {
    ["own_break_instant", "own_break_speed", "own_to_inv"]
        .iter()
        .any(|key| effect(effects, key) > 0.0)
}

fn place_line(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    stack: &ItemStack,
    pos: [i32; 3],
    state: BlockStateId,
    length: i32,
    dimension: &str,
) {
    let direction = rules::facing(acting.yaw, acting.pitch);
    let mut spare = if survival(acting) {
        // The click's own block has not left the stack yet.
        game.native_count_items(&stack.item).saturating_sub(1)
    } else {
        u32::MAX
    };
    PLACING_LINE.store(true, Ordering::Relaxed);
    for step in 1..=length {
        let next = add(pos, direction, step);
        if spare == 0 || !is_air(&game.native_block(next)) {
            break;
        }
        if !place(game, dimension, next, state) {
            break;
        }
        if survival(acting) {
            game.native_take_items(&stack.item, 1);
            spare -= 1;
        }
    }
    PLACING_LINE.store(false, Ordering::Relaxed);
}

/// A player placed `stack` at `pos`: the row, the saved block and the record of who built it.
pub fn on_place(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], state: BlockStateId) {
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let effects = player_effects(acting.uuid);
    let dimension = game.native_dimension().to_owned();
    if tracks_own(&effects) {
        remember_own(acting.uuid, &dimension, pos);
    }
    let length = effect(&effects, "line_len") as i32;
    if acting.sneaking && length > 0 && !PLACING_LINE.load(Ordering::Relaxed) {
        place_line(game, &acting, stack, pos, state, length, &dimension);
    }
    if !survival(&acting) {
        return;
    }
    let scaffold = &*stack.item == SCAFFOLDING && effect(&effects, "scaffold_free") > 0.0;
    if scaffold || chance(effect(&effects, "block_save")) {
        // Given back on a later tick: within this one the stack ends where it began, and the
        // client, which already took the block off, would never hear that it came back.
        crate::game::pct_fx_craft::give_later(
            acting.uuid,
            ItemStack {
                count: 1,
                ..stack.clone()
            },
        );
    }
}

// The tools: a stick marks a plane, paper keeps a blueprint, dyes recolour, and a stick in the
// other hand turns a block into the one held.

fn held(game: &Game<'_>) -> Option<(String, bool)> {
    game.native_held().map(|(item, _, plain)| (item, plain))
}

fn block_of(game: &Game<'_>, item: &str) -> Option<BlockStateId> {
    game.items().get(item).and_then(|info| info.block)
}

fn fill_plane(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    a: [i32; 3],
    b: [i32; 3],
    area: usize,
) {
    let Some(item) = game.offhand.clone().filter(|stack| stack.extra.is_none()) else {
        tell(game, acting.uuid, "Заливка: нужен блок в другой руке");
        return;
    };
    let Some(block) = block_of(game, &item.item) else {
        tell(game, acting.uuid, "Заливка: в другой руке не блок");
        return;
    };
    let Some(cells) = rules::plane_between(a, b, area) else {
        tell(game, acting.uuid, "Заливка: слишком большая площадь");
        return;
    };
    let empty: Vec<[i32; 3]> = cells
        .into_iter()
        .filter(|cell| is_air(&game.native_block(*cell)))
        .collect();
    let count = empty.len() as u32;
    if survival(acting) && game.native_count_items(&item.item) < count {
        tell(game, acting.uuid, "Заливка: не хватает блоков");
        return;
    }
    let dimension = game.native_dimension().to_owned();
    for cell in empty {
        if place(game, &dimension, cell, block) && survival(acting) {
            game.native_take_items(&item.item, 1);
        }
    }
}

fn on_fill_tool(game: &mut Game<'_>, acting: &NativeServerPlayer, pos: [i32; 3], area: usize) {
    let first = state().fill_points.remove(&acting.uuid);
    match first {
        None => {
            state().fill_points.insert(acting.uuid, pos);
            tell(game, acting.uuid, "Заливка: первая точка");
        }
        Some(corner) => fill_plane(game, acting, corner, pos, area),
    }
}

fn capture_blueprint(game: &Game<'_>, a: [i32; 3], b: [i32; 3]) -> Blueprint {
    let low = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let high = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let mut cells = Vec::new();
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                let name = game.native_block([x, y, z]);
                if !is_air(&name) {
                    let offset = [x - low[0], y - low[1], z - low[2]];
                    cells.push((offset, game.world.block(x, y, z), name));
                }
            }
        }
    }
    Blueprint {
        size: [high[0] - low[0], high[1] - low[1], high[2] - low[2]],
        cells,
    }
}

fn on_blueprint_corner(game: &mut Game<'_>, acting: &NativeServerPlayer, pos: [i32; 3], size: i32) {
    let first = state().print_points.remove(&acting.uuid);
    let Some(corner) = first else {
        state().print_points.insert(acting.uuid, pos);
        tell(game, acting.uuid, "Чертёж: первый угол");
        return;
    };
    if !rules::box_fits(corner, pos, size) {
        tell(game, acting.uuid, "Чертёж: слишком большая постройка");
        return;
    }
    let print = capture_blueprint(game, corner, pos);
    let line = format!("Чертёж сохранён: блоков {}", print.cells.len());
    state().blueprints.insert(acting.uuid, print);
    tell(game, acting.uuid, &line);
}

/// How many of each item a paste needs, for the cells whose block has an item at all.
fn items_needed(
    game: &Game<'_>,
    cells: &[([i32; 3], BlockStateId, String)],
) -> HashMap<String, u32> {
    let mut needed: HashMap<String, u32> = HashMap::new();
    for (_, _, name) in cells {
        if game.items().get(name).is_some() {
            *needed.entry(name.clone()).or_default() += 1;
        }
    }
    needed
}

fn missing_item(game: &mut Game<'_>, needed: &HashMap<String, u32>) -> Option<String> {
    needed
        .iter()
        .find(|(item, count)| game.native_count_items(item) < **count)
        .map(|(item, _)| item.clone())
}

fn paste_blueprint(game: &mut Game<'_>, acting: &NativeServerPlayer, anchor: [i32; 3]) {
    let cells = {
        let state = state();
        match state.blueprints.get(&acting.uuid) {
            Some(print) => print.cells.clone(),
            None => {
                drop(state);
                tell(game, acting.uuid, "Чертёж: сначала сохраните постройку");
                return;
            }
        }
    };
    let needed = items_needed(game, &cells);
    if survival(acting) {
        if let Some(item) = missing_item(game, &needed) {
            tell(game, acting.uuid, &format!("Чертёж: не хватает {item}"));
            return;
        }
    }
    let dimension = game.native_dimension().to_owned();
    for (offset, block, name) in cells {
        let target = add(anchor, offset, 1);
        let free = is_air(&game.native_block(target));
        if free && place(game, &dimension, target, block) && survival(acting) {
            game.native_take_items(&name, 1);
        }
    }
}

fn is_replaceable(tree: &Tree, block: &str) -> bool {
    !tree.in_list("not_building", block)
        && (tree.has_suffix_in("building_suffixes", block)
            || tree.has_suffix_in("deco_suffixes", block))
}

struct Swap<'a> {
    target: &'a str,
    held: &'a str,
    block: BlockStateId,
}

fn swap_block(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    cell: [i32; 3],
    swap: &Swap<'_>,
) -> bool {
    let free = !survival(acting) || game.native_take_items(swap.held, 1) == 1;
    if !free {
        return false;
    }
    let dimension = game.native_dimension().to_owned();
    if !place(game, &dimension, cell, swap.block) {
        return false;
    }
    if survival(acting) && game.items().get(swap.target).is_some() {
        game.give_or_drop(ItemStack::new(swap.target, 1));
    }
    true
}

fn on_replace(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    tree: &Tree,
    pos: [i32; 3],
    face: [i32; 3],
    held_item: &str,
    effects: &Effects,
) -> bool {
    let target = game.native_block(pos);
    let Some(block) = block_of(game, held_item).filter(|_| target != held_item) else {
        return false;
    };
    if !is_replaceable(tree, &target) {
        return false;
    }
    let cells = if effect(effects, "replace_area") > 0.0 {
        rules::square_around(pos, face)
    } else {
        vec![pos]
    };
    let swap = Swap {
        target: &target,
        held: held_item,
        block,
    };
    for cell in cells {
        if game.native_block(cell) == target && !swap_block(game, acting, cell, &swap) {
            break;
        }
    }
    true
}

fn on_palette(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    pos: [i32; 3],
    face: [i32; 3],
    dye: &str,
    effects: &Effects,
) -> bool {
    let Some(color) = rules::dye_color(dye) else {
        return false;
    };
    let area = acting.sneaking && effect(effects, "palette_area") > 0.0;
    let cells = if area {
        rules::square_around(pos, face)
    } else {
        vec![pos]
    };
    let dimension = game.native_dimension().to_owned();
    let mut changed = false;
    for cell in cells {
        let block = game.native_block(cell);
        let new_state = rules::recolored(&block, color).and_then(|name| block_of(game, &name));
        if let Some(new_state) = new_state {
            changed |= game.native_set_block(&dimension, cell, new_state, SET_BLOCK_FLAGS);
        }
    }
    if changed && survival(acting) {
        game.native_take_items(dye, 1);
    }
    changed
}

/// Whether the off hand's packet belongs to a click the main hand just used a tool for.
fn off_hand_follows_tool(game: &Game<'_>, player: u128, pos: [i32; 3]) -> bool {
    state().last_click.is_some_and(|(p, at, tick, used)| {
        used && p == player && at == pos && game.ticks.saturating_sub(tick) <= OFF_HAND_WINDOW_TICKS
    })
}

fn note_main_click(game: &Game<'_>, player: u128, pos: [i32; 3]) {
    state().last_click = Some((player, pos, game.ticks, false));
}

fn mark_tool_used() {
    if let Some(click) = state().last_click.as_mut() {
        click.3 = true;
    }
}

fn use_paper(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    pos: [i32; 3],
    face: [i32; 3],
    size: i32,
) -> bool {
    if acting.sneaking {
        on_blueprint_corner(game, acting, pos, size);
        return true;
    }
    let saved = state().blueprints.contains_key(&acting.uuid);
    if !saved || opens_menu(game, pos) {
        return false;
    }
    paste_blueprint(game, acting, add(pos, face, 1));
    true
}

fn opens_menu(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let has_entity = game
        .world
        .block_entities
        .get(&(pos[0], pos[1], pos[2]))
        .is_some();
    has_entity || MENU_BLOCKS.contains(&game.native_block(pos).as_str())
}

fn holds_stick_in_offhand(game: &Game<'_>) -> bool {
    game.offhand
        .as_ref()
        .is_some_and(|stack| &*stack.item == STICK)
}

fn use_tool(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    pos: [i32; 3],
    face: [i32; 3],
    item: &str,
    effects: &Effects,
) -> bool {
    if item == STICK && acting.sneaking && effect(effects, "fill_area") > 0.0 {
        // The clicked block itself is usually solid: the plane starts on the side that was clicked.
        on_fill_tool(
            game,
            acting,
            add(pos, face, 1),
            effect(effects, "fill_area") as usize,
        );
        return true;
    }
    if item == PAPER && effect(effects, "blueprint") > 0.0 {
        return use_paper(game, acting, pos, face, effect(effects, "blueprint") as i32);
    }
    if effect(effects, "palette") > 0.0 && on_palette(game, acting, pos, face, item, effects) {
        return true;
    }
    let replacing = acting.sneaking
        && effect(effects, "replace") > 0.0
        && block_of(game, item).is_some()
        && holds_stick_in_offhand(game);
    let Some(loaded) = glue::current_tree().filter(|_| replacing) else {
        return false;
    };
    on_replace(game, acting, &loaded.tree, pos, face, item, effects)
}

/// A right click: the hooks hears both hands, the first acts and the second is stopped with it.
pub fn on_right_click(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let effects = player_effects(acting.uuid);
    if effects.is_empty() {
        return;
    }
    // A client sends the main hand and the off hand as two packets, sometimes ticks apart. The
    // held item is always the main hand's, so only the main hand's call may act.
    if game.hand != Hand::Main {
        *canceled |= off_hand_follows_tool(game, acting.uuid, pos);
        return;
    }
    note_main_click(game, acting.uuid, pos);
    let Some((item, true)) = held(game) else {
        return;
    };
    if use_tool(game, &acting, pos, face, &item, &effects) {
        mark_tool_used();
        *canceled = true;
    }
}

/// Runs for each digging tick on both sides; a client only knows the scaffolding part.
pub fn on_break_speed(game: &Game<'_>, pos: [i32; 3], speed: &mut f32) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::BreakSpeed);
    let acting = game.native_acting_player();
    let effects = match &acting {
        Some(acting) => player_effects(acting.uuid),
        None => glue::client().effects.clone(),
    };
    if effects.is_empty() {
        return;
    }
    if effect(&effects, "scaffold_speed") > 0.0 && game.native_block(pos) == SCAFFOLDING {
        *speed *= SCAFFOLD_SPEED;
    }
    let Some(acting) = acting else {
        return;
    };
    if !tracks_own(&effects) || !is_own(acting.uuid, game.native_dimension(), pos) {
        return;
    }
    *speed *= 1.0 + effect(&effects, "own_break_speed") as f32;
    if effect(&effects, "own_break_instant") > 0.0 {
        *speed *= INSTANT_SPEED;
    }
}

/// A broken own block can go straight to the inventory.
pub fn on_loot(game: &mut Game<'_>, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::Loot);
    let (Some(player), Some((pos, _))) = (context.player, context.block) else {
        return;
    };
    if !is_own(player, context.dimension, pos) {
        return;
    }
    forget_own(player, context.dimension, pos);
    if effect(&player_effects(player), "own_to_inv") <= 0.0 {
        return;
    }
    let taken = std::mem::take(drops);
    for stack in taken {
        if let Some(rest) = game.native_give_stack(player, stack) {
            drops.push(rest);
        }
    }
}

/// On the client: holding blocks shortens the wait between placements.
pub fn on_client_frame(game: &mut Game<'_>) {
    let (build_speed, scaffold) = {
        let client = glue::client();
        (
            effect(&client.effects, "build_speed"),
            effect(&client.effects, "scaffold_speed"),
        )
    };
    if (build_speed <= 0.0 && scaffold <= 0.0) || !game.native_playing() {
        return;
    }
    let Some((item, _, _)) = game.native_held() else {
        return;
    };
    if block_of(game, &item).is_none() {
        return;
    }
    let fast = item == SCAFFOLDING && scaffold > 0.0;
    let target = if fast {
        FAST_USE_DELAY
    } else {
        ((NORMAL_USE_DELAY * (1.0 - build_speed * BUILD_SPEED_WEIGHT)).round() as u32).max(1)
    };
    if game.use_delay > target {
        game.use_delay = target;
    }
}

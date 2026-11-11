// Work that needs the network host: pocket stations, mass crafting and chests feeding the grid.
use crate::game::{
    Game,
    lan::LanHost,
    pct_mining::{effect, player_effects},
    stations::{Kind, Station},
};
use horizon_client::{containers::BlockEntity, items::ItemStack};
use horizon_protocol::packets::play::ContainerClick;
use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const GRID_CELLS: usize = 9;
const QUICK_MOVE: i32 = 1;
const RESULT_SLOT: i16 = 0;
const STALE_STATE: i32 = -1;
const DEFAULT_STACK: u32 = 64;

struct Pocket {
    pos: [i32; 3],
    kind: Option<Kind>,
}

#[derive(Default)]
struct State {
    wanted: Vec<u128>,
    pockets: HashMap<u128, Pocket>,
    last_was_loom: HashSet<u128>,
    grids: HashMap<u128, Vec<Option<ItemStack>>>,
    refill: HashSet<u128>,
    mass: Vec<(u32, ContainerClick, u32)>,
    mass_running: bool,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn request_pocket(player: u128) {
    state().wanted.push(player);
}

pub fn pocket_table_valid(player: u128, pos: [i32; 3]) -> bool {
    state()
        .pockets
        .get(&player)
        .is_some_and(|pocket| pocket.kind.is_none() && pocket.pos == pos)
}

pub fn pocket_station_valid(player: u128, kind: Kind, pos: [i32; 3]) -> bool {
    state()
        .pockets
        .get(&player)
        .is_some_and(|pocket| pocket.kind == Some(kind) && pocket.pos == pos)
}

/// Remembers a finished craft's grid, so it can be laid out again.
pub fn note_grid(player: u128, ingredients: &[Option<ItemStack>]) {
    let mut state = state();
    state.grids.insert(player, ingredients.to_vec());
    if effect(&player_effects(player), "nearby") > 0.0 {
        state.refill.insert(player);
    }
}

/// A shift-click on the result: the same click repeats on the next tick.
pub fn on_crafting_click(host: &LanHost, conn: u32, click: &ContainerClick) {
    if click.slot != RESULT_SLOT || click.input != QUICK_MOVE || state().mass_running {
        return;
    }
    let Some(player) = host
        .hub
        .guests
        .iter()
        .find(|g| g.conn == conn)
        .map(|g| g.uuid)
    else {
        return;
    };
    let rounds = effect(&player_effects(player), "mass_craft") as u32;
    if rounds > 0 {
        state().mass.push((conn, click.clone(), rounds));
    }
}

fn block_pos(position: [f64; 3]) -> [i32; 3] {
    position.map(|value| value.floor() as i32)
}

fn stations_to_open(player: u128, sneaking: bool) -> Option<Kind> {
    if !sneaking || effect(&player_effects(player), "pocket_stations") <= 0.0 {
        return None;
    }
    let mut state = state();
    if state.last_was_loom.remove(&player) {
        Some(Kind::Stonecutter)
    } else {
        state.last_was_loom.insert(player);
        Some(Kind::Loom)
    }
}

fn open_pocket(game: &mut Game<'_>, host: &mut LanHost, player: u128) {
    if effect(&player_effects(player), "pocket_table") <= 0.0 {
        return;
    }
    let Some((conn, pos, sneaking)) = host
        .hub
        .guests
        .iter()
        .find(|g| g.uuid == player)
        .map(|g| (g.conn, block_pos(g.position), g.sneaking))
    else {
        return;
    };
    let kind = stations_to_open(player, sneaking);
    state().pockets.insert(player, Pocket { pos, kind });
    match kind {
        Some(kind) => {
            let station = Station::new_kind(kind, pos);
            game.lan_open_station(host, conn, &station);
        }
        None => game.lan_open_crafting(host, conn, pos),
    }
}

fn take_matching(slots: &mut [Option<ItemStack>], want: &ItemStack, limit: u32) -> u32 {
    let mut taken = 0;
    for slot in slots.iter_mut() {
        let matches = slot
            .as_ref()
            .is_some_and(|s| s.same_item_same_components(want));
        if !matches || taken >= limit {
            continue;
        }
        if let Some(stack) = slot.as_mut() {
            let moved = stack.count.min(limit - taken);
            stack.count -= moved;
            taken += moved;
        }
        if slot.as_ref().is_some_and(|stack| stack.count == 0) {
            *slot = None;
        }
    }
    taken
}

fn take_from_chests(
    game: &mut Game<'_>,
    center: [i32; 3],
    radius: i32,
    want: &ItemStack,
    limit: u32,
) -> u32 {
    let mut taken = 0;
    for x in center[0] - radius..=center[0] + radius {
        for y in center[1] - radius..=center[1] + radius {
            for z in center[2] - radius..=center[2] + radius {
                if taken >= limit {
                    return taken;
                }
                if let Some(BlockEntity::Chest(items)) =
                    game.world.block_entities.get_mut(&(x, y, z))
                {
                    let moved = take_matching(items, want, limit - taken);
                    if moved > 0 {
                        taken += moved;
                        game.world.modified.insert((x >> 4, z >> 4));
                    }
                }
            }
        }
    }
    taken
}

fn take_from_inventory(host: &mut LanHost, gi: usize, want: &ItemStack, limit: u32) -> u32 {
    take_matching(&mut host.hub.guests[gi].inventory, want, limit)
}

fn count_matching(slots: &[Option<ItemStack>], want: &ItemStack) -> u32 {
    slots
        .iter()
        .flatten()
        .filter(|stack| stack.same_item_same_components(want))
        .map(|stack| stack.count)
        .sum()
}

fn count_in_chests(game: &Game<'_>, center: [i32; 3], radius: i32, want: &ItemStack) -> u32 {
    let mut total = 0;
    for x in center[0] - radius..=center[0] + radius {
        for y in center[1] - radius..=center[1] + radius {
            for z in center[2] - radius..=center[2] + radius {
                if let Some(BlockEntity::Chest(items)) = game.world.block_entities.get(&(x, y, z)) {
                    total += count_matching(items, want);
                }
            }
        }
    }
    total
}

/// A cell of the last grid to fill again, and the most it may take: an equal share of what is
/// there for every cell of that item, so that the pattern of the recipe survives.
struct Refill {
    cell: usize,
    want: ItemStack,
    share: u32,
}

fn empty_cell(host: &LanHost, conn: u32, cell: usize) -> bool {
    host.play
        .get(&conn)
        .and_then(|play| play.crafting.table.as_ref())
        .is_some_and(|table| table.2.slot(cell + 1).is_none())
}

fn plan_refills(
    game: &Game<'_>,
    host: &LanHost,
    conn: u32,
    gi: usize,
    ingredients: &[Option<ItemStack>],
    chests: Option<([i32; 3], i32)>,
    inventory_too: bool,
) -> Vec<Refill> {
    let wanted: Vec<(usize, &ItemStack)> = ingredients
        .iter()
        .take(GRID_CELLS)
        .enumerate()
        .filter_map(|(cell, want)| want.as_ref().map(|want| (cell, want)))
        .filter(|(cell, _)| empty_cell(host, conn, *cell))
        .collect();
    wanted
        .iter()
        .map(|(cell, want)| {
            let cells = wanted
                .iter()
                .filter(|(_, other)| other.same_item_same_components(want))
                .count() as u32;
            let mut available = 0;
            if inventory_too {
                available += count_matching(&host.hub.guests[gi].inventory, want);
            }
            if let Some((center, radius)) = chests {
                available += count_in_chests(game, center, radius, want);
            }
            let stack = game
                .items()
                .get(&want.item)
                .map_or(DEFAULT_STACK, |item| item.max_stack);
            Refill {
                cell: *cell,
                want: (*want).clone(),
                share: (available / cells).min(stack),
            }
        })
        .collect()
}

/// Lays the last grid out again from the inventory and, with the node, from nearby chests.
fn refill_cells(game: &mut Game<'_>, host: &mut LanHost, conn: u32, inventory_too: bool) -> bool {
    let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else {
        return false;
    };
    let player = host.hub.guests[gi].uuid;
    let Some(ingredients) = state().grids.get(&player).cloned() else {
        return false;
    };
    let radius = effect(&player_effects(player), "nearby") as i32;
    let Some(table_pos) = host
        .play
        .get(&conn)
        .and_then(|play| play.crafting.table.as_ref())
        .map(|table| table.3)
    else {
        return false;
    };
    let chests = (radius > 0).then_some((table_pos, radius));
    let plan = plan_refills(game, host, conn, gi, &ingredients, chests, inventory_too);
    let mut refills = Vec::new();
    for Refill { cell, want, share } in plan {
        let mut got = 0;
        if inventory_too {
            got += take_from_inventory(host, gi, &want, share);
        }
        if got < share && radius > 0 {
            got += take_from_chests(game, table_pos, radius, &want, share - got);
        }
        if got > 0 {
            refills.push((cell, ItemStack { count: got, ..want }));
        }
    }
    if refills.is_empty() {
        return false;
    }
    let env = game.menu_env();
    let Some(table) = host
        .play
        .get_mut(&conn)
        .and_then(|play| play.crafting.table.as_mut())
    else {
        return false;
    };
    for (cell, stack) in refills {
        table.2.set_slot(&env, cell + 1, Some(stack));
    }
    true
}

fn run_mass_crafts(game: &mut Game<'_>, host: &mut LanHost) {
    let work = std::mem::take(&mut state().mass);
    for (conn, click, rounds) in work {
        state().mass_running = true;
        for _ in 0..rounds {
            if !refill_cells(game, host, conn, true) {
                break;
            }
            let again = ContainerClick {
                state: STALE_STATE,
                ..click.clone()
            };
            if !game.lan_crafting_click(host, conn, &again) {
                break;
            }
        }
        state().mass_running = false;
    }
}

fn run_chest_refills(game: &mut Game<'_>, host: &mut LanHost) -> Vec<u32> {
    let players: Vec<u128> = state().refill.drain().collect();
    let mut changed = Vec::new();
    for player in players {
        let conn = host
            .hub
            .guests
            .iter()
            .find(|g| g.uuid == player)
            .map(|g| g.conn);
        if let Some(conn) = conn.filter(|conn| refill_cells(game, host, *conn, false)) {
            changed.push(conn);
        }
    }
    changed
}

/// Each server tick; returns the guests whose crafting table must be sent again.
pub fn run(game: &mut Game<'_>, host: &mut LanHost) -> Vec<u32> {
    let wanted = std::mem::take(&mut state().wanted);
    for player in wanted {
        open_pocket(game, host, player);
    }
    run_mass_crafts(game, host);
    run_chest_refills(game, host)
}

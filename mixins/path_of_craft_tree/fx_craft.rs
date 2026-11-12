// The craft sector's shared state and the entry points the bundle's shared flow calls.
use crate::game::{
    Game, pct_fx_craft_rules as rules,
    pct_keys::{self as keys, Key},
    pct_mining::{effect, player_effects},
};
use horizon_client::{items::ItemStack, mod_api::NativeServerPlayer, text::Text};
use horizon_world::player_inventory::EquipmentSlot;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
};

pub const KEY_CHANNEL: &str = "path_of_craft_tree:craft_key";
pub const KEY_POCKET: u8 = 0;
pub const KEY_SHARPEN: u8 = 1;
const MAX_QUEUED_CRAFTS: usize = 64;
/// Ticks a gift waits: long enough that the client hears the change after its own prediction.
const GIFT_DELAY_TICKS: u8 = 2;
const MAX_BODIES: usize = 4_096;
const REACH_ATTRIBUTE: &str = "minecraft:block_interaction_range";
const SLOW_FALLING: &str = "minecraft:slow_falling";
const SLOW_FALL_TICKS: i32 = 40;
const SLOW_CHECK_EVERY: u32 = 10;
const ARMOR_CHECK_EVERY: u32 = 20;
const ARMOR_SLOTS: [EquipmentSlot; 4] = [
    EquipmentSlot::Head,
    EquipmentSlot::Chest,
    EquipmentSlot::Legs,
    EquipmentSlot::Feet,
];
const FALL_DAMAGE: &str = "minecraft:fall";
const ARMOR_PIECES: f32 = 4.0;

#[derive(Default)]
struct Body {
    holds_block: bool,
    armor_pieces: u8,
    chain_mail: f64,
    no_fall: bool,
}

#[derive(Default)]
struct State {
    ticks: HashMap<u128, u32>,
    bodies: HashMap<i32, Body>,
    reach_sent: HashMap<u128, (i32, f64)>,
    craft_ingredients: HashMap<u128, Vec<u32>>,
    anvil_levels: HashMap<u128, i32>,
    gifts: HashMap<u128, Vec<(ItemStack, u8)>>,
    book_gifts: HashMap<u128, Vec<ItemStack>>,
    lines: HashMap<u128, Vec<String>>,
    sharpen: HashSet<u128>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A player's effects; on a client the server's table is empty, so its own are used.
pub fn effects_of(player: u128) -> Arc<crate::game::pct_rules::Effects> {
    let effects = player_effects(player);
    if effects.is_empty() {
        crate::game::pct_glue::client().effects.clone()
    } else {
        effects
    }
}

// Experience hand-offs: the integrator's shared flow reads these.

/// Remembers how many ingredients a grid craft used, for the experience of that craft.
pub fn note_craft_ingredients(player: u128, ingredients: u32) {
    let mut state = state();
    let queue = state.craft_ingredients.entry(player).or_default();
    if queue.len() >= MAX_QUEUED_CRAFTS {
        queue.remove(0);
    }
    queue.push(ingredients);
}

/// Crafter experience units for the oldest unpaid craft of this player.
pub fn craft_times(player: u128) -> f64 {
    let ingredients = {
        let mut state = state();
        match state.craft_ingredients.get_mut(&player) {
            Some(queue) if !queue.is_empty() => queue.remove(0),
            _ => 1,
        }
    };
    rules::craft_xp_times(ingredients, effect(&player_effects(player), "craft_xp"))
}

pub fn note_anvil_levels(player: u128, levels: i32) {
    state().anvil_levels.insert(player, levels.max(0));
}

/// The levels the anvil take that is being reported just charged; 0 when none is known.
pub fn take_anvil_levels(player: u128) -> f64 {
    f64::from(state().anvil_levels.remove(&player).unwrap_or(0))
}

// Things for a player's next turn, from hooks that do not have the player's state in place.

pub fn give_later(player: u128, stack: ItemStack) {
    state()
        .gifts
        .entry(player)
        .or_default()
        .push((stack, GIFT_DELAY_TICKS));
}

/// A reward paid for with a plain book from the player's inventory.
pub fn give_for_book_later(player: u128, stack: ItemStack) {
    state().book_gifts.entry(player).or_default().push(stack);
}

pub fn tell_later(player: u128, line: String) {
    state().lines.entry(player).or_default().push(line);
}

pub fn queue_sharpen(player: u128) {
    state().sharpen.insert(player);
}

/// The gifts whose wait is over; the rest stay for a later tick.
fn due_gifts(state: &mut State, player: u128) -> Vec<ItemStack> {
    let Some(queue) = state.gifts.get_mut(&player) else {
        return Vec::new();
    };
    for (_, wait) in queue.iter_mut() {
        *wait = wait.saturating_sub(1);
    }
    let (due, waiting): (Vec<_>, Vec<_>) = std::mem::take(queue)
        .into_iter()
        .partition(|(_, wait)| *wait == 0);
    if waiting.is_empty() {
        state.gifts.remove(&player);
    } else {
        *queue = waiting;
    }
    due.into_iter().map(|(stack, _)| stack).collect()
}

fn deliver_later(game: &mut Game<'_>, player: u128) {
    let (gifts, books, lines, sharpen) = {
        let mut state = state();
        if state.gifts.is_empty()
            && state.book_gifts.is_empty()
            && state.lines.is_empty()
            && state.sharpen.is_empty()
        {
            return;
        }
        (
            due_gifts(&mut state, player),
            state.book_gifts.remove(&player).unwrap_or_default(),
            state.lines.remove(&player).unwrap_or_default(),
            state.sharpen.remove(&player),
        )
    };
    for stack in gifts {
        game.give_or_drop(stack);
    }
    for stack in books {
        if game.native_take_items("minecraft:book", 1) == 1 {
            game.give_or_drop(stack);
        }
    }
    for line in lines {
        game.native_tell(player, Text::literal(line));
    }
    if sharpen {
        crate::game::pct_fx_craft_items::sharpen_held(game, player);
    }
}

fn apply_reach(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let effects = player_effects(acting.uuid);
    let wanted =
        rules::BASE_BLOCK_REACH * (1.0 + effect(&effects, "reach")) + effect(&effects, "long_arms");
    // A new entity id means a new life or session, where the engine forgot the value.
    let sent = (acting.entity_id, wanted);
    let changed = {
        let mut state = state();
        let known = state.reach_sent.insert(acting.uuid, sent);
        // Players without the nodes keep whatever other mods set.
        known != Some(sent) && wanted != rules::BASE_BLOCK_REACH
            || known.is_some_and(|(_, value)| value != wanted)
    };
    if changed {
        game.native_set_attribute_base(acting.uuid, REACH_ATTRIBUTE, wanted);
    }
}

fn holds_building_block(game: &Game<'_>) -> bool {
    game.native_held()
        .is_some_and(|(item, _, _)| game.items().get(&item).is_some_and(|i| i.block.is_some()))
}

fn slow_fall(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let effects = player_effects(acting.uuid);
    if effect(&effects, "no_fear") <= 0.0 {
        return;
    }
    let holds = holds_building_block(game);
    let no_fall = effect(&effects, "no_fear_fall") > 0.0;
    {
        let mut state = state();
        let body = state.bodies.entry(acting.entity_id).or_default();
        body.holds_block = holds;
        body.no_fall = no_fall;
    }
    if holds && !acting.on_ground && !acting.flying {
        let status = crate::game::native_player_state::NativeEffect {
            id: SLOW_FALLING.to_owned(),
            amplifier: 0,
            duration: SLOW_FALL_TICKS,
            ambient: false,
            particles: false,
        };
        game.native_add_effect(acting.uuid, &status);
    }
}

fn track_armor(game: &Game<'_>, acting: &NativeServerPlayer) {
    let chain_mail = effect(&player_effects(acting.uuid), "chain_mail");
    let pieces = if chain_mail > 0.0 {
        ARMOR_SLOTS
            .iter()
            .filter(|slot| game.native_equipment(**slot).is_some())
            .count() as u8
    } else {
        0
    };
    let mut state = state();
    let body = state.bodies.entry(acting.entity_id).or_default();
    body.armor_pieces = pieces;
    body.chain_mail = chain_mail;
}

/// Each player, each tick: only the deliveries are checked every time.
pub fn on_tick(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    deliver_later(game, acting.uuid);
    let tick = {
        let mut state = state();
        if state.bodies.len() >= MAX_BODIES {
            state.bodies.clear();
        }
        let tick = state.ticks.entry(acting.uuid).or_default();
        *tick = tick.wrapping_add(1);
        *tick
    };
    if tick.is_multiple_of(SLOW_CHECK_EVERY) {
        slow_fall(game, acting);
    }
    if tick.is_multiple_of(ARMOR_CHECK_EVERY) {
        track_armor(game, acting);
        apply_reach(game, acting);
    }
}

/// Worn armor protects better, and a builder holding a block takes no fall damage.
pub fn on_incoming_damage(entity: i32, damage_type: &str, from_mob: bool, amount: &mut f32) {
    let state = state();
    let Some(body) = state.bodies.get(&entity) else {
        return;
    };
    if damage_type == FALL_DAMAGE && body.no_fall && body.holds_block {
        *amount = 0.0;
    }
    if from_mob && body.chain_mail > 0.0 && body.armor_pieces > 0 {
        let cut = body.chain_mail as f32 * f32::from(body.armor_pieces) / ARMOR_PIECES;
        *amount *= (1.0 - cut).clamp(0.0, 1.0);
    }
}

// Hotkeys: the client names the key, the server checks the node.

pub fn on_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_playing() {
        return;
    }
    let message = if keys::is(game, key, Key::PocketBench) {
        KEY_POCKET
    } else if keys::is(game, key, Key::Sharpener) {
        KEY_SHARPEN
    } else {
        return;
    };
    if let Err(error) = game.native_net_send_to_server(KEY_CHANNEL, &[message]) {
        log::debug!("path_of_craft_tree: {KEY_CHANNEL} not sent: {error}");
    }
}

pub fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel != KEY_CHANNEL {
        return;
    }
    match data {
        [KEY_POCKET] => crate::game::pct_fx_craft_tables::request_pocket(player),
        [KEY_SHARPEN] => queue_sharpen(player),
        _ => log::warn!("path_of_craft_tree: bad craft key message from {player:032x}"),
    }
}

// The cook's nodes: what food does when eaten and how fast and how much of it is made.
use crate::game::{
    Game,
    pct_fx_farm::{queue_status, state, status},
    pct_fx_farm_alchemy as alchemy, pct_fx_farm_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects},
    pct_rules::{Effects, Tree},
};
use horizon_client::{containers::BlockEntity, items::ItemStack, mod_api::NativeServerPlayer};

const INVENTORY_SLOTS: usize = 36;
const RUN_PERIOD_TICKS: u32 = 5;
const RUN_STATUS_TICKS: i32 = 10;
const RUN_SPEED: u8 = 1;
const FOOD_REGEN_TICKS: i32 = 60;
const MAX_FOOD: i32 = 20;
const CAKE_RADIUS: f64 = 8.0;
const CAKE_REGEN_TICKS: i32 = 100;
const CAKE_STRENGTH_TICKS: i32 = 400;
const POISON_CUT: i32 = 2;
const TICKS_PER_SECOND: i32 = rules::TICKS_PER_SECOND;

pub fn on_tick(
    game: &mut Game<'_>,
    tree: &Tree,
    acting: &NativeServerPlayer,
    effects: &Effects,
    ticks: u32,
) {
    speed_up_furnaces(game, tree, acting.uuid, effects);
    if ticks.is_multiple_of(RUN_PERIOD_TICKS) {
        keep_running_while_eating(game, acting.uuid, effects);
    }
    if ticks.is_multiple_of(rules::SOUP_PERIOD_TICKS) {
        stack_soups(game, effect(effects, "soup_stack") as u32);
    }
}

/// Quick Stove: furnaces the player opened last cook food faster.
fn speed_up_furnaces(game: &mut Game<'_>, tree: &Tree, player: u128, effects: &Effects) {
    let rate = effect(effects, "cook_speed");
    if rate <= 0.0 {
        return;
    }
    let mine: Vec<[i32; 3]> = state()
        .stations
        .iter()
        .filter(|(_, owner)| **owner == player)
        .map(|(pos, _)| *pos)
        .collect();
    for pos in mine {
        let Some(BlockEntity::Furnace(furnace)) =
            game.world.block_entities.get_mut(&(pos[0], pos[1], pos[2]))
        else {
            continue;
        };
        let cooks_food = furnace.items[0]
            .as_ref()
            .is_some_and(|input| tree.in_list("raw_food", &input.item));
        if !cooks_food || furnace.cooking_timer <= 0 || furnace.lit_remaining <= 0 {
            continue;
        }
        let mut shared = state();
        let carry = shared.carry.entry(pos).or_default();
        let (extra, left) = rules::extra_ticks(rate, *carry);
        *carry = left;
        furnace.cooking_timer =
            rules::advanced(furnace.cooking_timer, extra, furnace.cooking_total);
    }
}

fn food_of(game: &Game<'_>, item: &str) -> Option<(i32, f32)> {
    let (nutrition, saturation) = game.res.items().get(item).and_then(|info| info.food)?;
    Some((nutrition as i32, saturation))
}

fn keep_running_while_eating(game: &mut Game<'_>, player: u128, effects: &Effects) {
    if effect(effects, "eat_run") <= 0.0 || game.using.is_none() {
        return;
    }
    let eating = game
        .native_held()
        .is_some_and(|(item, _, _)| food_of(game, &item).is_some());
    if eating {
        game.native_add_effect(
            player,
            &status("minecraft:speed", RUN_SPEED, RUN_STATUS_TICKS),
        );
    }
}

/// Field Kitchen: soups in the inventory gather into stacks past their usual limit.
fn stack_soups(game: &mut Game<'_>, cap: u32) {
    if cap <= 1 {
        return;
    }
    let slots = game.inventory.len().min(INVENTORY_SLOTS);
    for target in 0..slots {
        for source in (target + 1)..slots {
            merge_soup(&mut game.inventory, target, source, cap);
        }
    }
}

fn merge_soup(inventory: &mut [Option<ItemStack>], target: usize, source: usize, cap: u32) {
    let same = match (&inventory[target], &inventory[source]) {
        (Some(a), Some(b)) => {
            rules::is_soup(&a.item) && a.same_item_same_components(b) && a.count < cap
        }
        _ => false,
    };
    if !same {
        return;
    }
    let Some(mut moved) = inventory[source].take() else {
        return;
    };
    if let Some(stack) = inventory[target].as_mut() {
        let taken = moved.count.min(cap - stack.count);
        stack.count += taken;
        moved.count -= taken;
    }
    if moved.count > 0 {
        inventory[source] = Some(moved);
    }
}

/// Eater on the Run: food takes less time, on the server and the predicting client alike.
pub fn on_use_start(game: &Game<'_>, stack: &ItemStack, duration: &mut i32) {
    if food_of(game, &stack.item).is_none() {
        return;
    }
    let cut = match game.native_acting_player() {
        Some(player) => effect(&player_effects(player.uuid), "eat_fast"),
        None => effect(&glue::client().effects, "eat_fast"),
    };
    if cut > 0.0 {
        *duration = rules::eat_duration(*duration, cut);
    }
}

/// What finishing a food or a drink gives; this runs after the game has applied it.
pub fn on_use_finish(game: &mut Game<'_>, stack: &ItemStack) {
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let effects = player_effects(acting.uuid);
    if effects.is_empty() {
        return;
    }
    if &*stack.item == "minecraft:potion" {
        alchemy::on_drink(game, acting.uuid, &effects, stack);
    } else if let Some((_, saturation)) = food_of(game, &stack.item) {
        on_eat(game, acting.uuid, &effects, stack, saturation);
    }
}

fn on_eat(
    game: &mut Game<'_>,
    player: u128,
    effects: &Effects,
    stack: &ItemStack,
    saturation: f32,
) {
    let item = &*stack.item;
    feed_more(game, player, effects, item, saturation);
    let gourmet = effect(effects, "gourmet_seconds") as i32;
    if gourmet > 0 {
        let speed = status("minecraft:speed", 0, gourmet * TICKS_PER_SECOND);
        game.native_add_effect(player, &speed);
    }
    if chance(effect(effects, "food_regen")) {
        let regen = status("minecraft:regeneration", 0, FOOD_REGEN_TICKS);
        game.native_add_effect(player, &regen);
    }
    let more = effect(effects, "secret_more") > 0.0;
    if let Some((id, amplifier, seconds)) =
        rules::secret_dish(item, more).filter(|_| effect(effects, "secret_dishes") > 0.0)
    {
        game.native_add_effect(player, &status(id, amplifier, seconds * TICKS_PER_SECOND));
    }
    taste(game, player, effects, item);
    if chance(effect(effects, "food_save")) {
        game.native_give_items(item, 1);
    }
}

/// Rich Soil and Satiety: more food and saturation than the item itself gave.
fn feed_more(game: &mut Game<'_>, player: u128, effects: &Effects, item: &str, saturation: f32) {
    let extra_food = if rules::is_farm_food(item) {
        effect(effects, "farm_food") as i32
    } else {
        0
    };
    let share = effect(effects, "saturate_mult") + effect(effects, "food_value");
    let extra_saturation = saturation * share as f32;
    if extra_food == 0 && extra_saturation <= 0.0 {
        return;
    }
    if let Some((level, current, exhaustion)) = game.native_player_food(player) {
        let food = (level + extra_food).min(MAX_FOOD);
        let filled = (current + extra_saturation).min(food as f32);
        game.native_set_player_status(player, None, Some((food, filled, exhaustion)), None);
    }
}

/// Taster: spoiled food does no harm, and with the upgrade poison from food is cut short.
fn taste(game: &mut Game<'_>, player: u128, effects: &Effects, item: &str) {
    if effect(effects, "taster") <= 0.0 {
        return;
    }
    if rules::is_spoiled(item) {
        game.native_remove_effect(player, "minecraft:hunger");
        game.native_remove_effect(player, "minecraft:poison");
    }
    if rules::is_poison_food(item) && effect(effects, "taster_poison") > 0.0 {
        let Some(mut active) = game.native_effects(player) else {
            return;
        };
        for running in active.iter_mut().filter(|e| e.id == "minecraft:poison") {
            running.duration /= POISON_CUT;
        }
        game.native_set_effects(player, &active);
    }
}

/// A slice of cake eaten with Shared Treat, shared once the eater's turn is over.
pub struct CakeShare {
    dimension: String,
    position: [f64; 3],
    amplifier: u8,
}

/// Shared Treat: a slice of cake heals and strengthens everyone close, the eater included.
pub fn on_right_click(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    pos: [i32; 3],
    effects: &Effects,
) {
    if effect(effects, "cake_share") <= 0.0 || !game.native_block(pos).ends_with("cake") {
        return;
    }
    let hungry = game
        .native_player_food(acting.uuid)
        .is_some_and(|(level, _, _)| level < MAX_FOOD);
    if !hungry {
        return;
    }
    // Within a player's turn the other players cannot be listed, so the server tick shares it.
    let mut state = state();
    if state.cake_shares.len() < rules::MAX_PENDING {
        state.cake_shares.push(CakeShare {
            dimension: acting.dimension.clone(),
            position: acting.position,
            amplifier: effect(effects, "cake_amp") as u8,
        });
    }
}

/// Runs outside any player's turn, where every player can be listed.
pub fn share_cakes(game: &Game<'_>) {
    let shares = {
        let mut state = state();
        if state.cake_shares.is_empty() {
            return;
        }
        std::mem::take(&mut state.cake_shares)
    };
    let players = game.native_server_players();
    let reach = CAKE_RADIUS * CAKE_RADIUS;
    for share in shares {
        let close = players.iter().filter(|other| {
            other.dimension == share.dimension
                && distance_squared(other.position, share.position) <= reach
        });
        for other in close {
            queue_status(
                other.uuid,
                status("minecraft:regeneration", share.amplifier, CAKE_REGEN_TICKS),
            );
            queue_status(
                other.uuid,
                status("minecraft:strength", share.amplifier, CAKE_STRENGTH_TICKS),
            );
        }
    }
}

fn distance_squared(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    dx * dx + dy * dy + dz * dz
}

/// Rolls once per portion and hands over the extra ones on the player's own tick: the hooks
/// that call this may run while the player's state is not in place.
fn double_portion(player: u128, item: &str, portions: u32) {
    let effects = player_effects(player);
    let odds = effect(&effects, "chef") + effect(&effects, "extra_food");
    let bonus = (0..portions).filter(|_| chance(odds)).count() as u32;
    if bonus > 0 {
        crate::game::pct_fx_craft::give_later(player, ItemStack::new(item, bonus));
    }
}

/// Chef: cooked food taken from a furnace may bring extra portions.
pub fn on_furnace_extract(_game: &mut Game<'_>, player: u128, item: &str, count: u32) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    if loaded.tree.in_list("cooked_food", item) {
        double_portion(player, item, count);
    }
}

/// Chef: crafting a dish may bring one more portion.
pub fn on_item_crafted(_game: &mut Game<'_>, player: u128, stack: &ItemStack) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let item = &*stack.item;
    if item == "minecraft:cake" || tree.in_list("crafted_food", item) || tree.in_list("stews", item)
    {
        double_portion(player, item, stack.count);
    }
}

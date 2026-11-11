// Crafter and Smith nodes: what a craft, a furnace, an anvil or a grindstone gives back.
use crate::game::{
    Game, pct_actions as actions, pct_fx_craft as fx, pct_fx_craft_rules as rules,
    pct_glue as glue,
    pct_mining::{chance, effect, player_effects, random},
    pct_rules::{Effects, Tree},
};
use horizon_client::{containers::BlockEntity, items::ItemStack, stations};
use horizon_world::nbt::Tag;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const FURNACE_MENUS: [&str; 3] = [
    "minecraft:furnace",
    "minecraft:blast_furnace",
    "minecraft:smoker",
];
const MAX_REMEMBERED_FURNACES: usize = 4_096;
const NETHERITE_INGOT: &str = "minecraft:netherite_ingot";
const MAX_DAMAGE_COMPONENT: &str = "minecraft:max_damage";
const ENCHANTED_BOOK: &str = "minecraft:enchanted_book";
const GRID_SMALL: usize = 4;
const GRID_LARGE: usize = 9;
const SMITHING_INPUTS: usize = 3;
const STONECUTTER_INPUTS: usize = 1;
const MINECART: &str = "minecart";

struct Furnace {
    dimension: String,
    fuel_keep: f64,
    smelt_speed: f64,
}

#[derive(Default)]
struct State {
    furnaces: HashMap<(i32, i32, i32), Furnace>,
    anvil_users: HashMap<[i32; 3], u128>,
    grinds: HashMap<[i32; 3], (u128, Vec<(String, u32)>)>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn give(game: &mut Game<'_>, player: u128, stack: ItemStack) {
    if let Some(rest) = game.native_give_stack(player, stack) {
        fx::give_later(player, rest);
    }
}

fn is_reversible(tree: &Tree, recipe: &str) -> bool {
    tree.in_list("reversible_recipes", recipe)
}

// Grid crafts: `trigger_recipe_crafted` names the ingredients, `item_crafted` the result.

fn used_ingredients(ingredients: &[Option<ItemStack>]) -> Vec<&ItemStack> {
    ingredients.iter().flatten().collect()
}

fn refund_one(game: &mut Game<'_>, player: u128, used: &[&ItemStack]) {
    if used.is_empty() {
        return;
    }
    let index = (random() * used.len() as f64) as usize % used.len();
    give(
        game,
        player,
        ItemStack {
            count: 1,
            ..used[index].clone()
        },
    );
}

/// Each ingredient comes back with this chance; what makes a mechanism or a map cheaper.
fn refund_each(game: &mut Game<'_>, player: u128, used: &[&ItemStack], share: f64) {
    for stack in used {
        if chance(share) {
            give(
                game,
                player,
                ItemStack {
                    count: 1,
                    ..(*stack).clone()
                },
            );
        }
    }
}

fn apply_grid_refunds(
    game: &mut Game<'_>,
    player: u128,
    recipe: &str,
    used: &[&ItemStack],
    effects: &Effects,
) {
    if chance(effect(effects, "ingredient_save")) {
        refund_one(game, player, used);
    }
    if rules::is_redstone_recipe(recipe) {
        refund_each(game, player, used, effect(effects, "redstone_save"));
    }
    if rules::is_map_recipe(recipe) {
        refund_each(game, player, used, effect(effects, "map_save"));
    }
}

fn on_grid_craft(
    game: &mut Game<'_>,
    tree: &Tree,
    player: u128,
    recipe: &str,
    ingredients: &[Option<ItemStack>],
) {
    let used = used_ingredients(ingredients);
    fx::note_craft_ingredients(player, used.len() as u32);
    if ingredients.len() == GRID_LARGE {
        crate::game::pct_fx_craft_tables::note_grid(player, ingredients);
    }
    if !is_reversible(tree, recipe) {
        apply_grid_refunds(game, player, recipe, &used, &player_effects(player));
    }
}

fn on_smithing(game: &mut Game<'_>, player: u128, recipe: &str) {
    if !rules::is_upgrade_recipe(recipe) {
        return;
    }
    actions::earn_once(game, player, "act:upgrade", 1.0);
    let refund = effect(&player_effects(player), "netherite_refund");
    if rules::is_netherite_upgrade(recipe) && chance(refund) {
        give(game, player, ItemStack::new(NETHERITE_INGOT, 1));
    }
}

fn on_stonecut(game: &mut Game<'_>, player: u128, recipe: &str) {
    let Some(stack) = game
        .stations()
        .recipes
        .stonecutting
        .iter()
        .find(|cut| cut.id == recipe)
        .map(|cut| cut.result.clone())
    else {
        return;
    };
    if chance(effect(&player_effects(player), "stonecutter_bonus")) {
        give(game, player, ItemStack { count: 1, ..stack });
    }
}

/// Every take from a grid, smithing table or stonecutter passes here, as the acting player.
pub fn on_recipe_crafted(game: &mut Game<'_>, recipe: &str, ingredients: &[Option<ItemStack>]) {
    let (Some(acting), Some(loaded)) = (game.native_acting_player(), glue::current_tree()) else {
        return;
    };
    match ingredients.len() {
        GRID_SMALL | GRID_LARGE => {
            on_grid_craft(game, &loaded.tree, acting.uuid, recipe, ingredients)
        }
        SMITHING_INPUTS => on_smithing(game, acting.uuid, recipe),
        STONECUTTER_INPUTS => on_stonecut(game, acting.uuid, recipe),
        _ => {}
    }
}

/// The result of a grid craft: doubled, or a bigger batch of rails and wool.
pub fn on_item_crafted(game: &mut Game<'_>, player: u128, recipe: &str, stack: &ItemStack) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    if is_reversible(&loaded.tree, recipe) {
        return;
    }
    let effects = player_effects(player);
    if chance(effect(&effects, "craft_double")) {
        give(game, player, stack.clone());
    }
    let item = &*stack.item;
    if rules::is_rail(item) && effect(&effects, "rail_bonus") > 0.0 {
        let extra = (f64::from(stack.count) * effect(&effects, "rail_bonus")).ceil() as u32;
        give(
            game,
            player,
            ItemStack {
                count: extra,
                ..stack.clone()
            },
        );
    }
    if rules::is_woven(item) && chance(effect(&effects, "weaver")) {
        give(
            game,
            player,
            ItemStack {
                count: 1,
                ..stack.clone()
            },
        );
    }
}

// Gear made by the player: longer lasting, whatever the grid or smithing table shows.

fn boost_durability(game: &Game<'_>, player: u128, result: &mut Option<ItemStack>) {
    let bonus = effect(&fx::effects_of(player), "item_durability");
    let Some(stack) = result
        .as_mut()
        .filter(|stack| bonus > 0.0 && rules::is_gear(&stack.item))
    else {
        return;
    };
    let Some(default_max) = game.items().get(&stack.item).map(|item| item.max_damage) else {
        return;
    };
    let max = rules::boosted_max_damage(default_max, bonus);
    stack
        .data_mut()
        .set_other(MAX_DAMAGE_COMPONENT, Some(Tag::Int(max)));
}

pub fn on_prepare_craft(game: &Game<'_>, player: u128, result: &mut Option<ItemStack>) {
    boost_durability(game, player, result);
}

pub fn on_prepare_smithing(game: &Game<'_>, player: u128, result: &mut Option<ItemStack>) {
    boost_durability(game, player, result);
}

// Furnaces: the last player to open one owns its bonuses until someone else opens it.

pub fn on_container_open(game: &Game<'_>, player: u128, menu: &str, pos: Option<[i32; 3]>) {
    let Some([x, y, z]) = pos.filter(|_| FURNACE_MENUS.contains(&menu)) else {
        return;
    };
    let effects = player_effects(player);
    let fuel_keep = effect(&effects, "fuel_keep") + effect(&effects, "fuel_save");
    let smelt_speed = effect(&effects, "smelt_speed");
    let mut state = state();
    if fuel_keep <= 0.0 && smelt_speed <= 0.0 {
        state.furnaces.remove(&(x, y, z));
        return;
    }
    if state.furnaces.len() >= MAX_REMEMBERED_FURNACES {
        state.furnaces.clear();
    }
    let furnace = Furnace {
        dimension: game.native_dimension().to_owned(),
        fuel_keep: fuel_keep.min(rules::MAX_CHANCE),
        smelt_speed: smelt_speed.min(rules::MAX_CHANCE),
    };
    state.furnaces.insert((x, y, z), furnace);
}

fn bonuses_at(game: &Game<'_>, pos: (i32, i32, i32)) -> Option<(f64, f64)> {
    let state = state();
    let furnace = state.furnaces.get(&pos)?;
    (furnace.dimension == *game.native_dimension())
        .then_some((furnace.fuel_keep, furnace.smelt_speed))
}

/// Runs before a furnace's tick: a kept fuel tick and sometimes one extra tick of cooking.
pub fn on_tick_furnace(game: &mut Game<'_>, pos: (i32, i32, i32)) {
    let Some((fuel_keep, smelt_speed)) = bonuses_at(game, pos) else {
        return;
    };
    let Some(BlockEntity::Furnace(furnace)) = game.world.block_entities.get_mut(&pos) else {
        return;
    };
    if furnace.lit_remaining > 1 && chance(fuel_keep) {
        furnace.lit_remaining += 1;
    }
    let cooking = furnace.lit_remaining > 0 && furnace.cooking_timer + 1 < furnace.cooking_total;
    if cooking && chance(smelt_speed) {
        furnace.cooking_timer += 1;
    }
}

pub fn on_furnace_extract(player: u128, item: &str, count: u32) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    if !loaded.tree.in_list("ingots", item) {
        return;
    }
    let share = effect(&player_effects(player), "extra_ingot");
    let extra = (0..count).filter(|_| chance(share)).count() as u32;
    if extra > 0 {
        fx::give_later(player, ItemStack::new(item, extra));
    }
}

// The anvil.

pub fn on_anvil_pre(player: u128, pos: [i32; 3]) {
    state().anvil_users.insert(pos, player);
}

/// Whether the player using this anvil owns the node that keeps it from wearing out.
pub fn anvil_keeps(pos: [i32; 3]) -> bool {
    let user = state().anvil_users.get(&pos).copied();
    user.is_some_and(|player| effect(&player_effects(player), "anvil_master") > 0.0)
}

fn names_of(stack: &ItemStack) -> Vec<&str> {
    stack
        .crafting_enchantments()
        .iter()
        .map(|(id, _)| &**id)
        .collect()
}

/// Would this right-hand item combine in survival, allowing only the pairs the player unlocked?
fn merge_permitted(
    game: &Game<'_>,
    left: &ItemStack,
    right: &ItemStack,
    effects: &Effects,
) -> bool {
    if effect(effects, "forbidden") <= 0.0 && effect(effects, "anvil_no_cap") <= 0.0 {
        return false;
    }
    let data = game.stations();
    let more = effect(effects, "forbidden_more") > 0.0;
    let forbidden = effect(effects, "forbidden") > 0.0;
    let book = &*left.item == ENCHANTED_BOOK;
    names_of(right).iter().all(|id| {
        let fits = book || data.enchantments.can_enchant(game.items(), id, &left.item);
        fits && names_of(left).iter().all(|other| {
            other == id
                || data.enchantments.compatible(id, other)
                || (forbidden && rules::pair_allowed(id, other, more))
        })
    })
}

/// The combination vanilla refused: too expensive, or a pair only this player may combine.
fn unlocked_merge(
    game: &Game<'_>,
    left: &ItemStack,
    right: &ItemStack,
    item_name: Option<&str>,
    effects: &Effects,
) -> Option<(ItemStack, i32)> {
    if !merge_permitted(game, left, right, effects) {
        return None;
    }
    let data = game.stations();
    let hover = stations::hover_name(&game.pack.names, left);
    let answer = stations::anvil_result(
        &data.enchantments,
        game.items(),
        Some(left),
        Some(right),
        item_name,
        &hover,
        true,
    );
    let Some(output) = answer.result else {
        // Vanilla refuses an incompatible pair even in creative; the node combines it here.
        return (effect(effects, "forbidden") > 0.0)
            .then(|| forced_merge(game, left, right))
            .flatten();
    };
    let lifted = answer.cost >= rules::ANVIL_TOO_EXPENSIVE;
    if lifted && effect(effects, "anvil_no_cap") <= 0.0 {
        return None;
    }
    Some((output, answer.cost.min(rules::ANVIL_CAPPED_COST)))
}

/// Adds the right item's enchantments to the left one, whatever vanilla thinks of the pair.
fn forced_merge(game: &Game<'_>, left: &ItemStack, right: &ItemStack) -> Option<(ItemStack, i32)> {
    let data = &game.stations().enchantments;
    let book = &*right.item == ENCHANTED_BOOK;
    let mut output = left.clone();
    let mut cost = i64::from(left.repair_cost()) + i64::from(right.repair_cost());
    let mut changed = false;
    for (id, level) in right.crafting_enchantments() {
        let def = data.get(id)?;
        let current = left
            .crafting_enchantments()
            .iter()
            .find(|(other, _)| other == id)
            .map_or(0, |(_, known)| *known);
        let wanted = if current == *level {
            level + 1
        } else {
            (*level).max(current)
        };
        let merged = wanted.min(def.max_level);
        if merged > current {
            output.enchant(id, merged);
            let fee = i64::from(def.anvil_cost);
            let fee = if book { (fee / 2).max(1) } else { fee };
            cost += fee * i64::from(merged);
            changed = true;
        }
    }
    changed.then(|| {
        (
            output,
            cost.clamp(1, i64::from(rules::ANVIL_CAPPED_COST)) as i32,
        )
    })
}

/// Moves a worn item's enchantments onto another item of the same anvil.
fn transfer_result(
    game: &Game<'_>,
    left: &ItemStack,
    right: &ItemStack,
    effects: &Effects,
) -> Option<(ItemStack, i32)> {
    if effect(effects, "transfer") <= 0.0 || left.crafting_enchantments().is_empty() {
        return None;
    }
    let data = &game.stations().enchantments;
    if data.enchantable(&right.item) == 0
        || !right.enchantments().is_empty()
        || right.count != 1
        || left.item == right.item
    {
        return None;
    }
    let lossless = effect(effects, "transfer_keep") > 0.0;
    let moved: Vec<_> = left
        .crafting_enchantments()
        .iter()
        .filter(|(id, _)| data.can_enchant(game.items(), id, &right.item))
        .map(|(id, level)| (id.clone(), rules::transferred_level(*level, lossless)))
        .collect();
    if moved.is_empty() {
        return None;
    }
    let mut output = right.clone();
    let levels: Vec<u32> = moved.iter().map(|(_, level)| *level).collect();
    output.set_crafting_enchantments(moved);
    output.tidy();
    Some((output, rules::transfer_cost(&levels)))
}

fn is_repair(game: &Game<'_>, left: &ItemStack, right: &ItemStack) -> bool {
    left.item == right.item
        || stations::is_valid_repair_item(&game.stations().enchantments, game.items(), left, right)
}

/// `AnvilUpdateEvent`: cheaper repairs, and the merges the player's nodes unlock.
#[allow(clippy::too_many_arguments)]
pub fn on_anvil_update(
    game: &Game<'_>,
    player: u128,
    left: &ItemStack,
    right: Option<&ItemStack>,
    item_name: Option<&str>,
    vanilla_output: Option<&ItemStack>,
    output: &mut Option<ItemStack>,
    xp_cost: &mut i32,
) {
    let Some(right) = right else {
        return;
    };
    let effects = fx::effects_of(player);
    if vanilla_output.is_some() && is_repair(game, left, right) {
        let cut = effect(&effects, "temper") + effect(&effects, "repair_save");
        *xp_cost = rules::discounted_cost(*xp_cost, cut);
        return;
    }
    if vanilla_output.is_some() {
        return;
    }
    let unlocked = unlocked_merge(game, left, right, item_name, &effects)
        .or_else(|| transfer_result(game, left, right, &effects));
    if let Some((result, cost)) = unlocked {
        *output = Some(result);
        *xp_cost = cost;
    }
}

// The grindstone: dismantling plain gear, and a book back from an enchanted item.

/// What the grindstone offers for a single unenchanted piece of gear, if the player can dismantle.
pub fn dismantle_offer(game: &Game<'_>, player: u128, left: &ItemStack) -> Option<ItemStack> {
    let share = effect(&player_effects(player), "dismantle");
    if left.has_any_enchantments() || left.count != 1 {
        return None;
    }
    let max = game
        .items()
        .get(&left.item)
        .map_or(0, |item| item.max_damage);
    let (material, amount) = rules::dismantle_return(&left.item, left.damage, max, share)?;
    Some(ItemStack::new(material, amount))
}

/// Remembers what a grindstone holds, so its take can hand back an enchantment.
pub fn note_grind(pos: [i32; 3], player: u128, inputs: &[Option<&ItemStack>]) {
    let found: Vec<(String, u32)> = inputs
        .iter()
        .flatten()
        .flat_map(|stack| stack.crafting_enchantments().iter())
        .map(|(id, level)| (id.to_string(), *level))
        .collect();
    state().grinds.insert(pos, (player, found));
}

fn pick_enchantment(found: &[(String, u32)]) -> Option<(String, u32)> {
    if found.is_empty() {
        return None;
    }
    found
        .get((random() * found.len() as f64) as usize % found.len())
        .cloned()
}

pub fn on_grindstone_taken(pos: [i32; 3]) {
    let Some((player, found)) = state().grinds.remove(&pos) else {
        return;
    };
    if !chance(effect(&player_effects(player), "grind_book")) {
        return;
    }
    let Some((id, level)) = pick_enchantment(&found) else {
        return;
    };
    let mut book = ItemStack::new(ENCHANTED_BOOK, 1);
    book.enchant(&id, level);
    fx::give_for_book_later(player, book);
}

// The whetstone hotkey and the tougher minecarts.

fn repair_material_slot(game: &Game<'_>, held: &ItemStack) -> Option<usize> {
    let data = &game.stations().enchantments;
    game.inventory.iter().take(36).position(|slot| {
        slot.as_ref()
            .is_some_and(|stack| stations::is_valid_repair_item(data, game.items(), held, stack))
    })
}

/// The hotkey: one material from the inventory mends part of the held tool.
pub fn sharpen_held(game: &mut Game<'_>, player: u128) {
    let share = effect(&player_effects(player), "sharpen");
    if share <= 0.0 {
        return;
    }
    let selected = game.selected;
    let Some(held) = game.inventory.get(selected).cloned().flatten() else {
        return;
    };
    let max = game
        .items()
        .get(&held.item)
        .map_or(0, |item| item.max_damage);
    if held.damage == 0 || max == 0 {
        return;
    }
    let Some(material) = repair_material_slot(game, &held) else {
        return;
    };
    let points = rules::repair_points(max, share - rules::SHARPEN_BASE_SHARE);
    if let Some(slot) = game.inventory.get_mut(selected).and_then(Option::as_mut) {
        slot.damage = slot.damage.saturating_sub(points);
    }
    if let Some(stack) = game.inventory.get_mut(material) {
        let left = stack.as_ref().map_or(0, |s| s.count).saturating_sub(1);
        if left == 0 {
            *stack = None;
        } else if let Some(s) = stack.as_mut() {
            s.count = left;
        }
    }
}

pub fn on_vehicle_damage(game: &Game<'_>, kind: &str, damage: &mut f32) {
    if !kind.contains(MINECART) {
        return;
    }
    let cut = game.native_acting_player().map_or(0.0, |acting| {
        effect(&player_effects(acting.uuid), "cart_tough")
    });
    *damage *= (1.0 - cut as f32).clamp(0.0, 1.0);
}

pub fn on_map_filled(game: &mut Game<'_>, player: u128, map_id: i32) {
    if effect(&player_effects(player), "map_wide") <= 0.0 {
        return;
    }
    let position = game
        .native_acting_player()
        .filter(|acting| acting.uuid == player)
        .map(|acting| acting.position);
    let (Some([x, _, z]), Some(map)) = (position, game.map(map_id)) else {
        return;
    };
    if map.scale < MAX_MAP_SCALE {
        let wider = crate::game::maps::MapData::fresh(x, z, map.scale + 1, &map.dimension);
        *map = wider;
    }
}

const MAX_MAP_SCALE: u8 = 4;

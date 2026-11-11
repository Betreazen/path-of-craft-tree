// Enchanter nodes: everything added to an item after the table or the anvil made it.
use crate::game::{
    Game, pct_actions as actions, pct_fx_craft as fx, pct_fx_craft_items as items,
    pct_fx_craft_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects, random},
    pct_rules::Effects,
    stations::Kind,
};
use horizon_client::{
    items::ItemStack,
    menu::{AnvilCraft, Menu},
    stations,
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

const ENCHANTING_POOL: &str = "minecraft:in_enchanting_table";
const LAPIS: &str = "minecraft:lapis_lazuli";
const MAX_RESONANCE_LEVEL: u32 = 2;

#[derive(Default)]
struct State {
    pending: HashMap<u128, i32>,
    last_reroll: HashMap<u128, Instant>,
    shown: HashMap<u128, (i32, String, [i32; 3])>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn pay_first_enchantments(game: &mut Game<'_>, player: u128, stack: &ItemStack) {
    let ids: Vec<String> = stack
        .crafting_enchantments()
        .iter()
        .map(|(id, _)| id.to_string())
        .collect();
    for id in ids {
        if glue::mark_seen(player, format!("enchant:{id}")) {
            actions::earn_once(game, player, "act:enchant_first", 1.0);
        }
    }
}

pub fn on_enchant_item(game: &mut Game<'_>, player: u128, item: &ItemStack, levels: i32) {
    pay_first_enchantments(game, player, item);
    state().pending.insert(player, levels);
}

pub fn on_anvil_post(game: &mut Game<'_>, player: u128, craft: &AnvilCraft) {
    pay_first_enchantments(game, player, &craft.output);
}

pub fn on_prepare_enchant(player: u128, costs: &mut [i32; 3]) {
    let effects = player_effects(player);
    let shelves = effect(&effects, "shelves");
    if shelves > 0.0 {
        *costs = rules::raised_costs(*costs, shelves, effect(&effects, "shelves_more") > 0.0);
    }
}

/// Whether opening a table should give this player new offers for free.
pub fn should_reroll(player: u128) -> bool {
    let effects = player_effects(player);
    if effect(&effects, "table_reroll") <= 0.0 {
        return false;
    }
    let wait = rules::reroll_cooldown_seconds(effect(&effects, "table_reroll_free") > 0.0);
    let mut state = state();
    let ready = state
        .last_reroll
        .get(&player)
        .is_none_or(|at| at.elapsed() >= Duration::from_secs(wait));
    if ready {
        state.last_reroll.insert(player, Instant::now());
    }
    ready
}

pub fn on_xp_change(player: u128, amount: &mut i32) {
    let effects = player_effects(player);
    let bonus = effect(&effects, "xp_flow") + effect(&effects, "xp_gain");
    *amount = rules::boosted_points(*amount, bonus, random());
}

fn pick_extra_enchantment(game: &Game<'_>, item: &ItemStack) -> Option<(String, u32)> {
    let data = &game.stations().enchantments;
    let held: Vec<&str> = item
        .crafting_enchantments()
        .iter()
        .map(|(id, _)| &**id)
        .collect();
    let usable: Vec<_> = data
        .tag(ENCHANTING_POOL)
        .iter()
        .filter(|id| !held.contains(&&***id))
        .filter(|id| data.can_enchant(game.items(), id, &item.item))
        .filter(|id| held.iter().all(|other| data.compatible(id, other)))
        .collect();
    if usable.is_empty() {
        return None;
    }
    let id = usable[(random() * usable.len() as f64) as usize % usable.len()];
    let max = data
        .get(id)
        .map_or(1, |def| def.max_level)
        .min(MAX_RESONANCE_LEVEL);
    let level = 1 + (random() * f64::from(max)) as u32 % max.max(1);
    Some((id.to_string(), level))
}

fn raise_levels(game: &Game<'_>, item: &mut ItemStack, effects: &Effects) -> bool {
    let data = &game.stations().enchantments;
    let mut changed = false;
    for (id, level) in item.crafting_enchantments().to_vec() {
        let max = data.get(&id).map_or(level, |def| def.max_level);
        let raised = (level < max && chance(effect(effects, "enchant_power")))
            || (level >= max && chance(effect(effects, "beyond")));
        if raised {
            item.enchant(&id, level + 1);
            changed = true;
        }
    }
    changed
}

fn refund_lapis(game: &Game<'_>, menu: &mut Menu, need: i32, effects: &Effects) {
    let share = effect(effects, "lapis_save") + effect(effects, "enchant_save");
    let saved = (0..need).filter(|_| chance(share)).count() as u32;
    if saved == 0 {
        return;
    }
    let count = menu.slot(1).map_or(0, |stack| stack.count);
    let refunded = ItemStack::new(LAPIS, count + saved);
    let env = game.menu_env();
    menu.set_slot(&env, 1, Some(refunded));
}

/// Finishes an enchantment after the engine made it: the bonus enchantment and raised levels.
fn apply_pending(game: &Game<'_>, player: u128, menu: &mut Menu) {
    let Some(need) = state().pending.remove(&player) else {
        return;
    };
    let effects = player_effects(player);
    if let Some(mut item) = menu.slot(0).cloned() {
        let resonance = effect(&effects, "resonance") + effect(&effects, "enchant_luck");
        let extra = chance(resonance)
            .then(|| pick_extra_enchantment(game, &item))
            .flatten();
        let added = extra.is_some();
        if let Some((id, level)) = extra {
            item.enchant(&id, level);
        }
        if raise_levels(game, &mut item, &effects) || added {
            let env = game.menu_env();
            menu.set_slot(&env, 0, Some(item));
        }
    }
    refund_lapis(game, menu, need, &effects);
    menu.enchant_changed = false;
}

fn offer_line(game: &Game<'_>, seed: i32, slot: usize, cost: i32, item: &str) -> String {
    let data = game.stations();
    let list = stations::enchantment_list(&data.enchantments, game.items(), seed, slot, cost, item);
    let names: Vec<String> = list
        .iter()
        .map(|(id, level)| data.enchantments.full_name(id, *level).0)
        .collect();
    format!("Вариант {}: {}", slot + 1, names.join(", "))
}

/// Tells the player every enchantment of each offer, once for each set of offers.
fn show_offers(game: &Game<'_>, player: u128, menu: &Menu, effects: &Effects) {
    let (Some(item), Some(costs)) = (menu.slot(0), menu.data.get(0..3)) else {
        return;
    };
    let costs = [costs[0], costs[1], costs[2]];
    let seed = menu.data.get(3).copied().unwrap_or_default();
    let key = (seed, item.item.to_string(), costs);
    if costs.iter().all(|cost| *cost <= 0) || state().shown.get(&player) == Some(&key) {
        return;
    }
    state().shown.insert(player, key);
    for (slot, cost) in costs.iter().enumerate().filter(|(_, cost)| **cost > 0) {
        fx::tell_later(player, offer_line(game, seed, slot, *cost, &item.item));
    }
    if effect(effects, "sage_odds") > 0.0 {
        fx::tell_later(player, odds_line(effects));
    }
}

fn odds_line(effects: &Effects) -> String {
    let resonance = effect(effects, "resonance") + effect(effects, "enchant_luck");
    format!(
        "Шанс лишнего зачарования {:.0}%, уровня сверх предела {:.0}%",
        resonance * 100.0,
        effect(effects, "beyond") * 100.0
    )
}

fn grindstone_send(game: &Game<'_>, player: u128, menu: &mut Menu, pos: [i32; 3]) {
    let left = menu.slot(0).cloned();
    let right = menu.slot(1).cloned();
    items::note_grind(pos, player, &[left.as_ref(), right.as_ref()]);
    if menu.slot(2).is_some() || right.is_some() {
        return;
    }
    let Some(offer) = left.and_then(|left| items::dismantle_offer(game, player, &left)) else {
        return;
    };
    let env = game.menu_env();
    menu.set_slot(&env, 2, Some(offer));
}

/// The last look at a guest's station before the client is told what it holds.
pub fn on_station_send(game: &Game<'_>, player: u128, menu: &mut Menu, kind: Kind, pos: [i32; 3]) {
    match kind {
        Kind::Enchanting => {
            apply_pending(game, player, menu);
            let effects = player_effects(player);
            if effect(&effects, "sage") > 0.0 {
                show_offers(game, player, menu, &effects);
            }
        }
        Kind::Grindstone => grindstone_send(game, player, menu, pos),
        _ => {}
    }
}

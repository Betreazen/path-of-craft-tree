// The alchemist's nodes. A brewing stand knows no player, so each stand belongs to whoever opened
// it last, and its results are settled on that player's next tick.
use crate::game::{
    Game,
    native_player_state::NativeEffect,
    pct_actions as actions,
    pct_fx_farm::{queue_status, state, status},
    pct_fx_farm_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects},
    pct_rules::Effects,
};
use horizon_client::{
    brewing, containers::BlockEntity, items::ItemStack, mod_api::NativeServerPlayer,
};
use horizon_world::nbt::Tag;
use std::time::{Duration, Instant};

const BOTTLE_SLOTS: usize = 3;
const INGREDIENT_SLOT: usize = 3;
const DURATION_SCALE: &str = "minecraft:potion_duration_scale";
const CUSTOM_EFFECTS: &str = "custom_effects";
const SPLASH_RADIUS: f64 = 4.0;
const THROW_MEMORY: Duration = Duration::from_secs(30);
const RING_DURATION_SHARE: i32 = 2;
const SPLASH_ITEMS: [&str; 2] = ["minecraft:splash_potion", "minecraft:lingering_potion"];
const GLASS_BOTTLE: &str = "minecraft:glass_bottle";
const EXPERIENCE_BOTTLE: &str = "minecraft:experience_bottle";

pub fn on_tick(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects, ticks: u32) {
    speed_up_brews(game, acting.uuid, effects);
    settle_brews(game, acting.uuid, effects);
    if ticks.is_multiple_of(rules::EFFECT_TRIM_PERIOD_TICKS) {
        trim_bad_effects(game, acting.uuid, effects);
    }
}

fn own_stations(player: u128) -> Vec<[i32; 3]> {
    state()
        .stations
        .iter()
        .filter(|(_, owner)| **owner == player)
        .map(|(pos, _)| *pos)
        .collect()
}

/// Quick Brew: stands the player opened last count down faster.
fn speed_up_brews(game: &mut Game<'_>, player: u128, effects: &Effects) {
    let rate = effect(effects, "brew_speed");
    if rate <= 0.0 {
        return;
    }
    for pos in own_stations(player) {
        let Some(BlockEntity::Brewing(stand)) =
            game.world.block_entities.get_mut(&(pos[0], pos[1], pos[2]))
        else {
            continue;
        };
        if stand.brew_time <= 1 {
            continue;
        }
        let mut shared = state();
        let carry = shared.carry.entry(pos).or_default();
        let (extra, left) = rules::extra_ticks(rate, *carry);
        *carry = left;
        stand.brew_time = rules::brew_time_after(stand.brew_time, extra);
    }
}

pub fn on_brewing_complete(pos: [i32; 3]) {
    let mut state = state();
    if state.finished.len() < rules::MAX_PENDING {
        state.finished.push(pos);
    }
}

fn take_finished(player: u128) -> Vec<[i32; 3]> {
    let mut state = state();
    if state.finished.is_empty() {
        return Vec::new();
    }
    let owners = state.stations.clone();
    let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut state.finished)
        .into_iter()
        .partition(|pos| owners.get(pos) == Some(&player));
    // A stand nobody claimed cannot be settled later either.
    state.finished = rest
        .into_iter()
        .filter(|pos| owners.contains_key(pos))
        .collect();
    mine
}

fn settle_brews(game: &mut Game<'_>, player: u128, effects: &Effects) {
    for pos in take_finished(player) {
        settle_one(game, player, effects, pos);
    }
}

fn settle_one(game: &mut Game<'_>, player: u128, effects: &Effects, pos: [i32; 3]) {
    let Some(BlockEntity::Brewing(stand)) =
        game.world.block_entities.get_mut(&(pos[0], pos[1], pos[2]))
    else {
        return;
    };
    let bottles = &mut stand.items[..BOTTLE_SLOTS];
    let potions: Vec<String> = bottles
        .iter()
        .flatten()
        .filter_map(|stack| stack.potion().map(str::to_owned))
        .collect();
    let kinds = rules::distinct_potions(&potions);
    concentrate(bottles, effect(effects, "potion_strong"));
    mix(bottles, &kinds, effect(effects, "mix") as usize);
    prolong(bottles, effect(effects, "potion_duration"));
    let spare = bottles.iter().flatten().next().map(|stack| ItemStack {
        count: 1,
        ..stack.clone()
    });
    if chance(effect(effects, "brew_save")) {
        let ingredient = stand.ingredient.clone();
        give_ingredient_back(&mut stand.items, ingredient.as_deref());
    }
    if let Some(extra) = spare.filter(|_| chance(effect(effects, "brew_extra"))) {
        game.native_give_stack(player, extra);
    }
    if chance(effect(effects, "brew_xp_bottle")) {
        game.native_give_stack(player, ItemStack::new(EXPERIENCE_BOTTLE, 1));
    }
    for kind in kinds {
        if glue::mark_seen(player, format!("brew:{kind}")) {
            actions::earn_once(game, player, "act:brew_first", 1.0);
        }
    }
}

/// Concentrate: each bottle may come out a level stronger.
fn concentrate(bottles: &mut [Option<ItemStack>], odds: f64) {
    for slot in bottles.iter_mut() {
        let stronger = slot
            .as_ref()
            .and_then(|stack| stack.potion())
            .and_then(rules::stronger_potion);
        let Some(name) = stronger.filter(|_| chance(odds)) else {
            continue;
        };
        *slot = slot.take().map(|stack| stack.with_potion(&name));
    }
}

fn namespaced(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

fn effect_tag(id: &str, duration: i32, amplifier: i32) -> Tag {
    Tag::compound([
        ("id", Tag::String(namespaced(id).as_str().into())),
        ("duration", Tag::Int(duration)),
        ("amplifier", Tag::Int(amplifier)),
    ])
}

/// Mixing: bottles of different kinds in one stand also take the effects of the others.
fn mix(bottles: &mut [Option<ItemStack>], kinds: &[String], limit: usize) {
    if limit < 2 {
        return;
    }
    for stack in bottles.iter_mut().flatten() {
        let own = stack.potion().unwrap_or_default().to_owned();
        let list: Vec<Tag> = rules::mix_partners(&own, kinds, limit)
            .iter()
            .filter_map(|partner| brewing::potion(partner))
            .flat_map(|potion| potion.effects.iter())
            .map(|(id, duration, amplifier)| effect_tag(id, *duration, *amplifier))
            .collect();
        if list.is_empty() {
            continue;
        }
        let data = stack.data_mut();
        data.potion_extra.retain(|(key, _)| key != CUSTOM_EFFECTS);
        data.potion_extra
            .push((CUSTOM_EFFECTS.to_owned(), Tag::List(list)));
    }
}

/// Long Brew: the potion's own duration scale, so a drink and a splash both last longer.
fn prolong(bottles: &mut [Option<ItemStack>], share: f64) {
    if share <= 0.0 {
        return;
    }
    for stack in bottles.iter_mut().flatten() {
        let scale = stack.potion_duration_scale() * (1.0 + share as f32);
        stack
            .data_mut()
            .set_other(DURATION_SCALE, Some(Tag::Float(scale.to_bits())));
    }
}

/// Distillation: the ingredient the brew just used comes back.
fn give_ingredient_back(items: &mut [Option<ItemStack>], ingredient: Option<&str>) {
    let Some(ingredient) = ingredient else {
        return;
    };
    match items[INGREDIENT_SLOT].as_mut() {
        Some(stack) if &*stack.item == ingredient => stack.count += 1,
        None => items[INGREDIENT_SLOT] = Some(ItemStack::new(ingredient, 1)),
        Some(_) => {}
    }
}

/// Last Sip: a drunk potion may stay; the bottle the game already handed back goes.
pub fn on_drink(game: &mut Game<'_>, player: u128, effects: &Effects, stack: &ItemStack) {
    if !chance(effect(effects, "potion_keep")) {
        return;
    }
    let one = ItemStack {
        count: 1,
        ..stack.clone()
    };
    if game.native_give_stack(player, one).is_none() {
        game.native_take_items(GLASS_BOTTLE, 1);
    }
}

/// Poison Habit and the small resistance nodes: harmful effects run out faster.
fn trim_bad_effects(game: &mut Game<'_>, player: u128, effects: &Effects) {
    let immune = effect(effects, "poison_immune") > 0.0;
    let weakens = effect(effects, "poison_weak") > 0.0;
    let resist = effect(effects, "potion_resist");
    if !immune && !weakens && resist <= 0.0 {
        return;
    }
    let Some(mut active) = game.native_effects(player) else {
        return;
    };
    if !active
        .iter()
        .any(|running| rules::is_bad_effect(&running.id))
    {
        return;
    }
    active.retain(|running| !(immune && running.id == "minecraft:poison"));
    for running in active.iter_mut().filter(|e| rules::is_bad_effect(&e.id)) {
        let trim = rules::trim_ticks(&running.id, resist, weakens);
        if running.duration > 0 {
            running.duration = (running.duration - trim).max(1);
        }
    }
    game.native_set_effects(player, &active);
}

/// Remembers who threw a potion: the splash itself names no player.
pub fn on_use_item(game: &Game<'_>, stack: &ItemStack) {
    if !SPLASH_ITEMS.contains(&&*stack.item) {
        return;
    }
    if let Some(acting) = game.native_acting_player() {
        let mut state = state();
        if state.throwers.len() < rules::MAX_PENDING {
            state.throwers.push((acting.uuid, Instant::now()));
        }
    }
}

fn take_thrower() -> Option<u128> {
    let mut state = state();
    state.throwers.retain(|(_, at)| at.elapsed() < THROW_MEMORY);
    if state.throwers.is_empty() {
        return None;
    }
    Some(state.throwers.remove(0).0)
}

fn lasting_effects(stack: &ItemStack) -> Vec<(&'static str, i32, i32)> {
    brewing::contents_effects(stack)
        .into_iter()
        .filter(|(id, ..)| !brewing::effect(id).is_some_and(|known| known.instant))
        .collect()
}

fn ring_status(id: &str, duration: i32, amplifier: i32) -> NativeEffect {
    status(
        &namespaced(id),
        amplifier.clamp(0, 255) as u8,
        (duration / RING_DURATION_SHARE).max(1),
    )
}

fn in_ring(pos: [f64; 3], at: [f64; 3], outer: f64) -> bool {
    let (dx, dy, dz) = (pos[0] - at[0], pos[1] - at[1], pos[2] - at[2]);
    let distance = dx * dx + dy * dy + dz * dz;
    distance > SPLASH_RADIUS * SPLASH_RADIUS && distance <= outer * outer
}

/// Wide Splash: past the potion's normal reach, a ring takes its lasting effects at half time.
pub fn on_splash(game: &mut Game<'_>, stack: &ItemStack, at: [f64; 3]) {
    let Some(thrower) = take_thrower() else {
        return;
    };
    let bonus = effect(&player_effects(thrower), "splash_radius");
    let lasting = lasting_effects(stack);
    if bonus <= 0.0 || lasting.is_empty() {
        return;
    }
    let outer = SPLASH_RADIUS + bonus;
    let dimension = game.native_dimension();
    for player in game.native_server_players() {
        if player.dimension == dimension && in_ring(player.position, at, outer) {
            for (id, duration, amplifier) in &lasting {
                queue_status(player.uuid, ring_status(id, *duration, *amplifier));
            }
        }
    }
    for m in game.mobs.iter_mut().filter(|m| !m.removed && m.alive()) {
        if in_ring([m.position.x, m.position.y, m.position.z], at, outer) {
            for (id, duration, amplifier) in &lasting {
                m.add_effect(*id, (*duration / RING_DURATION_SHARE).max(1), *amplifier);
            }
        }
    }
}

// The husbandry nodes: everything around a player's animals.
use crate::game::{
    Game,
    pct_fx_farm::state,
    pct_fx_farm_rules as rules,
    pct_mining::{chance, effect, player_effects},
    pct_rules::{Effects, Tree},
};
use horizon_client::{
    items::ItemStack,
    lan::playerdata::uuid_ints,
    mobs::{Mob, Species},
    mod_api::NativeServerPlayer,
};
use std::sync::atomic::{AtomicBool, Ordering};

const NEARBY_RADIUS: f64 = 16.0;
const LEAD_PERIOD_TICKS: u32 = 4;
const LEAD_STOP_DISTANCE: f64 = 3.0;
const LEAD_PULL: f64 = 0.12;
const TAMED_EVENT: i8 = 7;
const FULL_HIVE: &str = "5";
const SHEEP_SHEARING: &str = "minecraft:shearing/sheep";
const MOUNT_SUFFIXES: [&str; 3] = ["horse", "donkey", "mule"];

/// Set while the second baby of a pair is born, so that birth does not roll for twins again.
static TWINNING: AtomicBool = AtomicBool::new(false);

fn close_to(m: &Mob, center: [f64; 3], radius: f64) -> bool {
    let (dx, dy, dz) = (
        m.position.x - center[0],
        m.position.y - center[1],
        m.position.z - center[2],
    );
    dx * dx + dy * dy + dz * dz <= radius * radius
}

pub fn on_tick(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects, ticks: u32) {
    if state().stung.remove(&acting.uuid) {
        game.native_remove_effect(acting.uuid, "minecraft:poison");
    }
    if ticks.is_multiple_of(LEAD_PERIOD_TICKS) {
        lead_animals(game, acting, effects);
    }
    if ticks.is_multiple_of(rules::AREA_PERIOD_TICKS) {
        tend_animals(game, acting, effects);
    }
}

/// One pass a second over the animals around the player.
fn tend_animals(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    let grow = effect(effects, "grow_fast") as i32;
    let love = effect(effects, "love_cut") as i32 * rules::TICKS_PER_SECOND;
    let heal_chance = effect(effects, "animal_health");
    let calm = effect(effects, "bee_friend") > 0.0;
    let owner = uuid_ints(acting.uuid);
    let riding = game.riding;
    let center = acting.position;
    for m in game
        .mobs
        .iter_mut()
        .filter(|m| !m.removed && m.alive() && close_to(m, center, NEARBY_RADIUS))
    {
        if m.kind.flags.animal {
            care_for_animal(m, grow, love, heal_chance);
        }
        match m.species() {
            Species::Wolf if m.extra.tame && m.extra.owner == Some(owner) => {
                strengthen_wolf(m, effects)
            }
            Species::Bee if calm => horizon_client::mobs::bee::stop_being_angry(m),
            _ if riding == Some(m.id) && is_horse(m) => boost_mount(m, effects),
            _ => {}
        }
    }
}

fn is_horse(m: &Mob) -> bool {
    MOUNT_SUFFIXES
        .iter()
        .any(|suffix| m.kind.id.ends_with(*suffix))
}

fn care_for_animal(m: &mut Mob, grow: i32, love: i32, heal_chance: f64) {
    if grow > 0 && horizon_client::mobs::ai::can_age_up(m) {
        horizon_client::mobs::ai::age_up(m, grow, false);
    }
    if love > 0 && m.extra.age > 0 {
        m.extra.age = (m.extra.age - love).max(0);
    }
    if m.health < m.attrs.max_health && chance(heal_chance) {
        m.heal(1.0);
    }
}

fn strengthen_wolf(m: &mut Mob, effects: &Effects) {
    let strong = effect(effects, "wolf_strong") as i32;
    let tough = effect(effects, "wolf_tough") as i32;
    if strong > 0 {
        m.add_effect("minecraft:strength", rules::WOLF_STATUS_TICKS, strong - 1);
    }
    if tough > 0 {
        m.add_effect("minecraft:resistance", rules::WOLF_STATUS_TICKS, tough - 1);
        m.add_effect("minecraft:regeneration", rules::WOLF_STATUS_TICKS, 0);
    }
}

fn boost_mount(m: &mut Mob, effects: &Effects) {
    let boost = effect(effects, "horse_boost") as i32;
    if boost > 0 {
        m.add_effect("minecraft:speed", rules::HORSE_STATUS_TICKS, boost - 1);
        m.add_effect("minecraft:jump_boost", rules::HORSE_STATUS_TICKS, boost - 1);
    }
}

fn is_sneaking(game: &Game<'_>, acting: &NativeServerPlayer) -> bool {
    game.native_entity(acting.entity_id as u32)
        .and_then(|entity| game.native_entity_is_shift_key_down(entity))
        .unwrap_or(false)
}

/// Leader: while the player sneaks, animals around walk towards them without food in hand.
fn lead_animals(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    let radius = effect(effects, "lead_radius");
    if radius <= 0.0 || !is_sneaking(game, acting) {
        return;
    }
    let center = acting.position;
    for m in game
        .mobs
        .iter_mut()
        .filter(|m| !m.removed && m.alive() && m.kind.flags.animal && close_to(m, center, radius))
    {
        let (dx, dz) = (center[0] - m.position.x, center[2] - m.position.z);
        let distance = (dx * dx + dz * dz).sqrt();
        if distance <= LEAD_STOP_DISTANCE {
            continue;
        }
        m.ai.nav.stop();
        m.velocity.x += dx / distance * LEAD_PULL;
        m.velocity.z += dz / distance * LEAD_PULL;
        m.needs_sync = true;
    }
}

/// Bee keeper's harvest: a full hive can pay twice.
pub fn on_right_click(game: &mut Game<'_>, tree: &Tree, pos: [i32; 3], effects: &Effects) {
    if !tree.in_list("hive_blocks", &game.native_block(pos))
        || game.native_block_property(pos, "honey_level").as_deref() != Some(FULL_HIVE)
        || !chance(effect(effects, "honey_extra"))
    {
        return;
    }
    let extra = match game.native_held().map(|(item, _, _)| item).as_deref() {
        Some("minecraft:glass_bottle") => "minecraft:honey_bottle",
        Some("minecraft:shears") => "minecraft:honeycomb",
        _ => return,
    };
    game.native_give_items(extra, 1);
}

/// Shearer: a sheep drops more of its wool.
pub fn on_loot(table: &str, player: u128, drops: &mut Vec<ItemStack>) {
    if !table.starts_with(SHEEP_SHEARING) {
        return;
    }
    let effects = player_effects(player);
    if chance(effect(&effects, "shear_extra") + effect(&effects, "extra_wool")) {
        let wool = drops.first().map(|stack| stack.item.to_string());
        if let Some(wool) = wool {
            drops.push(ItemStack::new(&wool, 1));
        }
    }
}

/// Breeder: whether this birth brings a second baby. Pair with `twins_born`.
pub fn roll_twins(player: u128) -> bool {
    if TWINNING.load(Ordering::Relaxed) {
        return false;
    }
    let effects = player_effects(player);
    if chance(effect(&effects, "twins") + effect(&effects, "breed_chance")) {
        TWINNING.store(true, Ordering::Relaxed);
        return true;
    }
    false
}

pub fn twins_born() {
    TWINNING.store(false, Ordering::Relaxed);
}

/// Tamer: the taming food can win outright, ahead of the game's own roll.
pub fn on_interact(
    game: &mut Game<'_>,
    entity: i32,
    kind: &str,
    stack: Option<&ItemStack>,
    claimed: &mut bool,
) {
    let (Some(acting), Some(stack)) = (game.native_acting_player(), stack) else {
        return;
    };
    if *claimed || !rules::tames_with(kind, &stack.item) {
        return;
    }
    let effects = player_effects(acting.uuid);
    let odds = rules::tame_chance(
        effect(&effects, "tame_first"),
        effect(&effects, "tame_chance"),
        effect(&effects, "tame_always"),
    );
    let Some(index) = game
        .mobs
        .iter()
        .position(|m| m.id == entity as u32 && !m.removed && !m.extra.tame)
    else {
        return;
    };
    if horizon_client::mobs::tame::angry(&game.mobs[index]) || !chance(odds) {
        return;
    }
    tame(game, index, acting.uuid);
    game.consume_held();
    *claimed = true;
}

fn tame(game: &mut Game<'_>, index: usize, player: u128) {
    let m = &mut game.mobs[index];
    let species = m.species();
    if species == Species::Wolf {
        horizon_client::mobs::tame::set_tame(m, true);
    } else {
        m.extra.tame = true;
    }
    m.extra.owner = Some(uuid_ints(player));
    m.persistent = true;
    m.ai.nav.stop();
    m.ai.target = None;
    m.extra.sitting = species != Species::Parrot;
    let (id, chunk) = (
        m.id,
        (
            m.position.x.floor() as i32 >> 4,
            m.position.z.floor() as i32 >> 4,
        ),
    );
    game.world.modified.insert(chunk);
    game.mob_event_particles(id, TAMED_EVENT);
    let view = game.mob_entity(&game.mobs[index]);
    game.trigger_entity("minecraft:tame_animal", "entity", &view);
}

// Exploration nodes and what keeps a player's attributes up to date.
use crate::game::{
    Game,
    native_player_state::NativeEffect,
    pct_actions as actions, pct_fx_roam_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects},
    pct_rules::Effects,
};
use horizon_client::{mod_api::NativeServerPlayer, text::Text};
use horizon_core::geometry::Vec3;
use horizon_world::{nbt::Tag, worldgen::generator::BiomeSearch};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

const CHANNEL: &str = "path_of_craft_tree:roam";
const OVERWORLD: &str = "minecraft:overworld";
const COMPASS: &str = "minecraft:compass";
const COMPASS_TRACKER: &str = "minecraft:lodestone_tracker";
const VILLAGE_TAG: &str = "minecraft:village";
const SPEED: &str = "minecraft:speed";
const HOME_KEY: &str = "key.keyboard.h";
const COMPASS_KEY: &str = "key.keyboard.j";
const SENSE_KEY: &str = "key.keyboard.i";
const JOB_HOME: u8 = 0;
const JOB_COMPASS: u8 = 1;
const JOB_SENSE: u8 = 2;
const STRUCTURE_CHECK_TICKS: u32 = 40;
const HEAVY_CHECK_TICKS: u32 = 20;
const STATUS_TICKS: i32 = 60;
const BIOME_RADIUS: i32 = 3200;
const BIOME_COLUMN_STEP: i32 = 32;
const BIOME_LAYER_STEP: i32 = 64;
const ATTRIBUTE_EPSILON: f64 = 1e-6;
const VILLAGER_BOOST_LIMIT: i32 = 500;
const BOAT_THRUST: f32 = 0.04;

#[derive(Default)]
struct State {
    jobs: HashMap<u128, Vec<u8>>,
    home_used: HashMap<u128, Instant>,
    sense_used: HashMap<u128, Instant>,
    compass_used: HashMap<u128, Instant>,
    compass_choice: HashMap<u128, usize>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn say(game: &mut Game<'_>, player: u128, line: String) {
    game.native_action_bar(player, Text::literal(line));
}

// Keys on the client; the server does the work on the player's own next turn.

pub fn on_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed {
        return;
    }
    let job = {
        let view = glue::client();
        match key {
            HOME_KEY if effect(&view.effects, "home") > 0.0 => Some(JOB_HOME),
            COMPASS_KEY if effect(&view.effects, "cartographer") > 0.0 => Some(JOB_COMPASS),
            SENSE_KEY if effect(&view.effects, "treasure_sense") > 0.0 => Some(JOB_SENSE),
            _ => None,
        }
    };
    if let Some(job) = job
        && let Err(error) = game.native_net_send_to_server(CHANNEL, &[job])
    {
        log::warn!("path_of_craft_tree: ability not sent: {error}");
    }
}

pub fn on_client_message(player: u128, channel: &str, data: &[u8]) {
    if channel == CHANNEL
        && let [job] = data
    {
        state().jobs.entry(player).or_default().push(*job);
    }
}

/// True when the cooldown has passed, and starts it again.
fn ready(
    used: fn(&mut State) -> &mut HashMap<u128, Instant>,
    player: u128,
    cooldown: f64,
) -> Result<(), f64> {
    let mut state = state();
    let map = used(&mut state);
    let wait = Duration::from_secs_f64(cooldown);
    if let Some(left) = map
        .get(&player)
        .and_then(|at| wait.checked_sub(at.elapsed()))
    {
        return Err(left.as_secs_f64());
    }
    map.insert(player, Instant::now());
    Ok(())
}

pub fn run_jobs(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    let jobs = state().jobs.remove(&acting.uuid).unwrap_or_default();
    for job in jobs {
        match job {
            JOB_HOME => go_home(game, acting, effects),
            JOB_COMPASS => point_compass(game, acting, effects),
            JOB_SENSE => sense_structure(game, acting, effects),
            _ => {}
        }
    }
}

fn wait_message(left: f64) -> String {
    format!("Рано, ещё {} с", left.ceil() as u64)
}

// Return Home.

fn home_position(game: &Game<'_>) -> Option<[f64; 3]> {
    let spawn = game.spawn.map(|at: Vec3| [at.x, at.y, at.z]);
    let world = game
        .level
        .world_spawn
        .map(|[x, y, z]| [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5]);
    spawn.or(world)
}

fn go_home(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    if effect(effects, "home") <= 0.0 {
        return;
    }
    let get = |key: &str| effect(effects, key);
    let Some(home) = home_position(game) else {
        return say(game, acting.uuid, "Дом не найден".to_owned());
    };
    match ready(
        |s| &mut s.home_used,
        acting.uuid,
        rules::home_cooldown_seconds(&get),
    ) {
        Ok(()) => {
            game.native_teleport_player(acting.uuid, OVERWORLD, home, acting.yaw, acting.pitch);
        }
        Err(left) => say(game, acting.uuid, wait_message(left)),
    }
}

// Cartographer: the held compass points at the nearest chosen biome.

fn next_biome(player: u128, biomes: &[&'static str]) -> &'static str {
    let mut state = state();
    let index = state.compass_choice.entry(player).or_default();
    let biome = biomes[*index % biomes.len()];
    *index = index.wrapping_add(1);
    biome
}

fn find_biome(game: &Game<'_>, wanted: &str, from: [i32; 3]) -> Option<[i32; 3]> {
    let allowed = |name: &str| name == wanted;
    let search = BiomeSearch {
        allowed: &allowed,
        origin: from,
        radius: BIOME_RADIUS,
        horizontal: BIOME_COLUMN_STEP,
        vertical: BIOME_LAYER_STEP,
        min_y: game.world.bottom_y(),
        max_y: game.world.top_y() - 1,
    };
    game.end
        .locator()
        .and_then(|terrain| terrain.find_closest_biome(&search))
        .map(|(_, at)| at)
}

fn compass_target(dimension: &str, at: [i32; 3]) -> Tag {
    Tag::compound([
        (
            "target",
            Tag::compound([
                ("dimension", Tag::String(dimension.into())),
                ("pos", Tag::IntArray(at.to_vec())),
            ]),
        ),
        ("tracked", Tag::Byte(0)),
    ])
}

fn feet(acting: &NativeServerPlayer) -> [i32; 3] {
    acting.position.map(|v| v.floor() as i32)
}

fn point_compass(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    if effect(effects, "cartographer") <= 0.0 {
        return;
    }
    let holds_compass = game.held().is_some_and(|stack| &*stack.item == COMPASS);
    if !holds_compass {
        return say(game, acting.uuid, "Нужен компас в руке".to_owned());
    }
    if let Err(left) = ready(
        |s| &mut s.compass_used,
        acting.uuid,
        rules::COMPASS_COOLDOWN_SECONDS,
    ) {
        return say(game, acting.uuid, wait_message(left));
    }
    let get = |key: &str| effect(effects, key);
    let biome = next_biome(acting.uuid, &rules::compass_biomes(&get));
    let from = feet(acting);
    let Some(at) = find_biome(game, biome, from) else {
        return say(game, acting.uuid, format!("Поблизости нет {biome}"));
    };
    if let Some(held) = game.held_mut().as_mut() {
        held.data_mut()
            .set_other(COMPASS_TRACKER, Some(compass_target(&acting.dimension, at)));
    }
    let distance = horizontal_distance(from, at);
    say(game, acting.uuid, format!("Компас: {biome}, {distance} м"));
}

fn horizontal_distance(from: [i32; 3], at: [i32; 3]) -> i32 {
    let (dx, dz) = (f64::from(at[0] - from[0]), f64::from(at[2] - from[2]));
    (dx * dx + dz * dz).sqrt().floor() as i32
}

// Treasure Sense: names the direction of the nearest structure.

fn sense_structure(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    if effect(effects, "treasure_sense") <= 0.0 {
        return;
    }
    if let Err(left) = ready(
        |s| &mut s.sense_used,
        acting.uuid,
        rules::SENSE_COOLDOWN_SECONDS,
    ) {
        return say(game, acting.uuid, wait_message(left));
    }
    let get = |key: &str| effect(effects, key);
    let mut names = game.structure_tag(VILLAGE_TAG);
    names.extend(rules::sense_extra_structures(&get));
    let from = feet(acting);
    let found = game
        .end
        .locator()
        .and_then(|terrain| terrain.locate(&names, from, rules::STRUCTURE_SEARCH_RADIUS));
    let Some((name, at)) = found else {
        return say(game, acting.uuid, "Ничего не чувствуется".to_owned());
    };
    let (dx, dz) = (f64::from(at[0] - from[0]), f64::from(at[2] - from[2]));
    let distance = horizontal_distance(from, at);
    let direction = rules::direction_name(dx, dz);
    say(
        game,
        acting.uuid,
        format!("{name}: {distance} м {direction}"),
    );
}

// Upkeep.

/// Keeps the attribute bases in step with the allocated nodes, also after a respec.
pub fn keep_attributes(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    let get = |key: &str| effect(effects, key);
    for (attribute, wanted) in rules::attribute_targets(&get) {
        let Some((base, _)) = game.native_attribute(acting.uuid, attribute) else {
            continue;
        };
        if (base - wanted).abs() > ATTRIBUTE_EPSILON
            && !game.native_set_attribute_base(acting.uuid, attribute, wanted)
        {
            log::warn!("path_of_craft_tree: {attribute} not set");
        }
    }
}

fn keep_heavy_step(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    let get = |key: &str| effect(effects, key);
    let heavy = game
        .armor
        .iter()
        .flatten()
        .filter(|stack| rules::is_heavy_armor(&stack.item))
        .count();
    if let Some(amplifier) = rules::heavy_step_amplifier(&get, heavy) {
        let speed = NativeEffect {
            id: SPEED.to_owned(),
            amplifier,
            duration: STATUS_TICKS,
            ambient: false,
            particles: false,
        };
        game.native_add_effect(acting.uuid, &speed);
    }
}

fn earn_structures(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    for name in game.structures_at(feet(acting)) {
        if glue::mark_seen(acting.uuid, format!("structure:{name}")) {
            actions::earn_once(game, acting.uuid, "act:structure", 1.0);
        }
    }
}

pub fn on_period(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects, ticks: u32) {
    if ticks.is_multiple_of(STRUCTURE_CHECK_TICKS) {
        earn_structures(game, acting);
    }
    if ticks.is_multiple_of(HEAVY_CHECK_TICKS) {
        keep_heavy_step(game, acting, effects);
    }
}

// Hooks.

/// Tireless and its small nodes: a chance that a point of hunger is not spent.
pub fn on_food_changed(player: u128, from: i32, to: i32, cancel: &mut bool) {
    if to < from && chance(effect(&player_effects(player), "hunger_save")) {
        *cancel = true;
    }
}

/// Charm: villagers price their trades as if they liked the player more.
pub fn reputation(value: i32, uuid: [i32; 4]) -> i32 {
    let player = horizon_client::lan::playerdata::ints_uuid(uuid);
    let charm = effect(&player_effects(player), "charm") as i32;
    value.saturating_add(charm).min(VILLAGER_BOOST_LIMIT)
}

/// Captain: the driver's boat pulls harder, added the way the engine adds its own paddling.
pub fn boat_thrust(
    game: &Game<'_>,
    velocity: &mut Vec3,
    yaw: f32,
    forward: bool,
    controlled: bool,
) {
    if !forward || !controlled {
        return;
    }
    let captain = match game.native_acting_player() {
        Some(acting) => effect(&player_effects(acting.uuid), "captain"),
        None => effect(&glue::client().effects, "captain"),
    } as f32;
    if captain <= 0.0 {
        return;
    }
    let radians = yaw.to_radians();
    let thrust = BOAT_THRUST * captain;
    velocity.x += f64::from((-radians).sin() * thrust);
    velocity.z += f64::from(radians.cos() * thrust);
}

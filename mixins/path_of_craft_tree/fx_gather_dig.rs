// Digging in an area, collapsing columns and everything a shovel does on a right click.
use crate::game::{
    Game,
    pct_actions::earn_once,
    pct_fx_gather::{
        self as gather, ChainGuard, KIND_COLLAPSE, KIND_DIG_AREA, KIND_MINE_AREA, KIND_UTILITY,
    },
    pct_fx_gather_rules as rules,
    pct_mining::{chance, effect, on_place, player_effects},
    pct_rules::Effects,
};
use horizon_client::{items::ItemStack, mod_api::NativeServerPlayer, pose::Pose};
use std::{
    collections::HashSet,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const PICKAXE_TAG: &str = "minecraft:mineable/pickaxe";
const SHOVEL_TAG: &str = "minecraft:mineable/shovel";
const PATH_ACTION: &str = "act:path";
const AIR: &str = "minecraft:air";
const MAX_REMEMBERED_PATHS: usize = 200_000;
const FILL_MATERIALS: [&str; 2] = ["minecraft:dirt", "minecraft:sand"];
const STUN_PUSH: f64 = 0.8;
const STUN_LIFT: f64 = 0.35;

/// Blocks already turned into a path, so re-walking one pays nothing twice.
static PATHS: LazyLock<Mutex<HashSet<(String, [i32; 3])>>> = LazyLock::new(Mutex::default);

fn paths() -> MutexGuard<'static, HashSet<(String, [i32; 3])>> {
    PATHS.lock().unwrap_or_else(PoisonError::into_inner)
}

fn hardness(game: &Game<'_>, pos: [i32; 3]) -> f32 {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    game.res.facts().get(state).hardness
}

fn can_take(game: &Game<'_>, pos: [i32; 3], tag: &str, origin_hardness: f32) -> bool {
    let name = game.native_block(pos);
    let hard = hardness(game, pos);
    name != AIR
        && game.block_in_tag(tag, &name)
        && (0.0..=origin_hardness + rules::AREA_HARDNESS_MARGIN).contains(&hard)
        && game
            .world
            .block_entities
            .get(&(pos[0], pos[1], pos[2]))
            .is_none()
        && !gather::is_placed(game, pos)
}

struct AreaSpec {
    kind: u8,
    tag: &'static str,
    radius: i32,
    tool_fits: fn(&str) -> bool,
}

fn break_area(game: &mut Game<'_>, acting: &NativeServerPlayer, pos: [i32; 3], spec: &AreaSpec) {
    let origin = game.native_block(pos);
    if !game.block_in_tag(spec.tag, &origin) {
        return;
    }
    let origin_hardness = hardness(game, pos);
    let axis = rules::view_axis(acting.yaw, acting.pitch);
    let targets: Vec<[i32; 3]> = rules::plane_offsets(axis, spec.radius)
        .into_iter()
        .map(|offset| rules::add(pos, offset))
        .filter(|target| can_take(game, *target, spec.tag, origin_hardness))
        .collect();
    let _chain = ChainGuard::start(spec.kind);
    for target in targets {
        let fits = game
            .native_held()
            .is_some_and(|(item, _, _)| (spec.tool_fits)(&item));
        if !fits {
            break;
        }
        game.native_player_break_block(acting.uuid, target, false);
    }
}

pub fn start_mine_area(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    pos: [i32; 3],
    effects: &Effects,
) {
    if effect(effects, "mine_area") <= 0.0 {
        return;
    }
    let spec = AreaSpec {
        kind: KIND_MINE_AREA,
        tag: PICKAXE_TAG,
        radius: 1,
        tool_fits: rules::is_pickaxe,
    };
    break_area(game, acting, pos, &spec);
}

fn start_collapse(game: &mut Game<'_>, player: u128, pos: [i32; 3], limit: usize) {
    let block = game.native_block(pos);
    if !rules::LOOSE_BLOCKS.contains(&block.as_str()) {
        return;
    }
    let above: Vec<[i32; 3]> = (1..=limit as i32)
        .map(|dy| [pos[0], pos[1] + dy, pos[2]])
        .take_while(|at| game.native_block(*at) == block)
        .collect();
    let _chain = ChainGuard::start(KIND_COLLAPSE);
    // From the top, so nothing is left hanging to fall as the column goes.
    for at in above.into_iter().rev() {
        if !game
            .native_held()
            .is_some_and(|(item, _, _)| rules::is_shovel(&item))
        {
            break;
        }
        game.native_player_break_block(player, at, false);
    }
}

pub fn start_shovel_chains(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    pos: [i32; 3],
    effects: &Effects,
) {
    let collapse = effect(effects, "collapse") as usize;
    if collapse > 0 {
        start_collapse(game, acting.uuid, pos, collapse);
    }
    if effect(effects, "dig_area") > 0.0 {
        let wide = effect(effects, "dig_area_wide") > 0.0;
        let spec = AreaSpec {
            kind: KIND_DIG_AREA,
            tag: SHOVEL_TAG,
            radius: if wide { 2 } else { 1 },
            tool_fits: rules::is_shovel,
        };
        break_area(game, acting, pos, &spec);
    }
}

// Right clicks.

fn is_crouching(game: &Game<'_>) -> bool {
    game.player
        .as_ref()
        .is_some_and(|p| p.pose == Pose::Crouching)
}

fn set_block(game: &mut Game<'_>, pos: [i32; 3], block: &str) -> bool {
    let Some(state) = game.native_default_state(block) else {
        return false;
    };
    let dimension = game.native_dimension();
    game.native_set_block(dimension, pos, state, gather::SET_BLOCK_FLAGS)
}

fn fill_material(game: &mut Game<'_>) -> Option<&'static str> {
    FILL_MATERIALS
        .into_iter()
        .find(|item| game.native_count_items(item) > 0)
}

fn axis_of(face: [i32; 3]) -> usize {
    face.iter().position(|part| *part != 0).unwrap_or(1)
}

/// Backfill: the blocks in front of the clicked face are filled from the inventory.
fn fill(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], effects: &Effects) {
    let radius = if effect(effects, "fill_wide") > 0.0 {
        2
    } else {
        1
    };
    let centre = rules::add(pos, face);
    let cells = rules::plane_offsets(axis_of(face), radius)
        .into_iter()
        .map(|offset| rules::add(centre, offset))
        .chain([centre]);
    let creative = game
        .native_acting_player()
        .is_some_and(|a| a.game_mode == 1);
    for cell in cells {
        if game.native_block(cell) != AIR {
            continue;
        }
        let Some(material) = fill_material(game) else {
            break;
        };
        if !set_block(game, cell, material) {
            continue;
        }
        on_place(game, cell);
        if !creative && !chance(effect(effects, "fill_save")) {
            game.native_take_items(material, 1);
        }
        game.damage_held(1);
    }
}

/// Pays for a path made on this block for the first time.
fn pay_path(game: &mut Game<'_>, player: u128, pos: [i32; 3]) {
    if gather::is_placed(game, pos) {
        return;
    }
    let key = (game.native_dimension().to_owned(), pos);
    let is_new = {
        let mut paths = paths();
        if paths.len() >= MAX_REMEMBERED_PATHS {
            paths.clear();
        }
        paths.insert(key)
    };
    if is_new {
        earn_once(game, player, PATH_ACTION, 1.0);
    }
}

fn can_path(game: &Game<'_>, pos: [i32; 3]) -> bool {
    rules::is_pathable(&game.native_block(pos))
        && game.native_block([pos[0], pos[1] + 1, pos[2]]) == AIR
}

fn make_path(game: &mut Game<'_>, player: u128, pos: [i32; 3]) {
    if set_block(game, pos, "minecraft:dirt_path") {
        pay_path(game, player, pos);
        game.damage_held(1);
    }
}

/// Landscaper's levelling: knocks down a bump, or lowers the path into a one-block dip.
fn path_cell(game: &mut Game<'_>, player: u128, cell: [i32; 3], levels: bool) {
    if can_path(game, cell) {
        make_path(game, player, cell);
        return;
    }
    if !levels {
        return;
    }
    let above = [cell[0], cell[1] + 1, cell[2]];
    if can_path(game, above) {
        {
            let _chain = ChainGuard::start(KIND_UTILITY);
            game.native_player_break_block(player, above, false);
        }
        if can_path(game, cell) {
            make_path(game, player, cell);
        }
        return;
    }
    let below = [cell[0], cell[1] - 1, cell[2]];
    if game.native_block(cell) == AIR && can_path(game, below) {
        make_path(game, player, below);
    }
}

fn shovel_use(game: &mut Game<'_>, player: u128, pos: [i32; 3], face: [i32; 3], effects: &Effects) {
    if face == [0, -1, 0] || !can_path(game, pos) {
        return;
    }
    // The game turns the clicked block itself right after this.
    pay_path(game, player, pos);
    if effect(effects, "path_area") <= 0.0 {
        return;
    }
    let radius = if effect(effects, "path_wide") > 0.0 {
        2
    } else {
        1
    };
    let levels = effect(effects, "path_level") > 0.0;
    for offset in rules::plane_offsets(1, radius) {
        path_cell(game, player, rules::add(pos, offset), levels);
    }
}

/// Everything an axe or a shovel does on a right click.
pub fn on_use_item_on(
    game: &mut Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    let Some(player) = game.native_acting_player().map(|acting| acting.uuid) else {
        return;
    };
    let effects = player_effects(player);
    if rules::is_axe(&stack.item) {
        crate::game::pct_fx_gather_tree::on_axe_use(game, player, pos, &effects);
    } else if rules::is_shovel(&stack.item) {
        if is_crouching(game) && effect(&effects, "fill") > 0.0 {
            fill(game, pos, face, &effects);
            *claimed = true;
            return;
        }
        shovel_use(game, player, pos, face, &effects);
    }
}

// Fighting with a shovel. Only mobs: the player target is the PvP path and stays untouched.

fn holds_shovel(game: &Game<'_>) -> bool {
    game.held()
        .is_some_and(|stack| rules::is_shovel(&stack.item))
}

/// War Shovel: the sword's extra damage, scaled by the attack strength like the base damage.
pub fn on_melee_damage(
    game: &Game<'_>,
    target_kind: &str,
    damage: (f32, f32, bool, bool),
) -> (f32, f32, bool, bool) {
    let Some(player) = game.native_acting_player().map(|acting| acting.uuid) else {
        return damage;
    };
    if target_kind == "minecraft:player"
        || !holds_shovel(game)
        || effect(&player_effects(player), "shovel_fight") <= 0.0
    {
        return damage;
    }
    let (amount, scale, crit, magic) = damage;
    let bonus = rules::SHOVEL_BONUS_DAMAGE * (0.2 + scale * scale * 0.8);
    (amount + bonus, scale, crit, magic)
}

pub fn on_hurt_enemy(game: &mut Game<'_>, stack: &ItemStack, entity: u32) {
    if !rules::is_shovel(&stack.item) {
        return;
    }
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    if !chance(effect(&player_effects(acting.uuid), "shovel_stun")) {
        return;
    }
    let radians = f64::from(acting.yaw).to_radians();
    let push = [
        -radians.sin() * STUN_PUSH,
        STUN_LIFT,
        radians.cos() * STUN_PUSH,
    ];
    let dimension = game.native_dimension();
    game.native_push_mob(dimension, entity, push);
}

// Archery nodes: the shot is changed where the projectile enters the world, the hit where the
// engine works out its damage, so a mob and another player can be treated differently.
use crate::game::{
    Game, pct_fx_roam_combat as combat, pct_fx_roam_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects},
    pct_rules::Effects,
};
use horizon_client::{
    arrows::Pickup,
    items::ItemStack,
    mobs::Who,
    mod_api::NativeServerPlayer,
    pct_fx_roam_flags::NO_USE_SLOWDOWN,
    projectiles::{Hit, Projectile, ProjectileKind},
};
use horizon_core::geometry::Vec3;
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    ops::ControlFlow,
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError, atomic::Ordering},
};

const BOW: &str = "minecraft:bow";
const CROSSBOW: &str = "minecraft:crossbow";
const CREATIVE: i32 = 1;
const MAX_EXTRAS: usize = 4_096;
const FULL_DRAW_TICKS: f32 = 20.0;

#[derive(Default)]
struct State {
    /// Arrows the mod added to a volley: they never hurt another player.
    extras: HashSet<u32>,
    /// Arrows owed to players, handed over on their next turn.
    owed: HashMap<u128, Vec<ItemStack>>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

thread_local! {
    static LAUNCHING: Cell<bool> = const { Cell::new(false) };
    static HITTING: Cell<bool> = const { Cell::new(false) };
    /// What is left of the next crossbow charge time, 0 for no change.
    static CROSSBOW_LEFT: Cell<f64> = const { Cell::new(0.0) };
}

/// Sets a flag until dropped, even if the launch or hit panics.
struct Busy(&'static std::thread::LocalKey<Cell<bool>>);

impl Busy {
    fn start(flag: &'static std::thread::LocalKey<Cell<bool>>) -> Busy {
        flag.set(true);
        Busy(flag)
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

fn is_arrow(p: &Projectile) -> bool {
    matches!(p.kind, ProjectileKind::Arrow { .. })
}

fn owe(player: u128, stack: ItemStack) {
    state().owed.entry(player).or_default().push(stack);
}

/// Hands over arrows owed to this player, whose inventory only their own turn can reach.
pub fn on_period(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let owed = state().owed.remove(&acting.uuid).unwrap_or_default();
    for stack in owed {
        game.native_give_stack(acting.uuid, stack);
    }
}

fn scale(velocity: Vec3, factor: f64) -> Vec3 {
    Vec3::new(
        velocity.x * factor,
        velocity.y * factor,
        velocity.z * factor,
    )
}

fn rotate_yaw(velocity: Vec3, degrees: f64) -> Vec3 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    Vec3::new(
        velocity.x * cos - velocity.z * sin,
        velocity.y,
        velocity.x * sin + velocity.z * cos,
    )
}

/// Far Shot speeds the arrow up without making it hit harder, in PvP or anywhere else.
fn apply_speed(p: &mut Projectile, get: &impl Fn(&str) -> f64) {
    let factor = 1.0 + get("projectile_speed");
    if factor <= 1.0 {
        return;
    }
    p.velocity = scale(p.velocity, factor);
    if let ProjectileKind::Arrow { base_damage, .. } = &mut p.kind {
        *base_damage /= factor;
    }
}

fn apply_pierce(p: &mut Projectile, get: &impl Fn(&str) -> f64) {
    let extra = get("pierce") as u8;
    if let Some(arrow) = p.arrow.as_mut().filter(|_| extra > 0) {
        arrow.pierce = arrow.pierce.saturating_add(extra);
    }
}

/// Keen Eye: the arrow comes back into the inventory and the shot one can no longer be collected.
fn apply_saving(p: &mut Projectile, player: u128, get: &impl Fn(&str) -> f64) {
    let Some(arrow) = p.arrow.as_mut() else {
        return;
    };
    if matches!(arrow.pickup, Pickup::Allowed) && chance(get("arrow_save")) {
        owe(player, arrow.pickup_item.clone());
        arrow.pickup = Pickup::CreativeOnly;
    }
}

/// Multishot's arrows are the weapon's own, so the volley is not doubled on top of it.
fn has_multishot(p: &Projectile) -> bool {
    p.arrow
        .as_ref()
        .and_then(|arrow| arrow.fired_from_weapon.as_ref())
        .is_some_and(|weapon| weapon.enchantment_level("minecraft:multishot") > 0)
}

fn launch_volley(game: &mut Game<'_>, p: &Projectile, get: &impl Fn(&str) -> f64) {
    let arrows = 1 + get("multi_shot").max(0.0) as usize;
    if arrows <= 1 || has_multishot(p) {
        return;
    }
    let _busy = Busy::start(&LAUNCHING);
    for yaw in rules::fan_yaws(arrows) {
        let mut extra = p.clone();
        extra.id = 0;
        extra.velocity = rotate_yaw(p.velocity, yaw);
        if let Some(arrow) = extra.arrow.as_mut() {
            arrow.pickup = Pickup::CreativeOnly;
        }
        game.add_projectile(extra);
        remember_extra(game.next_entity);
    }
}

fn remember_extra(id: u32) {
    let mut state = state();
    if state.extras.len() >= MAX_EXTRAS {
        state.extras.clear();
    }
    state.extras.insert(id);
}

/// A player's arrow or trident entering the world.
pub fn on_launch(game: &mut Game<'_>, p: &mut Projectile) {
    if LAUNCHING.get() || p.owner != Some(Who::Player) || p.arrow.is_none() {
        return;
    }
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let effects = player_effects(acting.uuid);
    if effects.is_empty() {
        return;
    }
    let get = |key: &str| effect(&effects, key);
    apply_speed(p, &get);
    if !is_arrow(p) {
        return;
    }
    apply_pierce(p, &get);
    if acting.game_mode != CREATIVE {
        apply_saving(p, acting.uuid, &get);
    }
    launch_volley(game, p, &get);
}

fn mob_health(game: &Game<'_>, id: u32) -> Option<f32> {
    game.mobs.iter().find(|m| m.id == id).map(|m| m.health)
}

fn flew_far(p: &Projectile, at: Vec3) -> bool {
    p.arrow
        .as_ref()
        .and_then(|arrow| arrow.spawn_pos)
        .is_some_and(|from| {
            let (dx, dy, dz) = (at.x - from.x, at.y - from.y, at.z - from.z);
            rules::is_far_shot((dx * dx + dy * dy + dz * dz).sqrt())
        })
}

fn after_mob_hit(game: &mut Game<'_>, p: &Projectile, id: u32, shooter: u128, effects: &Effects) {
    let get = |key: &str| effect(effects, key);
    let mark_seconds = get("hunter_mark");
    if mark_seconds > 0.0
        && let Some(mob) = game.mobs.iter_mut().find(|m| m.id == id)
    {
        mob.add_effect("minecraft:glowing", (mark_seconds * 20.0) as i32, 0);
    }
    let collectable = p
        .arrow
        .as_ref()
        .filter(|arrow| is_arrow(p) && matches!(arrow.pickup, Pickup::Allowed));
    if let Some(arrow) = collectable.filter(|_| chance(get("arrow_return"))) {
        owe(shooter, arrow.pickup_item.clone());
    }
}

fn hit_mob(
    game: &mut Game<'_>,
    p: &Projectile,
    hit: Hit,
    id: u32,
    at: Vec3,
    shooter: u128,
) -> ControlFlow<horizon_client::Result<()>> {
    let Some(before) = mob_health(game, id) else {
        return ControlFlow::Continue(());
    };
    let effects = player_effects(shooter);
    if effects.is_empty() && !flew_far(p, at) {
        return ControlFlow::Continue(());
    }
    let mut shot = p.clone();
    if let ProjectileKind::Arrow { base_damage, .. } = &mut shot.kind {
        *base_damage *= 1.0 + effect(&effects, "ranged_damage");
    }
    let result = {
        let _busy = Busy::start(&HITTING);
        game.projectile_hit(&shot, hit)
    };
    let landed = mob_health(game, id).is_none_or(|after| after < before);
    if landed {
        if flew_far(p, at) {
            combat::note_far_shot(id, shooter);
        }
        after_mob_hit(game, p, id, shooter, &effects);
    }
    ControlFlow::Break(result)
}

/// A player's shot reaching another player carries none of the archery nodes.
fn hit_player(
    game: &mut Game<'_>,
    p: &Projectile,
    hit: Hit,
) -> ControlFlow<horizon_client::Result<()>> {
    if state().extras.contains(&p.id) {
        return ControlFlow::Break(Ok(()));
    }
    if p.arrow.as_ref().is_none_or(|arrow| arrow.pierce == 0) {
        return ControlFlow::Continue(());
    }
    let mut shot = p.clone();
    if let Some(arrow) = shot.arrow.as_mut() {
        arrow.pierce = 0;
    }
    let _busy = Busy::start(&HITTING);
    ControlFlow::Break(game.projectile_hit(&shot, hit))
}

pub fn on_projectile_hit(
    game: &mut Game<'_>,
    p: &Projectile,
    hit: Hit,
    shooter: Option<u128>,
) -> ControlFlow<horizon_client::Result<()>> {
    if HITTING.get() || p.owner != Some(Who::Player) || p.arrow.is_none() {
        return ControlFlow::Continue(());
    }
    let Hit::Entity { who, at } = hit else {
        return ControlFlow::Continue(());
    };
    match (who, shooter) {
        (Who::Player, _) => hit_player(game, p, hit),
        (Who::Mob(id), Some(shooter)) => hit_mob(game, p, hit, id, at, shooter),
        (Who::Mob(_), None) => ControlFlow::Continue(()),
    }
}

/// Quick Draw: the same hold counts for more charge.
pub fn on_bow_loose(game: &Game<'_>, charge: &mut i32) {
    let Some(acting) = game.native_acting_player() else {
        return;
    };
    let effects = player_effects(acting.uuid);
    *charge = rules::draw_charge(&|key: &str| effect(&effects, key), *charge);
}

fn acting_or_client_effects(game: &Game<'_>) -> Arc<Effects> {
    match game.native_acting_player() {
        Some(acting) => player_effects(acting.uuid),
        None => glue::client().effects.clone(),
    }
}

/// Prepares the shorter charge for the crossbow the engine is about to tick.
pub fn before_crossbow_tick(game: &Game<'_>) {
    let effects = acting_or_client_effects(game);
    CROSSBOW_LEFT.set(rules::crossbow_time_left(&|key: &str| {
        effect(&effects, key)
    }));
}

pub fn crossbow_ticks(ticks: u32) -> u32 {
    let left = CROSSBOW_LEFT.replace(0.0);
    if left > 0.0 {
        rules::scaled_ticks(ticks, left)
    } else {
        ticks
    }
}

fn used_item(game: &Game<'_>) -> Option<(String, u32)> {
    let (slot, ticks) = game.using?;
    Some((game.stack_in(slot)?.item.to_string(), ticks))
}

/// Eagle Eye: the client narrows its view while the bow is drawn.
pub fn on_fov(game: &Game<'_>, modifier: &mut f32) {
    let Some((item, ticks)) = used_item(game) else {
        return;
    };
    if item != BOW && item != CROSSBOW {
        return;
    }
    let zoom = effect(&glue::client().effects, "eagle") as f32;
    let drawn = (ticks as f32 / FULL_DRAW_TICKS).min(1.0);
    *modifier *= 1.0 - zoom * drawn;
}

/// Shooting on the Move: keeps the client's speed while drawing. Read by the movement step.
pub fn on_movement_input(game: &Game<'_>) {
    let client = glue::client();
    let free = match used_item(game).map(|(item, _)| item).as_deref() {
        Some(BOW) => effect(&client.effects, "shoot_move") > 0.0,
        Some(CROSSBOW) => effect(&client.effects, "shoot_move_crossbow") > 0.0,
        _ => false,
    };
    NO_USE_SLOWDOWN.store(free, Ordering::Relaxed);
}

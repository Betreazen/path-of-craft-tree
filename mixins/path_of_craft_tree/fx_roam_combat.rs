// Melee and shield nodes. Hits are taken at the engine's own entry points, where the attacker and
// the weapon are known and PvP can be told apart.
use crate::game::{
    Game, lan_pvp,
    native_hooks::NativeLootDrops,
    pct_actions as actions, pct_effects, pct_fx_roam as roam, pct_fx_roam_rules as rules,
    pct_glue as glue,
    pct_mining::{chance, effect, player_effects},
    pct_rules::Effects,
};
use horizon_client::{
    health::HurtSource,
    items::BlocksAttacks,
    mobs::{Damage, DamageKind, Mob, Who},
    mod_api::NativeServerPlayer,
};
use std::{
    borrow::Cow,
    cell::Cell,
    collections::{HashMap, HashSet},
    ops::ControlFlow,
    sync::{
        LazyLock, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

// A shield loses this much plus one per blocked point; the engine's block step cannot be hooked.
const SHIELD_WEAR_BASE: i32 = 1;
const BASH_REACH: f64 = 4.0;
const CREEPER: &str = "minecraft:creeper";
const PERIOD_SECONDS: f32 = 0.5;
const REFLECT_DAMAGE: &str = "minecraft:thorns";
const BLEED_DAMAGE: &str = "minecraft:magic";
const BASH_DAMAGE: &str = "minecraft:generic";
const PLAYER_ATTACK: &str = "minecraft:player_attack";
const SHIELD: &str = "minecraft:shield";
const SHIELD_RAISED_TICKS: u32 = 100;
const PERIOD_TICKS: f64 = 10.0;
const MAX_REMEMBERED: usize = 4_096;
const BASH_LIFT: f64 = 0.3;
const DEFAULT_MOB_HEALTH: f64 = 20.0;

/// Damage types of a mob's shot: the mob behind them is named by `projectile_owner`.
const SHOT_DAMAGE: [&str; 4] = [
    "minecraft:arrow",
    "minecraft:trident",
    "minecraft:thrown",
    "minecraft:mob_projectile",
];

struct Bleed {
    owner: u128,
    per_second: f32,
    left: f32,
}

#[derive(Default)]
struct State {
    pending_blocks: Vec<(u128, f64)>,
    bleeding: HashMap<u32, Bleed>,
    last_block: HashMap<u128, Instant>,
    war_cry_until: HashMap<u128, Instant>,
    block_budget: HashMap<u128, (Instant, f64)>,
    attack_extra: HashMap<u128, u32>,
    spawner_mobs: HashSet<u32>,
    far_shots: HashMap<u32, u128>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

thread_local! {
    static HITTING_MOB: Cell<bool> = const { Cell::new(false) };
    static BEING_HIT: Cell<bool> = const { Cell::new(false) };
    static PUSHED: Cell<bool> = const { Cell::new(false) };
    static WIDE_SHIELD: Cell<bool> = const { Cell::new(false) };
}

/// Sets a flag until dropped, even if the hit panics.
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

fn is_weapon_hit(source: &Damage) -> bool {
    source.attacker == Some(Who::Player)
        && matches!(
            source.kind,
            DamageKind::PlayerAttack | DamageKind::MaceSmash | DamageKind::Spear
        )
}

fn take_countered(player: u128) -> bool {
    let window = Duration::from_secs_f64(rules::COUNTER_WINDOW_SECONDS);
    state()
        .last_block
        .remove(&player)
        .is_some_and(|at| at.elapsed() <= window)
}

fn war_cry_active(player: u128) -> bool {
    state()
        .war_cry_until
        .get(&player)
        .is_some_and(|until| Instant::now() < *until)
}

fn own_health_share(game: &mut Game<'_>, player: u128) -> f32 {
    let max = game
        .native_attribute(player, "minecraft:max_health")
        .map_or(20.0, |(_, value)| value as f32)
        .max(1.0);
    game.player
        .as_ref()
        .map_or(1.0, |p| p.health.health() / max)
}

/// A player's melee hit on a mob: their nodes change the damage, then react to the landed hit.
/// The caller is the engine's only way a player's weapon hurts a mob.
pub fn on_melee_hit(
    game: &mut Game<'_>,
    index: usize,
    amount: f32,
    source: Damage,
) -> ControlFlow<bool> {
    if HITTING_MOB.get() || !is_weapon_hit(&source) {
        return ControlFlow::Continue(());
    }
    let Some(acting) = game.native_acting_player() else {
        return ControlFlow::Continue(());
    };
    let effects = player_effects(acting.uuid);
    let Some(mob) = game.mobs.get(index) else {
        return ControlFlow::Continue(());
    };
    if effects.is_empty() {
        return ControlFlow::Continue(());
    }
    let (id, target_share) = (mob.id, mob.health / mob.attrs.max_health.max(1.0));
    let get = |key: &str| effect(&effects, key);
    let countered = get("counter") > 0.0 && take_countered(acting.uuid);
    let own_share = own_health_share(game, acting.uuid);
    let hit = rules::MeleeHit {
        target_share,
        own_share,
        countered,
        crit: chance(get("crit")),
        war_cry: war_cry_active(acting.uuid),
    };
    let dealt = amount * rules::melee_factor(&get, &hit);
    let hurt = {
        let _busy = Busy::start(&HITTING_MOB);
        game.hurt_by_player(index, dealt, source)
    };
    if hurt {
        after_melee_hit(game, &acting, &effects, id, dealt);
    }
    ControlFlow::Break(hurt)
}

fn heal(game: &mut Game<'_>, player: u128, amount: f32) {
    if amount <= 0.0 {
        return;
    }
    let max = game
        .native_attribute(player, "minecraft:max_health")
        .map_or(20.0, |(_, value)| value as f32);
    if let Some(p) = game.player.as_mut() {
        let next = (p.health.health() + amount).min(max);
        if p.set_health(next, false).is_err() {
            log::warn!("path_of_craft_tree: heal did not apply");
        }
    }
}

fn after_melee_hit(
    game: &mut Game<'_>,
    acting: &NativeServerPlayer,
    effects: &Effects,
    id: u32,
    dealt: f32,
) {
    let get = |key: &str| effect(effects, key);
    heal(game, acting.uuid, rules::lifesteal(&get, dealt));
    let alive = game.mobs.iter().any(|m| m.id == id && m.alive());
    if !alive {
        heal(game, acting.uuid, get("bloodsucker") as f32);
        start_war_cry(acting, get("war_cry"));
        return;
    }
    if chance(get("stun"))
        && let Some(mob) = game.mobs.iter_mut().find(|m| m.id == id)
    {
        mob.add_effect(
            "minecraft:slowness",
            rules::STUN_TICKS,
            rules::STUN_AMPLIFIER,
        );
    }
    let per_second = rules::bleed_per_second(&get);
    if per_second > 0.0 {
        let mut state = state();
        if state.bleeding.len() >= MAX_REMEMBERED {
            state.bleeding.clear();
        }
        state.bleeding.insert(
            id,
            Bleed {
                owner: acting.uuid,
                per_second,
                left: rules::BLEED_SECONDS as f32,
            },
        );
    }
}

/// War Cry: everyone near the killer hits mobs harder for a while, the killer too.
fn start_war_cry(acting: &NativeServerPlayer, radius: f64) {
    if radius <= 0.0 {
        return;
    }
    let until = Instant::now() + Duration::from_secs_f64(rules::WAR_CRY_SECONDS);
    let near: Vec<u128> = roam::seen_players()
        .into_iter()
        .filter(|(_, seen)| seen.dimension == acting.dimension)
        .filter(|(_, seen)| dist_sq(seen.position, acting.position) <= radius.powi(2))
        .map(|(_, seen)| seen.uuid)
        .collect();
    let mut state = state();
    state.war_cry_until.insert(acting.uuid, until);
    for ally in near {
        state.war_cry_until.insert(ally, until);
    }
}

fn dist_sq(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// A hit on a player: raises a shield at once and answers with Thorns. PvP is left alone.
pub fn on_player_hurt(
    game: &mut Game<'_>,
    amount: f32,
    from: Option<[f64; 3]>,
    damage_type: &str,
    known: Option<Option<u32>>,
) -> ControlFlow<bool> {
    if BEING_HIT.get() || damage_type == PLAYER_ATTACK {
        return ControlFlow::Continue(());
    }
    let Some(acting) = game.native_acting_player() else {
        return ControlFlow::Continue(());
    };
    if lan_pvp::pvp_hit(acting.entity_id).is_some() {
        return ControlFlow::Continue(());
    }
    let effects = player_effects(acting.uuid);
    let get = |key: &str| effect(&effects, key);
    let wide = get("shield_wide") > 0.0;
    let wall = get("shield_wall") > 0.0;
    let reflecting = get("thorns_reflect") > 0.0;
    // Shield Bash answers every blocked hit, also the weak ones the wear hook never hears of.
    let bashing = get("shield_bash") > 0.0;
    if !wide && !wall && !reflecting && !bashing {
        return ControlFlow::Continue(());
    }
    if wall {
        raise_shield_now(game);
    }
    let blocking = game.player_blocking(from, damage_type, false).is_some();
    let noted_before = noted_blocks(acting.uuid);
    let attacker = hit_attacker(game, from, damage_type, known);
    let health_before = game.player.as_ref().map_or(0.0, |p| p.health.health());
    WIDE_SHIELD.set(wide);
    let hurt = {
        let _busy = Busy::start(&BEING_HIT);
        game.hurt_player_from(amount, from, damage_type, known)
    };
    WIDE_SHIELD.set(false);
    let lost = health_before - game.player.as_ref().map_or(0.0, |p| p.health.health());
    if hurt && lost > 0.0 {
        reflect(game, &acting, attacker, rules::reflected(&get, lost));
    }
    // A blocked hit too weak to wear the shield never reaches the wear hook.
    if blocking && lost <= 0.0 && noted_blocks(acting.uuid) == noted_before {
        state()
            .pending_blocks
            .push((acting.uuid, f64::from(amount)));
    }
    ControlFlow::Break(hurt)
}

fn noted_blocks(player: u128) -> usize {
    state()
        .pending_blocks
        .iter()
        .filter(|(who, _)| *who == player)
        .count()
}

/// Shield Wall: the shield counts as raised for long enough the moment a mob swings.
fn raise_shield_now(game: &mut Game<'_>) {
    let Some((slot, ticks)) = game.using else {
        return;
    };
    let holds_shield = game
        .stack_in(slot)
        .is_some_and(|stack| &*stack.item == SHIELD);
    if holds_shield && ticks < SHIELD_RAISED_TICKS {
        game.using = Some((slot, SHIELD_RAISED_TICKS));
    }
}

/// The mob behind this hit, found the way the engine finds it.
fn hit_attacker(
    game: &Game<'_>,
    from: Option<[f64; 3]>,
    damage_type: &str,
    known: Option<Option<u32>>,
) -> Option<u32> {
    if let Some(known) = known {
        return known;
    }
    if let Some(owner) = game.combat.projectile_owner {
        return Some(owner);
    }
    if SHOT_DAMAGE.contains(&damage_type) {
        return None;
    }
    let from = from?;
    game.mobs
        .iter()
        .filter(|m| m.alive())
        .min_by(|a, b| {
            let d = |m: &horizon_client::mobs::Mob| {
                (m.position.x - from[0]).powi(2) + (m.position.z - from[2]).powi(2)
            };
            d(a).total_cmp(&d(b))
        })
        .map(|m| m.id)
}

fn reflect(game: &mut Game<'_>, acting: &NativeServerPlayer, attacker: Option<u32>, amount: f32) {
    let Some(mob) = attacker.filter(|_| amount > 0.0) else {
        return;
    };
    if !game.mobs.iter().any(|m| m.id == mob) {
        return;
    }
    game.native_hurt_entity(&acting.dimension, mob, amount, REFLECT_DAMAGE, None);
}

/// Shield Wall's upgrade: read while a mob's hit is being resolved.
pub fn widen_shield(value: Option<Cow<'_, BlocksAttacks>>) -> Option<Cow<'_, BlocksAttacks>> {
    if !WIDE_SHIELD.get() {
        return value;
    }
    let mut owned = value?.into_owned();
    for reduction in &mut owned.reductions {
        reduction.horizontal_angle += rules::SHIELD_WIDE_DEGREES;
    }
    Some(Cow::Owned(owned))
}

/// A blocked hit: pays the shield experience, primes Counter and answers with Shield Bash.
pub fn on_shield_wear(player: u128, item: &str, amount: i32) {
    if item != SHIELD || amount <= SHIELD_WEAR_BASE || pct_effects::was_pvp_hit_on(player) {
        return;
    }
    let blocked = f64::from(amount - SHIELD_WEAR_BASE);
    state().pending_blocks.push((player, blocked));
}

fn nearest_mob(game: &Game<'_>, acting: &NativeServerPlayer) -> Option<u32> {
    let [x, _, z] = acting.position;
    let distance = |mob: &Mob| (mob.position.x - x).powi(2) + (mob.position.z - z).powi(2);
    game.mobs
        .iter()
        .filter(|mob| distance(mob) <= BASH_REACH * BASH_REACH)
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .map(|mob| mob.id)
}

/// Pays for and answers the hits this player's shield took since the last tick.
pub fn finish_blocks(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let blocked: f64 = {
        let mut state = state();
        if state.pending_blocks.is_empty() {
            return;
        }
        let mine = state
            .pending_blocks
            .iter()
            .filter(|(player, _)| *player == acting.uuid)
            .map(|(_, amount)| *amount)
            .sum();
        state
            .pending_blocks
            .retain(|(player, _)| *player != acting.uuid);
        mine
    };
    if blocked <= 0.0 {
        return;
    }
    state().last_block.insert(acting.uuid, Instant::now());
    earn_block(game, acting.uuid, blocked);
    if let Some(attacker) = nearest_mob(game, acting) {
        shield_bash(game, acting, attacker);
    }
}

fn earn_block(game: &mut Game<'_>, player: u128, blocked: f64) {
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let per_point = loaded
        .tree
        .xp_for_action("act:block")
        .map_or(0.0, |(_, value)| value);
    let allowed = {
        let mut state = state();
        let budget = state
            .block_budget
            .entry(player)
            .or_insert((Instant::now(), 0.0));
        if budget.0.elapsed() >= Duration::from_secs(1) {
            *budget = (Instant::now(), 0.0);
        }
        let allowed = rules::allowed_block_damage(blocked, budget.1, per_point);
        budget.1 += allowed * per_point;
        allowed
    };
    if allowed > 0.0 {
        actions::earn_once(game, player, "act:block", allowed);
    }
}

fn shield_bash(game: &mut Game<'_>, acting: &NativeServerPlayer, attacker: u32) {
    let effects = player_effects(acting.uuid);
    let get = |key: &str| effect(&effects, key);
    if get("shield_bash") <= 0.0 {
        return;
    }
    let Some(mob) = game.mobs.iter_mut().find(|m| m.id == attacker) else {
        return;
    };
    let dx = mob.position.x - acting.position[0];
    let dz = mob.position.z - acting.position[2];
    let length = (dx * dx + dz * dz).sqrt().max(0.01);
    let push = [
        dx / length * rules::SHIELD_BASH_PUSH,
        BASH_LIFT,
        dz / length * rules::SHIELD_BASH_PUSH,
    ];
    // As Scarecrow does: a mob still walking its path at the player keeps cancelling the push.
    mob.ai.nav.stop();
    mob.ai.target = None;
    game.native_push_mob(&acting.dimension, attacker, push);
    let damage = get("shield_bash_damage") as f32;
    if damage > 0.0 {
        game.native_hurt_entity(&acting.dimension, attacker, damage, BASH_DAMAGE, None);
    }
}

/// Unyielding and the small nodes reduce a mob's push. PvP keeps the full one.
pub fn on_knockback(
    game: &mut Game<'_>,
    power: f64,
    xd: f64,
    zd: f64,
    source: HurtSource<'_>,
) -> ControlFlow<()> {
    if PUSHED.get() || source.damage_type == PLAYER_ATTACK {
        return ControlFlow::Continue(());
    }
    let Some(acting) = game.native_acting_player() else {
        return ControlFlow::Continue(());
    };
    if lan_pvp::pvp_hit(acting.entity_id).is_some() {
        return ControlFlow::Continue(());
    }
    let effects = player_effects(acting.uuid);
    let kept = rules::knockback_kept(&|key: &str| effect(&effects, key));
    if kept >= 1.0 {
        return ControlFlow::Continue(());
    }
    if kept > 0.0 {
        let _busy = Busy::start(&PUSHED);
        game.knock_back_player(power * kept, xd, zd, source);
    }
    ControlFlow::Break(())
}

fn advance_bleeds(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let due: Vec<(u32, f32)> = {
        let mut state = state();
        let mine: Vec<(u32, f32)> = state
            .bleeding
            .iter()
            .filter(|(_, bleed)| bleed.owner == acting.uuid)
            .map(|(id, bleed)| (*id, bleed.per_second * PERIOD_SECONDS))
            .collect();
        for (id, _) in &mine {
            let finished = state.bleeding.get_mut(id).is_some_and(|bleed| {
                bleed.left -= PERIOD_SECONDS;
                bleed.left <= 0.0
            });
            if finished {
                state.bleeding.remove(id);
            }
        }
        mine
    };
    for (id, amount) in due {
        game.native_hurt_entity(&acting.dimension, id, amount, BLEED_DAMAGE, None);
    }
}

fn delay_creepers(game: &mut Game<'_>, acting: &NativeServerPlayer, get: &impl Fn(&str) -> f64) {
    let (delay, cancel) = rules::creeper_fuse(get);
    if delay <= 0 && !cancel {
        return;
    }
    let reach = rules::CREEPER_REACH.powi(2);
    for mob in game
        .mobs
        .iter_mut()
        .filter(|m| m.kind.id.as_str() == CREEPER)
    {
        let at = [mob.position.x, mob.position.y, mob.position.z];
        if dist_sq(at, acting.position) > reach {
            continue;
        }
        mob.max_swell = mob.max_swell.max(30 + delay);
        if cancel {
            mob.swell = 0;
            mob.previous_swell = 0;
        }
    }
}

/// Taunt: mobs near a taunting player that are after another player turn on the taunter. Runs
/// outside any player's turn, because the host keeps which player each mob is after and a turn
/// takes the host out of the game.
pub fn provoke_all(game: &mut Game<'_>) {
    if !next_period_tick() {
        return;
    }
    let here = game.native_dimension();
    let taunters: Vec<(u128, [f64; 3], f64)> = game
        .native_server_players()
        .into_iter()
        .filter(|player| player.dimension == here)
        .filter_map(|player| {
            let radius = effect(&player_effects(player.uuid), "taunt");
            (radius > 0.0).then_some((player.uuid, player.position, radius))
        })
        .collect();
    let Some(host) = game.lan.as_mut().filter(|_| !taunters.is_empty()) else {
        return;
    };
    let taunters: Vec<(u32, [f64; 3], f64)> = taunters
        .into_iter()
        .filter_map(|(uuid, at, radius)| {
            let guest = host.hub.guests.iter().find(|guest| guest.uuid == uuid)?;
            Some((guest.conn, at, radius))
        })
        .collect();
    for mob in game
        .mobs
        .iter()
        .filter(|mob| mob.ai.target == Some(Who::Player))
    {
        let at = [mob.position.x, mob.position.y, mob.position.z];
        if let Some((conn, ..)) = taunters
            .iter()
            .find(|(_, centre, radius)| dist_sq(at, *centre) <= radius.powi(2))
        {
            host.mob_players.insert(mob.id, *conn);
        }
    }
}

fn next_period_tick() -> bool {
    static TICKS: AtomicU32 = AtomicU32::new(0);
    TICKS
        .fetch_add(1, Ordering::Relaxed)
        .is_multiple_of(PERIOD_TICKS as u32)
}

/// Attack charge runs a little faster on the server; `before_pvp_strike` takes it back for PvP.
fn hasten_attack(game: &mut Game<'_>, player: u128, attack_speed: f64) {
    if attack_speed <= 0.0 {
        return;
    }
    let mut state = state();
    let extra = state.attack_extra.entry(player).or_default();
    if f64::from(game.attack_ticks) <= PERIOD_TICKS {
        *extra = 0;
    }
    let gained = (PERIOD_TICKS * attack_speed).floor() as u32
        + u32::from(chance((PERIOD_TICKS * attack_speed).fract()));
    *extra += gained;
    game.attack_ticks = game.attack_ticks.saturating_add(gained);
}

pub fn before_pvp_strike(game: &mut Game<'_>) {
    let Some(player) = game.native_acting_player().map(|acting| acting.uuid) else {
        return;
    };
    if let Some(extra) = state().attack_extra.remove(&player) {
        game.attack_ticks = game.attack_ticks.saturating_sub(extra);
    }
}

/// The melee and defense work done twice a second for each player.
pub fn on_period(game: &mut Game<'_>, acting: &NativeServerPlayer, effects: &Effects) {
    advance_bleeds(game, acting);
    if effects.is_empty() {
        return;
    }
    let get = |key: &str| effect(effects, key);
    delay_creepers(game, acting, &get);
    hasten_attack(game, acting.uuid, get("attack_speed"));
}

// Experience from kills: spawner mobs pay a quarter, long shots a bonus.

pub fn mark_spawner_mobs(ids: &HashMap<u32, u32>) {
    let mut state = state();
    if state.spawner_mobs.len() >= MAX_REMEMBERED {
        state.spawner_mobs.clear();
    }
    state.spawner_mobs.extend(ids.values().copied());
}

/// The dying mob's own kind as the game knows it (a content mod's `soulscraft:revenant`), which a
/// kill hook or a machine's report may name by the vanilla template it is built on
/// (`minecraft:zombie`); `reported` when the mob can no longer be reached.
pub fn mob_kind(game: &Game<'_>, entity: u32, reported: &str) -> String {
    game.native_entity(entity)
        .and_then(|mob| game.native_entity_kind(mob))
        .filter(|kind| !kind.is_empty())
        .unwrap_or_else(|| reported.to_owned())
}

/// What share of a kill's experience this mob pays: a cut for mobs from spawners, times what its
/// `pct.xpNNN` tag asks for (a mob another mod made stronger). A dying mob keeps its tags until it
/// is removed, so every kill hook and a machine's report in the same tick still read them.
pub fn kill_xp_factor(game: &Game<'_>, entity: u32) -> f64 {
    let spawner = if state().spawner_mobs.contains(&entity) {
        rules::SPAWNER_XP_SHARE
    } else {
        1.0
    };
    let tagged = game
        .native_entity(entity)
        .and_then(|mob| game.native_entity_tags(mob))
        .map_or(1.0, |tags| {
            rules::tagged_xp_multiplier(tags.iter().map(String::as_str))
        });
    spawner * tagged
}

pub fn note_far_shot(mob: u32, shooter: u128) {
    let mut state = state();
    if state.far_shots.len() >= MAX_REMEMBERED {
        state.far_shots.clear();
    }
    state.far_shots.insert(mob, shooter);
}

/// A long shot that killed: pays the part of ×1.5 the shared kill reward does not.
pub fn on_kill_loot(game: &mut Game<'_>, context: &NativeLootDrops<'_>) {
    let Some((entity, kind)) = context.entity.filter(|_| !context.used) else {
        return;
    };
    let entity = entity as u32;
    let kind = &mob_kind(game, entity, kind);
    let Some(shooter) = state().far_shots.remove(&entity) else {
        return;
    };
    if context.player != Some(shooter) {
        return;
    }
    let Some(loaded) = glue::current_tree() else {
        return;
    };
    let tree = &loaded.tree;
    let health = tree
        .number("mob_health", kind)
        .unwrap_or(DEFAULT_MOB_HEALTH);
    let boss = if tree.in_list("bosses", kind) {
        tree.xp_for_action("act:boss")
            .map_or(1.0, |(_, value)| value)
    } else {
        1.0
    };
    let bonus = health * boss * rules::FAR_SHOT_BONUS * kill_xp_factor(game, entity);
    actions::earn_once(game, shooter, "act:shot", bonus);
}

// The fisher's nodes: what a roll of the fishing table yields, faster bites, lava and raw fish.
use crate::game::{
    Game,
    native_player_state::NativeEffect,
    pct_fx_gather_rules as rules, pct_glue as glue,
    pct_mining::{chance, effect, player_effects, random},
    pct_rules::{Effects, Tree},
};
use horizon_client::{
    items::ItemStack, mod_api::NativeServerPlayer, random_sequences::RandomSequences,
};
use horizon_core::random::LegacyRandom;
use horizon_world::loot::{LootData, LootStack};
use std::{
    ops::ControlFlow,
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
};

const FISHING_TABLE: &str = "minecraft:gameplay/fishing";
const MAX_REROLLS: usize = 6;
const RICH_SHARE_PER_NODE: f64 = 1.0;
const FISH_BONUS_TICKS: i32 = 200;
const LAVA_CATCH_LIFT: f64 = 0.5;
const MAX_LAVA_CATCHES: usize = 1_024;

struct Catch {
    player: u128,
    effects: Arc<Effects>,
    in_lava: bool,
}

/// Who reels in right now, set where the game has the player and read where it rolls the loot.
static CATCH: LazyLock<Mutex<Option<Catch>>> = LazyLock::new(Mutex::default);
/// A catch rolled over lava: at the bobber it would burn, so it waits for its fisher's next tick.
static LAVA_CATCHES: LazyLock<Mutex<Vec<(u128, ItemStack)>>> = LazyLock::new(Mutex::default);

fn catch() -> MutexGuard<'static, Option<Catch>> {
    CATCH.lock().unwrap_or_else(PoisonError::into_inner)
}

fn lava_catches() -> MutexGuard<'static, Vec<(u128, ItemStack)>> {
    LAVA_CATCHES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The reel-in starts: remembers who fishes and whether the hook floats in lava.
pub fn on_retrieve(game: &Game<'_>, hook_block: Option<[i32; 3]>) {
    let in_lava = hook_block.is_some_and(|pos| is_lava(game, pos));
    *catch() = game.native_acting_player().map(|acting| Catch {
        player: acting.uuid,
        effects: player_effects(acting.uuid),
        in_lava,
    });
}

pub fn is_lava(game: &Game<'_>, pos: [i32; 3]) -> bool {
    game.native_block(pos) == "minecraft:lava"
}

fn plain(item: &str, count: i32) -> LootStack {
    LootStack {
        item: item.to_owned(),
        count,
        damage: 0,
        enchantments: Vec::new(),
        potion: None,
        map: false,
        exploration: None,
        ominous_amplifier: None,
        stew_effects: Vec::new(),
        names: Vec::new(),
        components: Vec::new(),
    }
}

fn is_junk(tree: &Tree, stacks: &[LootStack]) -> bool {
    stacks
        .iter()
        .all(|stack| !tree.in_list("fish", &stack.item) && !tree.in_list("treasure", &stack.item))
}

fn duplicate_fish(tree: &Tree, stacks: &mut [LootStack]) {
    if let Some(stack) = stacks.iter_mut().find(|s| tree.in_list("fish", &s.item)) {
        stack.count += 1;
    }
}

/// The table's roll, as `fishing_loot` makes it: the table's own sequence, or the level's random.
struct Roller<'a> {
    loot: &'a LootData,
    sequences: &'a mut RandomSequences,
    level_random: &'a mut LegacyRandom,
    world_seed: i64,
    luck: f32,
    biome: Option<&'a str>,
    open_water: bool,
}

impl Roller<'_> {
    fn roll(&mut self) -> Vec<LootStack> {
        let (luck, biome, water) = (self.luck, self.biome, Some(self.open_water));
        match self.loot.random_sequence(FISHING_TABLE) {
            Some(sequence) => self.loot.roll(
                FISHING_TABLE,
                self.sequences.get(sequence, self.world_seed),
                luck,
                biome,
                water,
            ),
            None => self
                .loot
                .roll(FISHING_TABLE, &mut *self.level_random, luck, biome, water),
        }
    }
}

fn roll_clean(roller: &mut Roller<'_>, tree: &Tree, effects: &Effects) -> Vec<LootStack> {
    let mut stacks = roller.roll();
    let clean = effect(effects, "clean_water");
    for _ in 0..MAX_REROLLS {
        if !is_junk(tree, &stacks) || !chance(clean) {
            break;
        }
        stacks = roller.roll();
    }
    stacks
}

/// One catch: the nether table with the Lava Fishing upgrade, otherwise the fishing table.
fn roll_one(
    roller: &mut Roller<'_>,
    tree: &Tree,
    effects: &Effects,
    nether: bool,
) -> Vec<LootStack> {
    if nether {
        vec![plain(rules::nether_catch(random()), 1)]
    } else {
        roll_clean(roller, tree, effects)
    }
}

fn add_extras(
    roller: &mut Roller<'_>,
    tree: &Tree,
    effects: &Effects,
    nether: bool,
    stacks: &mut Vec<LootStack>,
) {
    if chance(effect(effects, "extra_fish")) {
        duplicate_fish(tree, stacks);
    }
    if chance(effect(effects, "double_catch")) {
        let second = roll_one(roller, tree, effects, nether);
        stacks.extend(second);
    }
    if chance(effect(effects, "deep_gift")) {
        let rich = chance(effect(effects, "deep_gift_rich") * RICH_SHARE_PER_NODE);
        stacks.push(plain(rules::deep_gift(random(), rich), 1));
    }
}

/// Replaces the fishing roll for a player whose nodes change it; Continue leaves it to the game.
#[allow(clippy::too_many_arguments)]
pub fn roll_catch(
    loot: &LootData,
    sequences: &mut RandomSequences,
    level_random: &mut LegacyRandom,
    world_seed: i64,
    luck: f32,
    biome: Option<&str>,
    open_water: bool,
) -> ControlFlow<Vec<LootStack>> {
    let Some(catch) = catch().take().filter(|catch| !catch.effects.is_empty()) else {
        return ControlFlow::Continue(());
    };
    let Some(loaded) = glue::current_tree() else {
        return ControlFlow::Continue(());
    };
    let effects = &catch.effects;
    let added = rules::catch_luck(&|key: &str| effect(effects, key));
    let mut roller = Roller {
        loot,
        sequences,
        level_random,
        world_seed,
        luck: luck + added,
        biome,
        open_water,
    };
    let nether = catch.in_lava && effect(effects, "lava_loot") > 0.0;
    let mut stacks = roll_one(&mut roller, &loaded.tree, effects, nether);
    add_extras(&mut roller, &loaded.tree, effects, nether, &mut stacks);
    if catch.in_lava {
        keep_from_lava(catch.player, &stacks);
        return ControlFlow::Break(Vec::new());
    }
    ControlFlow::Break(stacks)
}

fn to_item(stack: &LootStack) -> ItemStack {
    let mut item = ItemStack::new(&stack.item, stack.count.max(1) as u32);
    item.damage = stack.damage.max(0) as u32;
    for (id, level) in &stack.enchantments {
        let level = (*level).max(0) as u32;
        if stack.item == "minecraft:enchanted_book" {
            horizon_client::item_data::set_level(
                &mut item.data_mut().stored_enchantments,
                id,
                level,
            );
        } else {
            item.enchant(id, level);
        }
    }
    match &stack.potion {
        Some(potion) => item.with_potion(potion),
        None => item,
    }
}

fn keep_from_lava(player: u128, stacks: &[LootStack]) {
    let mut pending = lava_catches();
    for stack in stacks {
        if pending.len() < MAX_LAVA_CATCHES {
            pending.push((player, to_item(stack)));
        }
    }
}

/// Hands a lava catch over at the fisher's feet, where the usual pickup takes it.
pub fn deliver_lava_catch(game: &mut Game<'_>, acting: &NativeServerPlayer) {
    let mine: Vec<ItemStack> = {
        let mut pending = lava_catches();
        if pending.is_empty() {
            return;
        }
        let (mine, rest) = std::mem::take(&mut *pending)
            .into_iter()
            .partition(|(player, _)| *player == acting.uuid);
        *pending = rest;
        mine.into_iter().map(|(_, stack)| stack).collect()
    };
    let [x, y, z] = acting.position;
    for stack in mine {
        game.spawn_item(stack, [x, y + LAVA_CATCH_LIFT, z], [0.0; 3], 0);
    }
}

pub fn forget_player(player: u128) {
    lava_catches().retain(|(owner, _)| *owner != player);
}

/// Extra ticks a waiting bite loses this tick for the acting player.
pub fn lure_ticks(game: &Game<'_>) -> i32 {
    let Some(acting) = game.native_acting_player() else {
        return 0;
    };
    let effects = player_effects(acting.uuid);
    rules::lure_ticks(&|key: &str| effect(&effects, key), random())
}

/// Lava Fishing: whether this player's bobber floats on the lava at `pos`.
pub fn fishes_in_lava(game: &Game<'_>, pos: [i32; 3]) -> bool {
    is_lava(game, pos)
        && game
            .native_acting_player()
            .is_some_and(|acting| effect(&player_effects(acting.uuid), "lava_fish") > 0.0)
}

/// Harpoon: the extra pull and the damage for a hooked mob, if the node is taken.
pub fn harpoon(game: &Game<'_>) -> Option<f32> {
    let acting = game.native_acting_player()?;
    let effects = player_effects(acting.uuid);
    (effect(&effects, "harpoon") > 0.0).then(|| effect(&effects, "harpoon_damage") as f32)
}

/// Raw Is Fine: raw fish feeds like cooked fish, and the upgrade adds a short bonus.
pub fn on_eaten(game: &mut Game<'_>, stack: &ItemStack) {
    let Some((food, saturation)) = rules::cooked_gap(&stack.item) else {
        return;
    };
    let Some(player) = game.native_acting_player().map(|acting| acting.uuid) else {
        return;
    };
    let effects = player_effects(player);
    if effect(&effects, "raw_fish") <= 0.0 {
        return;
    }
    if let Some(body) = game.player.as_mut() {
        let data = &mut body.health.food;
        data.food = (data.food + food).clamp(0, 20);
        data.saturation = (data.saturation + saturation).clamp(0.0, data.food as f32);
    }
    if effect(&effects, "raw_fish_bonus") > 0.0 {
        let bonus = NativeEffect {
            id: "minecraft:water_breathing".to_owned(),
            amplifier: 0,
            duration: FISH_BONUS_TICKS,
            ambient: false,
            particles: false,
        };
        game.native_add_effect(player, &bonus);
    }
}

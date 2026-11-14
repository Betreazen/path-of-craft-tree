//! Leveling pace without the game: hours of focused play to reach each branch's max level, from
//! the experience values in data/tree.json and models of what a player does in an hour.
//! `cargo run --example pace`. Rates come from Minecraft's mechanics where the game fixes them
//! (break times, bite times, growth ticks, cooldowns); the rest are assumptions, marked as such.
//! No tree effects and no level bonus: every model plays without nodes (see docs/design.md, 14).
use path_of_craft_tree_rules::rules::Tree;

const TREE: &[u8] = include_bytes!("../mixins/path_of_craft_tree/data/tree.json");
const TARGET_HOURS: f64 = 50.0;
const TOO_FAST: f64 = 30.0;
const TOO_SLOW: f64 = 80.0;

// Minecraft mechanics (1.21 code, unchanged in 26.x as far as the mod relies on them).
const TICKS_PER_HOUR: f64 = 72_000.0;
/// Survival waits 5 ticks after a block that took more than one tick to break.
const BREAK_DELAY: f64 = 5.0;
/// A block that breaks at once is limited by aiming, not the game: 4 a second (assumption).
const INSTANT_TICKS: f64 = 5.0;
/// One random tick per block every 4096 / 3 game ticks at the default randomTickSpeed 3.
const RANDOM_TICK_MIN: f64 = 4096.0 / 3.0 / 20.0 / 60.0;
/// A fish swims in 20..80 ticks after the wait; reacting, reeling and recasting take ~40 ticks.
const SWIM_IN: f64 = 50.0;
const REEL_AND_CAST: f64 = 40.0;
/// An ore vein of config size n places about 0.75 n blocks in solid rock (approximation).
const VEIN_FILL: f64 = 0.75;
/// Branch mining 1x2 tunnels three apart shows the walls too: ores seen per block mined.
const WALLS_SEEN: f64 = 2.0;

#[derive(Clone, Copy)]
struct Tool {
    speed: f64,
    efficiency: u32,
    haste: u32,
}
const fn tool(speed: f64, efficiency: u32, haste: u32) -> Tool {
    Tool {
        speed,
        efficiency,
        haste,
    }
}
// Tier speeds: iron 6, diamond 8, netherite 9; Efficiency adds n² + 1; Haste adds 20% a level.
const IRON_EFF3: Tool = tool(6.0, 3, 0);
const DIAMOND_EFF4: Tool = tool(8.0, 4, 0);
const DIAMOND_EFF5: Tool = tool(8.0, 5, 0);
const NETHERITE_EFF5_HASTE2: Tool = tool(9.0, 5, 2);

/// Ticks per block: damage per tick is speed / hardness / 30 with the right tool.
fn break_ticks(hardness: f64, tool: Tool) -> f64 {
    let bonus = if tool.efficiency > 0 {
        f64::from(tool.efficiency.pow(2) + 1)
    } else {
        0.0
    };
    let speed = (tool.speed + bonus) * (1.0 + 0.2 * f64::from(tool.haste));
    let per_tick = speed / hardness / 30.0;
    if per_tick >= 1.0 {
        INSTANT_TICKS
    } else {
        (1.0 / per_tick).ceil() + BREAK_DELAY
    }
}

/// Blocks per hour when `share` of the hour is spent breaking them.
fn blocks(hardness: f64, tool: Tool, share: f64) -> f64 {
    TICKS_PER_HOUR / break_ticks(hardness, tool) * share
}

/// Ore features around Y=-58 (1.21 placed features): count per chunk, height density at -58 and
/// vein size. Diamond: ore_diamond, _medium, _large, _buried; redstone: ore_redstone, _lower;
/// iron: ore_iron_small; gold: ore_gold, ore_gold_lower; lapis: ore_lapis_buried.
type Feature = (f64, f64, f64);
#[rustfmt::skip]
const DEEP_ORES: [(&str, &[Feature]); 5] = [
    ("minecraft:blocks/deepslate_diamond_ore", &[(7.0, 74.0 / 6400.0, 4.0), (2.0, 1.0 / 60.0, 8.0),
        (1.0 / 9.0, 74.0 / 6400.0, 12.0), (4.0, 74.0 / 6400.0, 8.0)]),
    ("minecraft:blocks/deepslate_redstone_ore", &[(4.0, 1.0 / 79.0, 8.0), (8.0, 26.0 / 1024.0, 8.0)]),
    ("minecraft:blocks/deepslate_iron_ore", &[(10.0, 1.0 / 137.0, 4.0)]),
    ("minecraft:blocks/deepslate_gold_ore", &[(4.0, 6.0 / 2304.0, 9.0), (0.5, 1.0 / 17.0, 9.0)]),
    ("minecraft:blocks/deepslate_lapis_ore", &[(4.0, 1.0 / 129.0, 7.0)]),
];

/// Ore blocks per block of rock in that layer (a chunk layer is 256 blocks).
fn ore_density(features: &[Feature]) -> f64 {
    features
        .iter()
        .map(|(count, density, size)| count * density * VEIN_FILL * size)
        .sum::<f64>()
        / 256.0
}

/// Catches per hour: the wait is uniform 100..600 ticks minus 100 per Lure level (never below 0).
fn catches(lure: u32, focus: f64) -> f64 {
    let cut = 100.0 * f64::from(lure);
    let (low, high) = (100.0 - cut, 600.0 - cut);
    let wait = if low >= 0.0 {
        (low + high) / 2.0
    } else {
        high * high / (2.0 * (high - low))
    };
    TICKS_PER_HOUR / (wait + SWIM_IN + REEL_AND_CAST) * focus
}

/// Fish, junk and treasure shares: weights 85, 10, 5, moved by -1, -2, +2 per Luck of the Sea.
fn catch_split(luck: u32) -> [f64; 3] {
    let l = f64::from(luck);
    let w = [85.0 - l, 10.0 - 2.0 * l, 5.0 + 2.0 * l];
    let sum: f64 = w.iter().sum();
    w.map(|x| x / sum)
}

/// Minutes for a crop to ripen: each random tick moves a stage with chance 1 / (floor(25/f) + 1).
/// f = 10 on wet farmland among wet farmland; 5 when the same crop touches on both axes.
fn ripen_minutes(stages: f64, f: f64) -> f64 {
    stages * ((25.0 / f).floor() + 1.0) * RANDOM_TICK_MIN
}

/// Average health of a night's monsters from the tree's own table, weighted like the overworld
/// spawn list: zombie, skeleton, spider, creeper 100 each, enderman 10, witch 5.
fn night_mob_health(tree: &Tree) -> f64 {
    let mobs = [
        ("zombie", 100.0),
        ("skeleton", 100.0),
        ("spider", 100.0),
        ("creeper", 100.0),
        ("enderman", 10.0),
        ("witch", 5.0),
    ];
    let weight: f64 = mobs.iter().map(|m| m.1).sum();
    let health = |id: &str| {
        tree.number("mob_health", &format!("minecraft:{id}"))
            .unwrap_or(20.0)
    };
    mobs.iter().map(|(id, w)| health(id) * w).sum::<f64>() / weight
}

/// Minecraft experience points one level costs, from `level` to `level + 1`.
fn points_for_level(level: u32) -> f64 {
    f64::from(match level {
        0..=15 => 2 * level + 7,
        16..=30 => 5 * level - 38,
        _ => 9 * level - 158,
    })
}

/// `per_hour` actions, each paying `mult` times the branch's value for `key` (1 when empty);
/// `cross` pays it as related experience, at the cross share and without the general scale.
struct Src(&'static str, f64, f64, bool);
fn src(key: &'static str, mult: f64, per_hour: f64) -> Src {
    Src(key, mult, per_hour, false)
}
fn cross(mult: f64, per_hour: f64) -> Src {
    Src("", mult, per_hour, true)
}

#[derive(PartialEq)]
enum Pace {
    Focused,
    Farm,
}
struct Model {
    branch: &'static str,
    pace: Pace,
    hourly: Vec<Src>,
    /// One-time sources: (key, how many a player realistically collects).
    once: Vec<(&'static str, f64)>,
}

// A table of models reads best one source per line, so rustfmt leaves it as written.
#[rustfmt::skip]
fn models(tree: &Tree) -> Vec<Model> {
    use Pace::*;
    let m = |branch, pace, hourly, once| Model { branch, pace, hourly, once };
    let mob = night_mob_health(tree);
    let deep = |share| blocks(3.0, DIAMOND_EFF4, share);
    let mut mining = vec![src("minecraft:blocks/deepslate", 1.0, deep(0.4))];
    let mut mining_farm =
        vec![src("minecraft:blocks/deepslate", 1.0, blocks(3.0, NETHERITE_EFF5_HASTE2, 0.5))];
    for (ore, features) in DEEP_ORES {
        let seen = ore_density(features) * WALLS_SEEN;
        mining.push(src(ore, 1.0, deep(0.4) * seen));
        mining_farm.push(src(ore, 1.0, blocks(3.0, NETHERITE_EFF5_HASTE2, 0.5) * seen));
    }
    let [fish, junk, treasure] = catch_split(3);
    let lure3 = catches(3, 0.9);
    let crops = |count: f64, f: f64| count * 60.0 / ripen_minutes(7.0, f);
    // Potatoes drop 1 + Binomial(3, 4/7); one goes back into the ground.
    let potatoes_net = 3.0 * 4.0 / 7.0;
    // Slot 3 at level 30 costs 3 levels: the points from 27 to 30.
    let points_slot3: f64 = (27..30).map(points_for_level).sum();
    // Slot 1 at level 1..3 costs 1 level: about the average of 7, 9, 11 points.
    let points_slot1: f64 = (0..3).map(points_for_level).sum::<f64>() / 3.0;
    let mc_points = 3000.0; // assumption: XP orbs from a mob farm, mining and smelting per hour
    let cull_cow = tree.number("mob_health", "minecraft:cow").unwrap_or(10.0);
    vec![
        // Branch mining at Y=-58, 40% of the hour breaking (torches, walking, inventory).
        m("mining", Focused, mining, vec![]),
        m("mining", Farm, mining_farm, vec![]),
        // Iron shovel Eff III breaks sand and dirt at once; 35% of the hour sand, 15% gravel.
        m("digging", Focused, vec![
            src("minecraft:blocks/sand", 1.0, blocks(0.5, IRON_EFF3, 0.35)),
            src("minecraft:blocks/gravel", 1.0, blocks(0.6, IRON_EFF3, 0.15)),
            src("act:brush", 1.0, 6.0), // assumption: a few suspicious blocks an hour
        ], vec![]),
        // Soul soil valley, diamond Eff V (instant), 60% of the hour digging.
        m("digging", Farm, vec![src("minecraft:blocks/soul_soil", 1.0, blocks(0.5, DIAMOND_EFF5, 0.6))], vec![]),
        // A tree of 8 logs takes 8 breaks plus ~30 s of walking, pillaring and replanting.
        m("woodcutting", Focused, vec![src("minecraft:blocks/spruce_log", 1.0,
            8.0 * 3600.0 / (8.0 * break_ticks(2.0, DIAMOND_EFF4) / 20.0 + 30.0))], vec![]),
        // Bone-mealed 2x2 spruce, ~80 logs a tree, 60 s of setup per tree.
        m("woodcutting", Farm, vec![src("minecraft:blocks/spruce_log", 1.0,
            80.0 * 3600.0 / (80.0 * break_ticks(2.0, DIAMOND_EFF4) / 20.0 + 60.0))], vec![]),
        // Lure III and Luck of the Sea III, 90% focus.
        m("fishing", Focused, vec![src("act:fish", 1.0, lure3 * fish), src("act:junk", 1.0, lure3 * junk),
            src("act:treasure", 1.0, lure3 * treasure)], vec![]),
        // The same rod on an auto-clicker: no attention needed.
        m("fishing", Farm, vec![src("act:fish", 1.0, catches(3, 1.0) * fish),
            src("act:junk", 1.0, catches(3, 1.0) * junk), src("act:treasure", 1.0, catches(3, 1.0) * treasure)], vec![]),
        // 640 crops in solid blocks (f=5), 16 melon stems, 40 sugar cane columns (assumptions).
        m("farming", Focused, vec![
            src("minecraft:blocks/wheat", 1.0, crops(640.0, 5.0)),
            // A ripe stem tries a random side each random tick: 1/6 chance, 2 of 4 sides free.
            src("minecraft:blocks/melon", 1.0, 16.0 * 60.0 / (6.0 * 2.0 * RANDOM_TICK_MIN)),
            // Sugar cane grows a block per 16 random ticks.
            src("minecraft:blocks/sugar_cane", 1.0, 40.0 * 60.0 / (16.0 * RANDOM_TICK_MIN)),
            src("act:bone_meal", 1.0, 60.0),
        ], vec![]),
        // 2000 crops in alternating rows (f=10): ~0.5 s a harvest keeps the player busy 42 min.
        m("farming", Farm, vec![src("minecraft:blocks/wheat", 1.0, crops(2000.0, 10.0))], vec![]),
        // 12 cow pairs bred every 5 min, the grown calves culled; shearing, milk, honey.
        m("husbandry", Focused, vec![src("act:breed", 1.0, 12.0 * 12.0), cross(cull_cow, 12.0 * 12.0),
            src("act:shear", 1.0, 150.0), src("act:milk", 1.0, 60.0), src("act:honey", 1.0, 8.0)],
            vec![("act:tame", 15.0)]),
        // 40 pairs and 30 sheep (a sheep regrows wool ~50 s after it starts eating grass).
        m("husbandry", Farm, vec![src("act:breed", 1.0, 40.0 * 12.0), cross(cull_cow, 40.0 * 12.0),
            src("act:shear", 1.0, 900.0)], vec![]),
        // The focused farming field planted with potatoes, baked in smokers (5 s an item).
        m("cooking", Focused, vec![src("act:cook", 1.0, crops(640.0, 5.0) * potatoes_net),
            src("act:eat", 1.0, 40.0), src("act:cake", 1.0, 2.0), src("act:campfire", 1.0, 20.0)], vec![]),
        // 1000 potatoes in rows and a bank of smokers, taken out by shift-click.
        m("cooking", Farm, vec![src("act:cook", 1.0, crops(1000.0, 10.0) * potatoes_net)], vec![]),
        // One stand busy 60% of the time: a brew every 20 s pays for 3 bottles.
        m("alchemy", Focused, vec![src("act:brew", 3.0, 3600.0 * 0.6 / 20.0)], vec![("act:brew_first", 40.0)]),
        // Four stands kept 80% busy, but a player is paid at most one brew per 20 s: one full stand.
        m("alchemy", Farm, vec![src("act:brew", 3.0, 3600.0 / 20.0)], vec![]),
        // Assumption: 1500 blocks an hour on a real build, a quarter of them stairs and slabs.
        m("building", Focused, vec![src("act:build", 1.0, 1125.0), src("act:deco", 1.0, 375.0)], vec![]),
        // A broken paid block makes the next placement unpaid, so "place, break, place next to it"
        // pays once. The fastest left: cobblestone from a generator (netherite Eff V, Haste II),
        // every block put on a new spot; 35% of the hour mining, 35% placing at the same rate.
        m("building", Farm, vec![src("act:build", 1.0, blocks(2.0, NETHERITE_EFF5_HASTE2, 0.35))], vec![]),
        // Assumption: 2000 cobblestone into furnaces and 400 logs into planks and chests an hour.
        m("crafting", Focused, vec![src("act:craft", 8.0, 2000.0 / 8.0), src("act:craft", 1.0, 400.0),
            src("act:craft", 8.0, 1600.0 / 8.0)], vec![("act:craft_first", 400.0)]),
        // Cobblestone generator mined at once (netherite Eff V, Haste II) 60% of the hour.
        m("crafting", Farm, vec![src("act:craft", 8.0, blocks(1.5, NETHERITE_EFF5_HASTE2, 0.6) / 8.0)], vec![]),
        // Assumption: 150 copper and 60 iron ores an hour in caves; copper drops 2..5 raw.
        m("smithing", Focused, vec![src("act:ingot", 1.0, 150.0 * 3.5 + 60.0), src("act:gear", 1.0, 8.0),
            src("act:repair", 1.0, 4.0)], vec![("act:upgrade", 9.0)]),
        // Only metal gear pays. Assumption: a mid-size gold farm, 9000 nuggets (1000 ingots) an hour,
        // all into golden shovels (one ingot each). An iron farm's ~400 ingots give 400 shovels.
        m("smithing", Farm, vec![src("act:gear", 1.0, 1000.0)], vec![]),
        // The points spent at level 30 in slot 3; 6 book merges of ~5 levels an hour.
        m("enchanting", Focused, vec![src("act:enchant", 3.0, mc_points / points_slot3),
            src("act:book", 5.0, 6.0)], vec![("act:enchant_first", 40.0)]),
        // The same points spent one level at a time in slot 1 at levels 1..3.
        m("enchanting", Farm, vec![src("act:enchant", 1.0, mc_points / points_slot1)], vec![]),
        // Assumption: a monster every 24 s at night and in caves; bosses once.
        m("melee", Focused, vec![src("act:kill", mob, 150.0)],
            vec![("act:boss", 200.0 + 300.0 + 3.0 * 80.0)]),
        // A mob tower (natural spawns, full experience), killed by hand at the bottom.
        m("melee", Farm, vec![src("act:kill", mob, 500.0)], vec![]),
        // Assumption: 120 kills an hour, a fifth of them from 30 blocks (x1.5).
        m("archery", Focused, vec![src("act:shot", mob, 120.0 * 0.8), src("act:shot", mob * 1.5, 120.0 * 0.2)],
            vec![("act:shot", 3.0 * (200.0 + 300.0))]),
        m("archery", Farm, vec![src("act:shot", mob, 500.0)], vec![]),
        // Assumption: 150 monsters an hour, each lands 3 HP and 3 HP more hit the shield.
        m("defense", Focused, vec![src("act:hit", 3.0, 150.0), src("act:block", 3.0, 150.0)], vec![]),
        // A trapped zombie hitting a raised shield: the rules cap it at 20 experience a second.
        m("defense", Farm, vec![src("", 20.0, 3600.0)], vec![]),
        // Horse at ~9.5 m/s, 60% of chunks new; 30 trades an hour; one-time finds.
        m("exploration", Focused, vec![src("act:chunk", 1.0, 9.5 * 3600.0 / 16.0 * 0.6),
            src("act:trade", 1.0, 30.0)],
            vec![("act:biome", 64.0), ("act:structure", 34.0), ("act:advancement", 60.0), ("act:dimension", 2.0)]),
        // Elytra with rockets at ~30 m/s, 80% of chunks new.
        m("exploration", Farm, vec![src("act:chunk", 1.0, 30.0 * 3600.0 / 16.0 * 0.8)], vec![]),
    ]
}

/// The experience a branch value `key` pays, `mult` times (1 when `key` is empty).
fn value(tree: &Tree, branch: &str, key: &str, mult: f64) -> f64 {
    let base = if key.is_empty() {
        1.0
    } else {
        tree.branches[branch]
            .xp
            .get(key)
            .copied()
            .unwrap_or_else(|| panic!("{branch}: no {key}"))
    };
    base * mult
}

impl Model {
    fn per_hour(&self, tree: &Tree) -> (f64, f64) {
        let (mut main, mut related) = (0.0, 0.0);
        for Src(key, mult, per_hour, is_cross) in &self.hourly {
            let xp = value(tree, self.branch, key, *mult) * per_hour;
            if *is_cross {
                related += xp * tree.cross_share
            } else {
                main += xp
            }
        }
        (main, related)
    }
    fn once(&self, tree: &Tree) -> f64 {
        // Starts from +0, so an empty list does not print as -0.
        self.once.iter().fold(0.0, |sum, (key, count)| {
            sum + value(tree, self.branch, key, *count)
        })
    }
    fn rate(&self, tree: &Tree) -> f64 {
        let (main, related) = self.per_hour(tree);
        main + related
    }
    fn hours(&self, tree: &Tree, xp: f64) -> f64 {
        (xp - self.once(tree)).max(0.0) / self.rate(tree)
    }
}

fn xp_to_level(tree: &Tree, level: u32) -> f64 {
    (1..=level).map(|l| tree.xp_for_level(l)).sum()
}

fn flag(hours: f64) -> &'static str {
    if hours < TOO_FAST {
        "FAST"
    } else if hours > TOO_SLOW {
        "SLOW"
    } else {
        ""
    }
}

/// Mixed play: every branch gets the same share of each hour at its focused rate.
fn mixed(tree: &Tree, focused: &[&Model], xp_max: f64) {
    let max = tree.curve.max_level;
    let mut left: Vec<f64> = focused.iter().map(|m| m.once(tree)).collect();
    let mut profile = tree.new_profile();
    let share = 1.0 / focused.len() as f64;
    let checkpoints = [10, 25, 50, 100, 200, 400, 800, 1200, 1600, 2000];
    let (mut general_max_at, mut all_max_at) = (None, None);
    println!(
        "\nMixed play, equal time for all {} branches (focused rates):",
        focused.len()
    );
    println!(
        "{:>6} {:>10} {:>10} {:>8} {:>8}",
        "hours", "min lvl", "max lvl", "maxed", "general"
    );
    for hour in 1..=2000 {
        for (model, left) in focused.iter().zip(&mut left) {
            let (main, related) = model.per_hour(tree);
            // One-time finds come in evenly over the branch's own time to max.
            let once = (model.once(tree) / model.hours(tree, xp_max) * share).min(*left);
            *left -= once;
            tree.add_main_xp(&mut profile, model.branch, main * share + once);
            tree.add_cross_xp(
                &mut profile,
                model.branch,
                related / tree.cross_share * share,
            );
        }
        let levels: Vec<u32> = focused
            .iter()
            .map(|m| tree.branch_level(&profile, m.branch))
            .collect();
        let general = tree.general_level(&profile);
        if general >= max && general_max_at.is_none() {
            general_max_at = Some(hour)
        }
        if levels.iter().all(|l| *l >= max) && all_max_at.is_none() {
            all_max_at = Some(hour)
        }
        if checkpoints.contains(&hour) {
            let maxed = levels.iter().filter(|l| **l >= max).count();
            let (lo, hi) = (
                levels.iter().min().unwrap_or(&0),
                levels.iter().max().unwrap_or(&0),
            );
            println!("{hour:>6} {lo:>10} {hi:>10} {maxed:>8} {general:>8}");
        }
    }
    println!(
        "general scale reaches {max} after {general_max_at:?} h; every branch after {all_max_at:?} h"
    );
}

fn main() -> Result<(), String> {
    let tree = Tree::parse(TREE)?;
    let max = tree.curve.max_level;
    let xp_max = xp_to_level(&tree, max);
    println!(
        "XP to level {max}: {xp_max:.0}; for {TARGET_HOURS} h that is {:.0} XP/h",
        xp_max / TARGET_HOURS
    );
    println!(
        "night monster average health {:.1}; no tree effects, no level bonus\n",
        night_mob_health(&tree)
    );
    let all = models(&tree);
    let pick = |branch: &str, pace: Pace| all.iter().find(|m| m.branch == branch && m.pace == pace);
    // nodes: points the branch's nodes cost; once: one-time XP; h nodes: hours to the level
    // that pays for all of them; k50: multiply the branch's repeatable XP by k50 for 50 h.
    println!(
        "branch       nodes xp nodes    once    XP/h h max h nodes   flag  farm/h h farm   k50"
    );
    let mut focused = Vec::new();
    for id in tree.branches.keys() {
        let (Some(f), Some(farm)) = (pick(id, Pace::Focused), pick(id, Pace::Farm)) else {
            return Err(format!("no models for {id}"));
        };
        let nodes: u32 = tree
            .nodes
            .values()
            .filter(|n| n.branch.as_deref() == Some(id))
            .map(|n| u32::from(n.max_rank()))
            .sum();
        let xp_nodes = xp_to_level(&tree, nodes.min(max));
        let hours = f.hours(&tree, xp_max);
        println!(
            "{:<12} {:>5} {:>8.0} {:>7.0} {:>7.0} {:>5.0} {:>7.0} {:>6} {:>7.0} {:>6.1} {:>5.2}",
            id,
            nodes,
            xp_nodes,
            f.once(&tree),
            f.rate(&tree),
            hours,
            f.hours(&tree, xp_nodes),
            flag(hours),
            farm.rate(&tree),
            farm.hours(&tree, xp_max),
            hours / TARGET_HOURS
        );
        focused.push(f);
    }
    // Every biome (64), structure id (34), advancement (122) and the two other dimensions.
    let finite = [
        ("act:biome", 64.0),
        ("act:structure", 34.0),
        ("act:advancement", 122.0),
    ];
    let once_max = value(&tree, "exploration", "act:dimension", 2.0)
        + finite
            .iter()
            .map(|(k, n)| value(&tree, "exploration", k, *n))
            .sum::<f64>();
    println!(
        "\nexploration one-time sources at most {once_max:.0} XP: level {} of {max}",
        tree.level_progress(once_max).0
    );
    let sum: f64 = focused.iter().map(|m| m.hours(&tree, xp_max)).sum();
    let main_total: f64 = focused
        .iter()
        .map(|m| xp_max - m.per_hour(&tree).1 * m.hours(&tree, xp_max))
        .sum();
    println!(
        "every branch to {max}, one after another: {sum:.0} h; general scale by then {:.0} XP (level {})",
        main_total * tree.general_share,
        tree.level_progress(main_total * tree.general_share).0
    );
    let alone = xp_max * tree.general_share;
    println!(
        "one branch alone to {max}: general scale {alone:.0} XP (level {})",
        tree.level_progress(alone).0
    );
    mixed(&tree, &focused, xp_max);
    Ok(())
}

// Numbers and decisions of the gathering branches, free of game types so tests run without the engine.

use std::collections::{HashSet, VecDeque};

/// The Timber node's "no limit" still stops here so one tree cannot stall the server.
pub const TIMBER_TECH_LIMIT: usize = 512;
/// A tree scanned only to know whether it is finished, without the Timber node.
pub const TREE_SCAN_LIMIT: usize = 64;
pub const STRIP_LIMIT: usize = 32;
pub const LEAF_LIMIT: usize = 384;
pub const BREAKS_PER_TICK: usize = 16;
/// A block harder than the broken one by more than this stays in an area break.
pub const AREA_HARDNESS_MARGIN: f32 = 3.0;
pub const SENSE_COOLDOWN_SECONDS: f64 = 45.0;
pub const SENSE_DURATION_SECONDS: f64 = 10.0;
pub const SENSE_MAX_RIMS: usize = 256;
pub const SHOVEL_BONUS_DAMAGE: f32 = 1.5;
pub const LOOSE_BLOCKS: [&str; 3] = ["minecraft:sand", "minecraft:red_sand", "minecraft:gravel"];
const SOIL: [&str; 7] = [
    "minecraft:dirt",
    "minecraft:grass_block",
    "minecraft:coarse_dirt",
    "minecraft:podzol",
    "minecraft:mycelium",
    "minecraft:rooted_dirt",
    "minecraft:moss_block",
];
const PATHABLE: [&str; 6] = [
    "minecraft:dirt",
    "minecraft:grass_block",
    "minecraft:coarse_dirt",
    "minecraft:podzol",
    "minecraft:mycelium",
    "minecraft:rooted_dirt",
];
const WOODS: [&str; 9] = [
    "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak",
];
const NEIGHBOURS_26: usize = 26;
const NEIGHBOURS_6: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

pub fn is_axe(item: &str) -> bool {
    item.ends_with("_axe")
}

pub fn is_shovel(item: &str) -> bool {
    item.ends_with("_shovel")
}

pub fn is_pickaxe(item: &str) -> bool {
    item.ends_with("_pickaxe")
}

/// Which axis the player looks along: 0 x, 1 y, 2 z.
pub fn view_axis(yaw: f32, pitch: f32) -> usize {
    if pitch.abs() > 45.0 {
        return 1;
    }
    let radians = yaw.to_radians();
    let (x, z) = (-radians.sin(), radians.cos());
    if x.abs() > z.abs() { 0 } else { 2 }
}

/// Offsets of a square of half-width `radius` across `axis`, without its centre.
pub fn plane_offsets(axis: usize, radius: i32) -> Vec<[i32; 3]> {
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let mut offsets = Vec::new();
    for a in -radius..=radius {
        for b in -radius..=radius {
            if a == 0 && b == 0 {
                continue;
            }
            let mut offset = [0; 3];
            offset[u] = a;
            offset[v] = b;
            offsets.push(offset);
        }
    }
    offsets
}

pub fn add(pos: [i32; 3], offset: [i32; 3]) -> [i32; 3] {
    [pos[0] + offset[0], pos[1] + offset[1], pos[2] + offset[2]]
}

/// Wear after a chain's factor: the fraction is paid as a chance, as vein mining does.
pub fn scaled_wear(points: i32, factor: f64, roll: f64) -> i32 {
    let scaled = f64::from(points) * factor.max(0.0);
    let whole = scaled.floor();
    whole as i32 + i32::from(roll < scaled - whole)
}

pub fn is_tree_log(name: &str) -> bool {
    let wood = name.ends_with("_log") || name.ends_with("_stem");
    wood && !name.contains("stripped_") && name != "minecraft:mushroom_stem"
}

pub fn is_strippable(name: &str) -> bool {
    let wood = name.ends_with("_log")
        || name.ends_with("_stem")
        || name.ends_with("_wood")
        || name.ends_with("_hyphae");
    wood && !name.contains("stripped_") && name != "minecraft:mushroom_stem"
}

pub fn stripped_of(name: &str) -> Option<String> {
    let path = name.strip_prefix("minecraft:")?;
    is_strippable(name).then(|| format!("minecraft:stripped_{path}"))
}

pub fn is_leaf_block(name: &str) -> bool {
    name.ends_with("_leaves") || name.ends_with("_wart_block")
}

fn wood_of(log: &str) -> Option<&str> {
    let path = log.strip_prefix("minecraft:")?;
    path.strip_suffix("_log")
        .or_else(|| path.strip_suffix("_stem"))
}

pub fn planks_of(log: &str) -> Option<String> {
    let wood = wood_of(log)?;
    let known = WOODS.contains(&wood) || wood == "crimson" || wood == "warped";
    known.then(|| format!("minecraft:{wood}_planks"))
}

pub fn sapling_of(log: &str) -> Option<String> {
    let wood = wood_of(log)?;
    match wood {
        "mangrove" => Some("minecraft:mangrove_propagule".to_owned()),
        "crimson" => Some("minecraft:crimson_fungus".to_owned()),
        "warped" => Some("minecraft:warped_fungus".to_owned()),
        _ if WOODS.contains(&wood) => Some(format!("minecraft:{wood}_sapling")),
        _ => None,
    }
}

pub fn is_soil(block: &str) -> bool {
    SOIL.contains(&block) || block.ends_with("_nylium")
}

pub fn is_pathable(block: &str) -> bool {
    PATHABLE.contains(&block)
}

#[derive(Debug, PartialEq)]
pub struct Tree {
    /// The connected logs, nearest first, starting with the origin.
    pub logs: Vec<[i32; 3]>,
    pub has_leaves: bool,
}

fn neighbours_26(pos: [i32; 3]) -> Vec<[i32; 3]> {
    let mut around = Vec::with_capacity(NEIGHBOURS_26);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if dx != 0 || dy != 0 || dz != 0 {
                    around.push(add(pos, [dx, dy, dz]));
                }
            }
        }
    }
    around
}

/// Logs joined to `origin`, at most `limit`, and whether any of them touches natural leaves.
pub fn scan_tree(
    origin: [i32; 3],
    limit: usize,
    is_log: &impl Fn([i32; 3]) -> bool,
    is_natural_leaf: &impl Fn([i32; 3]) -> bool,
) -> Tree {
    let mut seen = HashSet::from([origin]);
    let mut frontier = VecDeque::from([origin]);
    let mut logs = Vec::new();
    let mut has_leaves = false;
    while let Some(at) = frontier.pop_front() {
        if logs.len() >= limit {
            break;
        }
        logs.push(at);
        has_leaves = has_leaves || NEIGHBOURS_6.iter().any(|o| is_natural_leaf(add(at, *o)));
        for next in neighbours_26(at) {
            if seen.insert(next) && is_log(next) {
                frontier.push_back(next);
            }
        }
    }
    Tree { logs, has_leaves }
}

/// Natural leaves spreading from the logs, at most `limit`.
pub fn scan_leaves(
    logs: &[[i32; 3]],
    limit: usize,
    is_natural_leaf: &impl Fn([i32; 3]) -> bool,
) -> Vec<[i32; 3]> {
    let mut seen: HashSet<[i32; 3]> = logs.iter().copied().collect();
    let mut frontier: VecDeque<[i32; 3]> = logs.iter().copied().collect();
    let mut leaves = Vec::new();
    while let Some(at) = frontier.pop_front() {
        for offset in NEIGHBOURS_6 {
            let next = add(at, offset);
            if leaves.len() >= limit {
                return leaves;
            }
            if seen.insert(next) && is_natural_leaf(next) {
                leaves.push(next);
                frontier.push_back(next);
            }
        }
    }
    leaves
}

/// Where saplings go: the lowest logs, one spot or a 2x2 for big trees with the upgrade.
pub fn plant_spots(logs: &[[i32; 3]], two_by_two: bool) -> Vec<[i32; 3]> {
    let Some(lowest) = logs.iter().map(|log| log[1]).min() else {
        return Vec::new();
    };
    let base: Vec<[i32; 3]> = logs
        .iter()
        .copied()
        .filter(|log| log[1] == lowest)
        .collect();
    if two_by_two && base.len() >= 4 {
        return base.into_iter().take(4).collect();
    }
    base.into_iter().take(1).collect()
}

/// How many logs one cut may take, the broken log included.
pub fn timber_limit(get: &impl Fn(&str) -> f64) -> usize {
    if get("timber_unlimited") > 0.0 {
        TIMBER_TECH_LIMIT
    } else {
        (get("timber") as usize).min(TIMBER_TECH_LIMIT)
    }
}

/// Logs a chain breaks besides the one the player broke.
pub fn logs_to_break(logs: usize, limit: usize) -> usize {
    limit.min(logs).saturating_sub(1)
}

/// True when a cut leaves no log of this tree standing.
pub fn tree_finished(logs: usize, limit: usize) -> bool {
    limit.max(1) >= logs
}

pub fn resin_period(get: &impl Fn(&str) -> f64) -> Option<u32> {
    let every = get("resin");
    (every >= 1.0).then_some(every as u32)
}

/// Luck added to a fishing roll.
pub fn catch_luck(get: &impl Fn(&str) -> f64) -> f32 {
    (get("sea_treasure") + get("sea_luck") * 10.0) as f32
}

/// Extra ticks a lure wait loses this tick; a fraction is paid as a chance.
pub fn lure_ticks(get: &impl Fn(&str) -> f64, roll: f64) -> i32 {
    let speed = get("lure").max(0.0);
    let whole = speed.floor();
    whole as i32 + i32::from(roll < speed - whole)
}

const NETHER_CATCH: [(&str, f64); 5] = [
    ("minecraft:magma_cream", 0.35),
    ("minecraft:quartz", 0.30),
    ("minecraft:gold_nugget", 0.25),
    ("minecraft:blaze_rod", 0.09),
    ("minecraft:netherite_scrap", 0.01),
];
const DEEP_PLAIN: [&str; 3] = [
    "minecraft:iron_ingot",
    "minecraft:gold_ingot",
    "minecraft:book",
];
const DEEP_RICH: [&str; 3] = [
    "minecraft:diamond",
    "minecraft:emerald",
    "minecraft:heart_of_the_sea",
];

pub fn nether_catch(roll: f64) -> &'static str {
    let mut left = roll;
    for (item, share) in NETHER_CATCH {
        if left < share {
            return item;
        }
        left -= share;
    }
    NETHER_CATCH[0].0
}

pub fn deep_gift(pick: f64, rich: bool) -> &'static str {
    let list = if rich { &DEEP_RICH } else { &DEEP_PLAIN };
    list[(pick * list.len() as f64) as usize % list.len()]
}

/// What a raw fish adds on top of its own food to match the cooked one.
pub fn cooked_gap(item: &str) -> Option<(i32, f32)> {
    match item {
        "minecraft:cod" => Some((3, 5.6)),
        "minecraft:salmon" => Some((4, 9.2)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn nodes(pairs: &[(&str, f64)]) -> impl Fn(&str) -> f64 {
        let map: HashMap<String, f64> = pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
        move |key| map.get(key).copied().unwrap_or(0.0)
    }

    #[test]
    fn view_axis_follows_the_look() {
        assert_eq!(view_axis(0.0, 80.0), 1);
        assert_eq!(view_axis(0.0, -60.0), 1);
        assert_eq!(view_axis(90.0, 10.0), 0);
        assert_eq!(view_axis(0.0, 10.0), 2);
    }

    #[test]
    fn a_three_by_three_has_eight_extra_blocks_across_the_axis() {
        let offsets = plane_offsets(2, 1);
        assert_eq!(offsets.len(), 8);
        assert!(offsets.iter().all(|o| o[2] == 0));
        assert_eq!(plane_offsets(1, 2).len(), 24);
    }

    #[test]
    fn wear_factor_charges_the_fraction_by_chance() {
        assert_eq!(scaled_wear(1, 2.0, 0.5), 2);
        assert_eq!(scaled_wear(1, 1.5, 0.4), 2);
        assert_eq!(scaled_wear(1, 1.5, 0.6), 1);
        assert_eq!(scaled_wear(1, 0.0, 0.0), 0);
    }

    #[test]
    fn logs_exclude_stripped_wood_and_mushroom_stems() {
        assert!(is_tree_log("minecraft:oak_log"));
        assert!(is_tree_log("minecraft:crimson_stem"));
        assert!(!is_tree_log("minecraft:stripped_oak_log"));
        assert!(!is_tree_log("minecraft:mushroom_stem"));
    }

    #[test]
    fn woods_map_to_their_products() {
        assert_eq!(
            planks_of("minecraft:dark_oak_log").as_deref(),
            Some("minecraft:dark_oak_planks")
        );
        assert_eq!(
            planks_of("minecraft:crimson_stem").as_deref(),
            Some("minecraft:crimson_planks")
        );
        assert_eq!(
            sapling_of("minecraft:mangrove_log").as_deref(),
            Some("minecraft:mangrove_propagule")
        );
        assert_eq!(
            sapling_of("minecraft:warped_stem").as_deref(),
            Some("minecraft:warped_fungus")
        );
        assert_eq!(
            stripped_of("minecraft:oak_log").as_deref(),
            Some("minecraft:stripped_oak_log")
        );
        assert_eq!(stripped_of("minecraft:stripped_oak_log"), None);
    }

    fn column_tree() -> (HashSet<[i32; 3]>, HashSet<[i32; 3]>) {
        let logs: HashSet<[i32; 3]> = (0..5).map(|y| [0, y, 0]).collect();
        let leaves: HashSet<[i32; 3]> = (-2..=2)
            .map(|x| [x, 5, 0])
            .chain([[0, 5, 1], [0, 5, -1]])
            .collect();
        (logs, leaves)
    }

    #[test]
    fn a_tree_scan_needs_natural_leaves() {
        let (logs, leaves) = column_tree();
        let tree = scan_tree([0, 0, 0], TIMBER_TECH_LIMIT, &|p| logs.contains(&p), &|p| {
            leaves.contains(&p)
        });
        assert!(tree.has_leaves);
        assert_eq!(tree.logs.len(), 5);
        assert_eq!(tree.logs[0], [0, 0, 0]);
        let village = scan_tree(
            [0, 0, 0],
            TIMBER_TECH_LIMIT,
            &|p| logs.contains(&p),
            &|_| false,
        );
        assert!(!village.has_leaves);
    }

    #[test]
    fn a_scan_stops_at_its_limit_and_reports_the_cut_as_unfinished() {
        let (logs, leaves) = column_tree();
        let tree = scan_tree([0, 0, 0], 3, &|p| logs.contains(&p), &|p| {
            leaves.contains(&p)
        });
        assert_eq!(tree.logs.len(), 3);
        assert_eq!(logs_to_break(32, 8), 7);
        assert_eq!(logs_to_break(3, 32), 2);
        assert!(tree_finished(3, 32));
        assert!(!tree_finished(40, 32));
        assert!(tree_finished(1, 0));
    }

    #[test]
    fn plants_go_on_the_lowest_logs() {
        let one = plant_spots(&[[0, 3, 0], [0, 1, 0], [0, 2, 0]], false);
        assert_eq!(one, vec![[0, 1, 0]]);
        let base = [[0, 1, 0], [1, 1, 0], [0, 1, 1], [1, 1, 1], [0, 2, 0]];
        assert_eq!(plant_spots(&base, false).len(), 1);
        assert_eq!(plant_spots(&base, true).len(), 4);
        assert!(plant_spots(&[], true).is_empty());
    }

    #[test]
    fn leaves_spread_until_their_limit() {
        let leaves: HashSet<[i32; 3]> = (1..=6).map(|x| [x, 0, 0]).collect();
        let found = scan_leaves(&[[0, 0, 0]], 4, &|p| leaves.contains(&p));
        assert_eq!(found.len(), 4);
    }

    #[test]
    fn timber_limit_grows_and_unlimited_stops_at_the_technical_limit() {
        assert_eq!(timber_limit(&nodes(&[("timber", 32.0)])), 32);
        assert_eq!(timber_limit(&nodes(&[("timber", 64.0)])), 64);
        assert_eq!(
            timber_limit(&nodes(&[("timber", 64.0), ("timber_unlimited", 1.0)])),
            TIMBER_TECH_LIMIT
        );
    }

    #[test]
    fn resin_period_is_absent_without_the_node() {
        assert_eq!(resin_period(&nodes(&[])), None);
        assert_eq!(resin_period(&nodes(&[("resin", 8.0)])), Some(8));
        assert_eq!(resin_period(&nodes(&[("resin", 3.0)])), Some(3));
    }

    #[test]
    fn luck_and_lure_come_from_their_nodes() {
        let get = nodes(&[("sea_treasure", 2.0), ("sea_luck", 0.1)]);
        assert_eq!(catch_luck(&get), 3.0);
        let lure = nodes(&[("lure", 1.25)]);
        assert_eq!(lure_ticks(&lure, 0.1), 2);
        assert_eq!(lure_ticks(&lure, 0.9), 1);
    }

    #[test]
    fn the_nether_catch_table_covers_every_roll() {
        assert_eq!(nether_catch(0.0), "minecraft:magma_cream");
        assert_eq!(nether_catch(0.995), "minecraft:netherite_scrap");
        assert_eq!(nether_catch(1.5), "minecraft:magma_cream");
        assert_eq!(deep_gift(0.0, false), "minecraft:iron_ingot");
        assert_eq!(deep_gift(0.99, true), "minecraft:heart_of_the_sea");
    }

    #[test]
    fn only_cod_and_salmon_cook_better() {
        assert_eq!(cooked_gap("minecraft:cod"), Some((3, 5.6)));
        assert_eq!(cooked_gap("minecraft:pufferfish"), None);
    }

    #[test]
    fn soil_and_path_blocks_are_told_apart() {
        assert!(is_soil("minecraft:crimson_nylium"));
        assert!(is_pathable("minecraft:podzol"));
        assert!(!is_pathable("minecraft:stone"));
    }
}

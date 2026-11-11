// The numbers and decisions behind the craft sector nodes, free of game types so tests can run.

pub const MAX_CRAFT_INGREDIENTS: u32 = 16;
pub const ANVIL_TOO_EXPENSIVE: i32 = 40;
pub const ANVIL_CAPPED_COST: i32 = 39;
pub const MAX_TABLE_COST: i32 = 30;
pub const RAISED_TABLE_COST: i32 = 40;
pub const SHELF_BONUS_STEP: i32 = 2;
pub const RAISED_COST_BONUS: i32 = 4;
pub const BASE_BLOCK_REACH: f64 = 4.5;
pub const REROLL_COOLDOWN_SECONDS: u64 = 300;
pub const SHARPEN_BASE_SHARE: f64 = 0.25;
pub const MAX_CHANCE: f64 = 1.0;

/// Crafter experience units for one craft: ingredients capped at the limit, raised by small nodes.
pub fn craft_xp_times(ingredients: u32, craft_xp: f64) -> f64 {
    f64::from(ingredients.min(MAX_CRAFT_INGREDIENTS)) * (1.0 + craft_xp)
}

/// Lives in the tree data as `act:craft`; kept here so the test documents the 0.5 to 8 range.
pub const CRAFT_XP_PER_INGREDIENT: f64 = 0.5;

const REDSTONE_RECIPES: [&str; 24] = [
    "piston",
    "sticky_piston",
    "observer",
    "repeater",
    "comparator",
    "hopper",
    "dropper",
    "dispenser",
    "lever",
    "redstone_torch",
    "redstone_lamp",
    "target",
    "daylight_detector",
    "note_block",
    "tripwire_hook",
    "stone_button",
    "stone_pressure_plate",
    "oak_button",
    "oak_pressure_plate",
    "light_weighted_pressure_plate",
    "heavy_weighted_pressure_plate",
    "crafter",
    "lightning_rod",
    "tnt",
];

pub fn is_redstone_recipe(recipe: &str) -> bool {
    let name = recipe.strip_prefix("minecraft:").unwrap_or(recipe);
    REDSTONE_RECIPES.contains(&name)
}

pub fn is_rail(item: &str) -> bool {
    matches!(
        item,
        "minecraft:rail"
            | "minecraft:powered_rail"
            | "minecraft:detector_rail"
            | "minecraft:activator_rail"
    )
}

pub fn is_woven(item: &str) -> bool {
    item.ends_with("_wool") || item.ends_with("_carpet")
}

pub fn is_map_recipe(recipe: &str) -> bool {
    recipe == "minecraft:map"
}

/// A smithing-table recipe that upgrades gear; armour trims are decoration and pay nothing.
pub fn is_upgrade_recipe(recipe: &str) -> bool {
    !recipe.contains("trim")
}

pub fn is_netherite_upgrade(recipe: &str) -> bool {
    is_upgrade_recipe(recipe) && recipe.contains("netherite")
}

/// New maximum durability, always from the item's own default so that repeated calls do not stack.
pub fn boosted_max_damage(default_max: u32, bonus: f64) -> i32 {
    (f64::from(default_max) * (1.0 + bonus.max(0.0))).round() as i32
}

pub fn is_gear(item: &str) -> bool {
    GEAR_SHAPES.iter().any(|(suffix, _)| item.ends_with(suffix))
}

const GEAR_SHAPES: [(&str, u32); 9] = [
    ("_sword", 2),
    ("_pickaxe", 3),
    ("_axe", 3),
    ("_shovel", 1),
    ("_hoe", 2),
    ("_helmet", 5),
    ("_chestplate", 8),
    ("_leggings", 7),
    ("_boots", 4),
];

const GEAR_MATERIALS: [(&str, &str); 6] = [
    ("minecraft:wooden_", "minecraft:stick"),
    ("minecraft:stone_", "minecraft:cobblestone"),
    ("minecraft:iron_", "minecraft:iron_ingot"),
    ("minecraft:golden_", "minecraft:gold_ingot"),
    ("minecraft:diamond_", "minecraft:diamond"),
    ("minecraft:netherite_", "minecraft:netherite_scrap"),
];

/// What dismantling one piece of gear returns: at least one when anything is returned at all.
pub fn dismantle_return(
    item: &str,
    damage: u32,
    max_damage: u32,
    share: f64,
) -> Option<(&'static str, u32)> {
    let (_, material) = GEAR_MATERIALS.iter().find(|(p, _)| item.starts_with(p))?;
    let (_, parts) = GEAR_SHAPES.iter().find(|(s, _)| item.ends_with(s))?;
    if share <= 0.0 || max_damage == 0 {
        return None;
    }
    let left = f64::from(max_damage.saturating_sub(damage)) / f64::from(max_damage);
    let amount = (f64::from(*parts) * share * left).floor() as u32;
    Some((material, amount.max(1)))
}

/// A repair's cost after the discount nodes; a cost of 0 could never be taken, so at least 1.
pub fn discounted_cost(cost: i32, cut: f64) -> i32 {
    if cost <= 0 {
        return cost;
    }
    ((f64::from(cost) * (1.0 - cut.clamp(0.0, 0.9))).round() as i32).max(1)
}

/// The enchanting table's offers as if `shelves` more bookshelves stood around it.
pub fn raised_costs(costs: [i32; 3], shelves: f64, raised: bool) -> [i32; 3] {
    let cap = if raised {
        RAISED_TABLE_COST
    } else {
        MAX_TABLE_COST
    };
    let extra = shelves as i32 + if raised { RAISED_COST_BONUS } else { 0 };
    let mut out = costs;
    for (slot, cost) in out.iter_mut().enumerate() {
        if *cost > 0 {
            let step = extra * (slot as i32 + 1) / (SHELF_BONUS_STEP + 1);
            *cost = (*cost + step).min(cap);
        }
    }
    out
}

pub fn reroll_cooldown_seconds(free: bool) -> u64 {
    if free { 0 } else { REROLL_COOLDOWN_SECONDS }
}

/// Levels a transferred enchantment keeps: one less without the upgrade, never below one.
pub fn transferred_level(level: u32, lossless: bool) -> u32 {
    if lossless {
        level
    } else {
        level.saturating_sub(1).max(1)
    }
}

/// The level cost of moving enchantments: the sum of what moves, at least one.
pub fn transfer_cost(levels: &[u32]) -> i32 {
    (levels.iter().sum::<u32>() as i32).max(1)
}

const FORBIDDEN_BASE: [&[&str]; 2] = [
    &[
        "minecraft:protection",
        "minecraft:fire_protection",
        "minecraft:blast_protection",
        "minecraft:projectile_protection",
    ],
    &[
        "minecraft:sharpness",
        "minecraft:smite",
        "minecraft:bane_of_arthropods",
    ],
];

const FORBIDDEN_MORE: [&[&str]; 4] = [
    &["minecraft:infinity", "minecraft:mending"],
    &["minecraft:fortune", "minecraft:silk_touch"],
    &["minecraft:depth_strider", "minecraft:frost_walker"],
    &[
        "minecraft:multishot",
        "minecraft:piercing",
        "minecraft:loyalty",
        "minecraft:riptide",
    ],
];

fn same_group(groups: &[&[&str]], a: &str, b: &str) -> bool {
    groups.iter().any(|g| g.contains(&a) && g.contains(&b))
}

/// Whether Forbidden Bond allows these two normally exclusive enchantments together.
pub fn pair_allowed(a: &str, b: &str, more: bool) -> bool {
    same_group(&FORBIDDEN_BASE, a, b) || (more && same_group(&FORBIDDEN_MORE, a, b))
}

/// Minecraft experience points after the bonus; the fraction is paid with that probability.
pub fn boosted_points(amount: i32, bonus: f64, roll: f64) -> i32 {
    if amount <= 0 || bonus <= 0.0 {
        return amount;
    }
    let raised = f64::from(amount) * (1.0 + bonus);
    let whole = raised.floor();
    whole as i32 + i32::from(roll < raised - whole)
}

/// One grinder-worth of durability back per material unit.
pub fn repair_points(max_damage: u32, extra_share: f64) -> u32 {
    let share = (SHARPEN_BASE_SHARE + extra_share).clamp(0.0, 1.0);
    ((f64::from(max_damage) * share).round() as u32).max(1)
}

const COLOR_FAMILIES: [&str; 7] = [
    "_wool",
    "_carpet",
    "_concrete",
    "_concrete_powder",
    "_terracotta",
    "_stained_glass",
    "_stained_glass_pane",
];

const COLORS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// The dye's colour, from `minecraft:red_dye`.
pub fn dye_color(item: &str) -> Option<&'static str> {
    let name = item.strip_prefix("minecraft:")?.strip_suffix("_dye")?;
    COLORS.iter().find(|color| **color == name).copied()
}

/// The same block in another colour, when the block comes in colours.
pub fn recolored(block: &str, color: &str) -> Option<String> {
    let name = block.strip_prefix("minecraft:")?;
    let family = COLOR_FAMILIES
        .iter()
        .filter(|family| name.ends_with(**family))
        .max_by_key(|family| family.len())?;
    let current = name.strip_suffix(family)?;
    if !COLORS.contains(&current) || current == color {
        return None;
    }
    Some(format!("minecraft:{color}{family}"))
}

fn span(a: i32, b: i32) -> (i32, i32) {
    (a.min(b), a.max(b))
}

/// The flat rectangle between two corners, flattened along their closest axis; None if too big.
pub fn plane_between(a: [i32; 3], b: [i32; 3], max_area: usize) -> Option<Vec<[i32; 3]>> {
    let lengths = [
        (a[0] - b[0]).abs(),
        (a[1] - b[1]).abs(),
        (a[2] - b[2]).abs(),
    ];
    let flat = (0..3).min_by_key(|axis| lengths[*axis]).unwrap_or(1);
    let (x0, x1) = span(a[0], if flat == 0 { a[0] } else { b[0] });
    let (y0, y1) = span(a[1], if flat == 1 { a[1] } else { b[1] });
    let (z0, z1) = span(a[2], if flat == 2 { a[2] } else { b[2] });
    let area = (x1 - x0 + 1) as usize * (y1 - y0 + 1) as usize * (z1 - z0 + 1) as usize;
    if area > max_area {
        return None;
    }
    let mut cells = Vec::with_capacity(area);
    for x in x0..=x1 {
        for y in y0..=y1 {
            for z in z0..=z1 {
                cells.push([x, y, z]);
            }
        }
    }
    Some(cells)
}

/// Whether the box between two corners fits a blueprint of `size` blocks per side.
pub fn box_fits(a: [i32; 3], b: [i32; 3], size: i32) -> bool {
    (0..3).all(|axis| (a[axis] - b[axis]).abs() < size)
}

/// The main direction a player faces: straight up or down when looking steeply, else a side.
pub fn facing(yaw: f32, pitch: f32) -> [i32; 3] {
    if pitch >= 60.0 {
        return [0, -1, 0];
    }
    if pitch <= -60.0 {
        return [0, 1, 0];
    }
    let angle = f64::from(yaw).to_radians();
    let (x, z) = (-angle.sin(), angle.cos());
    if x.abs() > z.abs() {
        [x.signum() as i32, 0, 0]
    } else {
        [0, 0, z.signum() as i32]
    }
}

/// The nine cells of a 3x3 centred on `pos`, in the plane facing `face`.
pub fn square_around(pos: [i32; 3], face: [i32; 3]) -> Vec<[i32; 3]> {
    let flat = (0..3).find(|axis| face[*axis] != 0).unwrap_or(1);
    let (first, second) = match flat {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let mut cells = Vec::with_capacity(9);
    for a in -1..=1 {
        for b in -1..=1 {
            let mut cell = pos;
            cell[first] += a;
            cell[second] += b;
            cells.push(cell);
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crafting_pays_half_a_point_per_ingredient_up_to_eight() {
        assert_eq!(craft_xp_times(1, 0.0) * CRAFT_XP_PER_INGREDIENT, 0.5);
        assert_eq!(craft_xp_times(9, 0.0) * CRAFT_XP_PER_INGREDIENT, 4.5);
        assert_eq!(craft_xp_times(40, 0.0) * CRAFT_XP_PER_INGREDIENT, 8.0);
        assert_eq!(craft_xp_times(2, 0.5), 3.0);
    }

    #[test]
    fn upgrades_exclude_trims_and_netherite_is_recognised() {
        assert!(is_upgrade_recipe("minecraft:netherite_sword_smithing"));
        assert!(!is_upgrade_recipe("minecraft:coast_armor_trim_smithing"));
        assert!(is_netherite_upgrade("minecraft:netherite_axe_smithing"));
        assert!(!is_netherite_upgrade("minecraft:coast_armor_trim_smithing"));
    }

    #[test]
    fn durability_boost_is_computed_from_the_default_each_time() {
        assert_eq!(boosted_max_damage(250, 0.2), 300);
        assert_eq!(boosted_max_damage(250, 0.2), 300);
    }

    #[test]
    fn dismantling_returns_part_of_the_material_scaled_by_wear() {
        let full = dismantle_return("minecraft:iron_pickaxe", 0, 250, 0.5);
        assert_eq!(full, Some(("minecraft:iron_ingot", 1)));
        let chest = dismantle_return("minecraft:diamond_chestplate", 0, 528, 0.75);
        assert_eq!(chest, Some(("minecraft:diamond", 6)));
        let worn = dismantle_return("minecraft:diamond_chestplate", 264, 528, 0.5);
        assert_eq!(worn, Some(("minecraft:diamond", 2)));
        assert_eq!(dismantle_return("minecraft:shield", 0, 336, 0.5), None);
    }

    #[test]
    fn discounts_never_make_an_anvil_free() {
        assert_eq!(discounted_cost(10, 0.5), 5);
        assert_eq!(discounted_cost(1, 0.9), 1);
        assert_eq!(discounted_cost(0, 0.9), 0);
    }

    #[test]
    fn extra_shelves_raise_costs_up_to_the_table_cap() {
        let raised = raised_costs([5, 12, 28], 6.0, false);
        assert_eq!(raised, [7, 16, 30]);
        assert_eq!(raised_costs([0, 0, 0], 6.0, false), [0, 0, 0]);
        assert_eq!(raised_costs([5, 12, 38], 6.0, true)[2], 40);
    }

    #[test]
    fn transfer_keeps_at_least_one_level() {
        assert_eq!(transferred_level(3, false), 2);
        assert_eq!(transferred_level(1, false), 1);
        assert_eq!(transferred_level(3, true), 3);
        assert_eq!(transfer_cost(&[3, 2]), 5);
        assert_eq!(transfer_cost(&[]), 1);
    }

    #[test]
    fn forbidden_pairs_need_the_upgrade_for_the_wider_list() {
        assert!(pair_allowed(
            "minecraft:protection",
            "minecraft:fire_protection",
            false
        ));
        assert!(!pair_allowed(
            "minecraft:infinity",
            "minecraft:mending",
            false
        ));
        assert!(pair_allowed(
            "minecraft:infinity",
            "minecraft:mending",
            true
        ));
        assert!(!pair_allowed(
            "minecraft:sharpness",
            "minecraft:mending",
            true
        ));
    }

    #[test]
    fn experience_bonus_pays_the_fraction_by_chance() {
        assert_eq!(boosted_points(10, 0.25, 0.9), 12);
        assert_eq!(boosted_points(10, 0.25, 0.1), 13);
        assert_eq!(boosted_points(10, 0.0, 0.1), 10);
    }

    #[test]
    fn dye_recolours_only_coloured_families() {
        assert_eq!(dye_color("minecraft:red_dye"), Some("red"));
        assert_eq!(dye_color("minecraft:stick"), None);
        assert_eq!(
            recolored("minecraft:white_wool", "red").as_deref(),
            Some("minecraft:red_wool")
        );
        assert_eq!(
            recolored("minecraft:blue_stained_glass_pane", "lime").as_deref(),
            Some("minecraft:lime_stained_glass_pane")
        );
        assert_eq!(recolored("minecraft:red_wool", "red"), None);
        assert_eq!(recolored("minecraft:stone", "red"), None);
    }

    #[test]
    fn plane_flattens_along_the_closest_axis_and_respects_the_limit() {
        let cells = plane_between([0, 5, 0], [2, 5, 3], 64).expect("fits");
        assert_eq!(cells.len(), 12);
        assert!(cells.iter().all(|cell| cell[1] == 5));
        let skew = plane_between([0, 0, 0], [3, 1, 3], 64).expect("fits");
        assert!(skew.iter().all(|cell| cell[1] == 0));
        assert!(plane_between([0, 0, 0], [20, 0, 20], 64).is_none());
    }

    #[test]
    fn blueprint_box_is_checked_per_side() {
        assert!(box_fits([0, 0, 0], [4, 4, 4], 5));
        assert!(!box_fits([0, 0, 0], [5, 4, 4], 5));
    }

    #[test]
    fn facing_picks_the_dominant_direction() {
        assert_eq!(facing(0.0, 0.0), [0, 0, 1]);
        assert_eq!(facing(90.0, 0.0), [-1, 0, 0]);
        assert_eq!(facing(0.0, 80.0), [0, -1, 0]);
        assert_eq!(facing(0.0, -80.0), [0, 1, 0]);
    }

    #[test]
    fn square_lies_in_the_clicked_plane() {
        let cells = square_around([5, 5, 5], [0, 1, 0]);
        assert_eq!(cells.len(), 9);
        assert!(cells.iter().all(|cell| cell[1] == 5));
        let wall = square_around([5, 5, 5], [1, 0, 0]);
        assert!(wall.iter().all(|cell| cell[0] == 5));
    }

    #[test]
    fn repair_points_grow_with_upgrades() {
        assert_eq!(repair_points(100, 0.0), 25);
        assert_eq!(repair_points(100, 0.25), 50);
    }
}

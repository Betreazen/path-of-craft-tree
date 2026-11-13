// Numbers and decisions of the Странствия sector: melee, archery, defense and exploration.
// Free of game types so the tests can run without the engine.

pub const FAR_SHOT_BLOCKS: f64 = 30.0;
pub const FAR_SHOT_BONUS: f64 = 0.5;
pub const SPAWNER_XP_SHARE: f64 = 0.25;
pub const BLOCK_XP_PER_SECOND: f64 = 20.0;
pub const COUNTER_WINDOW_SECONDS: f64 = 3.0;
pub const WAR_CRY_SECONDS: f64 = 8.0;
pub const STUN_TICKS: i32 = 30;
pub const STUN_AMPLIFIER: i32 = 6;
pub const BLEED_SECONDS: f64 = 4.0;
pub const CREEPER_REACH: f64 = 16.0;
pub const SHIELD_WIDE_DEGREES: f32 = 45.0;
pub const SHIELD_BASH_PUSH: f64 = 0.8;
pub const ALLY_GUARD_CUT: f32 = 0.15;
pub const HEAVY_PIECES_NEEDED: usize = 2;
pub const HOME_COOLDOWN_SECONDS: f64 = 3600.0;
pub const SENSE_COOLDOWN_SECONDS: f64 = 30.0;
pub const COMPASS_COOLDOWN_SECONDS: f64 = 5.0;
pub const STRUCTURE_SEARCH_RADIUS: i32 = 100;

const LOW_TARGET_SHARE: f32 = 0.3;
const LOW_OWN_SHARE: f32 = 0.35;
const CRIT_FACTOR: f32 = 1.5;
const WAR_CRY_FACTOR: f32 = 1.2;
const FAN_STEP_DEGREES: f64 = 10.0;
const MAX_CROSSBOW_CUT: f64 = 0.75;
const VANILLA_STEP_HEIGHT: f64 = 0.6;
const VANILLA_MAX_HEALTH: f64 = 20.0;
const VANILLA_SPEED: f64 = 0.1;
const LUCK_PER_KEY_UNIT: f64 = 10.0;
const HEAVY_MATERIALS: [&str; 4] = ["iron_", "chainmail_", "diamond_", "netherite_"];
const ARMOR_PIECES: [&str; 4] = ["_helmet", "_chestplate", "_leggings", "_boots"];

/// What the damage of one melee hit depends on.
pub struct MeleeHit {
    pub target_share: f32,
    pub own_share: f32,
    pub countered: bool,
    pub crit: bool,
    pub war_cry: bool,
}

/// How much a melee hit on a mob is multiplied by.
pub fn melee_factor(get: &impl Fn(&str) -> f64, hit: &MeleeHit) -> f32 {
    let mut factor = 1.0 + get("melee_damage") as f32;
    if hit.target_share < LOW_TARGET_SHARE {
        factor *= 1.0 + get("finisher") as f32;
    }
    if hit.own_share < LOW_OWN_SHARE {
        factor *= 1.0 + get("berserk") as f32;
    }
    if hit.countered {
        factor *= 1.0 + get("counter") as f32;
    }
    if hit.crit {
        factor *= CRIT_FACTOR;
    }
    if hit.war_cry {
        factor *= WAR_CRY_FACTOR;
    }
    factor
}

pub fn lifesteal(get: &impl Fn(&str) -> f64, dealt: f32) -> f32 {
    dealt * get("lifesteal") as f32
}

pub fn reflected(get: &impl Fn(&str) -> f64, received: f32) -> f32 {
    received * get("thorns_reflect") as f32
}

/// Damage a bleeding mob takes per second; 0 when the node is off.
pub fn bleed_per_second(get: &impl Fn(&str) -> f64) -> f32 {
    get("bleed") as f32
}

/// Extra fuse ticks for creepers near the player, and whether they cannot explode at all.
pub fn creeper_fuse(get: &impl Fn(&str) -> f64) -> (i32, bool) {
    (get("creeper_delay") as i32, get("creeper_cancel") > 0.0)
}

/// What is left of a knockback; PvP hits must not reach this.
pub fn knockback_kept(get: &impl Fn(&str) -> f64) -> f64 {
    (1.0 - get("unyielding") - get("knockback_resist")).clamp(0.0, 1.0)
}

/// Yaw offsets in degrees of the extra arrows of a volley of `arrows`.
pub fn fan_yaws(arrows: usize) -> Vec<f64> {
    (1..arrows)
        .map(|index| {
            let step = index.div_ceil(2) as f64 * FAN_STEP_DEGREES;
            if index % 2 == 1 { -step } else { step }
        })
        .collect()
}

/// The bow's charge after the draw speed nodes, in ticks.
pub fn draw_charge(get: &impl Fn(&str) -> f64, ticks: i32) -> i32 {
    (f64::from(ticks) * (1.0 + get("draw_speed"))).round() as i32
}

/// How much of the crossbow's charge time is left after its nodes.
pub fn crossbow_time_left(get: &impl Fn(&str) -> f64) -> f64 {
    1.0 - get("quick_crossbow").clamp(0.0, MAX_CROSSBOW_CUT)
}

pub fn scaled_ticks(ticks: u32, left: f64) -> u32 {
    ((f64::from(ticks) * left).ceil() as u32).max(1)
}

pub fn is_far_shot(distance: f64) -> bool {
    distance >= FAR_SHOT_BLOCKS
}

/// How much of `damage` may still earn block experience, given the experience already earned
/// in this second and the experience per point of damage.
pub fn allowed_block_damage(damage: f64, earned: f64, per_point: f64) -> f64 {
    if per_point <= 0.0 {
        return 0.0;
    }
    damage.min((BLOCK_XP_PER_SECOND - earned).max(0.0) / per_point)
}

/// The base values the sector keeps on a player's attributes.
pub fn attribute_targets(get: &impl Fn(&str) -> f64) -> [(&'static str, f64); 4] {
    [
        (
            "minecraft:max_health",
            VANILLA_MAX_HEALTH * (1.0 + get("max_health")),
        ),
        (
            "minecraft:step_height",
            VANILLA_STEP_HEIGHT + get("step_up"),
        ),
        (
            "minecraft:movement_speed",
            VANILLA_SPEED * (1.0 + get("move_speed")),
        ),
        ("minecraft:luck", get("loot_luck") * LUCK_PER_KEY_UNIT),
    ]
}

pub fn is_heavy_armor(item: &str) -> bool {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    HEAVY_MATERIALS.iter().any(|m| name.starts_with(m))
        && ARMOR_PIECES.iter().any(|p| name.ends_with(p))
}

/// The speed amplifier Heavy Step gives, if the player wears enough heavy armor.
pub fn heavy_step_amplifier(get: &impl Fn(&str) -> f64, heavy_pieces: usize) -> Option<u8> {
    let level = get("heavy_step");
    (level > 0.0 && heavy_pieces >= HEAVY_PIECES_NEEDED).then_some((level - 1.0) as u8)
}

pub fn home_cooldown_seconds(get: &impl Fn(&str) -> f64) -> f64 {
    HOME_COOLDOWN_SECONDS * (1.0 - get("home_cut")).clamp(0.05, 1.0)
}

const COMPASS_BIOMES: [&str; 4] = [
    "minecraft:plains",
    "minecraft:forest",
    "minecraft:desert",
    "minecraft:taiga",
];
const COMPASS_BIOMES_MORE: [&str; 6] = [
    "minecraft:jungle",
    "minecraft:savanna",
    "minecraft:badlands",
    "minecraft:swamp",
    "minecraft:snowy_plains",
    "minecraft:mushroom_fields",
];

/// The biomes the compass cycles through.
pub fn compass_biomes(get: &impl Fn(&str) -> f64) -> Vec<&'static str> {
    let mut list = COMPASS_BIOMES.to_vec();
    if get("cartographer_more") > 0.0 {
        list.extend(COMPASS_BIOMES_MORE);
    }
    list
}

const SENSE_MORE: [&str; 5] = [
    "minecraft:desert_pyramid",
    "minecraft:jungle_pyramid",
    "minecraft:igloo",
    "minecraft:shipwreck",
    "minecraft:mineshaft",
];

/// The structures Treasure Sense names beside villages.
pub fn sense_extra_structures(get: &impl Fn(&str) -> f64) -> Vec<String> {
    if get("treasure_more") > 0.0 {
        SENSE_MORE.iter().map(|id| (*id).to_owned()).collect()
    } else {
        Vec::new()
    }
}

/// Words for the direction of a target east by `dx` and south by `dz` from the player.
pub fn direction_name(dx: f64, dz: f64) -> &'static str {
    const NAMES: [&str; 8] = [
        "на севере",
        "на северо-востоке",
        "на востоке",
        "на юго-востоке",
        "на юге",
        "на юго-западе",
        "на западе",
        "на северо-западе",
    ];
    let degrees = dx.atan2(-dz).to_degrees().rem_euclid(360.0);
    NAMES[((degrees + 22.5) / 45.0) as usize % NAMES.len()]
}

/// The entity tag another mod (SoulsCraft) puts on a mob it made stronger: `pct.xp145` pays 145% of
/// the usual kill experience. Any mod may set it; the contract is in docs/combat-summary.md.
pub const XP_TAG_PREFIX: &str = "pct.xp";
/// The highest percent such a tag may ask for; larger or malformed ones are ignored.
pub const XP_TAG_MAX_PERCENT: u32 = 500;

/// The kill experience multiplier the mob's tags ask for: the largest valid `pct.xpNNN`, else 1.
pub fn tagged_xp_multiplier<'a>(tags: impl IntoIterator<Item = &'a str>) -> f64 {
    tags.into_iter()
        .filter_map(|tag| tag.strip_prefix(XP_TAG_PREFIX)?.parse::<u32>().ok())
        .filter(|percent| (1..=XP_TAG_MAX_PERCENT).contains(percent))
        .max()
        .map_or(1.0, |percent| f64::from(percent) / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn a_pct_xp_tag_scales_kill_experience() {
        assert_eq!(tagged_xp_multiplier([]), 1.0);
        assert_eq!(tagged_xp_multiplier(["soulscraft.revenant"]), 1.0);
        assert_eq!(tagged_xp_multiplier(["pct.xp145"]), 1.45);
        assert_eq!(tagged_xp_multiplier(["pct.xp80"]), 0.8);
        assert_eq!(tagged_xp_multiplier(["pct.xp120", "pct.xp145"]), 1.45);
        assert_eq!(tagged_xp_multiplier(["pct.xp500"]), 5.0);
    }

    #[test]
    fn malformed_or_too_large_xp_tags_are_ignored() {
        for tag in [
            "pct.xp0",
            "pct.xp501",
            "pct.xp-5",
            "pct.xp1.5",
            "pct.xp",
            "pct.xpabc",
        ] {
            assert_eq!(tagged_xp_multiplier([tag]), 1.0, "{tag}");
        }
        assert_eq!(tagged_xp_multiplier(["pct.xp9999", "pct.xp130"]), 1.3);
    }

    fn nodes(pairs: &[(&str, f64)]) -> impl Fn(&str) -> f64 {
        let map: HashMap<String, f64> = pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
        move |key| map.get(key).copied().unwrap_or(0.0)
    }

    fn plain_hit() -> MeleeHit {
        MeleeHit {
            target_share: 1.0,
            own_share: 1.0,
            countered: false,
            crit: false,
            war_cry: false,
        }
    }

    #[test]
    fn finisher_only_works_on_wounded_targets() {
        let get = nodes(&[("finisher", 0.5)]);
        assert_eq!(melee_factor(&get, &plain_hit()), 1.0);
        let wounded = MeleeHit {
            target_share: 0.2,
            ..plain_hit()
        };
        assert_eq!(melee_factor(&get, &wounded), 1.5);
    }

    #[test]
    fn berserk_needs_low_health_and_stacks_with_melee_damage() {
        let get = nodes(&[("berserk", 1.0), ("melee_damage", 0.1)]);
        let low = MeleeHit {
            own_share: 0.2,
            ..plain_hit()
        };
        assert_eq!(melee_factor(&get, &low), 2.2);
    }

    #[test]
    fn counter_applies_only_right_after_a_block() {
        let get = nodes(&[("counter", 0.5)]);
        let countered = MeleeHit {
            countered: true,
            ..plain_hit()
        };
        assert_eq!(melee_factor(&get, &countered), 1.5);
        assert_eq!(melee_factor(&get, &plain_hit()), 1.0);
    }

    #[test]
    fn crit_and_war_cry_multiply_the_hit() {
        let hit = MeleeHit {
            crit: true,
            war_cry: true,
            ..plain_hit()
        };
        assert!((melee_factor(&nodes(&[]), &hit) - 1.8).abs() < 1e-5);
    }

    #[test]
    fn lifesteal_and_reflection_scale_with_the_damage() {
        let get = nodes(&[("lifesteal", 0.05), ("thorns_reflect", 0.3)]);
        assert!((lifesteal(&get, 10.0) - 0.5).abs() < 1e-5);
        assert!((reflected(&get, 10.0) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn last_creeper_upgrade_cancels_the_explosion() {
        assert_eq!(
            creeper_fuse(&nodes(&[("creeper_delay", 40.0)])),
            (40, false)
        );
        assert_eq!(
            creeper_fuse(&nodes(&[("creeper_delay", 40.0), ("creeper_cancel", 1.0)])),
            (40, true)
        );
    }

    #[test]
    fn knockback_never_goes_negative() {
        assert_eq!(knockback_kept(&nodes(&[("unyielding", 0.5)])), 0.5);
        let full = nodes(&[("unyielding", 1.0), ("knockback_resist", 0.1)]);
        assert_eq!(knockback_kept(&full), 0.0);
    }

    #[test]
    fn volleys_fan_out_on_both_sides() {
        assert_eq!(fan_yaws(1), Vec::<f64>::new());
        assert_eq!(fan_yaws(3), vec![-10.0, 10.0]);
        assert_eq!(fan_yaws(5), vec![-10.0, 10.0, -20.0, 20.0]);
    }

    #[test]
    fn draw_speed_shortens_the_full_draw() {
        let get = nodes(&[("draw_speed", 0.5)]);
        assert_eq!(draw_charge(&get, 10), 15);
        assert_eq!(draw_charge(&nodes(&[]), 10), 10);
    }

    #[test]
    fn crossbow_never_loses_all_of_its_charge_time() {
        let get = nodes(&[("quick_crossbow", 2.0)]);
        assert_eq!(crossbow_time_left(&get), 0.25);
        assert_eq!(scaled_ticks(25, 0.25), 7);
        assert_eq!(scaled_ticks(1, 0.0), 1);
    }

    #[test]
    fn only_long_shots_count_as_far() {
        assert!(is_far_shot(30.0));
        assert!(!is_far_shot(29.9));
    }

    #[test]
    fn block_experience_is_capped_each_second() {
        assert_eq!(allowed_block_damage(2.0, 0.0, 6.0), 2.0);
        assert!((allowed_block_damage(10.0, 8.0, 6.0) - 2.0).abs() < 1e-9);
        assert_eq!(allowed_block_damage(1.0, 25.0, 6.0), 0.0);
        assert_eq!(allowed_block_damage(1.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn attributes_start_at_vanilla_values() {
        let targets = attribute_targets(&nodes(&[]));
        assert_eq!(targets[0], ("minecraft:max_health", 20.0));
        assert_eq!(targets[1], ("minecraft:step_height", 0.6));
        assert_eq!(targets[2], ("minecraft:movement_speed", 0.1));
        assert_eq!(targets[3], ("minecraft:luck", 0.0));
    }

    #[test]
    fn step_up_reaches_two_blocks_with_its_upgrade() {
        let targets = attribute_targets(&nodes(&[("step_up", 1.4)]));
        assert!((targets[1].1 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn heavy_step_needs_two_heavy_pieces() {
        assert!(is_heavy_armor("minecraft:netherite_boots"));
        assert!(!is_heavy_armor("minecraft:leather_boots"));
        assert!(!is_heavy_armor("minecraft:iron_sword"));
        let get = nodes(&[("heavy_step", 2.0)]);
        assert_eq!(heavy_step_amplifier(&get, 1), None);
        assert_eq!(heavy_step_amplifier(&get, 2), Some(1));
        assert_eq!(heavy_step_amplifier(&nodes(&[]), 4), None);
    }

    #[test]
    fn home_upgrade_halves_the_wait() {
        assert_eq!(home_cooldown_seconds(&nodes(&[])), 3600.0);
        assert_eq!(home_cooldown_seconds(&nodes(&[("home_cut", 0.5)])), 1800.0);
    }

    #[test]
    fn upgrades_extend_the_search_lists() {
        assert_eq!(compass_biomes(&nodes(&[])).len(), 4);
        assert_eq!(
            compass_biomes(&nodes(&[("cartographer_more", 1.0)])).len(),
            10
        );
        assert!(sense_extra_structures(&nodes(&[])).is_empty());
        assert_eq!(
            sense_extra_structures(&nodes(&[("treasure_more", 1.0)])).len(),
            5
        );
    }

    #[test]
    fn directions_follow_the_compass() {
        assert_eq!(direction_name(0.0, -10.0), "на севере");
        assert_eq!(direction_name(10.0, 0.0), "на востоке");
        assert_eq!(direction_name(0.0, 10.0), "на юге");
        assert_eq!(direction_name(-10.0, 0.0), "на западе");
        assert_eq!(direction_name(10.0, -10.0), "на северо-востоке");
    }
}

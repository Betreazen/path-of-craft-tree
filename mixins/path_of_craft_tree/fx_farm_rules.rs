// Numbers and decisions for the farming, husbandry, cooking and alchemy nodes.
// Free of game types so the tests can run without the engine.

pub const GROWTH_PERIOD_TICKS: u32 = 10;
pub const GROWTH_BASE_SAMPLES: usize = 2;
const GROWTH_BASE_RADIUS: i32 = 4;
pub const GROWTH_HEIGHT: i32 = 1;
pub const AREA_PERIOD_TICKS: u32 = 20;
pub const STALK_LIMIT: i32 = 32;
pub const SOUP_PERIOD_TICKS: u32 = 20;
pub const EFFECT_TRIM_PERIOD_TICKS: u32 = 20;
pub const WOLF_STATUS_TICKS: i32 = 60;
pub const HORSE_STATUS_TICKS: i32 = 40;
pub const TICKS_PER_SECOND: i32 = 20;
pub const MAX_STATIONS: usize = 4_096;
pub const MAX_PENDING: usize = 512;
pub const BAD_EFFECTS: [&str; 8] = [
    "minecraft:poison",
    "minecraft:hunger",
    "minecraft:weakness",
    "minecraft:slowness",
    "minecraft:mining_fatigue",
    "minecraft:nausea",
    "minecraft:blindness",
    "minecraft:wither",
];

const REAP_CROPS: [(&str, &str); 5] = [
    ("minecraft:wheat", "minecraft:wheat_seeds"),
    ("minecraft:carrots", "minecraft:carrot"),
    ("minecraft:potatoes", "minecraft:potato"),
    ("minecraft:beetroots", "minecraft:beetroot_seeds"),
    ("minecraft:nether_wart", "minecraft:nether_wart"),
];
const SEED_ITEMS: [&str; 8] = [
    "minecraft:wheat_seeds",
    "minecraft:carrot",
    "minecraft:potato",
    "minecraft:beetroot_seeds",
    "minecraft:nether_wart",
    "minecraft:melon_seeds",
    "minecraft:pumpkin_seeds",
    "minecraft:torchflower_seeds",
];
const STEMS: [&str; 2] = ["minecraft:melon_stem", "minecraft:pumpkin_stem"];
const STEM_RIPE_AGE: i32 = 7;
const TILLABLE: [&str; 3] = [
    "minecraft:dirt",
    "minecraft:grass_block",
    "minecraft:dirt_path",
];
const STALKS: [&str; 2] = ["minecraft:sugar_cane", "minecraft:bamboo"];
const FARM_FOODS: [&str; 10] = [
    "minecraft:wheat",
    "minecraft:bread",
    "minecraft:carrot",
    "minecraft:golden_carrot",
    "minecraft:potato",
    "minecraft:baked_potato",
    "minecraft:beetroot",
    "minecraft:beetroot_soup",
    "minecraft:melon_slice",
    "minecraft:sweet_berries",
];
const SOUPS: [&str; 4] = [
    "minecraft:mushroom_stew",
    "minecraft:rabbit_stew",
    "minecraft:beetroot_soup",
    "minecraft:suspicious_stew",
];
const SPOILED_FOODS: [&str; 3] = [
    "minecraft:rotten_flesh",
    "minecraft:chicken",
    "minecraft:poisonous_potato",
];
const POISON_FOODS: [&str; 2] = ["minecraft:spider_eye", "minecraft:pufferfish"];
pub const FURNACE_MENUS: [&str; 3] = [
    "minecraft:furnace",
    "minecraft:smoker",
    "minecraft:blast_furnace",
];
pub const BREWING_MENU: &str = "minecraft:brewing_stand";
const STRONG_POTIONS: [&str; 9] = [
    "leaping",
    "swiftness",
    "slowness",
    "poison",
    "regeneration",
    "strength",
    "harming",
    "healing",
    "turtle_master",
];
const PARROT_FOOD: [&str; 6] = [
    "minecraft:wheat_seeds",
    "minecraft:melon_seeds",
    "minecraft:pumpkin_seeds",
    "minecraft:beetroot_seeds",
    "minecraft:torchflower_seeds",
    "minecraft:pitcher_pod",
];
const CAT_FOOD: [&str; 2] = ["minecraft:cod", "minecraft:salmon"];
const SECRET_DISHES: [(&str, &str, u8, i32); 3] = [
    ("minecraft:rabbit_stew", "minecraft:jump_boost", 1, 40),
    ("minecraft:pumpkin_pie", "minecraft:resistance", 0, 30),
    ("minecraft:cookie", "minecraft:haste", 0, 30),
];
const SECRET_DISHES_MORE: [(&str, &str, u8, i32); 3] = [
    ("minecraft:mushroom_stew", "minecraft:night_vision", 0, 60),
    ("minecraft:baked_potato", "minecraft:absorption", 0, 40),
    ("minecraft:beetroot_soup", "minecraft:strength", 0, 30),
];

/// The item planted back when `block` is reaped, for crops that can be replanted.
pub fn seed_of(block: &str) -> Option<&'static str> {
    REAP_CROPS
        .iter()
        .find(|(crop, _)| *crop == block)
        .map(|(_, seed)| *seed)
}

pub fn is_seed(item: &str) -> bool {
    SEED_ITEMS.contains(&item)
}

pub fn is_tillable(block: &str) -> bool {
    TILLABLE.contains(&block)
}

pub fn is_stalk(block: &str) -> bool {
    STALKS.contains(&block)
}

pub fn is_stem(block: &str) -> bool {
    STEMS.contains(&block)
}

pub fn stem_ripe_age() -> i32 {
    STEM_RIPE_AGE
}

pub fn is_farm_food(item: &str) -> bool {
    FARM_FOODS.contains(&item)
}

pub fn is_soup(item: &str) -> bool {
    SOUPS.contains(&item)
}

pub fn is_spoiled(item: &str) -> bool {
    SPOILED_FOODS.contains(&item)
}

pub fn is_poison_food(item: &str) -> bool {
    POISON_FOODS.contains(&item)
}

pub fn is_bad_effect(id: &str) -> bool {
    BAD_EFFECTS.contains(&id)
}

/// Square offsets around a centre, the centre itself included.
pub fn area_offsets(radius: i32) -> Vec<[i32; 2]> {
    let radius = radius.max(0);
    (-radius..=radius)
        .flat_map(|x| (-radius..=radius).map(move |z| [x, z]))
        .collect()
}

/// Random growth checks per period, grown with the area so that the speed stays the same.
pub fn growth_samples(radius: i32) -> usize {
    let side = |r: i32| (2 * r + 1) * (2 * r + 1);
    let scaled = GROWTH_BASE_SAMPLES as i32 * side(radius) / side(GROWTH_BASE_RADIUS);
    scaled.max(GROWTH_BASE_SAMPLES as i32) as usize
}

/// The food that may be turned into taming with this animal, per the vanilla foods.
pub fn tames_with(kind: &str, item: &str) -> bool {
    match kind {
        "minecraft:wolf" => item == "minecraft:bone",
        "minecraft:cat" => CAT_FOOD.contains(&item),
        "minecraft:parrot" => PARROT_FOOD.contains(&item),
        _ => false,
    }
}

/// Chance that a taming attempt succeeds outright, before the game's own roll.
pub fn tame_chance(first: f64, small: f64, always: f64) -> f64 {
    if always > 0.0 {
        return 1.0;
    }
    (first + small).clamp(0.0, 1.0)
}

/// Spreads fractional speed bonuses over ticks: the whole extra ticks and what carries over.
pub fn extra_ticks(rate: f64, carry: f64) -> (i32, f64) {
    let total = carry + rate.max(0.0);
    let whole = total.floor();
    (whole as i32, total - whole)
}

/// A timer moved `extra` ticks towards its end, always leaving the final tick to the game.
pub fn advanced(timer: i32, extra: i32, total: i32) -> i32 {
    (timer + extra).min(total - 1).max(timer)
}

/// A brewing countdown cut by `extra`, leaving the final tick to the game.
pub fn brew_time_after(time: i32, extra: i32) -> i32 {
    (time - extra).max(1).min(time)
}

/// The stronger form of a potion, or none when it has no such form.
pub fn stronger_potion(potion: &str) -> Option<String> {
    let name = potion.strip_prefix("minecraft:")?;
    let base = name.strip_prefix("long_").unwrap_or(name);
    if base.starts_with("strong_") || !STRONG_POTIONS.contains(&base) {
        return None;
    }
    Some(format!("minecraft:strong_{base}"))
}

/// The distinct potion kinds, in slot order.
pub fn distinct_potions(potions: &[String]) -> Vec<String> {
    let mut kinds: Vec<String> = Vec::new();
    for potion in potions {
        if !kinds.contains(potion) {
            kinds.push(potion.clone());
        }
    }
    kinds
}

/// The other potion kinds whose effects a bottle of `own` also takes, at most `limit - 1`.
pub fn mix_partners(own: &str, kinds: &[String], limit: usize) -> Vec<String> {
    kinds
        .iter()
        .filter(|kind| kind.as_str() != own)
        .take(limit.saturating_sub(1))
        .cloned()
        .collect()
}

/// Ticks a harmful effect loses per trim: the resist share of a second, at least the weakening.
pub fn trim_ticks(id: &str, resist: f64, weakens: bool) -> i32 {
    let weak = weakens && (id == "minecraft:poison" || id == "minecraft:hunger");
    let base = if weak {
        f64::from(TICKS_PER_SECOND)
    } else {
        0.0
    };
    (base + f64::from(TICKS_PER_SECOND) * resist.max(0.0)) as i32
}

/// The dish effect of a food, when a secret recipe covers it.
pub fn secret_dish(item: &str, more: bool) -> Option<(&'static str, u8, i32)> {
    let known = |list: &[(&str, &'static str, u8, i32)]| {
        list.iter()
            .find(|(food, ..)| *food == item)
            .map(|(_, id, amp, secs)| (*id, *amp, *secs))
    };
    known(&SECRET_DISHES).or_else(|| {
        if more {
            known(&SECRET_DISHES_MORE)
        } else {
            None
        }
    })
}

pub fn eat_duration(duration: i32, cut: f64) -> i32 {
    let left = (1.0 - cut).clamp(0.1, 1.0);
    ((f64::from(duration) * left).ceil() as i32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_replant_with_their_own_seed() {
        assert_eq!(seed_of("minecraft:carrots"), Some("minecraft:carrot"));
        assert_eq!(
            seed_of("minecraft:beetroots"),
            Some("minecraft:beetroot_seeds")
        );
        assert_eq!(seed_of("minecraft:melon"), None);
    }

    #[test]
    fn area_is_a_full_square_with_the_centre() {
        assert_eq!(area_offsets(0), vec![[0, 0]]);
        assert_eq!(area_offsets(1).len(), 9);
        assert_eq!(area_offsets(2).len(), 25);
        assert!(area_offsets(2).contains(&[-2, 2]));
    }

    #[test]
    fn growth_keeps_its_speed_when_the_radius_grows() {
        assert_eq!(growth_samples(4), 2);
        assert_eq!(growth_samples(6), 4);
        assert_eq!(growth_samples(1), 2);
    }

    #[test]
    fn taming_foods_match_the_animal() {
        assert!(tames_with("minecraft:wolf", "minecraft:bone"));
        assert!(!tames_with("minecraft:wolf", "minecraft:cod"));
        assert!(tames_with("minecraft:cat", "minecraft:salmon"));
        assert!(tames_with("minecraft:parrot", "minecraft:wheat_seeds"));
        assert!(!tames_with("minecraft:cow", "minecraft:wheat"));
    }

    #[test]
    fn tame_chance_sums_and_always_wins() {
        assert_eq!(tame_chance(0.25, 0.04, 0.0), 0.29);
        assert_eq!(tame_chance(0.9, 0.5, 0.0), 1.0);
        assert_eq!(tame_chance(0.0, 0.0, 1.0), 1.0);
    }

    #[test]
    fn fractional_speed_carries_over() {
        let (first, carry) = extra_ticks(0.5, 0.0);
        assert_eq!(first, 0);
        let (second, left) = extra_ticks(0.5, carry);
        assert_eq!(second, 1);
        assert_eq!(left, 0.0);
        assert_eq!(extra_ticks(1.5, 0.25), (1, 0.75));
    }

    #[test]
    fn a_timer_never_finishes_by_itself() {
        assert_eq!(advanced(10, 5, 200), 15);
        assert_eq!(advanced(198, 5, 200), 199);
        assert_eq!(advanced(199, 5, 200), 199);
    }

    #[test]
    fn a_brew_leaves_its_last_tick_to_the_game() {
        assert_eq!(brew_time_after(100, 2), 98);
        assert_eq!(brew_time_after(2, 5), 1);
        assert_eq!(brew_time_after(1, 5), 1);
    }

    #[test]
    fn potions_get_stronger_where_a_stronger_form_exists() {
        assert_eq!(
            stronger_potion("minecraft:swiftness").as_deref(),
            Some("minecraft:strong_swiftness")
        );
        assert_eq!(
            stronger_potion("minecraft:long_poison").as_deref(),
            Some("minecraft:strong_poison")
        );
        assert_eq!(stronger_potion("minecraft:strong_healing"), None);
        assert_eq!(stronger_potion("minecraft:awkward"), None);
        assert_eq!(stronger_potion("minecraft:night_vision"), None);
    }

    #[test]
    fn mixing_needs_a_different_kind_and_respects_the_limit() {
        let kinds = vec![
            "minecraft:a".to_owned(),
            "minecraft:b".to_owned(),
            "minecraft:c".to_owned(),
        ];
        assert_eq!(mix_partners("minecraft:a", &kinds, 2), vec!["minecraft:b"]);
        assert_eq!(
            mix_partners("minecraft:b", &kinds, 3),
            vec!["minecraft:a", "minecraft:c"]
        );
        assert!(mix_partners("minecraft:a", &kinds[..1], 3).is_empty());
    }

    #[test]
    fn distinct_kinds_keep_slot_order() {
        let potions = ["b", "a", "b"].map(str::to_owned);
        assert_eq!(distinct_potions(&potions), vec!["b", "a"]);
    }

    #[test]
    fn only_poison_and_hunger_are_weakened() {
        assert_eq!(trim_ticks("minecraft:poison", 0.0, true), 20);
        assert_eq!(trim_ticks("minecraft:hunger", 0.5, true), 30);
        assert_eq!(trim_ticks("minecraft:slowness", 0.5, true), 10);
        assert_eq!(trim_ticks("minecraft:poison", 0.0, false), 0);
    }

    #[test]
    fn secret_dishes_grow_with_the_upgrade() {
        assert!(secret_dish("minecraft:cookie", false).is_some());
        assert!(secret_dish("minecraft:mushroom_stew", false).is_none());
        assert!(secret_dish("minecraft:mushroom_stew", true).is_some());
        assert!(secret_dish("minecraft:bread", true).is_none());
    }

    #[test]
    fn eating_never_takes_less_than_a_tick() {
        assert_eq!(eat_duration(32, 0.5), 16);
        assert_eq!(eat_duration(32, 5.0), 4);
        assert_eq!(eat_duration(1, 0.5), 1);
    }
}

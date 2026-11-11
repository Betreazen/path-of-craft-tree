// Which status effects a player should have and how much damage their nodes remove.
// Free of game types so the tests can run without the engine.

const MOLE_BASE_Y: f64 = 0.0;
const UNDERGROUND_Y: f64 = 40.0;
const LOW_HEALTH: f32 = 6.0;
const STATUS_TICKS: i32 = 60;
// Below this the game makes night vision flicker.
const NIGHT_VISION_TICKS: i32 = 300;
const SECOND_WIND_SECONDS: f64 = 600.0;

const FIRE_DAMAGE: [&str; 5] = [
    "minecraft:in_fire",
    "minecraft:on_fire",
    "minecraft:campfire",
    "minecraft:hot_floor",
    "minecraft:lava",
];
const FALL_DAMAGE: &str = "minecraft:fall";
const LAVA_DAMAGE: &str = "minecraft:lava";

pub struct Surroundings {
    pub y: f64,
    pub head_in_water: bool,
    pub in_forest: bool,
    pub in_nether_forest: bool,
    pub night: bool,
    pub idle: bool,
}

#[derive(Debug, PartialEq)]
pub struct Status {
    pub id: &'static str,
    pub amplifier: u8,
    pub ticks: i32,
}

fn status(id: &'static str, amplifier: f64) -> Status {
    Status {
        id,
        amplifier: amplifier as u8,
        ticks: STATUS_TICKS,
    }
}

fn night_vision() -> Status {
    Status {
        id: "minecraft:night_vision",
        amplifier: 0,
        ticks: NIGHT_VISION_TICKS,
    }
}

fn wants_mole_sight(get: &impl Fn(&str) -> f64, at: &Surroundings) -> bool {
    get("mole") > 0.0
        && at.y < MOLE_BASE_Y + get("mole_depth")
        && (!at.head_in_water || get("mole_water") > 0.0)
}

fn wants_night_sight(get: &impl Fn(&str) -> f64, at: &Surroundings) -> bool {
    get("night_walker_always") > 0.0 || (get("night_walker") > 0.0 && at.night)
}

/// The statuses that should be running now, refreshed about once a second.
pub fn statuses(get: &impl Fn(&str) -> f64, at: &Surroundings) -> Vec<Status> {
    let mut wanted = Vec::new();
    if wants_mole_sight(get, at) || wants_night_sight(get, at) {
        wanted.push(night_vision());
    }
    let in_woods = at.in_forest || (at.in_nether_forest && get("forest_nether") > 0.0);
    if get("forest") > 0.0 && in_woods {
        wanted.push(status("minecraft:speed", get("forest_amp")));
        wanted.push(status("minecraft:regeneration", 0.0));
    }
    if get("sea_step") > 0.0 && at.head_in_water {
        wanted.push(status("minecraft:dolphins_grace", 0.0));
    }
    if get("sea_breath") > 0.0 && at.head_in_water {
        wanted.push(status("minecraft:water_breathing", 0.0));
    }
    if get("swift") > 0.0 {
        wanted.push(status("minecraft:speed", get("swift") - 1.0));
    }
    if get("idle_regen") > 0.0 && at.idle {
        wanted.push(status("minecraft:regeneration", get("idle_regen") - 1.0));
    }
    wanted
}

fn kept(cut: f64) -> f32 {
    (1.0 - cut).clamp(0.0, 1.0) as f32
}

/// What is left of a hit before armor. PvP hits must not reach this.
pub fn after_cuts(
    get: &impl Fn(&str) -> f64,
    damage_type: &str,
    from_mob: bool,
    y: f64,
    health: f32,
    amount: f32,
) -> f32 {
    let mut left = amount;
    if damage_type == FALL_DAMAGE {
        left *= kept(get("fall_cut"));
    }
    if FIRE_DAMAGE.contains(&damage_type) {
        left *= kept(get("fire_cut"));
    }
    if y < UNDERGROUND_Y && (damage_type == FALL_DAMAGE || damage_type == LAVA_DAMAGE) {
        left *= kept(get("under_cut"));
    }
    if from_mob {
        left *= kept(get("armor"));
        if health < LOW_HEALTH {
            left *= kept(get("stone_skin"));
        }
    }
    left
}

/// Seconds until Second Wind can save the player again.
pub fn second_wind_cooldown(get: &impl Fn(&str) -> f64) -> f64 {
    SECOND_WIND_SECONDS * (1.0 - get("second_wind_cut")).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn nodes(pairs: &[(&str, f64)]) -> impl Fn(&str) -> f64 {
        let map: HashMap<String, f64> = pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
        move |key| map.get(key).copied().unwrap_or(0.0)
    }

    fn outdoors() -> Surroundings {
        Surroundings {
            y: 70.0,
            head_in_water: false,
            in_forest: false,
            in_nether_forest: false,
            night: false,
            idle: false,
        }
    }

    fn has(wanted: &[Status], id: &str) -> bool {
        wanted.iter().any(|status| status.id == id)
    }

    #[test]
    fn mole_sight_needs_depth_and_grows_with_upgrades() {
        let at = Surroundings {
            y: 20.0,
            ..outdoors()
        };
        assert!(statuses(&nodes(&[("mole", 1.0)]), &at).is_empty());
        let deeper = nodes(&[("mole", 1.0), ("mole_depth", 40.0)]);
        assert!(has(&statuses(&deeper, &at), "minecraft:night_vision"));
    }

    #[test]
    fn mole_sight_stops_under_water_without_its_upgrade() {
        let at = Surroundings {
            y: -10.0,
            head_in_water: true,
            ..outdoors()
        };
        assert!(statuses(&nodes(&[("mole", 1.0)]), &at).is_empty());
        let wet = nodes(&[("mole", 1.0), ("mole_water", 1.0)]);
        assert!(has(&statuses(&wet, &at), "minecraft:night_vision"));
    }

    #[test]
    fn forest_guard_reaches_the_nether_only_with_its_upgrade() {
        let at = Surroundings {
            in_nether_forest: true,
            ..outdoors()
        };
        assert!(statuses(&nodes(&[("forest", 1.0)]), &at).is_empty());
        let wanted = statuses(&nodes(&[("forest", 1.0), ("forest_nether", 1.0)]), &at);
        assert!(has(&wanted, "minecraft:regeneration"));
    }

    #[test]
    fn swift_adds_one_speed_level_per_node() {
        let wanted = statuses(&nodes(&[("swift", 3.0)]), &outdoors());
        assert_eq!(wanted, vec![status("minecraft:speed", 2.0)]);
    }

    #[test]
    fn night_walker_works_at_night_until_upgraded() {
        let day = outdoors();
        let night = Surroundings {
            night: true,
            ..outdoors()
        };
        let base = nodes(&[("night_walker", 1.0)]);
        assert!(statuses(&base, &day).is_empty());
        assert!(has(&statuses(&base, &night), "minecraft:night_vision"));
        let always = nodes(&[("night_walker_always", 1.0)]);
        assert!(has(&statuses(&always, &day), "minecraft:night_vision"));
    }

    #[test]
    fn fall_cuts_stack_and_never_go_below_zero() {
        let get = nodes(&[("fall_cut", 0.5), ("under_cut", 0.5)]);
        assert_eq!(after_cuts(&get, FALL_DAMAGE, false, 70.0, 20.0, 8.0), 4.0);
        assert_eq!(after_cuts(&get, FALL_DAMAGE, false, 10.0, 20.0, 8.0), 2.0);
        let full = nodes(&[("fall_cut", 1.5)]);
        assert_eq!(after_cuts(&full, FALL_DAMAGE, false, 70.0, 20.0, 8.0), 0.0);
    }

    #[test]
    fn stone_skin_only_helps_at_low_health_against_mobs() {
        let get = nodes(&[("stone_skin", 0.5)]);
        let mob = "minecraft:mob_attack";
        assert_eq!(after_cuts(&get, mob, true, 70.0, 20.0, 4.0), 4.0);
        assert_eq!(after_cuts(&get, mob, true, 70.0, 5.0, 4.0), 2.0);
        assert_eq!(after_cuts(&get, FALL_DAMAGE, false, 70.0, 5.0, 4.0), 4.0);
    }

    #[test]
    fn second_wind_upgrade_halves_the_wait() {
        assert_eq!(second_wind_cooldown(&nodes(&[])), 600.0);
        assert_eq!(
            second_wind_cooldown(&nodes(&[("second_wind_cut", 0.5)])),
            300.0
        );
    }
}

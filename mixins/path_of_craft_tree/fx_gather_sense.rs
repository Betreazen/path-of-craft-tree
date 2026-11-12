// Ore Sense: a client-side highlight of nearby ore, started with a hotkey.
use crate::game::{
    Game, pct_fx_gather_rules as rules, pct_glue as glue,
    pct_keys::{self as keys, Key},
};
use horizon_client::gizmos::{Gizmos, Line};
use std::{
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

const SENSE_KEY_EFFECT: &str = "ore_sense";
const RESCAN_EVERY: Duration = Duration::from_secs(1);
const RIM_COLOUR: u32 = 0xFFFF_D24A;
const RIM_WIDTH: f32 = 2.0;
const SNOW_LIFT: f64 = 0.06;
const SNOW_EFFECT: &str = "light_step_snow";
const POWDER_SNOW: &str = "minecraft:powder_snow";
type Corner = (f64, f64, f64);
const CUBE_EDGES: [[Corner; 2]; 12] = [
    [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)],
    [(0.0, 0.0, 1.0), (1.0, 0.0, 1.0)],
    [(0.0, 1.0, 0.0), (1.0, 1.0, 0.0)],
    [(0.0, 1.0, 1.0), (1.0, 1.0, 1.0)],
    [(0.0, 0.0, 0.0), (0.0, 1.0, 0.0)],
    [(1.0, 0.0, 0.0), (1.0, 1.0, 0.0)],
    [(0.0, 0.0, 1.0), (0.0, 1.0, 1.0)],
    [(1.0, 0.0, 1.0), (1.0, 1.0, 1.0)],
    [(0.0, 0.0, 0.0), (0.0, 0.0, 1.0)],
    [(1.0, 0.0, 0.0), (1.0, 0.0, 1.0)],
    [(0.0, 1.0, 0.0), (0.0, 1.0, 1.0)],
    [(1.0, 1.0, 0.0), (1.0, 1.0, 1.0)],
];

#[derive(Default)]
struct Sense {
    until: Option<Instant>,
    ready_at: Option<Instant>,
    scanned_at: Option<Instant>,
    ores: Vec<[i32; 3]>,
}

static SENSE: LazyLock<Mutex<Sense>> = LazyLock::new(Mutex::default);

fn sense() -> MutexGuard<'static, Sense> {
    SENSE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn client_effect(key: &str) -> f64 {
    glue::client().effects.get(key).copied().unwrap_or(0.0)
}

pub fn on_key(game: &Game<'_>, key: &str, pressed: bool) {
    if !pressed || !keys::is(game, key, Key::OreSense) || !game.native_playing() {
        return;
    }
    if client_effect(SENSE_KEY_EFFECT) <= 0.0 {
        return;
    }
    let now = Instant::now();
    let mut sense = sense();
    if sense.ready_at.is_some_and(|ready| now < ready) {
        return;
    }
    sense.until = Some(now + Duration::from_secs_f64(rules::SENSE_DURATION_SECONDS));
    sense.ready_at = Some(now + Duration::from_secs_f64(rules::SENSE_COOLDOWN_SECONDS));
    sense.scanned_at = None;
}

fn is_ore(tree: &crate::game::pct_rules::Tree, block: &str) -> bool {
    block
        .strip_prefix("minecraft:")
        .is_some_and(|name| tree.ores.contains_key(&format!("minecraft:blocks/{name}")))
}

fn distance_squared(a: [i32; 3], b: [i32; 3]) -> i32 {
    (0..3).map(|i| (a[i] - b[i]).pow(2)).sum()
}

fn scan(game: &Game<'_>, radius: i32) -> Vec<[i32; 3]> {
    let _timer = crate::game::pct_perf::time(crate::game::pct_perf::Probe::OreScan);
    let (Some(body), Some(tree)) = (game.player.as_ref(), glue::client().tree.clone()) else {
        return Vec::new();
    };
    let centre = [
        body.position.x.floor() as i32,
        body.position.y.floor() as i32,
        body.position.z.floor() as i32,
    ];
    let mut found = Vec::new();
    for x in -radius..=radius {
        for y in -radius..=radius {
            for z in -radius..=radius {
                let pos = [centre[0] + x, centre[1] + y, centre[2] + z];
                if is_ore(&tree, &game.native_block(pos)) {
                    found.push(pos);
                }
            }
        }
    }
    found.sort_by_key(|pos| distance_squared(*pos, centre));
    found.truncate(rules::SENSE_MAX_RIMS);
    found
}

/// Outlines the ore while the sense lasts, rescanning about once a second.
pub fn on_visuals(game: &Game<'_>) {
    let now = Instant::now();
    let due = {
        let mut sense = sense();
        let until = sense.until;
        match until {
            Some(until) if now < until => {
                sense.scanned_at.is_none_or(|at| now - at >= RESCAN_EVERY)
            }
            Some(_) => {
                sense.until = None;
                sense.ores.clear();
                false
            }
            None => return,
        }
    };
    if due {
        let found = scan(game, client_effect(SENSE_KEY_EFFECT) as i32);
        let mut sense = sense();
        sense.ores = found;
        sense.scanned_at = Some(now);
    }
}

/// The twelve edges of each sensed ore, drawn over everything: the point is to see it through rock.
pub fn add_ore_outlines(gizmos: &mut Gizmos) {
    let sense = sense();
    for pos in &sense.ores {
        let corner = |dx, dy, dz| {
            [
                f64::from(pos[0]) + dx,
                f64::from(pos[1]) + dy,
                f64::from(pos[2]) + dz,
            ]
        };
        for [a, b] in CUBE_EDGES {
            gizmos.on_top.push(Line {
                start: corner(a.0, a.1, a.2),
                end: corner(b.0, b.1, b.2),
                color: RIM_COLOUR,
                width: RIM_WIDTH,
            });
        }
    }
}

/// Light Step's upgrade: powder snow holds the player up instead of pulling them in.
pub fn on_client_tick(game: &mut Game<'_>) {
    if client_effect(SNOW_EFFECT) <= 0.0 {
        return;
    }
    let Some(body) = game.player.as_ref() else {
        return;
    };
    let feet = [
        body.position.x.floor() as i32,
        body.position.y.floor() as i32,
        body.position.z.floor() as i32,
    ];
    if game.native_block(feet) != POWDER_SNOW {
        return;
    }
    if let Some(body) = game.player.as_mut() {
        body.velocity.y = body.velocity.y.max(SNOW_LIFT);
    }
}

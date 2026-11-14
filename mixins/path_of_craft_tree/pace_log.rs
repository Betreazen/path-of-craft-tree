// The real leveling pace on a server: once an interval the server log and
// <world>/path_of_craft_tree/pace.jsonl get, per player who played in it, the experience gained
// per branch and the minutes played. Players are named only by a short hash of their UUID salted
// per world (pace_salt.txt beside it), never by name or UUID. Creative and spectator time is not
// counted, nor the self-test's pretend owners. The counting is in pace_counter.rs.
use crate::game::{
    Game, pct_glue as glue,
    pct_pace_counter::{self as counter, PaceCounter},
};
use crate::mod_api::NativeCommand;
use horizon_client::text::Text;
use std::{
    collections::BTreeMap,
    hash::{BuildHasher, RandomState},
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// How often the pace is written out.
pub const INTERVAL: Duration = Duration::from_secs(60 * 60);
/// The console command that prints the interval so far.
pub const COMMAND: &str = "pct_pace";
/// Play time is counted on every this many server ticks, by the real time since the last count.
const PLAY_SAMPLE_TICKS: u32 = 20;
const SALT_FILE: &str = "pace_salt.txt";
const LINES_FILE: &str = "pace.jsonl";
const SALT_BYTES: usize = 16;
const CREATIVE: i32 = 1;
const SPECTATOR: i32 = 3;

#[derive(Default)]
struct State {
    counter: PaceCounter,
    /// When the current interval began; none before the first server tick of a world.
    started: Option<Instant>,
    sampled: Option<Instant>,
    ticks: u32,
    /// This world's salt, once read or made.
    salt: Option<Vec<u8>>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A grant changed `player`'s experience table from `before` to `after` and the general scale by
/// `general`. Called with the server's lock held, so this takes only its own.
pub fn note_gain(
    player: u128,
    before: &BTreeMap<String, f64>,
    after: &BTreeMap<String, f64>,
    general: f64,
) {
    if !glue::is_test_player(player) {
        state().counter.note_gain(player, before, after, general);
    }
}

pub fn on_server_tick(game: &mut Game<'_>) {
    let (due, sample) = {
        let mut state = state();
        state.ticks = state.ticks.wrapping_add(1);
        let now = Instant::now();
        let started = *state.started.get_or_insert(now);
        let sample = state.ticks.is_multiple_of(PLAY_SAMPLE_TICKS).then(|| {
            let since = state.sampled.replace(now).unwrap_or(now);
            now.saturating_duration_since(since).as_secs_f64()
        });
        (now.saturating_duration_since(started) >= INTERVAL, sample)
    };
    if let Some(seconds) = sample {
        count_play(game, seconds);
    }
    if due {
        write_out(game);
    }
}

/// Adds `seconds` of play to everyone online in survival or adventure.
fn count_play(game: &Game<'_>, seconds: f64) {
    let playing: Vec<u128> = game
        .native_server_players()
        .into_iter()
        .filter(|p| p.game_mode != CREATIVE && p.game_mode != SPECTATOR)
        .map(|p| p.uuid)
        .filter(|uuid| !glue::is_test_player(*uuid))
        .collect();
    let mut state = state();
    for player in playing {
        state.counter.note_play(player, seconds);
    }
}

/// Ends the interval: one log line and one JSON line per player who played, then a fresh count.
/// Also called when the server stops, with the shorter interval it had.
pub fn write_out(game: &Game<'_>) {
    let (taken, elapsed) = {
        let mut state = state();
        let now = Instant::now();
        let elapsed = state
            .started
            .replace(now)
            .map_or(Duration::ZERO, |at| now.saturating_duration_since(at));
        (state.counter.take(), elapsed)
    };
    if taken.is_empty() {
        return;
    }
    let Some(salt) = world_salt(game) else {
        log::warn!("path_of_craft_tree: pace not written: no salt for this world");
        return;
    };
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |t| t.as_secs());
    let mut lines = String::new();
    for (player, pace) in &taken {
        let hash = counter::player_hash(&salt, *player);
        log::info!("path_of_craft_tree: {}", counter::log_line(&hash, pace));
        lines.push_str(&counter::json_line(at, elapsed.as_secs(), &hash, pace));
        lines.push('\n');
    }
    if let Some(path) = mod_file(game, LINES_FILE)
        && let Err(error) = append(&path, lines.as_bytes())
    {
        log::warn!("path_of_craft_tree: pace lines not saved: {error}");
    }
}

fn mod_file(game: &Game<'_>, name: &str) -> Option<PathBuf> {
    Some(glue::mod_dir(game)?.join(name))
}

/// The world's salt: read from its file, else made and saved there. None without a world folder
/// or when a new salt cannot be saved, so hashes never change within a world.
fn world_salt(game: &Game<'_>) -> Option<Vec<u8>> {
    if let Some(salt) = state().salt.clone() {
        return Some(salt);
    }
    let path = mod_file(game, SALT_FILE)?;
    let salt = match std::fs::read_to_string(&path) {
        Ok(text) => counter::parse_salt(&text)?,
        Err(_) => {
            let salt = new_salt();
            let text = counter::salt_text(&salt);
            if let Err(error) = glue::write_in_folder(&path, text.as_bytes()) {
                log::warn!("path_of_craft_tree: pace salt not saved: {error}");
                return None;
            }
            salt
        }
    };
    state().salt = Some(salt.clone());
    Some(salt)
}

/// Random bytes from the standard library's randomly keyed hasher, which the OS seeds.
fn new_salt() -> Vec<u8> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |t| t.as_nanos());
    (0..SALT_BYTES / 8)
        .flat_map(|round| {
            RandomState::new()
                .hash_one((nanos, round, std::process::id()))
                .to_le_bytes()
        })
        .collect()
}

fn append(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| e.to_string())
}

/// The world closed: its count, interval and salt are forgotten, so the next world starts its own.
pub fn forget_world() {
    *state() = State::default();
}

pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op(COMMAND, 4));
}

/// `pct_pace`: the interval so far for each player hash, and when it is written out.
pub fn on_console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if command.trim() != COMMAND {
        return;
    }
    *ran = true;
    let (so_far, left) = {
        let state = state();
        let left = state.started.map_or(INTERVAL, |at| {
            INTERVAL.saturating_sub(Instant::now().saturating_duration_since(at))
        });
        (state.counter.so_far().clone(), left)
    };
    let mut lines = vec![format!(
        "{COMMAND}: {} players this interval, written out in {} min",
        so_far.len(),
        left.as_secs().div_ceil(60)
    )];
    if !so_far.is_empty() {
        match world_salt(game) {
            Some(salt) => lines.extend(so_far.iter().map(|(player, pace)| {
                counter::log_line(&counter::player_hash(&salt, *player), pace)
            })),
            None => lines.push(format!("{COMMAND}: no salt for this world")),
        }
    }
    for line in lines {
        log::info!("path_of_craft_tree: {line}");
        game.native_feedback(Text::literal(line));
    }
}

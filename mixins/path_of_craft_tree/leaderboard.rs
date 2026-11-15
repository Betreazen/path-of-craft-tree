// The leaderboard for the server's site (docs/leaderboard.md): `<world>/path_of_craft_tree/
// leaderboard.json` holds the players who agreed with `/pct leaderboard on`, and nobody else. The
// file is its own record of who agreed: a row goes in on `on` and out on `off`, both written at
// once; the periodic save refreshes the rows of those online.
use crate::game::{
    Game, pct_board_rules as rules, pct_glue as glue, pct_groups as groups, pct_rules::Tree,
};
use rules::Board;
use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
struct State {
    /// The file the board was read from; another world reads its own.
    path: Option<PathBuf>,
    board: Board,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Mutex::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn file(game: &Game<'_>) -> Option<PathBuf> {
    Some(glue::mod_dir(game)?.join(rules::FILE_NAME))
}

/// The board of the open world, read from its file the first time. A file that cannot be read
/// is kept aside as `.bad`, so a later write cannot lose who agreed.
fn loaded(game: &Game<'_>) -> Option<MutexGuard<'static, State>> {
    let path = file(game)?;
    let mut state = state();
    if state.path.as_ref() == Some(&path) {
        return Some(state);
    }
    state.board = match std::fs::read(&path) {
        Ok(bytes) => Board::parse(&bytes).unwrap_or_else(|error| {
            log::error!("path_of_craft_tree: {} is damaged: {error}", path.display());
            if let Err(error) = std::fs::rename(&path, path.with_extension("bad")) {
                log::error!("path_of_craft_tree: damaged file not kept aside: {error}");
            }
            Board::default()
        }),
        Err(_) => Board::default(),
    };
    state.path = Some(path);
    Some(state)
}

fn write(state: &State, tree: &Tree) -> Result<(), String> {
    let path = state.path.as_ref().ok_or("no world is open")?;
    let bytes = state.board.encode(tree, now())?;
    glue::write_in_folder(path, &bytes)
}

/// Whether the player agreed to be on the board.
pub fn is_on(game: &Game<'_>, player: u128) -> bool {
    loaded(game).is_some_and(|state| state.board.has(player))
}

/// `/pct leaderboard on|off`: puts the player on the board or takes them off, and writes the file
/// at once. True when that changed anything.
pub fn set(game: &Game<'_>, player: u128, name: &str, on: bool) -> Result<bool, String> {
    let loaded_tree = glue::current_tree().ok_or("the tree is not loaded")?;
    let profile = glue::current_profile(player).ok_or("progress is not loaded yet")?;
    let mut state = loaded(game).ok_or("no world is open")?;
    let changed = if on {
        let was = state.board.has(player);
        state
            .board
            .upsert(rules::row(&loaded_tree.tree, &profile, player, name, now()));
        !was
    } else {
        state.board.remove(player)
    };
    write(&state, &loaded_tree.tree)?;
    log::info!(
        "path_of_craft_tree: {player:032x} ({name}) turned the leaderboard {}",
        if on { "on" } else { "off" }
    );
    Ok(changed)
}

/// The periodic save: the rows of players on the board who are online are refreshed, and the
/// file is written when one of them changed.
pub fn on_save(game: &Game<'_>) {
    let Some(loaded_tree) = glue::current_tree() else {
        return;
    };
    let Some(mut state) = loaded(game) else {
        return;
    };
    let tree = &loaded_tree.tree;
    let on_board: Vec<u128> = glue::server()
        .players
        .keys()
        .copied()
        .filter(|player| state.board.has(*player))
        .collect();
    let mut changed = false;
    for player in on_board {
        let Some(profile) = glue::current_profile(player) else {
            continue;
        };
        let id = format!("{player:032x}");
        let old_name = state
            .board
            .players
            .iter()
            .find(|row| row.uuid == id)
            .map(|row| row.name.clone());
        let name = groups::known_name(player).or(old_name).unwrap_or(id);
        let fresh = rules::row(tree, &profile, player, &name, now());
        let same = state.board.players.iter().any(|row| {
            row.uuid == fresh.uuid
                && row.name == fresh.name
                && row.levels == fresh.levels
                && row.general == fresh.general
        });
        if !same {
            state.board.upsert(fresh);
            changed = true;
        }
    }
    if changed {
        if let Err(error) = write(&state, tree) {
            log::warn!("path_of_craft_tree: leaderboard not saved: {error}");
        }
    }
}

pub fn forget_world() {
    *state() = State::default();
}
